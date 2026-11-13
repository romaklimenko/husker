use std::{io, path::Path, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use chrono::{DateTime, Local, NaiveDate, Utc};
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Row, TransactionBehavior, params, types::Type,
};

use crate::model::{Status, Task, TaskInput, TaskList};

mod sync;

pub use sync::{MergeOutcome, SyncCounters};

const TASK_COLUMNS: &str = "id, list_id, title, description, due, status, status_changed_on, deleted, position, version, status_changed_at";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ListDeletionPreview {
    pub list_id: i64,
    pub list_name: String,
    pub destination_id: i64,
    pub destination_name: String,
    pub open_count: usize,
    pub done_count: usize,
    pub cancelled_count: usize,
    pub trashed_count: usize,
    task_versions: Vec<(i64, i64)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ListDeletionOutcome {
    Deleted,
    Changed(ListDeletionPreview),
    Missing,
}

/// The tasks in Trash when a user was asked to confirm emptying it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrashPreview {
    pub count: usize,
    task_versions: Vec<(i64, i64)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EmptyTrashOutcome {
    Emptied(usize),
    Changed(TrashPreview),
}

#[derive(Clone, Debug, PartialEq)]
pub enum TaskPurgeOutcome {
    Deleted,
    Changed(Task),
    NotInTrash,
}

pub struct Store {
    connection: Connection,
    list_scope: Option<i64>,
    read_only: bool,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let mut connection = Connection::open(path)
            .with_context(|| format!("Cannot open database {}", path.display()))?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", true)?;

        {
            let transaction =
                connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let version: i64 =
                transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
            match version {
                0 => {
                    let tables: i64 = transaction.query_row(
                        "SELECT COUNT(*) FROM sqlite_master
                         WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
                        [],
                        |row| row.get(0),
                    )?;
                    ensure!(
                        tables == 0,
                        "The database contains unversioned tables. No changes were made."
                    );
                    transaction.execute_batch(
                        "CREATE TABLE lists (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            name TEXT NOT NULL CHECK(length(name) > 0),
                            name_key TEXT NOT NULL UNIQUE,
                            position INTEGER NOT NULL DEFAULT 0,
                            uid TEXT
                        );
                        CREATE TABLE tasks (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            list_id INTEGER NOT NULL REFERENCES lists(id),
                            title TEXT NOT NULL CHECK(length(title) > 0),
                            description TEXT NOT NULL DEFAULT '',
                            due TEXT,
                            status TEXT NOT NULL CHECK(status IN ('open', 'done', 'cancelled')),
                            status_changed_on TEXT,
                            status_changed_at TEXT,
                            deleted INTEGER NOT NULL CHECK(deleted IN (0, 1)),
                            position INTEGER NOT NULL,
                            version INTEGER NOT NULL CHECK(version > 0),
                            uid TEXT
                        );
                        CREATE INDEX tasks_list ON tasks(list_id);
                        CREATE INDEX tasks_planning ON tasks(deleted, status, due, position);
                        CREATE UNIQUE INDEX lists_uid ON lists(uid);
                        CREATE UNIQUE INDEX tasks_uid ON tasks(uid);
                        CREATE TABLE sync_state (
                            key TEXT PRIMARY KEY,
                            value TEXT NOT NULL
                        ) WITHOUT ROWID;
                        INSERT INTO lists(name, name_key, uid)
                            VALUES ('Inbox', 'inbox', lower(hex(randomblob(16))));
                        PRAGMA user_version = 5;",
                    )?;
                }
                1 => {
                    migrate_schema_one(&transaction)?;
                    migrate_schema_two(&transaction)?;
                    migrate_schema_three(&transaction)?;
                    migrate_schema_four(&transaction)?;
                }
                2 => {
                    migrate_schema_two(&transaction)?;
                    migrate_schema_three(&transaction)?;
                    migrate_schema_four(&transaction)?;
                }
                3 => {
                    migrate_schema_three(&transaction)?;
                    migrate_schema_four(&transaction)?;
                }
                4 => migrate_schema_four(&transaction)?,
                5 => {}
                _ => bail!(
                    "Unsupported database schema version {version}. This Husker supports version 5."
                ),
            }
            transaction.commit()?;
        }

        connection.pragma_update(None, "journal_mode", "WAL")?;
        Ok(Self {
            connection,
            list_scope: None,
            read_only: false,
        })
    }

    pub fn open_for_mcp(path: &Path, list_name: Option<&str>, read_only: bool) -> Result<Self> {
        let mut store = if read_only {
            let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
                .with_context(|| format!("Cannot open database {}", path.display()))?;
            connection.busy_timeout(Duration::from_secs(5))?;
            let version: i64 =
                connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
            require_current_schema(version)?;
            Self {
                connection,
                list_scope: None,
                read_only: true,
            }
        } else {
            Self::open(path)?
        };
        if let Some(name) = list_name {
            let name_key = name.trim().to_lowercase();
            let id = store
                .connection
                .query_row(
                    "SELECT id FROM lists WHERE name_key = ?1",
                    [name_key],
                    |row| row.get(0),
                )
                .optional()?
                .with_context(|| format!("List '{name}' does not exist."))?;
            store.list_scope = Some(id);
        }
        Ok(store)
    }

    pub fn list_scope(&self) -> Option<i64> {
        self.list_scope
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub fn data_version(&self) -> Result<i64> {
        Ok(self
            .connection
            .pragma_query_value(None, "data_version", |row| row.get(0))?)
    }

    pub fn default_list_id(&self) -> Result<i64> {
        self.connection
            .query_row(
                "SELECT MIN(id) FROM lists WHERE (?1 IS NULL OR id = ?1)",
                [self.list_scope],
                |row| row.get::<_, Option<i64>>(0),
            )?
            .context("No accessible list exists.")
    }

    fn require_writable(&self) -> Result<()> {
        ensure!(!self.read_only, "This store is read-only.");
        Ok(())
    }

    fn require_list_mutation(&self) -> Result<()> {
        self.require_writable()?;
        ensure!(
            self.list_scope.is_none(),
            "A list-scoped store cannot create, rename, reorder, or delete lists."
        );
        Ok(())
    }

    fn require_permanent_deletion(&self) -> Result<()> {
        self.require_writable()?;
        ensure!(
            self.list_scope.is_none(),
            "A list-scoped store cannot delete tasks permanently."
        );
        Ok(())
    }

    pub fn lists(&self) -> Result<Vec<TaskList>> {
        let mut statement = self.connection.prepare(
            "SELECT lists.id, lists.name,
                    COUNT(CASE WHEN tasks.status = 'open' AND tasks.deleted = 0 THEN 1 END)
             FROM lists LEFT JOIN tasks ON tasks.list_id = lists.id
             WHERE (?1 IS NULL OR lists.id = ?1)
             GROUP BY lists.id
             ORDER BY lists.id = (SELECT MIN(id) FROM lists) DESC, lists.position, lists.id",
        )?;
        let lists = statement
            .query_map([self.list_scope], |row| {
                Ok(TaskList {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    count: usize::try_from(row.get::<_, i64>(2)?).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(2, Type::Integer, Box::new(error))
                    })?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(lists)
    }

    pub fn tasks(&self) -> Result<Vec<Task>> {
        let mut statement = self.connection.prepare(&format!(
            "SELECT {TASK_COLUMNS} FROM tasks
                 WHERE (?1 IS NULL OR list_id = ?1) ORDER BY id"
        ))?;
        let tasks = statement
            .query_map([self.list_scope], read_task)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(tasks)
    }

    pub fn task(&self, id: i64) -> Result<Task> {
        find_task(&self.connection, id, self.list_scope)
    }

    pub fn create_list(&mut self, name: &str) -> Result<TaskList> {
        self.require_list_mutation()?;
        let (name, name_key) = normalize_list_name(name)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_unique_list_name(&transaction, &name_key, None)?;
        transaction.execute(
            "INSERT INTO lists(name, name_key, position, uid)
             VALUES (?1, ?2, (SELECT COALESCE(MAX(position), 0) + 1 FROM lists),
                lower(hex(randomblob(16))))",
            params![name, name_key],
        )?;
        let id = transaction.last_insert_rowid();
        transaction.commit()?;
        Ok(TaskList { id, name, count: 0 })
    }

    pub fn rename_list(&mut self, id: i64, name: &str) -> Result<TaskList> {
        self.require_list_mutation()?;
        let (name, name_key) = normalize_list_name(name)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_list(&transaction, id)?;
        require_unique_list_name(&transaction, &name_key, Some(id))?;
        transaction.execute(
            "UPDATE lists SET name = ?1, name_key = ?2 WHERE id = ?3",
            params![name, name_key, id],
        )?;
        let count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM tasks WHERE list_id = ?1 AND status = 'open' AND deleted = 0",
            [id],
            |row| row.get(0),
        )?;
        transaction.commit()?;
        Ok(TaskList {
            id,
            name,
            count: usize::try_from(count)?,
        })
    }

    pub fn move_list(&mut self, id: i64, down: bool) -> Result<()> {
        self.require_list_mutation()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_list(&transaction, id)?;
        let default_id: i64 =
            transaction.query_row("SELECT MIN(id) FROM lists", [], |row| row.get(0))?;
        if id == default_id {
            return Ok(());
        }
        let movable = {
            let mut statement = transaction
                .prepare("SELECT id, position FROM lists WHERE id != ?1 ORDER BY position, id")?;
            statement
                .query_map([default_id], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        let index = movable
            .iter()
            .position(|(list_id, _)| *list_id == id)
            .context("The selected list no longer exists.")?;
        let neighbor_index = if down {
            index.checked_add(1)
        } else {
            index.checked_sub(1)
        };
        let Some(&(neighbor_id, neighbor_position)) =
            neighbor_index.and_then(|index| movable.get(index))
        else {
            return Ok(());
        };
        transaction.execute(
            "UPDATE lists SET position = ?1 WHERE id = ?2",
            params![neighbor_position, id],
        )?;
        transaction.execute(
            "UPDATE lists SET position = ?1 WHERE id = ?2",
            params![movable[index].1, neighbor_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn preview_list_deletion(&self, id: i64) -> Result<ListDeletionPreview> {
        self.require_list_mutation()?;
        let transaction = self.connection.unchecked_transaction()?;
        let preview = list_deletion_preview(&transaction, id)?
            .with_context(|| format!("List {id} does not exist."))?;
        transaction.commit()?;
        Ok(preview)
    }

    pub fn delete_list(&mut self, preview: &ListDeletionPreview) -> Result<ListDeletionOutcome> {
        self.require_list_mutation()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let Some(current) = list_deletion_preview(&transaction, preview.list_id)? else {
            return Ok(ListDeletionOutcome::Missing);
        };
        if &current != preview {
            return Ok(ListDeletionOutcome::Changed(current));
        }
        transaction.execute(
            "UPDATE tasks SET list_id = ?1, version = version + 1 WHERE list_id = ?2",
            params![current.destination_id, current.list_id],
        )?;
        transaction.execute("DELETE FROM lists WHERE id = ?1", [current.list_id])?;
        transaction.commit()?;
        Ok(ListDeletionOutcome::Deleted)
    }

    pub fn create_task(&mut self, input: &TaskInput) -> Result<Task> {
        self.require_writable()?;
        let input = normalize_input(input)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_destination(self.list_scope, input.list_id)?;
        require_list(&transaction, input.list_id)?;
        transaction.execute(
            "INSERT INTO tasks
                (list_id, title, description, due, status, status_changed_on, deleted, position,
                 version, uid)
             VALUES (?1, ?2, ?3, ?4, 'open', NULL, 0,
                (SELECT COALESCE(MAX(position), 0) + 1 FROM tasks), 1,
                lower(hex(randomblob(16))))",
            params![
                input.list_id,
                input.title,
                input.description,
                input.due.map(|date| date.to_string()),
            ],
        )?;
        let task = find_task(
            &transaction,
            transaction.last_insert_rowid(),
            self.list_scope,
        )?;
        transaction.commit()?;
        Ok(task)
    }

    pub fn update_task(&mut self, id: i64, version: i64, input: &TaskInput) -> Result<Task> {
        self.require_writable()?;
        let input = normalize_input(input)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let stored = find_task(&transaction, id, self.list_scope)?;
        require_destination(self.list_scope, input.list_id)?;
        require_version(&stored, version)?;
        require_list(&transaction, input.list_id)?;
        transaction.execute(
            "UPDATE tasks SET list_id = ?1, title = ?2, description = ?3, due = ?4,
                version = version + 1 WHERE id = ?5 AND version = ?6",
            params![
                input.list_id,
                input.title,
                input.description,
                input.due.map(|date| date.to_string()),
                id,
                version,
            ],
        )?;
        let task = find_task(&transaction, id, self.list_scope)?;
        transaction.commit()?;
        Ok(task)
    }

    pub fn set_due(&mut self, id: i64, version: i64, due: Option<NaiveDate>) -> Result<Task> {
        self.require_writable()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let stored = find_task(&transaction, id, self.list_scope)?;
        require_version(&stored, version)?;
        if stored.due == due {
            transaction.commit()?;
            return Ok(stored);
        }
        transaction.execute(
            "UPDATE tasks SET due = ?1, version = version + 1 WHERE id = ?2 AND version = ?3",
            params![due.map(|date| date.to_string()), id, version],
        )?;
        let task = find_task(&transaction, id, self.list_scope)?;
        transaction.commit()?;
        Ok(task)
    }

    pub fn set_list(&mut self, id: i64, version: i64, list_id: i64) -> Result<Task> {
        self.require_writable()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let stored = find_task(&transaction, id, self.list_scope)?;
        require_destination(self.list_scope, list_id)?;
        require_version(&stored, version)?;
        if stored.list_id == list_id {
            transaction.commit()?;
            return Ok(stored);
        }
        require_list(&transaction, list_id)?;
        transaction.execute(
            "UPDATE tasks SET list_id = ?1, version = version + 1 WHERE id = ?2 AND version = ?3",
            params![list_id, id, version],
        )?;
        let task = find_task(&transaction, id, self.list_scope)?;
        transaction.commit()?;
        Ok(task)
    }

    pub fn restore_status(
        &mut self,
        id: i64,
        version: i64,
        status: Status,
        status_changed_on: Option<NaiveDate>,
        status_changed_at: Option<DateTime<Utc>>,
    ) -> Result<Task> {
        self.require_writable()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let stored = find_task(&transaction, id, self.list_scope)?;
        require_version(&stored, version)?;
        let (status_changed_on, status_changed_at) = match status {
            Status::Open => (None, None),
            _ => (status_changed_on, status_changed_at),
        };
        transaction.execute(
            "UPDATE tasks SET status = ?1, status_changed_on = ?2, status_changed_at = ?3,
                 version = version + 1 WHERE id = ?4 AND version = ?5",
            params![
                status.as_str(),
                status_changed_on.map(|date| date.to_string()),
                status_changed_at.map(|instant| instant.to_rfc3339()),
                id,
                version
            ],
        )?;
        let task = find_task(&transaction, id, self.list_scope)?;
        transaction.commit()?;
        Ok(task)
    }

    pub fn set_status(&mut self, id: i64, version: i64, status: Status) -> Result<Task> {
        let now = Local::now();
        self.set_status_at(
            id,
            version,
            status,
            now.date_naive(),
            now.with_timezone(&Utc),
        )
    }

    pub fn set_status_on(
        &mut self,
        id: i64,
        version: i64,
        status: Status,
        today: NaiveDate,
    ) -> Result<Task> {
        self.set_status_at(id, version, status, today, Utc::now())
    }

    pub fn set_status_at(
        &mut self,
        id: i64,
        version: i64,
        status: Status,
        today: NaiveDate,
        now: DateTime<Utc>,
    ) -> Result<Task> {
        self.require_writable()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let stored = find_task(&transaction, id, self.list_scope)?;
        require_version(&stored, version)?;
        let (status_changed_on, status_changed_at) = match status {
            Status::Open => (None, None),
            _ if status == stored.status => (stored.status_changed_on, stored.status_changed_at),
            _ => (Some(today), Some(now)),
        };
        transaction.execute(
            "UPDATE tasks SET status = ?1, status_changed_on = ?2, status_changed_at = ?3,
                 version = version + 1 WHERE id = ?4 AND version = ?5",
            params![
                status.as_str(),
                status_changed_on.map(|date| date.to_string()),
                status_changed_at.map(|instant| instant.to_rfc3339()),
                id,
                version
            ],
        )?;
        let task = find_task(&transaction, id, self.list_scope)?;
        transaction.commit()?;
        Ok(task)
    }

    pub fn set_deleted(&mut self, id: i64, version: i64, deleted: bool) -> Result<Task> {
        self.require_writable()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_version(&find_task(&transaction, id, self.list_scope)?, version)?;
        transaction.execute(
            "UPDATE tasks SET deleted = ?1, version = version + 1 WHERE id = ?2 AND version = ?3",
            params![deleted, id, version],
        )?;
        let task = find_task(&transaction, id, self.list_scope)?;
        transaction.commit()?;
        Ok(task)
    }

    /// Removes one task in Trash if it still has the version the caller confirmed.
    pub fn purge_task(&mut self, id: i64, version: i64) -> Result<TaskPurgeOutcome> {
        self.require_permanent_deletion()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let Some(task) = try_find_task(&transaction, id, None)?.filter(|task| task.deleted) else {
            return Ok(TaskPurgeOutcome::NotInTrash);
        };
        if task.version != version {
            return Ok(TaskPurgeOutcome::Changed(task));
        }
        transaction.execute("DELETE FROM tasks WHERE id = ?1", [id])?;
        transaction.commit()?;
        Ok(TaskPurgeOutcome::Deleted)
    }

    pub fn preview_empty_trash(&self) -> Result<TrashPreview> {
        self.require_permanent_deletion()?;
        trash_preview(&self.connection)
    }

    /// Removes every task in Trash if Trash still matches the confirmed preview.
    pub fn empty_trash(&mut self, preview: &TrashPreview) -> Result<EmptyTrashOutcome> {
        self.require_permanent_deletion()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = trash_preview(&transaction)?;
        if &current != preview {
            return Ok(EmptyTrashOutcome::Changed(current));
        }
        let deleted = transaction.execute("DELETE FROM tasks WHERE deleted = 1", [])?;
        transaction.commit()?;
        Ok(EmptyTrashOutcome::Emptied(deleted))
    }

    pub fn swap_positions(&mut self, first: &Task, second: &Task) -> Result<()> {
        self.require_writable()?;
        ensure!(
            first.id != second.id,
            "Select two different tasks to reorder."
        );
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let stored_first = find_task(&transaction, first.id, self.list_scope)?;
        let stored_second = find_task(&transaction, second.id, self.list_scope)?;
        require_version(&stored_first, first.version)?;
        require_version(&stored_second, second.version)?;
        ensure!(
            !stored_first.deleted && !stored_second.deleted,
            "Tasks in Trash cannot be reordered."
        );
        ensure!(
            stored_first.due == stored_second.due,
            "Reorder tasks within the same date or undated group."
        );
        ensure!(
            stored_first.status == Status::Open && stored_second.status == Status::Open,
            "Only open tasks can be reordered."
        );
        transaction.execute(
            "UPDATE tasks SET position = ?1, version = version + 1 WHERE id = ?2",
            params![stored_second.position, stored_first.id],
        )?;
        transaction.execute(
            "UPDATE tasks SET position = ?1, version = version + 1 WHERE id = ?2",
            params![stored_first.position, stored_second.id],
        )?;
        transaction.commit()?;
        Ok(())
    }
}

fn list_deletion_preview(connection: &Connection, id: i64) -> Result<Option<ListDeletionPreview>> {
    let Some(list_name) = connection
        .query_row("SELECT name FROM lists WHERE id = ?1", [id], |row| {
            row.get::<_, String>(0)
        })
        .optional()?
    else {
        return Ok(None);
    };
    let (destination_id, destination_name): (i64, String) = connection.query_row(
        "SELECT id, name FROM lists ORDER BY id LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    ensure!(id != destination_id, "The default list cannot be deleted.");
    let mut preview = ListDeletionPreview {
        list_id: id,
        list_name,
        destination_id,
        destination_name,
        open_count: 0,
        done_count: 0,
        cancelled_count: 0,
        trashed_count: 0,
        task_versions: Vec::new(),
    };
    let mut statement = connection
        .prepare("SELECT id, version, status, deleted FROM tasks WHERE list_id = ?1 ORDER BY id")?;
    let mut rows = statement.query([id])?;
    while let Some(row) = rows.next()? {
        preview.task_versions.push((row.get(0)?, row.get(1)?));
        let status: String = row.get(2)?;
        let deleted: bool = row.get(3)?;
        if deleted {
            preview.trashed_count += 1;
        } else {
            match status.parse::<Status>()? {
                Status::Open => preview.open_count += 1,
                Status::Done => preview.done_count += 1,
                Status::Cancelled => preview.cancelled_count += 1,
            }
        }
    }
    Ok(Some(preview))
}

fn trash_preview(connection: &Connection) -> Result<TrashPreview> {
    let mut statement =
        connection.prepare("SELECT id, version FROM tasks WHERE deleted = 1 ORDER BY id")?;
    let task_versions = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<Vec<(i64, i64)>>>()?;
    Ok(TrashPreview {
        count: task_versions.len(),
        task_versions,
    })
}

fn require_current_schema(version: i64) -> Result<()> {
    if (1..=4).contains(&version) {
        bail!(
            "Database schema version {version} needs an upgrade. Open this profile with writable Husker before connecting read-only."
        );
    }
    ensure!(
        version == 5,
        "Unsupported database schema version {version}. This Husker supports version 5."
    );
    Ok(())
}

fn migrate_schema_one(connection: &Connection) -> Result<()> {
    let legacy_tasks = {
        let mut statement =
            connection.prepare("SELECT id, description, tags FROM tasks ORDER BY id")?;
        statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (id, mut description, tags) in legacy_tasks {
        let tags: Vec<String> = serde_json::from_str(&tags)
            .with_context(|| format!("Cannot migrate tags for task {id}."))?;
        if tags.is_empty() {
            continue;
        }
        if !description.is_empty() {
            description.push_str("\n\n");
        }
        description.push_str(
            &tags
                .iter()
                .map(|tag| format!("#{tag}"))
                .collect::<Vec<_>>()
                .join(" "),
        );
        connection.execute(
            "UPDATE tasks SET description = ?1, version = version + 1 WHERE id = ?2",
            params![description, id],
        )?;
    }
    connection.execute_batch(
        "ALTER TABLE tasks ADD COLUMN status_changed_on TEXT;
         ALTER TABLE tasks DROP COLUMN ready;
         ALTER TABLE tasks DROP COLUMN tags;
         PRAGMA user_version = 2;",
    )?;
    Ok(())
}

fn migrate_schema_two(connection: &Connection) -> Result<()> {
    connection
        .execute_batch("ALTER TABLE lists ADD COLUMN position INTEGER NOT NULL DEFAULT 0;")?;
    let list_ids = {
        let mut statement = connection.prepare(
            "SELECT id FROM lists ORDER BY id = (SELECT MIN(id) FROM lists) DESC, name_key, id",
        )?;
        statement
            .query_map([], |row| row.get::<_, i64>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    for (position, id) in list_ids.iter().enumerate() {
        connection.execute(
            "UPDATE lists SET position = ?1 WHERE id = ?2",
            params![i64::try_from(position)?, id],
        )?;
    }
    connection.pragma_update(None, "user_version", 3)?;
    Ok(())
}

fn migrate_schema_three(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "ALTER TABLE tasks ADD COLUMN status_changed_at TEXT;
         PRAGMA user_version = 4;",
    )?;
    Ok(())
}

/// Adds the sync identifiers and the sync-state table. Task versions do not change.
fn migrate_schema_four(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "ALTER TABLE lists ADD COLUMN uid TEXT;
         ALTER TABLE tasks ADD COLUMN uid TEXT;
         UPDATE lists SET uid = lower(hex(randomblob(16)));
         UPDATE tasks SET uid = lower(hex(randomblob(16)));
         CREATE UNIQUE INDEX lists_uid ON lists(uid);
         CREATE UNIQUE INDEX tasks_uid ON tasks(uid);
         CREATE TABLE sync_state (
             key TEXT PRIMARY KEY,
             value TEXT NOT NULL
         ) WITHOUT ROWID;
         PRAGMA user_version = 5;",
    )?;
    Ok(())
}

fn normalize_list_name(name: &str) -> Result<(String, String)> {
    let name = name.trim().to_owned();
    ensure!(!name.is_empty(), "A list name cannot be empty.");
    let name_key = name.to_lowercase();
    ensure!(
        name_key != "trash" && name_key != "papirkurv",
        "Trash and Papirkurv are reserved list names."
    );
    Ok((name, name_key))
}

fn require_unique_list_name(
    connection: &Connection,
    name_key: &str,
    id: Option<i64>,
) -> Result<()> {
    let duplicate: Option<i64> = connection
        .query_row(
            "SELECT id FROM lists WHERE name_key = ?1",
            [name_key],
            |row| row.get(0),
        )
        .optional()?;
    ensure!(
        duplicate.is_none() || duplicate == id,
        "A list with that name already exists."
    );
    Ok(())
}

fn require_list(connection: &Connection, id: i64) -> Result<()> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM lists WHERE id = ?1)",
        [id],
        |row| row.get(0),
    )?;
    ensure!(exists, "List {id} does not exist.");
    Ok(())
}

fn require_destination(list_scope: Option<i64>, list_id: i64) -> Result<()> {
    ensure!(
        list_scope.is_none_or(|allowed| allowed == list_id),
        "This store is restricted to one list."
    );
    Ok(())
}

fn normalize_input(input: &TaskInput) -> Result<TaskInput> {
    let mut input = input.clone();
    input.title = input.title.trim().to_owned();
    ensure!(!input.title.is_empty(), "A task title cannot be empty.");
    Ok(input)
}

fn require_version(task: &Task, expected: i64) -> Result<()> {
    ensure!(
        task.version == expected,
        "Task {} changed in another process. Reload it before saving.",
        task.id
    );
    Ok(())
}

fn try_find_task(
    connection: &Connection,
    id: i64,
    list_scope: Option<i64>,
) -> Result<Option<Task>> {
    Ok(connection
        .query_row(
            &format!(
                "SELECT {TASK_COLUMNS} FROM tasks
                 WHERE id = ?1 AND (?2 IS NULL OR list_id = ?2)"
            ),
            params![id, list_scope],
            read_task,
        )
        .optional()?)
}

fn find_task(connection: &Connection, id: i64, list_scope: Option<i64>) -> Result<Task> {
    try_find_task(connection, id, list_scope)?.with_context(|| match list_scope {
        Some(_) => "Task not found.".to_owned(),
        None => format!("Task {id} does not exist."),
    })
}

fn read_task(row: &Row<'_>) -> rusqlite::Result<Task> {
    let status: String = row.get(5)?;
    let status = match status.as_str() {
        "open" => Status::Open,
        "done" => Status::Done,
        "cancelled" => Status::Cancelled,
        _ => {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                5,
                Type::Text,
                Box::new(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid task status",
                )),
            ));
        }
    };
    Ok(Task {
        id: row.get(0)?,
        list_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        due: read_date(row, 4)?,
        status,
        status_changed_on: read_date(row, 6)?,
        status_changed_at: read_timestamp(row, 10)?,
        deleted: row.get(7)?,
        position: row.get(8)?,
        version: row.get(9)?,
    })
}

fn read_date(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<NaiveDate>> {
    let value: Option<String> = row.get(index)?;
    value
        .map(|value| {
            NaiveDate::parse_from_str(&value, "%Y-%m-%d").map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
            })
        })
        .transpose()
}

fn read_timestamp(row: &Row<'_>, index: usize) -> rusqlite::Result<Option<DateTime<Utc>>> {
    let value: Option<String> = row.get(index)?;
    value
        .map(|value| {
            DateTime::parse_from_rfc3339(&value)
                .map(|instant| instant.with_timezone(&Utc))
                .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
                })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(title: &str) -> TaskInput {
        TaskInput {
            list_id: 1,
            title: title.into(),
            description: String::new(),
            due: None,
        }
    }

    fn memory_store() -> Store {
        Store::open(Path::new(":memory:")).unwrap()
    }

    fn tasks_without_recorded_times(connection: &Connection) -> Vec<Task> {
        let columns = TASK_COLUMNS.replace("status_changed_at", "NULL AS status_changed_at");
        connection
            .prepare(&format!("SELECT {columns} FROM tasks ORDER BY id"))
            .unwrap()
            .query_map([], read_task)
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    fn timestamp(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn legacy_database(path: &Path) -> Connection {
        let connection = Connection::open(path).unwrap();
        connection.execute_batch(
            r#"CREATE TABLE lists (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL CHECK(length(name) > 0),
                name_key TEXT NOT NULL UNIQUE
            );
            CREATE TABLE tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                list_id INTEGER NOT NULL REFERENCES lists(id),
                title TEXT NOT NULL CHECK(length(title) > 0),
                description TEXT NOT NULL DEFAULT '',
                due TEXT,
                status TEXT NOT NULL CHECK(status IN ('open', 'done', 'cancelled')),
                ready INTEGER NOT NULL CHECK(ready IN (0, 1)),
                tags TEXT NOT NULL DEFAULT '[]',
                deleted INTEGER NOT NULL CHECK(deleted IN (0, 1)),
                position INTEGER NOT NULL,
                version INTEGER NOT NULL CHECK(version > 0)
            );
            CREATE INDEX tasks_list ON tasks(list_id);
            CREATE INDEX tasks_planning ON tasks(deleted, status, due, position);
            INSERT INTO lists VALUES (1, 'Inbox', 'inbox'), (2, 'Home', 'home');
            INSERT INTO tasks VALUES
                (7, 2, 'Milk', 'Keep this description', '2026-11-08', 'open', 1, '["shopping","ærinder"]', 0, 5, 3),
                (9, 2, 'Finished', '', NULL, 'done', 0, '["archive"]', 0, 8, 4),
                (10, 1, 'Discarded', 'Exact original text', NULL, 'cancelled', 1, '[]', 1, 9, 6),
                (99, 1, 'Historical ID', '', NULL, 'open', 0, '[]', 0, 10, 1);
            DELETE FROM tasks WHERE id = 99;
            PRAGMA user_version = 1;"#,
        ).unwrap();
        connection
    }

    #[test]
    fn migration_preserves_fields_tags_and_id_sequence() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        drop(legacy_database(&path));
        let mut store = Store::open(&path).unwrap();
        let milk = store.task(7).unwrap();
        assert_eq!(milk.list_id, 2);
        assert_eq!(milk.title, "Milk");
        assert_eq!(
            milk.description,
            "Keep this description\n\n#shopping #ærinder"
        );
        assert_eq!(milk.due, NaiveDate::from_ymd_opt(2026, 11, 8));
        assert_eq!(milk.status, Status::Open);
        assert_eq!(milk.status_changed_on, None);
        assert_eq!(milk.status_changed_at, None);
        assert!(!milk.deleted);
        assert_eq!((milk.position, milk.version), (5, 4));
        let mut before_upgrade = TaskInput::from(&milk);
        before_upgrade.description = "Keep this description".into();
        assert!(store.update_task(milk.id, 3, &before_upgrade).is_err());
        assert_eq!(store.task(milk.id).unwrap(), milk);
        let done = store.task(9).unwrap();
        assert_eq!(done.status, Status::Done);
        assert_eq!(done.status_changed_on, None);
        assert_eq!(done.status_changed_at, None);
        assert_eq!(done.description, "#archive");
        assert_eq!((done.position, done.version), (8, 5));
        let cancelled = store.task(10).unwrap();
        assert_eq!(cancelled.status, Status::Cancelled);
        assert_eq!(cancelled.status_changed_on, None);
        assert_eq!(cancelled.status_changed_at, None);
        assert_eq!(cancelled.description, "Exact original text");
        assert!(cancelled.deleted);
        assert_eq!((cancelled.position, cancelled.version), (9, 6));
        assert_eq!(store.create_task(&input("Next task")).unwrap().id, 100);
        let columns: Vec<String> = store
            .connection
            .prepare("SELECT name FROM pragma_table_info('tasks')")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert!(!columns.iter().any(|name| name == "tags" || name == "ready"));
        let version: i64 = store
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 5);
        drop(store);
        let scoped = Store::open_for_mcp(&path, Some("Home"), true).unwrap();
        assert_eq!(scoped.default_list_id().unwrap(), 2);
        assert_eq!(scoped.tasks().unwrap().len(), 2);
        assert_eq!(scoped.task(7).unwrap(), milk);
        assert!(scoped.task(10).is_err());
    }

    #[test]
    fn failed_migration_and_readonly_open_leave_legacy_data_intact() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let connection = legacy_database(&path);
        let error = match Store::open_for_mcp(&path, Some("Home"), true) {
            Ok(_) => panic!("Read-only schema 1 must require an upgrade"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("Open this profile with writable Husker"));
        connection
            .execute("UPDATE tasks SET tags = 'invalid JSON' WHERE id = 9", [])
            .unwrap();
        assert!(Store::open(&path).is_err());
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
        let (description, tags, ready): (String, String, bool) = connection
            .query_row(
                "SELECT description, tags, ready FROM tasks WHERE id = 7",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(description, "Keep this description");
        assert_eq!(tags, "[\"shopping\",\"ærinder\"]");
        assert!(ready);
        assert!(
            connection
                .prepare("SELECT status_changed_on FROM tasks")
                .is_err()
        );
    }

    #[test]
    fn schema_two_migration_pins_default_and_keeps_every_task_field() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut connection = legacy_database(&path);
        {
            let transaction = connection.transaction().unwrap();
            migrate_schema_one(&transaction).unwrap();
            transaction.commit().unwrap();
        }
        connection.execute_batch(
            "INSERT INTO lists VALUES (3, 'Zebra', 'zebra'), (4, 'apple', 'apple'), (5, 'Beta', 'beta');
             UPDATE tasks SET status_changed_on = '2026-11-08' WHERE id = 9;",
        ).unwrap();
        let before = tasks_without_recorded_times(&connection);
        assert!(Store::open_for_mcp(&path, None, true).is_err());
        let old_version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(old_version, 2);
        assert!(connection.prepare("SELECT position FROM lists").is_err());
        drop(connection);

        let store = Store::open(&path).unwrap();
        assert_eq!(
            store
                .lists()
                .unwrap()
                .iter()
                .map(|list| list.id)
                .collect::<Vec<_>>(),
            [1, 4, 5, 2, 3]
        );
        assert_eq!(store.tasks().unwrap(), before);
        assert_eq!(store.default_list_id().unwrap(), 1);
        drop(store);
        let readonly = Store::open_for_mcp(&path, None, true).unwrap();
        assert_eq!(readonly.tasks().unwrap(), before);
        assert_eq!(
            readonly
                .lists()
                .unwrap()
                .iter()
                .map(|list| list.id)
                .collect::<Vec<_>>(),
            [1, 4, 5, 2, 3]
        );
    }

    #[test]
    fn schema_three_migration_preserves_tasks_lists_and_unknown_completion_times() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut connection = legacy_database(&path);
        {
            let transaction = connection.transaction().unwrap();
            migrate_schema_one(&transaction).unwrap();
            migrate_schema_two(&transaction).unwrap();
            transaction.commit().unwrap();
        }
        connection.execute_batch(
            "UPDATE lists SET name = 'Personal', name_key = 'personal', position = 99 WHERE id = 1;
             INSERT INTO lists(id, name, name_key, position) VALUES (3, 'Alpha', 'alpha', -8);
             UPDATE tasks SET status_changed_on = '2026-11-08' WHERE id = 9;",
        ).unwrap();
        let before = tasks_without_recorded_times(&connection);
        let list_rows = |connection: &Connection| {
            connection
                .prepare("SELECT id, name, name_key, position FROM lists ORDER BY id")
                .unwrap()
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        };
        let before_lists = list_rows(&connection);
        let error = Store::open_for_mcp(&path, Some("Home"), true)
            .err()
            .unwrap();
        assert!(
            error
                .to_string()
                .contains("Open this profile with writable Husker")
        );
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 3);
        assert!(
            connection
                .prepare("SELECT status_changed_at FROM tasks")
                .is_err()
        );
        assert_eq!(tasks_without_recorded_times(&connection), before);
        assert_eq!(list_rows(&connection), before_lists);
        drop(connection);

        let mut store = Store::open(&path).unwrap();
        assert_eq!(store.tasks().unwrap(), before);
        assert_eq!(list_rows(&store.connection), before_lists);
        assert_eq!(
            store
                .lists()
                .unwrap()
                .iter()
                .map(|list| list.id)
                .collect::<Vec<_>>(),
            [1, 3, 2]
        );
        let version: i64 = store
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 5);
        assert_eq!(store.create_task(&input("Next task")).unwrap().id, 100);
        drop(store);
        let readonly = Store::open_for_mcp(&path, Some("Home"), true).unwrap();
        assert_eq!(readonly.list_scope(), Some(2));
        assert_eq!(
            readonly.task(9).unwrap(),
            before.iter().find(|task| task.id == 9).unwrap().clone()
        );
        assert!(readonly.task(10).is_err());
    }

    #[test]
    fn failed_timestamp_migration_rolls_back_every_prior_upgrade_stage() {
        let directory = tempfile::tempdir().unwrap();
        for version in [1, 2, 3] {
            let path = directory.path().join(format!("schema-{version}.db"));
            let mut connection = legacy_database(&path);
            {
                let transaction = connection.transaction().unwrap();
                if version >= 2 {
                    migrate_schema_one(&transaction).unwrap();
                }
                if version >= 3 {
                    migrate_schema_two(&transaction).unwrap();
                }
                transaction.commit().unwrap();
            }
            // This conflicting column fails only the final migration stage.
            connection
                .execute_batch(
                    "ALTER TABLE tasks ADD COLUMN status_changed_at TEXT DEFAULT 'untouched';",
                )
                .unwrap();
            let schema = |connection: &Connection| {
                connection
                    .prepare(
                        "SELECT name, sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY name",
                    )
                    .unwrap()
                    .query_map([], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })
                    .unwrap()
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .unwrap()
            };
            let rows = |connection: &Connection| {
                connection
                    .prepare(
                        "SELECT id, description, version, status_changed_at FROM tasks ORDER BY id",
                    )
                    .unwrap()
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    })
                    .unwrap()
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .unwrap()
            };
            let original_schema = schema(&connection);
            let original_rows = rows(&connection);
            assert!(Store::open(&path).is_err());
            let actual: i64 = connection
                .pragma_query_value(None, "user_version", |row| row.get(0))
                .unwrap();
            assert_eq!(actual, version);
            assert_eq!(schema(&connection), original_schema);
            assert_eq!(rows(&connection), original_rows);
        }
    }

    #[test]
    fn failed_second_migration_rolls_back_the_entire_schema_one_upgrade() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let connection = legacy_database(&path);
        // A conflicting column makes the second migration fail after tag conversion.
        connection
            .execute_batch("ALTER TABLE lists ADD COLUMN position INTEGER DEFAULT 42;")
            .unwrap();
        assert!(Store::open(&path).is_err());
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 1);
        let (description, tags, ready, task_version): (String, String, bool, i64) = connection
            .query_row(
                "SELECT description, tags, ready, version FROM tasks WHERE id = 7",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(description, "Keep this description");
        assert_eq!(tags, "[\"shopping\",\"ærinder\"]");
        assert!(ready);
        assert_eq!(task_version, 3);
        assert!(
            connection
                .prepare("SELECT status_changed_on FROM tasks")
                .is_err()
        );
        let positions: Vec<i64> = connection
            .prepare("SELECT position FROM lists ORDER BY id")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(positions, [42, 42]);
    }

    #[test]
    fn status_dates_follow_transitions_and_survive_other_changes() {
        let mut store = memory_store();
        let today = NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        let tomorrow = today + chrono::Duration::days(1);
        let task = store.create_task(&input("Finish")).unwrap();
        let task = store
            .set_status_on(task.id, task.version, Status::Done, today)
            .unwrap();
        assert_eq!(task.status_changed_on, Some(today));
        let task = store
            .set_status_on(task.id, task.version, Status::Done, tomorrow)
            .unwrap();
        assert_eq!(task.status_changed_on, Some(today));
        let mut fields = TaskInput::from(&task);
        fields.description = "New details".into();
        let task = store.update_task(task.id, task.version, &fields).unwrap();
        let task = store.set_deleted(task.id, task.version, true).unwrap();
        let task = store.set_deleted(task.id, task.version, false).unwrap();
        let neighbor = store.create_task(&input("Neighbor")).unwrap();
        let neighbor = store
            .set_status_on(neighbor.id, neighbor.version, Status::Done, today)
            .unwrap();
        assert!(store.swap_positions(&task, &neighbor).is_err());
        assert_eq!(store.task(task.id).unwrap(), task);
        assert_eq!(store.task(neighbor.id).unwrap(), neighbor);
        assert_eq!(task.status_changed_on, Some(today));
        let task = store
            .set_status_on(task.id, task.version, Status::Cancelled, tomorrow)
            .unwrap();
        assert_eq!(task.status_changed_on, Some(tomorrow));
        let task = store
            .set_status_on(task.id, task.version, Status::Cancelled, today)
            .unwrap();
        assert_eq!(task.status_changed_on, Some(tomorrow));
        let task = store
            .set_status_on(task.id, task.version, Status::Open, tomorrow)
            .unwrap();
        assert_eq!(task.status_changed_on, None);
        let neighbor = store
            .set_status_on(neighbor.id, neighbor.version, Status::Open, tomorrow)
            .unwrap();
        store.swap_positions(&task, &neighbor).unwrap();
        let task = store.task(task.id).unwrap();
        assert_eq!(task.status_changed_on, None);
        assert_eq!(store.task(neighbor.id).unwrap().status_changed_on, None);
        let task = store
            .set_status_on(task.id, task.version, Status::Done, tomorrow)
            .unwrap();
        assert_eq!(task.status_changed_on, Some(tomorrow));
    }

    #[test]
    fn closure_timestamps_follow_transitions_and_survive_unrelated_writes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut store = Store::open(&path).unwrap();
        let list = store.create_list("Work").unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        let tomorrow = NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
        let first = timestamp("2026-11-07T23:30:00.123456789Z");
        let second = timestamp("2026-11-08T15:30:00Z");
        let third = timestamp("2026-11-08T15:31:00Z");
        let mut fields = input("Original");
        fields.list_id = list.id;
        let mut task = store.create_task(&fields).unwrap();
        assert_eq!(task.status_changed_at, None);
        task = store
            .set_status_at(task.id, task.version, Status::Done, today, first)
            .unwrap();
        assert_eq!(task.status_changed_on, Some(today));
        assert_eq!(task.status_changed_at, Some(first));
        task = store
            .set_status_at(task.id, task.version, Status::Done, tomorrow, second)
            .unwrap();
        fields.title = "Edited".into();
        task = store.update_task(task.id, task.version, &fields).unwrap();
        task = store
            .set_due(task.id, task.version, Some(tomorrow))
            .unwrap();
        task = store.set_deleted(task.id, task.version, true).unwrap();
        task = store.set_deleted(task.id, task.version, false).unwrap();
        let preview = store.preview_list_deletion(list.id).unwrap();
        assert_eq!(
            store.delete_list(&preview).unwrap(),
            ListDeletionOutcome::Deleted
        );
        let moved = store.task(task.id).unwrap();
        assert_eq!(moved.version, task.version + 1);
        assert_eq!(moved.list_id, 1);
        task = moved;
        assert_eq!(task.status_changed_on, Some(today));
        assert_eq!(task.status_changed_at, Some(first));
        task = store
            .set_status_at(task.id, task.version, Status::Cancelled, today, second)
            .unwrap();
        assert_eq!(task.status_changed_on, Some(today));
        assert_eq!(task.status_changed_at, Some(second));
        task = store
            .set_status_at(task.id, task.version, Status::Cancelled, tomorrow, third)
            .unwrap();
        assert_eq!(task.status_changed_on, Some(today));
        assert_eq!(task.status_changed_at, Some(second));
        task = store
            .set_status_at(task.id, task.version, Status::Done, today, third)
            .unwrap();
        assert_eq!(task.status_changed_at, Some(third));
        task = store
            .set_status_at(task.id, task.version, Status::Open, tomorrow, third)
            .unwrap();
        assert_eq!(task.status_changed_on, None);
        assert_eq!(task.status_changed_at, None);
        let final_instant = timestamp("2026-11-09T11:00:00.987654321Z");
        task = store
            .set_status_at(task.id, task.version, Status::Done, tomorrow, final_instant)
            .unwrap();
        assert_eq!(task.status_changed_on, Some(tomorrow));
        assert_eq!(task.status_changed_at, Some(final_instant));
        drop(store);
        let reopened = Store::open(&path).unwrap();
        assert_eq!(reopened.task(task.id).unwrap(), task);
        let json = serde_json::to_value(&task).unwrap();
        assert_eq!(json["status_changed_at"], "2026-11-09T11:00:00.987654321Z");
    }

    #[test]
    fn repeated_closed_status_does_not_invent_an_unknown_completion_time() {
        let mut store = memory_store();
        let original_date = NaiveDate::from_ymd_opt(2026, 11, 7).unwrap();
        let later_date = NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        let now = timestamp("2026-11-08T15:30:00Z");
        for status in [Status::Done, Status::Cancelled] {
            let task = store.create_task(&input("Legacy completion")).unwrap();
            store
                .connection
                .execute(
                    "UPDATE tasks SET status = ?1, status_changed_on = ?2 WHERE id = ?3",
                    params![status.as_str(), original_date.to_string(), task.id],
                )
                .unwrap();
            let changed = store
                .set_status_at(task.id, task.version, status, later_date, now)
                .unwrap();
            assert_eq!(changed.status_changed_on, Some(original_date));
            assert_eq!(changed.status_changed_at, None);
            assert_eq!(changed.version, task.version + 1);
        }
    }

    #[test]
    fn data_version_changes_for_external_commits_only() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut first = Store::open(&path).unwrap();
        let mut second = Store::open(&path).unwrap();
        let initial = first.data_version().unwrap();
        first.create_task(&input("Own write")).unwrap();
        assert_eq!(first.data_version().unwrap(), initial);
        second.create_task(&input("External write")).unwrap();
        let changed = first.data_version().unwrap();
        assert_ne!(changed, initial);
        assert_eq!(first.data_version().unwrap(), changed);
    }

    #[test]
    fn task_fields_survive_reopening_and_list_rename() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut store = Store::open(&path).unwrap();
        assert_eq!(store.lists().unwrap()[0].name, "Inbox");
        let list = store.create_list("Private").unwrap();
        let mut fields = input("  Buy milk  ");
        fields.list_id = list.id;
        fields.description = "Two cartons\nOat milk #shopping".into();
        fields.due = NaiveDate::from_ymd_opt(2026, 11, 8);
        let task = store.create_task(&fields).unwrap();
        store.rename_list(list.id, "Home").unwrap();
        drop(store);
        let store = Store::open(&path).unwrap();
        let saved = store.task(task.id).unwrap();
        assert_eq!(saved.title, "Buy milk");
        assert_eq!(saved.description, fields.description);
        assert_eq!(saved.due, fields.due);
        assert_eq!(saved.list_id, list.id);
        assert_eq!(saved.status_changed_on, None);
        assert_eq!(store.lists().unwrap()[1].name, "Home");
    }

    #[test]
    fn list_names_are_trimmed_and_unicode_case_insensitive() {
        let mut store = memory_store();
        let list = store.create_list("  Ærinder  ").unwrap();
        assert_eq!(list.name, "Ærinder");
        assert!(store.create_list("ÆRINDER").is_err());
        assert!(store.rename_list(1, "ærinder").is_err());
        assert_eq!(store.lists().unwrap()[0].name, "Inbox");
        assert!(store.create_list(" ").is_err());
        assert!(store.create_list("TRASH").is_err());
        assert!(store.create_list("Papirkurv").is_err());
        assert!(store.rename_list(999, "Missing").is_err());
        assert_eq!(
            store.rename_list(list.id, "ærinder").unwrap().name,
            "ærinder"
        );
    }

    #[test]
    fn lists_append_and_rename_without_changing_order_or_default() {
        let mut store = memory_store();
        let beta = store.create_list("Beta").unwrap();
        store.create_list("apple").unwrap();
        store.create_list("Ærinder").unwrap();
        assert_eq!(
            store
                .lists()
                .unwrap()
                .iter()
                .map(|list| list.name.as_str())
                .collect::<Vec<_>>(),
            ["Inbox", "Beta", "apple", "Ærinder"]
        );
        assert_eq!(store.default_list_id().unwrap(), 1);
        store.rename_list(beta.id, "aardvark").unwrap();
        assert_eq!(store.lists().unwrap()[1].id, beta.id);
        store.rename_list(1, "Personal").unwrap();
        assert_eq!(store.lists().unwrap()[0].name, "Personal");
        assert_eq!(store.default_list_id().unwrap(), 1);
    }

    #[test]
    fn list_moves_persist_without_changing_tasks_or_pinned_boundaries() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut store = Store::open(&path).unwrap();
        let work = store.create_list("Work").unwrap();
        let home = store.create_list("Home").unwrap();
        let mut fields = input("Task to preserve");
        fields.list_id = work.id;
        fields.description = "Keep the task fields".into();
        fields.due = NaiveDate::from_ymd_opt(2026, 11, 8);
        let task = store.create_task(&fields).unwrap();
        store
            .set_status_on(
                task.id,
                task.version,
                Status::Done,
                NaiveDate::from_ymd_opt(2026, 11, 7).unwrap(),
            )
            .unwrap();
        let tasks_before = store.tasks().unwrap();

        store.move_list(1, true).unwrap();
        store.move_list(1, false).unwrap();
        store.move_list(work.id, false).unwrap();
        store.move_list(home.id, true).unwrap();
        assert_eq!(
            store
                .lists()
                .unwrap()
                .iter()
                .map(|list| list.id)
                .collect::<Vec<_>>(),
            [1, work.id, home.id]
        );
        store.move_list(home.id, false).unwrap();
        store.rename_list(home.id, "Zebra").unwrap();
        store.rename_list(1, "Personal").unwrap();
        let admin = store.create_list("Admin").unwrap();
        assert!(store.move_list(999, true).is_err());
        drop(store);

        let store = Store::open(&path).unwrap();
        let lists = store.lists().unwrap();
        assert_eq!(
            lists.iter().map(|list| list.id).collect::<Vec<_>>(),
            [1, home.id, work.id, admin.id]
        );
        assert_eq!(
            lists
                .iter()
                .map(|list| list.name.as_str())
                .collect::<Vec<_>>(),
            ["Personal", "Zebra", "Work", "Admin"]
        );
        assert_eq!(store.default_list_id().unwrap(), 1);
        assert_eq!(store.tasks().unwrap(), tasks_before);
    }

    #[test]
    fn list_moves_use_current_neighbors_after_another_connection_reorders() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut first = Store::open(&path).unwrap();
        let a = first.create_list("A").unwrap();
        let b = first.create_list("B").unwrap();
        let c = first.create_list("C").unwrap();
        let original = first.lists().unwrap();
        let mut second = Store::open(&path).unwrap();
        second.move_list(b.id, true).unwrap();
        assert_eq!(
            second
                .lists()
                .unwrap()
                .iter()
                .map(|list| list.id)
                .collect::<Vec<_>>(),
            [1, a.id, c.id, b.id]
        );
        first.move_list(b.id, false).unwrap();
        assert_eq!(first.lists().unwrap(), original);
        assert_eq!(second.lists().unwrap(), original);
    }

    #[test]
    fn list_deletion_preserves_all_tasks_and_invalidates_old_versions() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut store = Store::open(&path).unwrap();
        store.rename_list(1, "Personal").unwrap();
        let work = store.create_list("Work").unwrap();
        let other = store.create_list("Other").unwrap();
        let untouched = store.create_task(&input("Already in Personal")).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 11, 8).unwrap();
        let mut source_tasks = Vec::new();
        for (index, (status, deleted)) in [
            (Status::Open, false),
            (Status::Open, false),
            (Status::Done, false),
            (Status::Cancelled, false),
            (Status::Open, true),
            (Status::Done, true),
            (Status::Cancelled, true),
        ]
        .into_iter()
        .enumerate()
        {
            let mut fields = input(&format!("Task {index}"));
            fields.list_id = work.id;
            fields.description = "Keep æbler\nand #shopping".into();
            fields.due = Some(today);
            let mut task = store.create_task(&fields).unwrap();
            if status != Status::Open {
                task = store
                    .set_status_on(task.id, task.version, status, today)
                    .unwrap();
            }
            if deleted {
                task = store.set_deleted(task.id, task.version, true).unwrap();
            }
            source_tasks.push(task);
        }
        let preview = store.preview_list_deletion(work.id).unwrap();
        assert_eq!(preview.list_name, "Work");
        assert_eq!(preview.destination_id, 1);
        assert_eq!(preview.destination_name, "Personal");
        assert_eq!(
            (
                preview.open_count,
                preview.done_count,
                preview.cancelled_count,
                preview.trashed_count,
            ),
            (2, 1, 1, 3)
        );
        assert_eq!(
            store.delete_list(&preview).unwrap(),
            ListDeletionOutcome::Deleted
        );
        assert_eq!(
            store
                .lists()
                .unwrap()
                .iter()
                .map(|list| list.id)
                .collect::<Vec<_>>(),
            [1, other.id]
        );
        assert_eq!(store.lists().unwrap()[0].count, 3);
        assert_eq!(store.task(untouched.id).unwrap(), untouched);
        for original in &source_tasks {
            let mut expected = original.clone();
            expected.list_id = 1;
            expected.version += 1;
            assert_eq!(store.task(original.id).unwrap(), expected);
            assert!(
                store
                    .update_task(original.id, original.version, &TaskInput::from(original))
                    .unwrap_err()
                    .to_string()
                    .contains("changed in another process")
            );
        }
        drop(store);
        let mut store = Store::open(&path).unwrap();
        let trashed = store.task(source_tasks[5].id).unwrap();
        assert!(trashed.deleted);
        let restored = store
            .set_deleted(trashed.id, trashed.version, false)
            .unwrap();
        assert_eq!(restored.list_id, 1);
        assert_eq!(restored.status, Status::Done);
        assert_eq!(restored.status_changed_on, Some(today));
        let schema: i64 = store
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(schema, 5);
    }

    #[test]
    fn list_deletion_reconfirms_names_and_task_edits() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut store = Store::open(&path).unwrap();
        let work = store.create_list("Work").unwrap();
        let mut fields = input("Original");
        fields.list_id = work.id;
        let original = store.create_task(&fields).unwrap();
        let mut external = Store::open(&path).unwrap();
        let preview = store.preview_list_deletion(work.id).unwrap();
        external.rename_list(work.id, "Project").unwrap();
        let ListDeletionOutcome::Changed(renamed) = store.delete_list(&preview).unwrap() else {
            panic!("Renaming the source must require confirmation again");
        };
        assert_eq!(renamed.list_name, "Project");
        assert_eq!(store.task(original.id).unwrap(), original);
        external.rename_list(1, "Personal").unwrap();
        let ListDeletionOutcome::Changed(destination) = store.delete_list(&renamed).unwrap() else {
            panic!("Renaming the destination must require confirmation again");
        };
        assert_eq!(destination.destination_name, "Personal");
        fields.description = "New content from another process".into();
        let saved = external
            .update_task(original.id, original.version, &fields)
            .unwrap();
        let ListDeletionOutcome::Changed(edited) = store.delete_list(&destination).unwrap() else {
            panic!("Editing a task must require confirmation again");
        };
        assert_eq!(edited.open_count, destination.open_count);
        assert_eq!(store.task(original.id).unwrap(), saved);
        external.create_task(&fields).unwrap();
        let ListDeletionOutcome::Changed(added) = store.delete_list(&edited).unwrap() else {
            panic!("Adding a task must require confirmation again");
        };
        assert_eq!(added.open_count, 2);
        assert_eq!(store.tasks().unwrap().len(), 2);
        assert!(
            store
                .tasks()
                .unwrap()
                .iter()
                .all(|task| task.list_id == work.id)
        );
        assert_eq!(store.lists().unwrap().len(), 2);
    }

    #[test]
    fn list_deletion_detects_replaced_tasks_with_unchanged_counts() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut store = Store::open(&path).unwrap();
        let work = store.create_list("Work").unwrap();
        let mut fields = input("Original");
        fields.list_id = work.id;
        let original = store.create_task(&fields).unwrap();
        let preview = store.preview_list_deletion(work.id).unwrap();
        let mut external = Store::open(&path).unwrap();
        fields.list_id = 1;
        let moved = external
            .update_task(original.id, original.version, &fields)
            .unwrap();
        fields.list_id = work.id;
        let replacement = external.create_task(&fields).unwrap();
        let ListDeletionOutcome::Changed(current) = store.delete_list(&preview).unwrap() else {
            panic!("Replacing a task must require confirmation even when counts match");
        };
        assert_eq!(current.open_count, preview.open_count);
        assert_eq!(store.task(original.id).unwrap(), moved);
        assert_eq!(store.task(replacement.id).unwrap(), replacement);
        assert_eq!(store.lists().unwrap().len(), 2);
    }

    #[test]
    fn list_deletion_rolls_back_when_deleting_metadata_fails() {
        let mut store = memory_store();
        let work = store.create_list("Work").unwrap();
        let mut fields = input("Keep this task");
        fields.list_id = work.id;
        store.create_task(&fields).unwrap();
        let tasks_before = store.tasks().unwrap();
        let lists_before = store.lists().unwrap();
        let preview = store.preview_list_deletion(work.id).unwrap();
        store
            .connection
            .execute_batch(
                "CREATE TRIGGER block_list_deletion BEFORE DELETE ON lists
             BEGIN SELECT RAISE(ABORT, 'test list deletion failure'); END;",
            )
            .unwrap();
        assert!(store.delete_list(&preview).is_err());
        assert_eq!(store.tasks().unwrap(), tasks_before);
        assert_eq!(store.lists().unwrap(), lists_before);
        store
            .connection
            .execute_batch("DROP TRIGGER block_list_deletion")
            .unwrap();
        assert_eq!(
            store.delete_list(&preview).unwrap(),
            ListDeletionOutcome::Deleted
        );
    }

    #[test]
    fn list_deletion_protects_the_default_and_restricted_stores() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut store = Store::open(&path).unwrap();
        let work = store.create_list("Work").unwrap();
        store.rename_list(1, "Personal").unwrap();
        let preview = store.preview_list_deletion(work.id).unwrap();
        let lists_before = store.lists().unwrap();
        assert_eq!(
            store.preview_list_deletion(1).unwrap_err().to_string(),
            "The default list cannot be deleted."
        );
        assert!(store.preview_list_deletion(999).is_err());
        let mut default_attempt = preview.clone();
        default_attempt.list_id = 1;
        assert_eq!(
            store.delete_list(&default_attempt).unwrap_err().to_string(),
            "The default list cannot be deleted."
        );
        for (scope, read_only) in [(Some("Work"), false), (Some("Work"), true), (None, true)] {
            let mut restricted = Store::open_for_mcp(&path, scope, read_only).unwrap();
            let expected = if read_only {
                "This store is read-only."
            } else {
                "A list-scoped store cannot create, rename, reorder, or delete lists."
            };
            for id in [work.id, 1, 999] {
                assert_eq!(
                    restricted
                        .preview_list_deletion(id)
                        .unwrap_err()
                        .to_string(),
                    expected
                );
            }
            assert_eq!(
                restricted.delete_list(&preview).unwrap_err().to_string(),
                expected
            );
        }
        assert_eq!(store.lists().unwrap(), lists_before);
        assert!(store.tasks().unwrap().is_empty());
        assert_eq!(
            store.delete_list(&preview).unwrap(),
            ListDeletionOutcome::Deleted
        );
        assert_eq!(
            store.delete_list(&preview).unwrap(),
            ListDeletionOutcome::Missing
        );
        assert_eq!(store.lists().unwrap().len(), 1);
    }

    #[test]
    fn removed_list_scopes_do_not_follow_transferred_tasks_or_reused_names() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut store = Store::open(&path).unwrap();
        let work = store.create_list("Work").unwrap();
        let mut fields = input("Transferred task");
        fields.list_id = work.id;
        let original = store.create_task(&fields).unwrap();
        let trashed = store.create_task(&fields).unwrap();
        store
            .set_deleted(trashed.id, trashed.version, true)
            .unwrap();
        let mut scoped = Store::open_for_mcp(&path, Some("Work"), false).unwrap();
        let readonly = Store::open_for_mcp(&path, Some("Work"), true).unwrap();
        let preview = store.preview_list_deletion(work.id).unwrap();
        assert_eq!(
            store.delete_list(&preview).unwrap(),
            ListDeletionOutcome::Deleted
        );
        let replacement = store.create_list("Work").unwrap();
        assert_ne!(replacement.id, work.id);
        fields.list_id = replacement.id;
        let replacement_task = store.create_task(&fields).unwrap();
        for connection in [&scoped, &readonly] {
            assert_eq!(connection.list_scope(), Some(work.id));
            assert!(connection.lists().unwrap().is_empty());
            assert!(connection.tasks().unwrap().is_empty());
            assert!(connection.default_list_id().is_err());
            for id in [original.id, trashed.id, replacement_task.id, 999] {
                assert_eq!(
                    connection.task(id).unwrap_err().to_string(),
                    "Task not found."
                );
            }
        }
        let transferred = store.task(original.id).unwrap();
        for version in [original.version, transferred.version] {
            assert_eq!(
                scoped
                    .update_task(original.id, version, &TaskInput::from(&original))
                    .unwrap_err()
                    .to_string(),
                "Task not found."
            );
            assert_eq!(
                scoped
                    .set_status(original.id, version, Status::Done)
                    .unwrap_err()
                    .to_string(),
                "Task not found."
            );
            assert_eq!(
                scoped
                    .set_deleted(original.id, version, true)
                    .unwrap_err()
                    .to_string(),
                "Task not found."
            );
        }
        for destination in [work.id, 1, replacement.id] {
            fields.list_id = destination;
            assert!(scoped.create_task(&fields).is_err());
        }
        assert_eq!(store.task(original.id).unwrap(), transferred);
        assert_eq!(store.task(replacement_task.id).unwrap(), replacement_task);
        assert_eq!(store.tasks().unwrap().len(), 3);
    }

    #[test]
    fn invalid_task_fields_preserve_the_existing_task() {
        let mut store = memory_store();
        let task = store.create_task(&input("Buy milk")).unwrap();
        let mut fields = TaskInput::from(&task);
        fields.title = " \n ".into();
        assert!(store.update_task(task.id, task.version, &fields).is_err());
        fields.title = "New title".into();
        fields.list_id = 999;
        assert!(store.update_task(task.id, task.version, &fields).is_err());
        assert_eq!(store.task(task.id).unwrap().title, "Buy milk");
        assert_eq!(store.task(task.id).unwrap().version, task.version);
    }

    #[test]
    fn trash_preserves_status_and_list_counts_only_open_tasks() {
        let mut store = memory_store();
        let open = store.create_task(&input("Open")).unwrap();
        let done = store.create_task(&input("Done")).unwrap();
        let done = store
            .set_status(done.id, done.version, Status::Done)
            .unwrap();
        let cancelled = store.create_task(&input("Cancelled")).unwrap();
        store
            .set_status(cancelled.id, cancelled.version, Status::Cancelled)
            .unwrap();
        let deleted = store.create_task(&input("Deleted")).unwrap();
        store
            .set_deleted(deleted.id, deleted.version, true)
            .unwrap();
        assert_eq!(store.lists().unwrap()[0].count, 1);
        let done = store.set_deleted(done.id, done.version, true).unwrap();
        let restored = store.set_deleted(done.id, done.version, false).unwrap();
        assert_eq!(restored.status, Status::Done);
        assert_eq!(restored.list_id, open.list_id);
        assert!(!restored.deleted);
        assert_eq!(store.lists().unwrap()[0].count, 1);
    }

    #[test]
    fn purge_removes_only_a_current_trash_task_and_never_reuses_its_id() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut store = Store::open(&path).unwrap();
        let mut other = Store::open(&path).unwrap();
        let kept = store.create_task(&input("Kept")).unwrap();
        let open = store.create_task(&input("Open")).unwrap();
        let trashed = store.create_task(&input("Trashed")).unwrap();
        let trashed = store
            .set_deleted(trashed.id, trashed.version, true)
            .unwrap();
        let before = store.tasks().unwrap();

        assert_eq!(
            store.purge_task(open.id, open.version).unwrap(),
            TaskPurgeOutcome::NotInTrash
        );
        assert_eq!(
            store.purge_task(999, 1).unwrap(),
            TaskPurgeOutcome::NotInTrash
        );
        let mut edited_input = TaskInput::from(&trashed);
        edited_input.title = "Edited elsewhere".into();
        let edited = other
            .update_task(trashed.id, trashed.version, &edited_input)
            .unwrap();
        assert_eq!(
            store.purge_task(trashed.id, trashed.version).unwrap(),
            TaskPurgeOutcome::Changed(edited.clone())
        );
        assert_eq!(store.task(trashed.id).unwrap(), edited);
        assert_eq!(store.tasks().unwrap()[..2], before[..2]);

        assert_eq!(
            store.purge_task(trashed.id, edited.version).unwrap(),
            TaskPurgeOutcome::Deleted
        );
        assert_eq!(
            store.task(trashed.id).unwrap_err().to_string(),
            format!("Task {} does not exist.", trashed.id)
        );
        assert_eq!(store.tasks().unwrap(), before[..2]);
        assert_eq!(store.task(kept.id).unwrap(), kept);
        let created = store.create_task(&input("After purge")).unwrap();
        assert_eq!(created.id, trashed.id + 1);
    }

    #[test]
    fn purge_refuses_a_task_restored_elsewhere() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut store = Store::open(&path).unwrap();
        let mut other = Store::open(&path).unwrap();
        let task = store.create_task(&input("Restored")).unwrap();
        let task = store.set_deleted(task.id, task.version, true).unwrap();
        let restored = other.set_deleted(task.id, task.version, false).unwrap();
        assert_eq!(
            store.purge_task(task.id, restored.version).unwrap(),
            TaskPurgeOutcome::NotInTrash
        );
        assert_eq!(store.task(task.id).unwrap(), restored);
    }

    #[test]
    fn empty_trash_revalidates_the_confirmed_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut store = Store::open(&path).unwrap();
        let mut other = Store::open(&path).unwrap();
        let work = store.create_list("Work").unwrap();
        let mut work_input = input("Work task");
        work_input.list_id = work.id;
        let kept = store.create_task(&input("Kept")).unwrap();
        let first = store.create_task(&work_input).unwrap();
        let first = store.set_deleted(first.id, first.version, true).unwrap();
        let second = store.create_task(&input("Done")).unwrap();
        let second = store
            .set_status(second.id, second.version, Status::Done)
            .unwrap();
        let second = store.set_deleted(second.id, second.version, true).unwrap();
        let replacement = store.create_task(&input("Replacement")).unwrap();

        let preview = store.preview_empty_trash().unwrap();
        assert_eq!(preview.count, 2);
        // Swapping one task for another keeps the count but changes the snapshot.
        other.set_deleted(first.id, first.version, false).unwrap();
        let replacement = other
            .set_deleted(replacement.id, replacement.version, true)
            .unwrap();
        let before = store.tasks().unwrap();
        let EmptyTrashOutcome::Changed(fresh) = store.empty_trash(&preview).unwrap() else {
            panic!("A changed Trash must not be emptied");
        };
        assert_eq!(fresh.count, 2);
        assert_eq!(
            fresh.task_versions,
            [
                (second.id, second.version),
                (replacement.id, replacement.version)
            ]
        );
        assert_eq!(store.tasks().unwrap(), before);

        assert_eq!(
            store.empty_trash(&fresh).unwrap(),
            EmptyTrashOutcome::Emptied(2)
        );
        let remaining = store.tasks().unwrap();
        assert!(remaining.iter().all(|task| !task.deleted));
        assert_eq!(
            remaining.iter().map(|task| task.id).collect::<Vec<_>>(),
            [kept.id, first.id]
        );
        assert_eq!(store.task(kept.id).unwrap(), kept);
        assert_eq!(store.lists().unwrap().len(), 2);
        assert_eq!(store.preview_empty_trash().unwrap().count, 0);
        assert_eq!(
            store
                .empty_trash(&store.preview_empty_trash().unwrap())
                .unwrap(),
            EmptyTrashOutcome::Emptied(0)
        );
    }

    #[test]
    fn scoped_stores_cannot_delete_permanently() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut store = Store::open(&path).unwrap();
        let task = store.create_task(&input("Trashed")).unwrap();
        let task = store.set_deleted(task.id, task.version, true).unwrap();
        let preview = store.preview_empty_trash().unwrap();
        let mut scoped = Store::open_for_mcp(&path, Some("Inbox"), false).unwrap();
        let errors = [
            scoped.purge_task(task.id, task.version).unwrap_err(),
            scoped.preview_empty_trash().unwrap_err(),
            scoped.empty_trash(&preview).unwrap_err(),
        ];
        for error in errors {
            assert_eq!(
                error.to_string(),
                "A list-scoped store cannot delete tasks permanently."
            );
        }
        assert_eq!(store.task(task.id).unwrap(), task);
    }

    #[test]
    fn stale_mutations_preserve_the_first_saved_version() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut first = Store::open(&path).unwrap();
        let mut second = Store::open(&path).unwrap();
        let original = first.create_task(&input("Original")).unwrap();
        let mut fields = TaskInput::from(&original);
        fields.title = "First save".into();
        let saved = first
            .update_task(original.id, original.version, &fields)
            .unwrap();
        fields.title = "Stale save".into();
        assert!(
            second
                .update_task(original.id, original.version, &fields)
                .is_err()
        );
        assert!(
            second
                .set_status(original.id, original.version, Status::Done)
                .is_err()
        );
        assert!(
            second
                .set_deleted(original.id, original.version, true)
                .is_err()
        );
        let current = second.task(original.id).unwrap();
        assert_eq!(current.title, "First save");
        assert_eq!(current.version, saved.version);
        assert_eq!(current.status, Status::Open);
        assert!(!current.deleted);
    }

    #[test]
    fn reorder_checks_both_versions_before_swapping() {
        let mut store = memory_store();
        let first = store.create_task(&input("First")).unwrap();
        let second = store.create_task(&input("Second")).unwrap();
        store
            .set_status(second.id, second.version, Status::Done)
            .unwrap();
        assert!(store.swap_positions(&first, &second).is_err());
        assert_eq!(store.task(first.id).unwrap().position, first.position);
        assert_eq!(store.task(first.id).unwrap().version, first.version);
        assert_eq!(store.task(second.id).unwrap().position, second.position);
    }

    #[test]
    fn reorder_stays_within_a_date_or_undated_group() {
        let mut store = memory_store();
        let mut fields = input("First");
        fields.due = NaiveDate::from_ymd_opt(2026, 11, 8);
        let first = store.create_task(&fields).unwrap();
        fields.due = NaiveDate::from_ymd_opt(2026, 11, 9);
        let next_day = store.create_task(&fields).unwrap();
        assert!(store.swap_positions(&first, &next_day).is_err());
        fields.due = None;
        let undated = store.create_task(&fields).unwrap();
        let another_undated = store.create_task(&fields).unwrap();
        assert!(store.swap_positions(&first, &undated).is_err());
        assert!(store.swap_positions(&first, &first).is_err());
        store.swap_positions(&undated, &another_undated).unwrap();
    }

    #[test]
    fn reorder_rejects_every_closed_task_without_changing_either_row() {
        let mut store = memory_store();
        let open = store.create_task(&input("Open")).unwrap();
        let other = store.create_task(&input("Other")).unwrap();
        let done = store
            .set_status(other.id, other.version, Status::Done)
            .unwrap();
        assert!(store.swap_positions(&open, &done).is_err());
        assert_eq!(store.task(open.id).unwrap(), open);
        assert_eq!(store.task(done.id).unwrap(), done);
        let cancelled = store
            .set_status(open.id, open.version, Status::Cancelled)
            .unwrap();
        assert_eq!(
            store
                .swap_positions(&cancelled, &done)
                .unwrap_err()
                .to_string(),
            "Only open tasks can be reordered."
        );
        let mut claimed_open = cancelled.clone();
        claimed_open.status = Status::Open;
        let mut other_claimed_open = done.clone();
        other_claimed_open.status = Status::Open;
        assert!(
            store
                .swap_positions(&claimed_open, &other_claimed_open)
                .is_err()
        );
        assert_eq!(store.task(done.id).unwrap(), done);
        assert_eq!(store.task(cancelled.id).unwrap(), cancelled);
    }

    #[test]
    fn valid_reorder_changes_only_position_and_version() {
        let mut store = memory_store();
        let first = store.create_task(&input("First")).unwrap();
        let second = store.create_task(&input("Second")).unwrap();
        store.swap_positions(&first, &second).unwrap();
        let moved = store.task(first.id).unwrap();
        assert_eq!(moved.position, second.position);
        assert_eq!(moved.version, first.version + 1);
        assert_eq!(moved.title, first.title);
        assert_eq!(moved.due, first.due);
        assert_eq!(moved.status_changed_on, first.status_changed_on);
        let second = store.task(second.id).unwrap();
        let deleted = store.set_deleted(second.id, second.version, true).unwrap();
        assert!(store.swap_positions(&moved, &deleted).is_err());
    }

    #[test]
    fn clearing_a_due_date_preserves_other_fields() {
        let mut store = memory_store();
        let mut fields = input("Dated");
        fields.due = NaiveDate::from_ymd_opt(2026, 11, 8);
        let task = store.create_task(&fields).unwrap();
        fields.due = None;
        let task = store.update_task(task.id, task.version, &fields).unwrap();
        assert_eq!(task.due, None);
        assert_eq!(task.status, Status::Open);
        assert_eq!(task.status_changed_on, None);
        assert_eq!(task.title, "Dated");
    }

    #[test]
    fn due_only_updates_preserve_exact_fields_for_all_statuses_and_trash() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut store = Store::open(&path).unwrap();
        let list = store.create_list("Work").unwrap();
        let mut fields = input("Original");
        fields.list_id = list.id;
        let mut expected_tasks = Vec::new();
        for status in [Status::Open, Status::Done, Status::Cancelled] {
            for deleted in [false, true] {
                let task = store.create_task(&fields).unwrap();
                if status != Status::Open {
                    store
                        .set_status_at(
                            task.id,
                            task.version,
                            status,
                            NaiveDate::from_ymd_opt(2026, 11, 7).unwrap(),
                            timestamp("2026-11-07T15:30:00.123456789Z"),
                        )
                        .unwrap();
                }
                store
                    .connection
                    .execute(
                        "UPDATE tasks SET title = ?1, description = ?2, status = ?3,
                         status_changed_on = ?4, deleted = ?5, position = -19 WHERE id = ?6",
                        params![
                            "\t Legacy title \r\n",
                            "One\r\næble\0界",
                            status.as_str(),
                            (status != Status::Open).then_some("2026-11-07"),
                            deleted,
                            task.id,
                        ],
                    )
                    .unwrap();
                let mut expected = store.task(task.id).unwrap();
                for due in [
                    NaiveDate::from_ymd_opt(2026, 11, 8),
                    NaiveDate::from_ymd_opt(2027, 1, 4),
                    None,
                ] {
                    let changed = store.set_due(expected.id, expected.version, due).unwrap();
                    expected.due = due;
                    expected.version += 1;
                    assert_eq!(changed, expected);
                    assert_eq!(store.task(expected.id).unwrap(), expected);
                }
                expected_tasks.push(expected);
            }
        }
        drop(store);
        let reopened = Store::open(&path).unwrap();
        for expected in expected_tasks {
            assert_eq!(reopened.task(expected.id).unwrap(), expected);
        }
    }

    #[test]
    fn due_noops_write_nothing_but_still_reject_stale_versions() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut first = Store::open(&path).unwrap();
        let mut second = Store::open(&path).unwrap();
        let original = first.create_task(&input("Original")).unwrap();
        let mut current = original.clone();
        for due in [None, NaiveDate::from_ymd_opt(2026, 11, 8)] {
            if current.due != due {
                current = first.set_due(current.id, current.version, due).unwrap();
            }
            let data_version = second.data_version().unwrap();
            let writes: i64 = first
                .connection
                .query_row("SELECT total_changes()", [], |row| row.get(0))
                .unwrap();
            assert_eq!(
                first.set_due(current.id, current.version, due).unwrap(),
                current
            );
            assert_eq!(second.data_version().unwrap(), data_version);
            let writes_after: i64 = first
                .connection
                .query_row("SELECT total_changes()", [], |row| row.get(0))
                .unwrap();
            assert_eq!(writes_after, writes);
        }
        for due in [current.due, None] {
            let error = second
                .set_due(current.id, original.version, due)
                .unwrap_err();
            assert_eq!(
                error.to_string(),
                format!(
                    "Task {} changed in another process. Reload it before saving.",
                    current.id,
                )
            );
            assert_eq!(second.task(current.id).unwrap(), current);
        }
    }

    #[test]
    fn list_only_updates_change_membership_and_keep_every_other_field() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut store = Store::open(&path).unwrap();
        let work = store.create_list("Work").unwrap();
        let home = store.create_list("Home").unwrap();
        let mut fields = input("Original");
        fields.list_id = work.id;
        fields.due = NaiveDate::from_ymd_opt(2026, 11, 8);
        let task = store.create_task(&fields).unwrap();
        let task = store
            .set_status_at(
                task.id,
                task.version,
                Status::Done,
                NaiveDate::from_ymd_opt(2026, 11, 7).unwrap(),
                timestamp("2026-11-07T15:30:00Z"),
            )
            .unwrap();
        let task = store.set_deleted(task.id, task.version, true).unwrap();
        let mut expected = task.clone();
        let data_version = store.data_version().unwrap();
        assert_eq!(
            store.set_list(task.id, task.version, work.id).unwrap(),
            expected
        );
        assert_eq!(store.data_version().unwrap(), data_version);
        let moved = store.set_list(task.id, task.version, home.id).unwrap();
        expected.list_id = home.id;
        expected.version += 1;
        assert_eq!(moved, expected);
        assert_eq!(store.task(task.id).unwrap(), expected);
        let stale = store.set_list(task.id, task.version, home.id).unwrap_err();
        assert!(stale.to_string().contains("changed in another process"));
        let missing = store.set_list(task.id, moved.version, 999).unwrap_err();
        assert_eq!(missing.to_string(), "List 999 does not exist.");
        assert_eq!(store.task(task.id).unwrap(), expected);
        let mut scoped = Store::open_for_mcp(&path, Some("Home"), false).unwrap();
        let denied = scoped
            .set_list(task.id, moved.version, work.id)
            .unwrap_err();
        assert_eq!(denied.to_string(), "This store is restricted to one list.");
        assert_eq!(
            scoped.set_list(task.id, moved.version, home.id).unwrap(),
            expected
        );
        let mut readonly = Store::open_for_mcp(&path, Some("Home"), true).unwrap();
        assert_eq!(
            readonly
                .set_list(task.id, moved.version, home.id)
                .unwrap_err()
                .to_string(),
            "This store is read-only."
        );
        assert_eq!(store.task(task.id).unwrap(), expected);
    }

    #[test]
    fn restoring_a_status_writes_the_supplied_closure_fields() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut store = Store::open(&path).unwrap();
        let task = store.create_task(&input("Original")).unwrap();
        let closed_on = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap();
        let closed_at = timestamp("2026-11-01T08:15:00Z");
        let done = store
            .set_status_at(task.id, task.version, Status::Done, closed_on, closed_at)
            .unwrap();
        let reopened = store
            .set_status(done.id, done.version, Status::Open)
            .unwrap();
        assert_eq!(reopened.status_changed_on, None);
        let restored = store
            .restore_status(
                reopened.id,
                reopened.version,
                Status::Done,
                Some(closed_on),
                Some(closed_at),
            )
            .unwrap();
        let mut expected = reopened.clone();
        expected.status = Status::Done;
        expected.status_changed_on = Some(closed_on);
        expected.status_changed_at = Some(closed_at);
        expected.version += 1;
        assert_eq!(restored, expected);
        let unknown = store
            .restore_status(restored.id, restored.version, Status::Cancelled, None, None)
            .unwrap();
        assert_eq!(unknown.status, Status::Cancelled);
        assert_eq!(unknown.status_changed_on, None);
        assert_eq!(unknown.status_changed_at, None);
        let open = store
            .restore_status(
                unknown.id,
                unknown.version,
                Status::Open,
                Some(closed_on),
                Some(closed_at),
            )
            .unwrap();
        assert_eq!(open.status, Status::Open);
        assert_eq!(open.status_changed_on, None);
        assert_eq!(open.status_changed_at, None);
        let stale = store
            .restore_status(open.id, unknown.version, Status::Done, None, None)
            .unwrap_err();
        assert!(stale.to_string().contains("changed in another process"));
        assert_eq!(store.task(open.id).unwrap(), open);
        let mut readonly = Store::open_for_mcp(&path, None, true).unwrap();
        assert_eq!(
            readonly
                .restore_status(open.id, open.version, Status::Done, None, None)
                .unwrap_err()
                .to_string(),
            "This store is read-only."
        );
    }

    #[test]
    fn failed_due_write_rolls_back_the_task_and_trigger_changes() {
        let mut store = memory_store();
        let task = store.create_task(&input("Original")).unwrap();
        let neighbor = store.create_task(&input("Neighbor")).unwrap();
        let baseline = store.tasks().unwrap();
        store
            .connection
            .execute_batch(
                "CREATE TRIGGER fail_due AFTER UPDATE OF due ON tasks
             BEGIN
                 UPDATE tasks SET description = 'Must roll back';
                 SELECT RAISE(ABORT, 'Reject due write');
             END;",
            )
            .unwrap();
        let due = NaiveDate::from_ymd_opt(2026, 11, 8);
        assert!(store.set_due(task.id, task.version, due).is_err());
        assert_eq!(store.tasks().unwrap(), baseline);
        store
            .connection
            .execute_batch("DROP TRIGGER fail_due;")
            .unwrap();
        let changed = store.set_due(task.id, task.version, due).unwrap();
        assert_eq!(changed.due, due);
        assert_eq!(changed.version, task.version + 1);
        assert_eq!(store.task(neighbor.id).unwrap(), neighbor);
    }

    #[test]
    fn newer_and_unversioned_databases_are_not_changed() {
        let directory = tempfile::tempdir().unwrap();
        for (name, version) in [("newer.db", 6), ("unversioned.db", 0)] {
            let path = directory.path().join(name);
            let connection = Connection::open(&path).unwrap();
            connection
                .execute_batch(
                    "CREATE TABLE existing(value TEXT); INSERT INTO existing VALUES ('keep');",
                )
                .unwrap();
            connection
                .pragma_update(None, "user_version", version)
                .unwrap();
            drop(connection);
            assert!(Store::open(&path).is_err());
            let connection = Connection::open(&path).unwrap();
            let value: String = connection
                .query_row("SELECT value FROM existing", [], |row| row.get(0))
                .unwrap();
            assert_eq!(value, "keep");
            let actual: i64 = connection
                .pragma_query_value(None, "user_version", |row| row.get(0))
                .unwrap();
            assert_eq!(actual, version);
        }
    }

    #[test]
    fn malformed_database_is_not_replaced() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let bytes = b"This file is not a database. Keep it intact.";
        std::fs::write(&path, bytes).unwrap();
        assert!(Store::open(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }

    #[test]
    fn scoped_reads_hide_other_lists_statuses_and_trash() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut unrestricted = Store::open(&path).unwrap();
        let allowed = unrestricted.create_list("Ærinder").unwrap();
        let forbidden = unrestricted.create_list("Work").unwrap();
        let mut allowed_ids = Vec::new();
        let mut forbidden_ids = Vec::new();
        for list in [&allowed, &forbidden] {
            for status in [Status::Open, Status::Done, Status::Cancelled] {
                let mut fields = input("Task");
                fields.list_id = list.id;
                let task = unrestricted.create_task(&fields).unwrap();
                let task = unrestricted
                    .set_status(task.id, task.version, status)
                    .unwrap();
                if status == Status::Cancelled {
                    unrestricted
                        .set_deleted(task.id, task.version, true)
                        .unwrap();
                }
                if list.id == allowed.id {
                    allowed_ids.push(task.id);
                } else {
                    forbidden_ids.push(task.id);
                }
            }
        }
        let scoped = Store::open_for_mcp(&path, Some("  ÆRINDER  "), false).unwrap();
        assert_eq!(scoped.list_scope(), Some(allowed.id));
        assert!(!scoped.is_read_only());
        let lists = scoped.lists().unwrap();
        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0].id, allowed.id);
        assert_eq!(lists[0].count, 1);
        let tasks = scoped.tasks().unwrap();
        assert_eq!(
            tasks.iter().map(|task| task.id).collect::<Vec<_>>(),
            allowed_ids
        );
        assert_eq!(tasks.iter().filter(|task| task.deleted).count(), 1);
        for id in forbidden_ids {
            assert_eq!(
                scoped.task(id).unwrap_err().to_string(),
                scoped.task(999).unwrap_err().to_string()
            );
        }
    }

    #[test]
    fn scoped_writes_reject_other_tasks_and_destinations() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut unrestricted = Store::open(&path).unwrap();
        let allowed = unrestricted.create_list("Private").unwrap();
        let forbidden = unrestricted.create_task(&input("Outside scope")).unwrap();
        let mut scoped = Store::open_for_mcp(&path, Some("private"), false).unwrap();
        let mut fields = input("Allowed task");
        fields.list_id = allowed.id;
        let task = scoped.create_task(&fields).unwrap();
        let baseline = unrestricted.tasks().unwrap();
        let missing = scoped.task(999).unwrap_err().to_string();
        for due in [None, NaiveDate::from_ymd_opt(2026, 11, 8)] {
            assert_eq!(
                scoped
                    .set_due(forbidden.id, forbidden.version, due)
                    .unwrap_err()
                    .to_string(),
                missing,
            );
        }
        assert_eq!(
            scoped
                .update_task(forbidden.id, forbidden.version, &fields)
                .unwrap_err()
                .to_string(),
            missing
        );
        assert_eq!(
            scoped
                .set_status(forbidden.id, forbidden.version, Status::Done)
                .unwrap_err()
                .to_string(),
            missing
        );
        for deleted in [true, false] {
            assert_eq!(
                scoped
                    .set_deleted(forbidden.id, forbidden.version, deleted)
                    .unwrap_err()
                    .to_string(),
                missing
            );
        }
        assert_eq!(
            scoped
                .swap_positions(&task, &forbidden)
                .unwrap_err()
                .to_string(),
            missing
        );
        fields.list_id = forbidden.list_id;
        let other_destination = scoped.create_task(&fields).unwrap_err().to_string();
        fields.list_id = 999;
        assert_eq!(
            scoped.create_task(&fields).unwrap_err().to_string(),
            other_destination
        );
        assert_eq!(
            scoped
                .update_task(task.id, task.version, &fields)
                .unwrap_err()
                .to_string(),
            other_destination
        );
        assert!(scoped.create_list("Another").is_err());
        assert!(scoped.rename_list(allowed.id, "Renamed").is_err());
        assert!(scoped.rename_list(999, "Renamed").is_err());
        assert!(scoped.move_list(allowed.id, true).is_err());
        assert!(scoped.move_list(999, false).is_err());
        assert_eq!(unrestricted.tasks().unwrap(), baseline);
        assert_eq!(unrestricted.lists().unwrap().len(), 2);
        assert_eq!(unrestricted.lists().unwrap()[1].name, "Private");
    }

    #[test]
    fn scoped_mutations_recheck_membership_after_an_external_move() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut other = Store::open(&path).unwrap();
        let allowed = other.create_list("Private").unwrap();
        let mut fields = input("Original");
        fields.list_id = allowed.id;
        let original = other.create_task(&fields).unwrap();
        let neighbor = other.create_task(&fields).unwrap();
        let mut scoped = Store::open_for_mcp(&path, Some("Private"), false).unwrap();
        let captured = scoped.task(original.id).unwrap();
        fields.list_id = 1;
        let moved = other
            .update_task(original.id, original.version, &fields)
            .unwrap();
        let baseline = other.tasks().unwrap();
        let mut attempt = TaskInput::from(&captured);
        attempt.title = "Must not save".into();
        for version in [captured.version, moved.version] {
            for due in [None, NaiveDate::from_ymd_opt(2026, 11, 8)] {
                assert_eq!(
                    scoped
                        .set_due(captured.id, version, due)
                        .unwrap_err()
                        .to_string(),
                    "Task not found.",
                );
            }
            assert_eq!(
                scoped
                    .update_task(captured.id, version, &attempt)
                    .unwrap_err()
                    .to_string(),
                "Task not found."
            );
            assert_eq!(
                scoped
                    .set_status(captured.id, version, Status::Done)
                    .unwrap_err()
                    .to_string(),
                "Task not found."
            );
            assert_eq!(
                scoped
                    .set_deleted(captured.id, version, true)
                    .unwrap_err()
                    .to_string(),
                "Task not found."
            );
        }
        assert!(scoped.swap_positions(&neighbor, &moved).is_err());
        assert_eq!(other.tasks().unwrap(), baseline);
    }

    #[test]
    fn scoped_access_stays_with_the_original_list_id_after_rename() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut other = Store::open(&path).unwrap();
        let original = other.create_list("Private").unwrap();
        let mut scoped = Store::open_for_mcp(&path, Some("Private"), false).unwrap();
        other.rename_list(original.id, "Home").unwrap();
        let replacement = other.create_list("Private").unwrap();
        assert_eq!(scoped.list_scope(), Some(original.id));
        let lists = scoped.lists().unwrap();
        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0].id, original.id);
        assert_eq!(lists[0].name, "Home");
        let mut fields = input("Still allowed");
        fields.list_id = original.id;
        assert_eq!(scoped.create_task(&fields).unwrap().list_id, original.id);
        fields.list_id = replacement.id;
        assert!(scoped.create_task(&fields).is_err());
        assert!(Store::open_for_mcp(&path, Some("Missing"), false).is_err());
    }

    #[test]
    fn scoped_store_allows_changes_inside_its_list() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let other = Store::open(&path).unwrap();
        let mut scoped = Store::open_for_mcp(&path, Some("Inbox"), false).unwrap();
        let first = scoped.create_task(&input("First")).unwrap();
        let second = scoped.create_task(&input("Second")).unwrap();
        scoped.swap_positions(&first, &second).unwrap();
        let first = scoped.task(first.id).unwrap();
        assert_eq!(first.position, second.position);
        let mut fields = TaskInput::from(&first);
        fields.title = "Changed".into();
        let first = scoped
            .update_task(first.id, first.version, &fields)
            .unwrap();
        let first = scoped
            .set_status(first.id, first.version, Status::Done)
            .unwrap();
        let first = scoped.set_deleted(first.id, first.version, true).unwrap();
        let first = scoped.set_deleted(first.id, first.version, false).unwrap();
        let due = NaiveDate::from_ymd_opt(2026, 11, 8);
        let first = scoped.set_due(first.id, first.version, due).unwrap();
        assert_eq!(other.task(first.id).unwrap(), first);
        assert_eq!(first.due, due);
        assert_eq!(first.title, "Changed");
        assert_eq!(first.status, Status::Done);
        assert!(!first.deleted);
    }

    #[test]
    fn read_only_store_blocks_every_mutation_and_keeps_scoped_reads() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let mut other = Store::open(&path).unwrap();
        let hidden = other.create_list("Hidden").unwrap();
        let mut fields = input("Hidden task");
        fields.list_id = hidden.id;
        other.create_task(&fields).unwrap();
        let first = other.create_task(&input("First")).unwrap();
        let second = other.create_task(&input("Second")).unwrap();
        let trashed = other.create_task(&input("Trashed")).unwrap();
        let trashed = other
            .set_deleted(trashed.id, trashed.version, true)
            .unwrap();
        let trash_preview = other.preview_empty_trash().unwrap();
        let baseline = other.tasks().unwrap();
        let mut readonly = Store::open_for_mcp(&path, Some("Inbox"), true).unwrap();
        assert!(readonly.is_read_only());
        assert_eq!(readonly.list_scope(), Some(1));
        assert_eq!(readonly.lists().unwrap().len(), 1);
        assert_eq!(readonly.tasks().unwrap().len(), 3);
        assert_eq!(readonly.task(first.id).unwrap(), first);
        let errors = [
            readonly.create_list("New").unwrap_err(),
            readonly.rename_list(1, "Renamed").unwrap_err(),
            readonly.move_list(1, true).unwrap_err(),
            readonly.create_task(&input("New")).unwrap_err(),
            readonly
                .update_task(first.id, first.version, &TaskInput::from(&first))
                .unwrap_err(),
            readonly.set_due(first.id, first.version, None).unwrap_err(),
            readonly
                .set_due(
                    first.id,
                    first.version,
                    NaiveDate::from_ymd_opt(2026, 11, 8),
                )
                .unwrap_err(),
            readonly
                .set_status(first.id, first.version, Status::Open)
                .unwrap_err(),
            readonly
                .set_deleted(first.id, first.version, false)
                .unwrap_err(),
            readonly.swap_positions(&first, &second).unwrap_err(),
            readonly
                .purge_task(trashed.id, trashed.version)
                .unwrap_err(),
            readonly.preview_empty_trash().unwrap_err(),
            readonly.empty_trash(&trash_preview).unwrap_err(),
        ];
        for error in errors {
            assert_eq!(error.to_string(), "This store is read-only.");
        }
        assert!(
            readonly
                .connection
                .execute("DELETE FROM tasks", [])
                .is_err()
        );
        assert_eq!(other.tasks().unwrap(), baseline);
        assert_eq!(other.lists().unwrap().len(), 2);
    }

    #[test]
    fn read_only_open_preserves_journal_mode_and_requires_an_existing_schema() {
        let directory = tempfile::tempdir().unwrap();
        let missing = directory.path().join("missing.db");
        assert!(Store::open_for_mcp(&missing, None, true).is_err());
        assert!(!missing.exists());
        let path = directory.path().join("tasks.db");
        drop(Store::open(&path).unwrap());
        let connection = Connection::open(&path).unwrap();
        connection
            .pragma_update(None, "journal_mode", "DELETE")
            .unwrap();
        drop(connection);
        let readonly = Store::open_for_mcp(&path, None, true).unwrap();
        let mode: String = readonly
            .connection
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "delete");
        assert!(readonly.list_scope().is_none());
        drop(readonly);
        for version in [0, 1, 2, 3, 4, 6] {
            let connection = Connection::open(&path).unwrap();
            connection
                .pragma_update(None, "user_version", version)
                .unwrap();
            drop(connection);
            assert!(Store::open_for_mcp(&path, None, true).is_err());
            let connection = Connection::open(&path).unwrap();
            let actual: i64 = connection
                .pragma_query_value(None, "user_version", |row| row.get(0))
                .unwrap();
            assert_eq!(actual, version);
            let list_count: i64 = connection
                .query_row("SELECT COUNT(*) FROM lists", [], |row| row.get(0))
                .unwrap();
            assert_eq!(list_count, 1);
        }
    }

    fn schema_four_database(path: &Path) -> Connection {
        let connection = Connection::open(path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE lists (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL CHECK(length(name) > 0),
                    name_key TEXT NOT NULL UNIQUE,
                    position INTEGER NOT NULL DEFAULT 0
                );
                CREATE TABLE tasks (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    list_id INTEGER NOT NULL REFERENCES lists(id),
                    title TEXT NOT NULL CHECK(length(title) > 0),
                    description TEXT NOT NULL DEFAULT '',
                    due TEXT,
                    status TEXT NOT NULL CHECK(status IN ('open', 'done', 'cancelled')),
                    status_changed_on TEXT,
                    status_changed_at TEXT,
                    deleted INTEGER NOT NULL CHECK(deleted IN (0, 1)),
                    position INTEGER NOT NULL,
                    version INTEGER NOT NULL CHECK(version > 0)
                );
                CREATE INDEX tasks_list ON tasks(list_id);
                CREATE INDEX tasks_planning ON tasks(deleted, status, due, position);
                INSERT INTO lists(id, name, name_key, position)
                    VALUES (1, 'Inbox', 'inbox', 0), (2, 'Home', 'home', 4);
                INSERT INTO tasks
                    (id, list_id, title, description, due, status, status_changed_on,
                     status_changed_at, deleted, position, version)
                VALUES
                    (7, 2, 'Milk', 'Notes', '2026-11-08', 'done', '2026-11-08',
                     '2026-11-08T10:00:00+00:00', 0, 5, 3),
                    (9, 1, 'Old idea', '', NULL, 'open', NULL, NULL, 1, 8, 4);
                PRAGMA user_version = 4;",
            )
            .unwrap();
        connection
    }

    fn uids(connection: &Connection, table: &str) -> Vec<Option<String>> {
        connection
            .prepare(&format!("SELECT uid FROM {table} ORDER BY id"))
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap()
    }

    #[test]
    fn schema_four_migration_adds_sync_identifiers_without_changing_tasks() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let connection = schema_four_database(&path);
        let before: Vec<Task> = connection
            .prepare(&format!("SELECT {TASK_COLUMNS} FROM tasks ORDER BY id"))
            .unwrap()
            .query_map([], read_task)
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        let error = Store::open_for_mcp(&path, None, true).err().unwrap();
        assert!(
            error
                .to_string()
                .contains("Open this profile with writable Husker")
        );
        assert!(connection.prepare("SELECT uid FROM tasks").is_err());
        drop(connection);

        let mut store = Store::open(&path).unwrap();
        assert_eq!(store.tasks().unwrap(), before);
        assert_eq!(
            store
                .lists()
                .unwrap()
                .iter()
                .map(|list| (list.id, list.name.as_str()))
                .collect::<Vec<_>>(),
            [(1, "Inbox"), (2, "Home")]
        );
        let version: i64 = store
            .connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 5);
        let created = store.create_task(&input("Next task")).unwrap();
        assert_eq!(created.id, 10);
        let list = store.create_list("New list").unwrap();
        assert_eq!(list.id, 3);
        let mut all = uids(&store.connection, "lists");
        all.extend(uids(&store.connection, "tasks"));
        assert_eq!(all.len(), 6);
        for uid in &all {
            let uid = uid.as_deref().unwrap();
            assert_eq!(uid.len(), 32);
            assert!(
                uid.chars()
                    .all(|character| matches!(character, '0'..='9' | 'a'..='f'))
            );
        }
        all.sort();
        all.dedup();
        assert_eq!(all.len(), 6, "every row has its own identifier");
        let state: i64 = store
            .connection
            .query_row("SELECT COUNT(*) FROM sync_state", [], |row| row.get(0))
            .unwrap();
        assert_eq!(state, 0);
        assert_eq!(store.sync_counters().unwrap(), SyncCounters::default());
        drop(store);
        let readonly = Store::open_for_mcp(&path, None, true).unwrap();
        assert_eq!(readonly.tasks().unwrap().len(), 3);
    }

    #[test]
    fn failed_schema_four_migration_keeps_the_database_at_version_four() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tasks.db");
        let connection = schema_four_database(&path);
        // An existing table with the new name makes the migration fail after it added columns.
        connection
            .execute_batch("CREATE TABLE sync_state (other TEXT);")
            .unwrap();
        assert!(Store::open(&path).is_err());
        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 4);
        assert!(connection.prepare("SELECT uid FROM tasks").is_err());
        let (title, task_version): (String, i64) = connection
            .query_row("SELECT title, version FROM tasks WHERE id = 7", [], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap();
        assert_eq!((title.as_str(), task_version), ("Milk", 3));
    }

    #[test]
    fn new_databases_give_every_list_and_task_an_identifier() {
        let mut store = memory_store();
        store.create_list("Work").unwrap();
        store.create_task(&input("First")).unwrap();
        store.create_task(&input("Second")).unwrap();
        let mut all = uids(&store.connection, "lists");
        all.extend(uids(&store.connection, "tasks"));
        assert!(
            all.iter()
                .all(|uid| uid.as_ref().is_some_and(|uid| uid.len() == 32))
        );
        all.sort();
        all.dedup();
        assert_eq!(all.len(), 4);
    }
}
