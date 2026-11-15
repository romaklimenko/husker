//! Changes to one list or task of a sync file snapshot.
//!
//! Everything here is pure: no I/O, no clock, and no random source. The caller
//! supplies new identifiers and the current date and time, so a retry applies
//! exactly the same change.

use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use chrono::{DateTime, NaiveDate, Utc};
use sha2::{Digest, Sha256};

use crate::{
    model::Status,
    rules,
    sync::snapshot::{ListRecord, Snapshot, TaskRecord, name_key},
};

/// IDs and positions above this are refused, as the sync file validation does.
const MAX_NUMBER: i64 = 1 << 53;

/// The longest task title the online server accepts, in characters.
pub const MAX_TITLE_CHARS: usize = 1_000;

/// The longest task description the online server accepts, in characters.
pub const MAX_DESCRIPTION_CHARS: usize = 50_000;

/// The longest list name the online server accepts, in characters.
pub const MAX_LIST_NAME_CHARS: usize = 200;

/// What one request may read and change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Access {
    /// The `uid` of the only accessible list, or `None` for every list.
    pub list: Option<String>,
    pub read_only: bool,
}

impl Access {
    /// Reports whether a task in the list `list` is accessible.
    pub fn covers(&self, list: &str) -> bool {
        self.list.as_deref().is_none_or(|scope| scope == list)
    }
}

/// The date and time of one request. Status changes store them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Stamp {
    pub today: NaiveDate,
    pub now: DateTime<Utc>,
}

/// One change to one list or task.
///
/// A `version` of `None` skips the stale check. The MCP tools always send one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Op {
    AddTask {
        /// The `uid` of the new task. The caller creates it once per request.
        uid: String,
        /// The `uid` of the list, or `None` for the default accessible list.
        list: Option<String>,
        title: String,
        description: String,
        due: Option<NaiveDate>,
    },
    UpdateTask {
        uid: String,
        version: Option<String>,
        list: Option<String>,
        title: Option<String>,
        description: Option<String>,
        /// `Some(None)` removes the date.
        due: Option<Option<NaiveDate>>,
    },
    SetStatus {
        uid: String,
        version: Option<String>,
        status: Status,
    },
    SetDeleted {
        uid: String,
        version: Option<String>,
        deleted: bool,
    },
    CreateList {
        uid: String,
        name: String,
    },
    RenameList {
        uid: String,
        name: String,
    },
}

/// The list or task an operation created, changed, or left as it was.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Changed {
    Task(TaskRecord),
    List(ListRecord),
}

/// Why an operation or an upload was refused. The sentences match the store's.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocError {
    ReadOnly,
    EmptyTitle,
    ListName(String),
    DuplicateListName,
    TaskNotFound {
        uid: String,
        scoped: bool,
    },
    ListNotFound(String),
    NoAccessibleList,
    Restricted,
    ScopedLists,
    Stale(String),
    TaskExists(String),
    ListExists(String),
    OutOfRange,
    /// A text is longer than the online server accepts. Every upload sends the whole file.
    TooLong {
        field: &'static str,
        limit: usize,
    },
    FileTooLarge,
    Guard(&'static str),
}

impl fmt::Display for DocError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadOnly => formatter.write_str("This store is read-only."),
            Self::EmptyTitle => formatter.write_str("A task title cannot be empty."),
            Self::ListName(message) => formatter.write_str(message),
            Self::DuplicateListName => formatter.write_str("A list with that name already exists."),
            Self::TaskNotFound { scoped: true, .. } => formatter.write_str("Task not found."),
            Self::TaskNotFound { uid, scoped: false } => {
                write!(formatter, "Task {uid} does not exist.")
            }
            Self::ListNotFound(uid) => write!(formatter, "List {uid} does not exist."),
            Self::NoAccessibleList => formatter.write_str("No accessible list exists."),
            Self::Restricted => formatter.write_str("This store is restricted to one list."),
            Self::ScopedLists => formatter
                .write_str("A list-scoped store cannot create, rename, reorder, or delete lists."),
            Self::Stale(uid) => write!(
                formatter,
                "Task {uid} changed in another process. Reload it before saving."
            ),
            Self::TaskExists(uid) => write!(formatter, "Task {uid} already exists."),
            Self::ListExists(uid) => write!(formatter, "List {uid} already exists."),
            Self::OutOfRange => {
                formatter.write_str("The sync file cannot hold more lists or tasks.")
            }
            Self::TooLong { field, limit } => write!(
                formatter,
                "A {field} can have at most {limit} characters online."
            ),
            Self::FileTooLarge => {
                formatter.write_str("The sync file is too large for the online server.")
            }
            Self::Guard(reason) => write!(formatter, "The change was not uploaded: {reason}."),
        }
    }
}

impl std::error::Error for DocError {}

/// The version token of a task: a hash of the fields a client can see or set.
///
/// `id` and `position` are left out. A merge on a device renumbers positions,
/// and that must not make a client's version stale.
pub fn version(task: &TaskRecord) -> String {
    let fields = serde_json::json!([
        task.uid,
        task.list,
        task.title,
        task.description,
        task.due,
        task.status,
        task.status_changed_on,
        task.status_changed_at,
        task.deleted,
    ]);
    Sha256::digest(fields.to_string().as_bytes())[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The list a task counts as belonging to: its own list, or the default list when that list is missing.
///
/// A merge on a device moves such a task to the default list. Reads, writes, and the
/// guard all use this rule, so a list scope whose list is missing reaches no task.
pub fn home<'a>(snapshot: &'a Snapshot, task: &'a TaskRecord) -> &'a str {
    if snapshot.lists.iter().any(|list| list.uid == task.list) {
        &task.list
    } else {
        &snapshot.default_list
    }
}

/// Finds a list by name, or else by `uid`. Returns its `uid`.
pub fn resolve_list(snapshot: &Snapshot, name_or_uid: &str) -> Option<String> {
    let key = name_key(name_or_uid);
    snapshot
        .lists
        .iter()
        .find(|list| name_key(&list.name) == key)
        .or_else(|| snapshot.lists.iter().find(|list| list.uid == name_or_uid))
        .map(|list| list.uid.clone())
}

/// Applies one change to the snapshot and returns the affected list or task.
///
/// On an error the snapshot is unchanged. The checks run in the order a local
/// MCP tool gives: write access, the task within the scope, input rules, the
/// destination within the scope, the version, and the destination's existence.
pub fn apply(
    snapshot: &mut Snapshot,
    access: &Access,
    op: &Op,
    stamp: &Stamp,
) -> Result<Changed, DocError> {
    if access.read_only {
        return Err(DocError::ReadOnly);
    }
    match op {
        Op::AddTask {
            uid,
            list,
            title,
            description,
            due,
        } => {
            let title = rules::normalize_title(title).map_err(|_| DocError::EmptyTitle)?;
            within(&title, "task title", MAX_TITLE_CHARS)?;
            within(description, "task description", MAX_DESCRIPTION_CHARS)?;
            let list = match list {
                Some(list) => list.clone(),
                None => default_list(snapshot, access)?,
            };
            if !access.covers(&list) {
                return Err(DocError::Restricted);
            }
            require_list(snapshot, &list)?;
            if let Some(existing) = snapshot.tasks.iter().find(|task| task.uid == *uid) {
                // A repeated request after a lost answer finds its own task.
                let same = existing.list == list
                    && existing.title == title
                    && existing.description == *description
                    && existing.due == *due;
                return if same && access.covers(&existing.list) {
                    Ok(Changed::Task(existing.clone()))
                } else {
                    Err(DocError::TaskExists(uid.clone()))
                };
            }
            let task = TaskRecord {
                uid: uid.clone(),
                id: next_number(snapshot.tasks.iter().map(|task| task.id))?,
                list,
                title,
                description: description.clone(),
                due: *due,
                status: Status::Open,
                status_changed_on: None,
                status_changed_at: None,
                deleted: false,
                position: next_number(snapshot.tasks.iter().map(|task| task.position))?,
            };
            snapshot.tasks.push(task.clone());
            snapshot
                .tasks
                .sort_by(|left, right| left.uid.cmp(&right.uid));
            Ok(Changed::Task(task))
        }
        Op::UpdateTask {
            uid,
            version: expected,
            list,
            title,
            description,
            due,
        } => {
            let index = find_task(snapshot, access, uid)?;
            let current = &snapshot.tasks[index];
            let title = match title {
                Some(title) => rules::normalize_title(title).map_err(|_| DocError::EmptyTitle)?,
                None => current.title.clone(),
            };
            if title != current.title {
                within(&title, "task title", MAX_TITLE_CHARS)?;
            }
            if let Some(description) = description
                .as_ref()
                .filter(|text| **text != current.description)
            {
                within(description, "task description", MAX_DESCRIPTION_CHARS)?;
            }
            let list = list.clone().unwrap_or_else(|| current.list.clone());
            if !access.covers(&list) {
                return Err(DocError::Restricted);
            }
            require_version(current, expected.as_deref())?;
            // A task of a missing list keeps that list until a client moves it.
            if list != current.list {
                require_list(snapshot, &list)?;
            }
            let task = &mut snapshot.tasks[index];
            task.title = title;
            task.list = list;
            if let Some(description) = description {
                task.description = description.clone();
            }
            if let Some(due) = due {
                task.due = *due;
            }
            Ok(Changed::Task(task.clone()))
        }
        Op::SetStatus {
            uid,
            version: expected,
            status,
        } => {
            let index = find_task(snapshot, access, uid)?;
            require_version(&snapshot.tasks[index], expected.as_deref())?;
            let task = &mut snapshot.tasks[index];
            let (changed_on, changed_at) = rules::status_stamp(
                task.status,
                (task.status_changed_on, task.status_changed_at),
                *status,
                stamp.today,
                stamp.now,
            );
            task.status = *status;
            task.status_changed_on = changed_on;
            task.status_changed_at = changed_at;
            Ok(Changed::Task(task.clone()))
        }
        Op::SetDeleted {
            uid,
            version: expected,
            deleted,
        } => {
            let index = find_task(snapshot, access, uid)?;
            require_version(&snapshot.tasks[index], expected.as_deref())?;
            let task = &mut snapshot.tasks[index];
            task.deleted = *deleted;
            Ok(Changed::Task(task.clone()))
        }
        Op::CreateList { uid, name } => {
            if access.list.is_some() {
                return Err(DocError::ScopedLists);
            }
            let (name, key) = rules::normalize_list_name(name)
                .map_err(|error| DocError::ListName(error.to_string()))?;
            within(&name, "list name", MAX_LIST_NAME_CHARS)?;
            if let Some(existing) = snapshot.lists.iter().find(|list| list.uid == *uid) {
                return if existing.name == name {
                    Ok(Changed::List(existing.clone()))
                } else {
                    Err(DocError::ListExists(uid.clone()))
                };
            }
            require_unique_name(snapshot, &key, None)?;
            let list = ListRecord {
                uid: uid.clone(),
                id: next_number(snapshot.lists.iter().map(|list| list.id))?,
                name,
                position: next_number(snapshot.lists.iter().map(|list| list.position))?,
            };
            snapshot.lists.push(list.clone());
            snapshot
                .lists
                .sort_by(|left, right| left.uid.cmp(&right.uid));
            Ok(Changed::List(list))
        }
        Op::RenameList { uid, name } => {
            if access.list.is_some() {
                return Err(DocError::ScopedLists);
            }
            let (name, key) = rules::normalize_list_name(name)
                .map_err(|error| DocError::ListName(error.to_string()))?;
            within(&name, "list name", MAX_LIST_NAME_CHARS)?;
            require_list(snapshot, uid)?;
            require_unique_name(snapshot, &key, Some(uid))?;
            let list = snapshot
                .lists
                .iter_mut()
                .find(|list| list.uid == *uid)
                .expect("the list was found above");
            list.name = name;
            Ok(Changed::List(list.clone()))
        }
    }
}

/// Refuses an upload that differs from the download in more than one record.
///
/// `Snapshot::parse` accepts files that a device would repair and upload again.
/// This check keeps a mistake in `apply` from reaching more than one record,
/// and it checks the scope once more on the exact snapshot that is uploaded.
pub fn guard(before: &Snapshot, after: &Snapshot, access: &Access) -> Result<(), DocError> {
    if access.read_only {
        return Err(DocError::Guard("the access is read-only"));
    }
    if before.format != after.format
        || before.collection != after.collection
        || before.default_list != after.default_list
        || before.uploads != after.uploads
    {
        return Err(DocError::Guard("a header member changed"));
    }

    let mut differences = 0;
    let old_lists: HashMap<&str, &ListRecord> = before
        .lists
        .iter()
        .map(|list| (list.uid.as_str(), list))
        .collect();
    if after.lists.len() < old_lists.len() {
        return Err(DocError::Guard("a list is missing"));
    }
    let mut kept = 0;
    let mut seen = HashSet::new();
    for list in &after.lists {
        if !seen.insert(list.uid.as_str()) {
            return Err(DocError::Guard("a list is repeated"));
        }
        match old_lists.get(list.uid.as_str()) {
            Some(old) => {
                kept += 1;
                if old.id != list.id || old.position != list.position {
                    return Err(DocError::Guard("a list changed its ID or position"));
                }
                if *old != list {
                    differences += 1;
                    if access.list.is_some() {
                        return Err(DocError::Guard("a list changed under a list scope"));
                    }
                }
            }
            None => {
                differences += 1;
                if access.list.is_some() {
                    return Err(DocError::Guard("a list was added under a list scope"));
                }
            }
        }
    }
    if kept != old_lists.len() {
        return Err(DocError::Guard("a list is missing"));
    }

    let old_tasks: HashMap<&str, &TaskRecord> = before
        .tasks
        .iter()
        .map(|task| (task.uid.as_str(), task))
        .collect();
    let mut kept = 0;
    let mut seen = HashSet::new();
    let list_exists = |uid: &str| after.lists.iter().any(|list| list.uid == uid);
    for task in &after.tasks {
        if !seen.insert(task.uid.as_str()) {
            return Err(DocError::Guard("a task is repeated"));
        }
        match old_tasks.get(task.uid.as_str()) {
            Some(old) => {
                kept += 1;
                if old.id != task.id || old.position != task.position {
                    return Err(DocError::Guard("a task changed its ID or position"));
                }
                if *old != task {
                    differences += 1;
                    if !access.covers(home(before, old)) || !access.covers(home(after, task)) {
                        return Err(DocError::Guard("a task changed outside the list scope"));
                    }
                    if old.list != task.list && !list_exists(&task.list) {
                        return Err(DocError::Guard(
                            "a task moved to a list that does not exist",
                        ));
                    }
                }
            }
            None => {
                differences += 1;
                if !list_exists(&task.list) {
                    return Err(DocError::Guard(
                        "a task was added to a list that does not exist",
                    ));
                }
                if !access.covers(&task.list) {
                    return Err(DocError::Guard("a task was added outside the list scope"));
                }
            }
        }
    }
    if kept != old_tasks.len() {
        return Err(DocError::Guard("a task is missing"));
    }
    if differences > 1 {
        return Err(DocError::Guard("more than one list or task changed"));
    }
    Ok(())
}

/// Refuses a text that is longer than the online server accepts.
fn within(text: &str, field: &'static str, limit: usize) -> Result<(), DocError> {
    if text.chars().count() > limit {
        Err(DocError::TooLong { field, limit })
    } else {
        Ok(())
    }
}

/// The list a new task gets when the client names none.
fn default_list(snapshot: &Snapshot, access: &Access) -> Result<String, DocError> {
    let list = access
        .list
        .clone()
        .unwrap_or_else(|| snapshot.default_list.clone());
    if snapshot.lists.iter().any(|record| record.uid == list) {
        Ok(list)
    } else {
        Err(DocError::NoAccessibleList)
    }
}

fn find_task(snapshot: &Snapshot, access: &Access, uid: &str) -> Result<usize, DocError> {
    snapshot
        .tasks
        .iter()
        .position(|task| task.uid == uid && access.covers(home(snapshot, task)))
        .ok_or_else(|| DocError::TaskNotFound {
            uid: uid.to_owned(),
            scoped: access.list.is_some(),
        })
}

fn require_list(snapshot: &Snapshot, uid: &str) -> Result<(), DocError> {
    if snapshot.lists.iter().any(|list| list.uid == uid) {
        Ok(())
    } else {
        Err(DocError::ListNotFound(uid.to_owned()))
    }
}

fn require_unique_name(
    snapshot: &Snapshot,
    key: &str,
    except: Option<&str>,
) -> Result<(), DocError> {
    let duplicate = snapshot
        .lists
        .iter()
        .any(|list| Some(list.uid.as_str()) != except && name_key(&list.name) == key);
    if duplicate {
        Err(DocError::DuplicateListName)
    } else {
        Ok(())
    }
}

fn require_version(task: &TaskRecord, expected: Option<&str>) -> Result<(), DocError> {
    match expected {
        Some(expected) if expected != version(task) => Err(DocError::Stale(task.uid.clone())),
        _ => Ok(()),
    }
}

/// One more than the highest number in use, starting at 1.
fn next_number(numbers: impl Iterator<Item = i64>) -> Result<i64, DocError> {
    let next = numbers.max().unwrap_or(0).max(0) + 1;
    if next > MAX_NUMBER {
        Err(DocError::OutOfRange)
    } else {
        Ok(next)
    }
}
