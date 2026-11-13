//! The storage side of sync: export a snapshot, apply a merge, and keep the sync state.
//!
//! Every write happens in one immediate transaction together with the sync
//! state, so the stored base always matches the stored tasks.

use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::{Store, read_date, read_timestamp};
use crate::{
    model::Status,
    sync::{
        Downloaded,
        merge::{Merged, merge},
        snapshot::{ListRecord, Snapshot, TaskRecord, name_key},
    },
};

/// The result of merging one download into the local database.
#[derive(Clone, Debug, PartialEq)]
pub struct MergeOutcome {
    /// The snapshot to upload. `None` when the file already equals the local database.
    pub upload: Option<Snapshot>,
    /// The ETag the upload must replace. `None` means the file must not exist yet.
    pub replaces: Option<String>,
    /// Local lists and tasks inserted, changed, or deleted.
    pub received: usize,
    /// Lists and tasks the upload adds, changes, or removes in the file.
    pub sent: usize,
    /// Conflict copies created.
    pub conflicts: usize,
}

/// Counters the terminal reads to tell a sync commit from another process's commit.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SyncCounters {
    /// Sync transactions that wrote anything.
    pub commits: i64,
    /// Sync transactions that changed a list or task.
    pub applied: i64,
    /// Conflict copies created so far.
    pub conflicts: i64,
}

impl Store {
    fn require_sync(&self) -> Result<()> {
        ensure!(
            !self.read_only && self.list_scope.is_none(),
            "Sync needs an unrestricted writable store."
        );
        Ok(())
    }

    /// The ETag of the file the stored base came from.
    pub fn sync_etag(&self) -> Result<Option<String>> {
        self.require_sync()?;
        state(&self.connection, "etag")
    }

    /// Reads the counters. Any store may read them, because they hold no task data.
    pub fn sync_counters(&self) -> Result<SyncCounters> {
        let counter = |key: &str| -> Result<i64> {
            Ok(state(&self.connection, key)?
                .and_then(|value| value.parse().ok())
                .unwrap_or(0))
        };
        Ok(SyncCounters {
            commits: counter("commits")?,
            applied: counter("applied")?,
            conflicts: counter("conflicts")?,
        })
    }

    /// Merges one download into the database and returns what to upload.
    ///
    /// `since` is the ETag the download was conditioned on. Returns `None`
    /// without changing anything when another process moved the stored ETag
    /// in the meantime. The caller then downloads again.
    pub fn sync_merge(
        &mut self,
        since: Option<&str>,
        downloaded: Downloaded,
    ) -> Result<Option<MergeOutcome>> {
        self.require_sync()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let stored_etag = state(&transaction, "etag")?;
        if stored_etag.as_deref() != since {
            return Ok(None);
        }
        let mut wrote = backfill_uids(&transaction)? > 0;
        let mut base = load_snapshot(&transaction, "base")?;
        let sent = load_snapshot(&transaction, "sent")?;
        let mut rejoin = false;
        let (remote, remote_etag) = match downloaded {
            Downloaded::Unchanged => {
                let Some(base) = base.clone() else {
                    return Ok(None);
                };
                (Some(base), stored_etag.clone())
            }
            // The base stays stored. If the same file comes back, the merge still
            // knows which fields this device changed.
            Downloaded::Missing => (None, None),
            Downloaded::File { snapshot, etag } => {
                // An earlier upload reached the storage although its answer was
                // lost. The file descends from it, so it is the real base. Merging
                // against the older base would count this device's own upload as
                // another device's change and undo edits made since.
                if let Some(sent) = sent.as_ref().filter(|sent| snapshot.includes_upload(sent)) {
                    base = Some(sent.clone());
                }
                rejoin = base
                    .as_ref()
                    .is_some_and(|base| base.collection != snapshot.collection);
                (Some(snapshot), Some(etag))
            }
        };

        let local = export(&transaction)?;
        let merged = merge(base.as_ref(), &local, remote.as_ref(), rejoin);
        let received = apply(&transaction, &merged)?;

        let mut current = export(&transaction)?;
        match &remote {
            Some(remote) => current = current.framed_like(remote),
            None => current.collection = random_uid(&transaction)?,
        }
        let sent_count = match &remote {
            Some(remote) => remote.changes_to(&current),
            None => current.lists.len() + current.tasks.len(),
        };
        let mut upload = (remote.as_ref() != Some(&current)).then_some(current);
        if let Some(upload) = &mut upload {
            upload.record_upload(random_uid(&transaction)?);
        }

        // The base becomes the file that was just merged. Local changes are then
        // exactly the difference between the database and the base.
        match (&remote, &remote_etag) {
            (Some(remote), Some(etag)) if stored_etag.as_ref() != Some(etag) => {
                set_state(&transaction, "base", &json(remote)?)?;
                set_state(&transaction, "etag", etag)?;
                wrote = true;
            }
            (None, _) if stored_etag.is_some() => {
                transaction.execute("DELETE FROM sync_state WHERE key = 'etag'", [])?;
                wrote = true;
            }
            _ => {}
        }
        // Remember what is about to be uploaded, in case the answer never arrives.
        match &upload {
            Some(upload) => {
                set_state(&transaction, "sent", &json(upload)?)?;
                wrote = true;
            }
            None if sent.is_some() => {
                transaction.execute("DELETE FROM sync_state WHERE key = 'sent'", [])?;
                wrote = true;
            }
            None => {}
        }
        if received > 0 {
            bump(&transaction, "applied", 1)?;
            wrote = true;
        }
        if merged.conflicts > 0 {
            bump(&transaction, "conflicts", i64::try_from(merged.conflicts)?)?;
        }
        if wrote {
            bump(&transaction, "commits", 1)?;
        }
        transaction.commit()?;
        Ok(Some(MergeOutcome {
            upload,
            replaces: remote_etag,
            received,
            sent: sent_count,
            conflicts: merged.conflicts,
        }))
    }

    /// Records a successful upload as the new base.
    ///
    /// Nothing changes when another process moved the stored ETag since the
    /// merge. Its base is at least as new, and the next download settles it.
    pub fn sync_uploaded(
        &mut self,
        replaces: Option<&str>,
        uploaded: &Snapshot,
        etag: &str,
    ) -> Result<()> {
        self.require_sync()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if state(&transaction, "etag")?.as_deref() != replaces {
            return Ok(());
        }
        set_state(&transaction, "base", &json(uploaded)?)?;
        set_state(&transaction, "etag", etag)?;
        transaction.execute("DELETE FROM sync_state WHERE key = 'sent'", [])?;
        bump(&transaction, "commits", 1)?;
        transaction.commit()?;
        Ok(())
    }

    /// Reports whether the database holds changes the last synced file lacks.
    ///
    /// A profile without a synced file always has unsent changes.
    pub fn sync_pending(&mut self) -> Result<bool> {
        self.require_sync()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if backfill_uids(&transaction)? > 0 {
            bump(&transaction, "commits", 1)?;
        }
        let pending = match (
            load_snapshot(&transaction, "base")?,
            state(&transaction, "etag")?,
        ) {
            (Some(base), Some(_)) => export(&transaction)?.framed_like(&base) != base,
            _ => true,
        };
        transaction.commit()?;
        Ok(pending)
    }
}

fn state(connection: &Connection, key: &str) -> Result<Option<String>> {
    Ok(connection
        .query_row(
            "SELECT value FROM sync_state WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()?)
}

fn set_state(connection: &Connection, key: &str, value: &str) -> Result<()> {
    connection.execute(
        "INSERT INTO sync_state(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

fn bump(connection: &Connection, key: &str, by: i64) -> Result<()> {
    connection.execute(
        "INSERT INTO sync_state(key, value) VALUES (?1, CAST(?2 AS TEXT))
         ON CONFLICT(key) DO UPDATE
         SET value = CAST(CAST(value AS INTEGER) + ?2 AS TEXT)",
        params![key, by],
    )?;
    Ok(())
}

/// Reads a snapshot from the sync state: the `base`, or the `sent` upload.
fn load_snapshot(connection: &Connection, key: &str) -> Result<Option<Snapshot>> {
    state(connection, key)?
        .map(|json| serde_json::from_str(&json).context("The stored sync state is not readable."))
        .transpose()
}

fn json(snapshot: &Snapshot) -> Result<String> {
    String::from_utf8(snapshot.to_json()).context("Snapshot is not UTF-8.")
}

fn random_uid(connection: &Connection) -> Result<String> {
    Ok(connection.query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))?)
}

/// Gives a `uid` to rows that an older running Husker inserted without one.
fn backfill_uids(connection: &Connection) -> Result<usize> {
    Ok(connection.execute(
        "UPDATE lists SET uid = lower(hex(randomblob(16))) WHERE uid IS NULL",
        [],
    )? + connection.execute(
        "UPDATE tasks SET uid = lower(hex(randomblob(16))) WHERE uid IS NULL",
        [],
    )?)
}

/// Reads the lists and tasks as a snapshot. The caller sets the collection.
fn export(connection: &Connection) -> Result<Snapshot> {
    let default_list: Option<String> = connection
        .query_row("SELECT uid FROM lists ORDER BY id LIMIT 1", [], |row| {
            row.get(0)
        })
        .optional()?
        .flatten();
    let lists = connection
        .prepare("SELECT uid, id, name, position FROM lists WHERE uid IS NOT NULL")?
        .query_map([], |row| {
            Ok(ListRecord {
                uid: row.get(0)?,
                id: row.get(1)?,
                name: row.get(2)?,
                position: row.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let tasks = connection
        .prepare(
            "SELECT tasks.uid, tasks.id, lists.uid, tasks.title, tasks.description, tasks.due,
                    tasks.status, tasks.status_changed_on, tasks.status_changed_at,
                    tasks.deleted, tasks.position
             FROM tasks JOIN lists ON lists.id = tasks.list_id
             WHERE tasks.uid IS NOT NULL AND lists.uid IS NOT NULL",
        )?
        .query_map([], |row| {
            let status: String = row.get(6)?;
            Ok(TaskRecord {
                uid: row.get(0)?,
                id: row.get(1)?,
                list: row.get(2)?,
                title: row.get(3)?,
                description: row.get(4)?,
                due: read_date(row, 5)?,
                status: status.parse::<Status>().map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        6,
                        rusqlite::types::Type::Text,
                        error.into(),
                    )
                })?,
                status_changed_on: read_date(row, 7)?,
                status_changed_at: read_timestamp(row, 8)?,
                deleted: row.get(9)?,
                position: row.get(10)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(Snapshot::new(
        String::new(),
        default_list.unwrap_or_default(),
        lists,
        tasks,
    ))
}

/// The highest ID the table has ever assigned. A hinted ID above it was never used here.
fn highest_assigned(connection: &Connection, table: &str) -> Result<i64> {
    Ok(connection
        .query_row(
            "SELECT seq FROM sqlite_sequence WHERE name = ?1",
            [table],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(0))
}

/// Changes the database to match `merged` and returns the number of rows changed.
fn apply(connection: &Connection, merged: &Merged) -> Result<usize> {
    let mut changed = 0;
    for (from, to) in &merged.adopted {
        changed += connection.execute(
            "UPDATE lists SET uid = ?2 WHERE uid = ?1",
            params![from, to],
        )?;
    }
    let local = export(connection)?;

    // Lists. A list about to be renamed or deleted first releases its name, so
    // an insert or a swap of two names cannot hit the unique name index.
    let wanted: HashMap<&str, &ListRecord> = merged
        .lists
        .iter()
        .map(|list| (list.uid.as_str(), list))
        .collect();
    let existing: HashMap<&str, &ListRecord> = local
        .lists
        .iter()
        .map(|list| (list.uid.as_str(), list))
        .collect();
    for list in &local.lists {
        let keeps_name = wanted
            .get(list.uid.as_str())
            .is_some_and(|wanted| name_key(&wanted.name) == name_key(&list.name));
        if !keeps_name {
            connection.execute(
                "UPDATE lists SET name_key = char(1) || uid WHERE uid = ?1",
                [&list.uid],
            )?;
        }
    }
    let mut new_lists: Vec<&ListRecord> = merged
        .lists
        .iter()
        .filter(|list| !existing.contains_key(list.uid.as_str()))
        .collect();
    new_lists.sort_by_key(|list| (list.id, list.uid.as_str()));
    for list in new_lists {
        let id = (list.id > highest_assigned(connection, "lists")?).then_some(list.id);
        changed += connection.execute(
            "INSERT INTO lists(id, name, name_key, position, uid) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, list.name, name_key(&list.name), list.position, list.uid],
        )?;
    }
    for list in &merged.lists {
        if let Some(stored) = existing.get(list.uid.as_str())
            && !stored.same_content(list)
        {
            let rows = connection.execute(
                "UPDATE lists SET name = ?1, name_key = ?2, position = ?3 WHERE uid = ?4",
                params![list.name, name_key(&list.name), list.position, list.uid],
            )?;
            // A list that took a new identifier above is already counted once.
            if !merged.adopted.iter().any(|(_, to)| *to == list.uid) {
                changed += rows;
            }
        }
    }

    // Tasks.
    let list_ids: HashMap<String, i64> = connection
        .prepare("SELECT uid, id FROM lists WHERE uid IS NOT NULL")?
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let list_id = |uid: &str| -> Result<i64> {
        list_ids
            .get(uid)
            .copied()
            .context("A merged task names a list that does not exist.")
    };
    let existing: HashMap<&str, &TaskRecord> = local
        .tasks
        .iter()
        .map(|task| (task.uid.as_str(), task))
        .collect();
    let wanted: HashSet<&str> = merged.tasks.iter().map(|task| task.uid.as_str()).collect();
    let mut new_tasks: Vec<&TaskRecord> = merged
        .tasks
        .iter()
        .filter(|task| !existing.contains_key(task.uid.as_str()))
        .collect();
    new_tasks.sort_by_key(|task| (task.id, task.uid.as_str()));
    for task in new_tasks {
        let id = (task.id > highest_assigned(connection, "tasks")?).then_some(task.id);
        changed += connection.execute(
            "INSERT INTO tasks
                (id, list_id, title, description, due, status, status_changed_on,
                 status_changed_at, deleted, position, version, uid)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11)",
            params![
                id,
                list_id(&task.list)?,
                task.title,
                task.description,
                task.due.map(|date| date.to_string()),
                task.status.as_str(),
                task.status_changed_on.map(|date| date.to_string()),
                task.status_changed_at.map(|instant| instant.to_rfc3339()),
                task.deleted,
                task.position,
                task.uid,
            ],
        )?;
    }
    for task in &merged.tasks {
        if let Some(stored) = existing.get(task.uid.as_str())
            && !stored.same_content(task)
        {
            changed += connection.execute(
                "UPDATE tasks SET list_id = ?1, title = ?2, description = ?3, due = ?4,
                     status = ?5, status_changed_on = ?6, status_changed_at = ?7,
                     deleted = ?8, position = ?9, version = version + 1
                 WHERE uid = ?10",
                params![
                    list_id(&task.list)?,
                    task.title,
                    task.description,
                    task.due.map(|date| date.to_string()),
                    task.status.as_str(),
                    task.status_changed_on.map(|date| date.to_string()),
                    task.status_changed_at.map(|instant| instant.to_rfc3339()),
                    task.deleted,
                    task.position,
                    task.uid,
                ],
            )?;
        }
    }
    for task in &local.tasks {
        if !wanted.contains(task.uid.as_str()) {
            changed += connection.execute("DELETE FROM tasks WHERE uid = ?1", [&task.uid])?;
        }
    }

    // Lists the merge dropped. Their tasks were moved or deleted above.
    let wanted: HashSet<&str> = merged.lists.iter().map(|list| list.uid.as_str()).collect();
    for list in &local.lists {
        if !wanted.contains(list.uid.as_str()) {
            changed += connection.execute("DELETE FROM lists WHERE uid = ?1", [&list.uid])?;
        }
    }
    Ok(changed)
}
