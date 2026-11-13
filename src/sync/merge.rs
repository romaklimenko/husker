//! Merges the local database with the sync file against the last synced snapshot.
//!
//! The merge reads no clock and no random source, so every device computes
//! the same result from the same three snapshots. A field changed on one side
//! wins. A field changed on both sides takes the value from the file, and a
//! text conflict also keeps the local version as a copy.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use super::snapshot::{ListRecord, Snapshot, TaskRecord, name_key};

/// The text appended to the title of a conflict copy.
pub const CONFLICT_TAG: &str = "#conflict";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Merged {
    pub lists: Vec<ListRecord>,
    pub tasks: Vec<TaskRecord>,
    /// Local list identifiers replaced by the identifier of a list in the file.
    pub adopted: Vec<(String, String)>,
    /// The number of conflict copies this merge added to `tasks`.
    pub conflicts: usize,
}

/// Picks the merged value of one field. The flag reports a conflict that the file won.
fn pick<T: Clone + PartialEq>(base: Option<&T>, local: &T, remote: &T) -> (T, bool) {
    if local == remote {
        return (local.clone(), false);
    }
    match base {
        Some(base) if local == base => (remote.clone(), false),
        Some(base) if remote == base => (local.clone(), false),
        _ => (remote.clone(), true),
    }
}

fn by_uid<T>(items: &[T], uid: impl Fn(&T) -> &str) -> HashMap<String, &T> {
    items
        .iter()
        .map(|item| (uid(item).to_owned(), item))
        .collect()
}

/// Derives the identifier of a conflict copy from the task and the text it keeps.
///
/// Two devices that keep the same text of the same task create the same copy,
/// so the copies merge into one. FNV-1a is enough, because the input is not
/// chosen by an attacker and a clash only merges two copies.
fn copy_uid(original: &str, title: &str, description: &str, attempt: u32) -> String {
    let hash = |seed: u64| -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325 ^ seed;
        let attempt = attempt.to_le_bytes();
        let parts: [&[u8]; 4] = [
            original.as_bytes(),
            title.as_bytes(),
            description.as_bytes(),
            &attempt,
        ];
        for part in parts {
            for byte in part.iter().chain(&[0xff]) {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        hash
    };
    format!("{:016x}{:016x}", hash(0), hash(0x9e37_79b9_7f4a_7c15))
}

/// Merges `local` and `remote` against `base`.
///
/// `base` is ignored when `remote` is absent, because nothing can then be
/// compared with the file. The result always keeps the local default list.
///
/// `rejoin` says that the file does not descend from `base`: it was deleted
/// and written again, or it belongs to another profile. A rejoin deletes
/// nothing, joins the default lists as a first sync does, and keeps a copy of
/// every local text that the file replaces.
pub fn merge(
    base: Option<&Snapshot>,
    local: &Snapshot,
    remote: Option<&Snapshot>,
    rejoin: bool,
) -> Merged {
    let base = remote.and(base);
    let joining = base.is_none() || rejoin;
    let empty_lists: &[ListRecord] = &[];
    let empty_tasks: &[TaskRecord] = &[];
    let base_lists = base.map_or(empty_lists, |snapshot| &snapshot.lists);
    let base_tasks = base.map_or(empty_tasks, |snapshot| &snapshot.tasks);
    let remote_lists = remote.map_or(empty_lists, |snapshot| &snapshot.lists);
    let remote_tasks = remote.map_or(empty_tasks, |snapshot| &snapshot.tasks);

    // Step 1: local lists that take the identity of a list in the file.
    let local_uids: HashSet<&str> = local.lists.iter().map(|list| list.uid.as_str()).collect();
    let remote_uids: HashSet<&str> = remote_lists.iter().map(|list| list.uid.as_str()).collect();
    let base_uids: HashSet<&str> = base_lists.iter().map(|list| list.uid.as_str()).collect();
    let mut adopted: Vec<(String, String)> = Vec::new();
    let mut default_uid = local.default_list.clone();
    if joining
        && let Some(remote) = remote
        && remote.default_list != default_uid
        && remote_uids.contains(remote.default_list.as_str())
        && !local_uids.contains(remote.default_list.as_str())
        && !remote_uids.contains(default_uid.as_str())
    {
        adopted.push((default_uid.clone(), remote.default_list.clone()));
        default_uid = remote.default_list.clone();
    }
    for list in &local.lists {
        let uid = list.uid.as_str();
        if (!rejoin && base_uids.contains(uid))
            || remote_uids.contains(uid)
            || adopted.iter().any(|(from, _)| from == uid)
        {
            continue;
        }
        let key = name_key(&list.name);
        if let Some(twin) = remote_lists.iter().find(|candidate| {
            name_key(&candidate.name) == key
                && !local_uids.contains(candidate.uid.as_str())
                && !adopted.iter().any(|(_, to)| *to == candidate.uid)
        }) {
            adopted.push((list.uid.clone(), twin.uid.clone()));
        }
    }
    let adopt = |uid: &str| -> String {
        adopted
            .iter()
            .find(|(from, _)| from == uid)
            .map_or_else(|| uid.to_owned(), |(_, to)| to.clone())
    };
    let local_lists: Vec<ListRecord> = local
        .lists
        .iter()
        .map(|list| ListRecord {
            uid: adopt(&list.uid),
            ..list.clone()
        })
        .collect();
    let local_tasks: Vec<TaskRecord> = local
        .tasks
        .iter()
        .map(|task| TaskRecord {
            list: adopt(&task.list),
            ..task.clone()
        })
        .collect();

    // Step 2: lists.
    let base_by = by_uid(base_lists, |list| &list.uid);
    let local_by = by_uid(&local_lists, |list| &list.uid);
    let remote_by = by_uid(remote_lists, |list| &list.uid);
    let uids: BTreeSet<&String> = local_by.keys().chain(remote_by.keys()).collect();
    let mut lists: Vec<ListRecord> = Vec::new();
    for uid in uids {
        let base = base_by.get(uid).copied();
        match (local_by.get(uid).copied(), remote_by.get(uid).copied()) {
            (Some(local), Some(remote)) => lists.push(ListRecord {
                uid: uid.clone(),
                id: remote.id,
                name: pick(base.map(|list| &list.name), &local.name, &remote.name).0,
                position: pick(
                    base.map(|list| &list.position),
                    &local.position,
                    &remote.position,
                )
                .0,
            }),
            (Some(survivor), None) | (None, Some(survivor)) => {
                // Deleted on the other side. An untouched list goes, an edited list stays.
                if rejoin || !base.is_some_and(|base| base.same_except_order(survivor)) {
                    lists.push(survivor.clone());
                }
            }
            (None, None) => {}
        }
    }
    if !lists.iter().any(|list| list.uid == default_uid)
        && let Some(default) = local_by.get(&default_uid)
    {
        lists.push((*default).clone());
    }

    // Step 3: tasks.
    let base_by = by_uid(base_tasks, |task| &task.uid);
    let local_by = by_uid(&local_tasks, |task| &task.uid);
    let remote_by = by_uid(remote_tasks, |task| &task.uid);
    let uids: BTreeSet<&String> = local_by.keys().chain(remote_by.keys()).collect();
    let mut tasks: Vec<TaskRecord> = Vec::new();
    let mut copies: Vec<TaskRecord> = Vec::new();
    for uid in uids {
        let base = base_by.get(uid).copied();
        match (local_by.get(uid).copied(), remote_by.get(uid).copied()) {
            (Some(local), Some(remote)) => {
                let title = pick(base.map(|task| &task.title), &local.title, &remote.title).0;
                let description = pick(
                    base.map(|task| &task.description),
                    &local.description,
                    &remote.description,
                )
                .0;
                // The local text is lost when the merged text differs from it and the
                // file's text won a conflict. In a rejoin the file cannot be trusted to
                // be newer, so every replaced local text is kept.
                let text_replaced = title != local.title || description != local.description;
                let text_conflict = (title != local.title
                    && base.is_none_or(|base| base.title != local.title))
                    || (description != local.description
                        && base.is_none_or(|base| base.description != local.description));
                let status_of = |task: &TaskRecord| {
                    (task.status, task.status_changed_on, task.status_changed_at)
                };
                let (status, status_changed_on, status_changed_at) = pick(
                    base.map(status_of).as_ref(),
                    &status_of(local),
                    &status_of(remote),
                )
                .0;
                tasks.push(TaskRecord {
                    uid: uid.clone(),
                    id: remote.id,
                    list: pick(base.map(|task| &task.list), &local.list, &remote.list).0,
                    title,
                    description,
                    due: pick(base.map(|task| &task.due), &local.due, &remote.due).0,
                    status,
                    status_changed_on,
                    status_changed_at,
                    deleted: pick(
                        base.map(|task| &task.deleted),
                        &local.deleted,
                        &remote.deleted,
                    )
                    .0,
                    position: pick(
                        base.map(|task| &task.position),
                        &local.position,
                        &remote.position,
                    )
                    .0,
                });
                if text_conflict || (rejoin && text_replaced) {
                    let mut copy = local.clone();
                    copy.id = 0;
                    if !copy.title.ends_with(CONFLICT_TAG) {
                        copy.title = format!("{} {CONFLICT_TAG}", copy.title);
                    }
                    copies.push(TaskRecord {
                        uid: uid.clone(),
                        ..copy
                    });
                }
            }
            (Some(survivor), None) | (None, Some(survivor)) => {
                if rejoin || !base.is_some_and(|base| base.same_except_order(survivor)) {
                    tasks.push(survivor.clone());
                }
            }
            (None, None) => {}
        }
    }
    // Each copy gets an identifier derived from its content. A copy that already
    // exists with the same text is not added again.
    let mut conflicts = 0;
    for mut copy in copies {
        let original = std::mem::take(&mut copy.uid);
        for attempt in 0.. {
            let uid = copy_uid(&original, &copy.title, &copy.description, attempt);
            match tasks.iter().find(|task| task.uid == uid) {
                None => {
                    copy.uid = uid;
                    tasks.push(copy);
                    conflicts += 1;
                    break;
                }
                Some(existing)
                    if existing.title == copy.title && existing.description == copy.description =>
                {
                    break;
                }
                Some(_) => {}
            }
        }
    }

    // Step 4: a task whose list is gone moves to the default list.
    let kept: HashSet<String> = lists.iter().map(|list| list.uid.clone()).collect();
    for task in &mut tasks {
        if !kept.contains(&task.list) {
            task.list = default_uid.clone();
        }
    }

    // Step 5: list names stay unique. The list with the greater identifier gets a suffix.
    lists.sort_by(|left, right| left.uid.cmp(&right.uid));
    let mut names: HashSet<String> = HashSet::new();
    let mut duplicates: Vec<usize> = Vec::new();
    for (index, list) in lists.iter().enumerate() {
        if !names.insert(name_key(&list.name)) {
            duplicates.push(index);
        }
    }
    for index in duplicates {
        let name = lists[index].name.trim().to_owned();
        let mut number = 2;
        loop {
            let candidate = format!("{name} ({number})");
            if names.insert(name_key(&candidate)) {
                lists[index].name = candidate;
                break;
            }
            number += 1;
        }
    }

    // Step 6: positions stay unique, so two tasks can always be swapped.
    let mut order: BTreeMap<(i64, String), usize> = BTreeMap::new();
    for (index, list) in lists.iter().enumerate() {
        order.insert((list.position, list.uid.clone()), index);
    }
    let mut previous: Option<i64> = None;
    for index in order.into_values() {
        if let Some(previous) = previous
            && lists[index].position <= previous
        {
            lists[index].position = previous.saturating_add(1);
        }
        previous = Some(lists[index].position);
    }
    let mut order: BTreeMap<(i64, String), usize> = BTreeMap::new();
    for (index, task) in tasks.iter().enumerate() {
        order.insert((task.position, task.uid.clone()), index);
    }
    let mut previous: Option<i64> = None;
    for index in order.into_values() {
        if let Some(previous) = previous
            && tasks[index].position <= previous
        {
            tasks[index].position = previous.saturating_add(1);
        }
        previous = Some(tasks[index].position);
    }
    tasks.sort_by(|left, right| left.uid.cmp(&right.uid));

    Merged {
        lists,
        tasks,
        adopted,
        conflicts,
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    use crate::model::Status;

    fn list(uid: &str, id: i64, name: &str) -> ListRecord {
        ListRecord {
            uid: uid.into(),
            id,
            name: name.into(),
            position: id,
        }
    }

    fn task(uid: &str, id: i64, list: &str, title: &str) -> TaskRecord {
        TaskRecord {
            uid: uid.into(),
            id,
            list: list.into(),
            title: title.into(),
            description: String::new(),
            due: None,
            status: Status::Open,
            status_changed_on: None,
            status_changed_at: None,
            deleted: false,
            position: id,
        }
    }

    fn snapshot_with(default: &str, lists: Vec<ListRecord>, tasks: Vec<TaskRecord>) -> Snapshot {
        Snapshot::new("c".into(), default.into(), lists, tasks)
    }

    fn snapshot(lists: Vec<ListRecord>, tasks: Vec<TaskRecord>) -> Snapshot {
        snapshot_with("inbox", lists, tasks)
    }

    fn run(base: Option<&Snapshot>, local: &Snapshot, remote: Option<&Snapshot>) -> Merged {
        merge(base, local, remote, false)
    }

    fn find<'a>(merged: &'a Merged, uid: &str) -> &'a TaskRecord {
        merged.tasks.iter().find(|task| task.uid == uid).unwrap()
    }

    fn copies(merged: &Merged) -> Vec<&TaskRecord> {
        merged
            .tasks
            .iter()
            .filter(|task| task.title.ends_with(CONFLICT_TAG))
            .collect()
    }

    fn date(day: u32) -> Option<NaiveDate> {
        NaiveDate::from_ymd_opt(2026, 11, day)
    }

    #[test]
    fn edits_to_different_fields_combine() {
        let base = snapshot(
            vec![list("inbox", 1, "Inbox")],
            vec![task("a", 7, "inbox", "Milk")],
        );
        let mut local = base.clone();
        local.tasks[0].status = Status::Done;
        local.tasks[0].status_changed_on = date(13);
        let mut remote = base.clone();
        remote.tasks[0].due = date(20);
        remote.tasks[0].description = "Oat".into();
        let merged = run(Some(&base), &local, Some(&remote));
        let milk = find(&merged, "a");
        assert_eq!(milk.status, Status::Done);
        assert_eq!(milk.status_changed_on, date(13));
        assert_eq!(milk.due, date(20));
        assert_eq!(milk.description, "Oat");
        assert_eq!(merged.conflicts, 0);
        assert_eq!(merged.tasks.len(), 1);
    }

    #[test]
    fn one_sided_records_are_kept_and_unchanged_deletions_apply() {
        let base = snapshot(
            vec![list("inbox", 1, "Inbox")],
            vec![
                task("gone-remote", 1, "inbox", "Deleted on the other device"),
                task("gone-local", 2, "inbox", "Deleted here"),
            ],
        );
        let local = snapshot(
            vec![list("inbox", 1, "Inbox")],
            vec![
                task("gone-remote", 1, "inbox", "Deleted on the other device"),
                task("new-local", 3, "inbox", "Created here"),
            ],
        );
        let remote = snapshot(
            vec![list("inbox", 1, "Inbox")],
            vec![
                task("gone-local", 2, "inbox", "Deleted here"),
                task("new-remote", 4, "inbox", "Created there"),
            ],
        );
        let merged = run(Some(&base), &local, Some(&remote));
        let uids: Vec<&str> = merged.tasks.iter().map(|task| task.uid.as_str()).collect();
        assert_eq!(uids, ["new-local", "new-remote"]);
    }

    #[test]
    fn an_edit_survives_a_deletion_but_a_new_position_alone_does_not() {
        let base = snapshot(
            vec![list("inbox", 1, "Inbox"), list("moved", 2, "Moved")],
            vec![
                task("a", 1, "inbox", "Old"),
                task("b", 2, "inbox", "Old"),
                task("reordered", 3, "inbox", "Only reordered"),
            ],
        );
        let mut local = base.clone();
        local.tasks.retain(|task| task.uid != "a");
        local.tasks[0].title = "Edited here".into();
        local.tasks[1].position = 99;
        local.lists[1].position = 99;
        let mut remote = base.clone();
        remote
            .tasks
            .retain(|task| task.uid != "b" && task.uid != "reordered");
        remote.tasks[0].title = "Edited there".into();
        remote.lists.retain(|list| list.uid != "moved");
        let merged = run(Some(&base), &local, Some(&remote));
        assert_eq!(find(&merged, "a").title, "Edited there");
        assert_eq!(find(&merged, "b").title, "Edited here");
        assert!(merged.tasks.iter().all(|task| task.uid != "reordered"));
        assert!(merged.lists.iter().all(|list| list.uid != "moved"));
    }

    #[test]
    fn a_text_conflict_keeps_the_file_text_and_copies_the_local_version() {
        let base = snapshot(
            vec![list("inbox", 1, "Inbox")],
            vec![task("a", 7, "inbox", "Buy milk")],
        );
        let mut local = base.clone();
        local.tasks[0].title = "Buy whole milk".into();
        local.tasks[0].due = date(14);
        let mut remote = base.clone();
        remote.tasks[0].title = "Buy oat milk".into();
        let merged = run(Some(&base), &local, Some(&remote));
        assert_eq!(merged.conflicts, 1);
        let original = find(&merged, "a");
        assert_eq!(original.title, "Buy oat milk");
        assert_eq!(original.due, date(14));
        let copy = copies(&merged)[0];
        assert_eq!(copy.title, "Buy whole milk #conflict");
        assert_eq!(copy.due, date(14));
        assert_eq!(copy.id, 0);
        assert_eq!(copy.uid.len(), 32);
        // The copy sits next to the original after positions are made unique.
        assert_eq!((original.position - copy.position).abs(), 1);
        // A second merge of the result against the file creates no further copy.
        let merged_snapshot = snapshot(merged.lists.clone(), merged.tasks.clone());
        let again = run(Some(&remote), &merged_snapshot, Some(&remote));
        assert_eq!(again.conflicts, 0);
        assert_eq!(again.tasks, merged.tasks);
        // Another device that kept the same text creates the same copy, not a second one.
        let elsewhere = run(Some(&base), &local, Some(&merged_snapshot));
        assert_eq!(elsewhere.conflicts, 0);
        assert_eq!(copies(&elsewhere).len(), 1);
        assert_eq!(copies(&elsewhere)[0].uid, copy.uid);
    }

    #[test]
    fn a_description_conflict_copies_and_equal_edits_do_not() {
        let base = snapshot(
            vec![list("inbox", 1, "Inbox")],
            vec![task("a", 1, "inbox", "A"), task("b", 2, "inbox", "B")],
        );
        let mut local = base.clone();
        local.tasks[0].description = "Local notes".into();
        local.tasks[1].description = "Same notes".into();
        let mut remote = base.clone();
        remote.tasks[0].description = "Remote notes".into();
        remote.tasks[1].description = "Same notes".into();
        let merged = run(Some(&base), &local, Some(&remote));
        assert_eq!(merged.conflicts, 1);
        assert_eq!(find(&merged, "a").description, "Remote notes");
        assert_eq!(copies(&merged)[0].description, "Local notes");
        assert_eq!(copies(&merged)[0].title, "A #conflict");
        assert_eq!(merged.tasks.len(), 3);
    }

    #[test]
    fn other_conflicts_take_the_file_value_without_a_copy() {
        let base = snapshot(
            vec![list("inbox", 1, "Inbox"), list("work", 2, "Work")],
            vec![task("a", 7, "inbox", "Milk")],
        );
        let mut local = base.clone();
        local.tasks[0].due = date(20);
        local.tasks[0].status = Status::Cancelled;
        local.tasks[0].deleted = true;
        let mut remote = base.clone();
        remote.tasks[0].due = date(16);
        remote.tasks[0].status = Status::Done;
        remote.tasks[0].list = "work".into();
        let merged = run(Some(&base), &local, Some(&remote));
        let milk = find(&merged, "a");
        assert_eq!(milk.due, date(16));
        assert_eq!(milk.status, Status::Done);
        assert_eq!(milk.list, "work");
        assert!(milk.deleted);
        assert_eq!(merged.conflicts, 0);
        assert_eq!(merged.tasks.len(), 1);
    }

    #[test]
    fn a_first_sync_joins_default_lists_and_keeps_every_task() {
        let local = snapshot_with(
            "mine",
            vec![list("mine", 1, "Inbox"), list("local-work", 2, "work ")],
            vec![
                task("t1", 1, "mine", "Local task"),
                task("t2", 2, "local-work", "Local work"),
            ],
        );
        // The default list of the file is named, so a lower ID elsewhere does not matter.
        let remote = snapshot_with(
            "theirs",
            vec![
                list("theirs", 4, "Private"),
                list("remote-work", 5, "Work"),
                list("remote-inbox", 2, "Inbox"),
            ],
            vec![task("t3", 1, "theirs", "Remote task")],
        );
        let merged = merge(None, &local, Some(&remote), false);
        assert_eq!(
            merged.adopted,
            [
                ("mine".to_owned(), "theirs".to_owned()),
                ("local-work".to_owned(), "remote-work".to_owned())
            ]
        );
        let names: Vec<(&str, &str)> = merged
            .lists
            .iter()
            .map(|list| (list.uid.as_str(), list.name.as_str()))
            .collect();
        assert_eq!(
            names,
            [
                ("remote-inbox", "Inbox"),
                ("remote-work", "Work"),
                ("theirs", "Private")
            ]
        );
        assert_eq!(find(&merged, "t1").list, "theirs");
        assert_eq!(find(&merged, "t2").list, "remote-work");
        assert_eq!(find(&merged, "t3").list, "theirs");
        assert_eq!(merged.conflicts, 0);
    }

    #[test]
    fn a_missing_file_ignores_the_base_and_keeps_local_data() {
        let base = snapshot(
            vec![list("inbox", 1, "Inbox")],
            vec![task("a", 1, "inbox", "Synced before")],
        );
        let local = snapshot(
            vec![list("inbox", 1, "Inbox")],
            vec![task("b", 2, "inbox", "Still here")],
        );
        let merged = run(Some(&base), &local, None);
        assert_eq!(merged.tasks, local.tasks);
        assert_eq!(merged.lists, local.lists);
    }

    #[test]
    fn a_rejoin_deletes_nothing_keeps_unsent_changes_and_copies_replaced_text() {
        let base = snapshot(
            vec![list("inbox", 1, "Inbox"), list("old", 2, "Old list")],
            vec![
                task("unsent", 1, "inbox", "Mark me done"),
                task("replaced", 2, "inbox", "Agreed title"),
                task("missing", 3, "old", "Not in the new file"),
                task("same", 4, "inbox", "Same everywhere"),
            ],
        );
        let mut local = base.clone();
        local.tasks[3].status = Status::Done;
        local.tasks[3].title = "Mark me done today".into();
        // Another device wrote the file again from an older state.
        let remote = snapshot(
            vec![list("inbox", 1, "Inbox")],
            vec![
                task("unsent", 1, "inbox", "Mark me done"),
                task("replaced", 2, "inbox", "Older title"),
                task("same", 4, "inbox", "Same everywhere"),
            ],
        );
        let merged = merge(Some(&base), &local, Some(&remote), true);
        // The change this device had not sent wins over the unchanged file value.
        assert_eq!(find(&merged, "unsent").status, Status::Done);
        assert_eq!(find(&merged, "unsent").title, "Mark me done today");
        // Nothing is deleted.
        assert_eq!(find(&merged, "missing").list, "old");
        assert!(merged.lists.iter().any(|list| list.uid == "old"));
        // The file's text replaces the local text, which is kept as a copy.
        assert_eq!(find(&merged, "replaced").title, "Older title");
        assert_eq!(merged.conflicts, 1);
        assert_eq!(copies(&merged)[0].title, "Agreed title #conflict");
        assert_eq!(merged.tasks.len(), 5);
        // The same file merged normally would have deleted the missing task without a copy.
        let normal = merge(Some(&base), &local, Some(&remote), false);
        assert!(normal.tasks.iter().all(|task| task.uid != "missing"));
        assert_eq!(normal.conflicts, 0);
    }

    #[test]
    fn tasks_of_a_deleted_list_move_to_the_default_list_which_is_always_kept() {
        let base = snapshot(
            vec![list("inbox", 1, "Inbox"), list("errands", 2, "Errands")],
            vec![],
        );
        let mut local = base.clone();
        local.tasks.push(task("a", 1, "errands", "Added here"));
        let mut remote = base.clone();
        remote.lists.clear();
        let merged = run(Some(&base), &local, Some(&remote));
        assert_eq!(
            merged
                .lists
                .iter()
                .map(|list| list.uid.as_str())
                .collect::<Vec<_>>(),
            ["inbox"]
        );
        assert_eq!(find(&merged, "a").list, "inbox");
    }

    #[test]
    fn a_renamed_list_survives_deletion_and_duplicate_names_get_a_suffix() {
        let base = snapshot(
            vec![
                list("inbox", 1, "Inbox"),
                list("a", 2, "Alpha"),
                list("b", 3, "Beta"),
            ],
            vec![],
        );
        let mut local = base.clone();
        local.lists[0].name = "Work".into();
        local.lists.retain(|list| list.uid != "b");
        let mut remote = base.clone();
        remote.lists[1].name = "WORK".into();
        let merged = run(Some(&base), &local, Some(&remote));
        let names: Vec<(&str, &str)> = merged
            .lists
            .iter()
            .map(|list| (list.uid.as_str(), list.name.as_str()))
            .collect();
        assert_eq!(
            names,
            [("a", "Work"), ("b", "WORK (2)"), ("inbox", "Inbox")]
        );
    }

    #[test]
    fn equal_positions_become_unique_in_the_same_order_everywhere() {
        let base = snapshot(vec![list("inbox", 1, "Inbox")], vec![]);
        let mut local = base.clone();
        local.tasks.push(task("b", 60, "inbox", "Here"));
        local.tasks.push(task("z", 61, "inbox", "Later"));
        let mut remote = base.clone();
        remote.tasks.push(task("a", 60, "inbox", "There"));
        let here = run(Some(&base), &local, Some(&remote));
        let there = run(Some(&base), &remote, Some(&local));
        let positions = |merged: &Merged| -> Vec<(String, i64)> {
            merged
                .tasks
                .iter()
                .map(|task| (task.uid.clone(), task.position))
                .collect()
        };
        assert_eq!(
            positions(&here),
            [("a".into(), 60), ("b".into(), 61), ("z".into(), 62)]
        );
        assert_eq!(positions(&here), positions(&there));
    }

    #[test]
    fn records_without_a_base_merge_as_changed_on_both_sides() {
        let local = snapshot(
            vec![list("inbox", 1, "Inbox")],
            vec![task("a", 3, "inbox", "Same"), task("b", 4, "inbox", "Mine")],
        );
        let remote = snapshot(
            vec![list("inbox", 1, "Inbox")],
            vec![
                task("a", 9, "inbox", "Same"),
                task("b", 8, "inbox", "Theirs"),
            ],
        );
        let merged = run(None, &local, Some(&remote));
        assert_eq!(merged.adopted, []);
        assert_eq!(find(&merged, "a").id, 9);
        assert_eq!(find(&merged, "b").title, "Theirs");
        assert_eq!(copies(&merged)[0].title, "Mine #conflict");
        assert_eq!(merged.conflicts, 1);
    }

    #[test]
    fn a_copy_with_other_text_under_the_derived_identifier_does_not_block_a_new_copy() {
        let base = snapshot(
            vec![list("inbox", 1, "Inbox")],
            vec![task("a", 1, "inbox", "Title")],
        );
        let mut local = base.clone();
        local.tasks[0].title = "Mine".into();
        let mut remote = base.clone();
        remote.tasks[0].title = "Theirs".into();
        // A task already uses the identifier the copy would get, with other text.
        let taken = copy_uid("a", "Mine #conflict", "", 0);
        remote.tasks.push(task(&taken, 2, "inbox", "Edited copy"));
        let merged = run(Some(&base), &local, Some(&remote));
        assert_eq!(merged.conflicts, 1);
        assert_eq!(find(&merged, &taken).title, "Edited copy");
        assert_eq!(
            find(&merged, &copy_uid("a", "Mine #conflict", "", 1)).title,
            "Mine #conflict"
        );
    }
}
