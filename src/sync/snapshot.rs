//! The JSON document that devices exchange through the sync file.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use super::{Failure, FailureKind};
use crate::model::Status;

/// The only sync file format this Husker reads and writes.
pub const FORMAT: u32 = 1;

/// How many upload identifiers the sync file remembers.
pub const UPLOAD_HISTORY: usize = 20;

/// IDs and positions outside this range are refused, so arithmetic on them cannot overflow.
const MAX_NUMBER: i64 = 1 << 53;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ListRecord {
    pub uid: String,
    /// The ID on the device that first uploaded the list. Other devices use it when it is free.
    pub id: i64,
    pub name: String,
    pub position: i64,
}

impl ListRecord {
    /// Compares the merged fields, ignoring the ID hint.
    pub fn same_content(&self, other: &Self) -> bool {
        self.name == other.name && self.position == other.position
    }

    /// Reports whether the user changed nothing but the order.
    ///
    /// A new position alone does not keep a list that another device deleted.
    pub fn same_except_order(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskRecord {
    pub uid: String,
    /// The ID on the device that first uploaded the task. Other devices use it when it is free.
    pub id: i64,
    /// The `uid` of the task's list.
    pub list: String,
    pub title: String,
    pub description: String,
    pub due: Option<NaiveDate>,
    pub status: Status,
    pub status_changed_on: Option<NaiveDate>,
    pub status_changed_at: Option<DateTime<Utc>>,
    pub deleted: bool,
    pub position: i64,
}

impl TaskRecord {
    /// Compares the merged fields, ignoring the ID hint.
    pub fn same_content(&self, other: &Self) -> bool {
        let mut other = other.clone();
        other.id = self.id;
        *self == other
    }

    /// Reports whether the user changed nothing but the order.
    ///
    /// Positions also move when a neighbour is reordered or a merge renumbers
    /// them, so a new position alone does not keep a task that another device
    /// deleted forever.
    pub fn same_except_order(&self, other: &Self) -> bool {
        let mut other = other.clone();
        other.id = self.id;
        other.position = self.position;
        *self == other
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub format: u32,
    /// Identifies the sync file's lineage. A base from another collection deletes nothing.
    pub collection: String,
    /// The `uid` of the default list.
    pub default_list: String,
    /// Identifiers of the most recent uploads, oldest first.
    ///
    /// A device finds its own identifier here when an upload reached the
    /// storage but the answer was lost.
    pub uploads: Vec<String>,
    pub lists: Vec<ListRecord>,
    pub tasks: Vec<TaskRecord>,
}

#[derive(Deserialize)]
struct Header {
    format: Option<u32>,
}

impl Snapshot {
    /// Builds a snapshot with lists and tasks in `uid` order, so equal states compare equal.
    pub fn new(
        collection: String,
        default_list: String,
        mut lists: Vec<ListRecord>,
        mut tasks: Vec<TaskRecord>,
    ) -> Self {
        lists.sort_by(|left, right| left.uid.cmp(&right.uid));
        tasks.sort_by(|left, right| left.uid.cmp(&right.uid));
        Self {
            format: FORMAT,
            collection,
            default_list,
            uploads: Vec::new(),
            lists,
            tasks,
        }
    }

    /// Adds the identifier of a new upload and forgets the oldest beyond the history size.
    pub fn record_upload(&mut self, identifier: String) {
        self.uploads.push(identifier);
        let excess = self.uploads.len().saturating_sub(UPLOAD_HISTORY);
        self.uploads.drain(..excess);
    }

    /// Reports whether the file descends from `upload`, a snapshot this device sent.
    pub fn includes_upload(&self, upload: &Snapshot) -> bool {
        upload
            .uploads
            .last()
            .is_some_and(|identifier| self.uploads.contains(identifier))
    }

    /// Takes the collection, default list, upload history, and ID hints from `other`.
    ///
    /// Two snapshots then compare equal exactly when their lists and tasks hold the same values.
    pub fn framed_like(mut self, other: &Snapshot) -> Self {
        self.collection = other.collection.clone();
        self.default_list = other.default_list.clone();
        self.uploads = other.uploads.clone();
        self.with_ids_from(other)
    }

    /// Reads and validates a downloaded sync file. A rejected file never reaches the merge.
    pub fn parse(bytes: &[u8]) -> Result<Self, Failure> {
        let invalid = |detail: String| Failure::new(FailureKind::Invalid, detail);
        let header: Header = serde_json::from_slice(bytes)
            .map_err(|error| invalid(format!("The sync file is not valid JSON: {error}")))?;
        match header.format {
            Some(FORMAT) => {}
            Some(format) => {
                return Err(Failure::new(
                    FailureKind::Incompatible,
                    format!(
                        "The sync file uses format {format}. This Husker reads format {FORMAT}. Update Husker."
                    ),
                ));
            }
            None => return Err(invalid("The sync file has no format number.".into())),
        }
        let snapshot: Snapshot = serde_json::from_slice(bytes)
            .map_err(|error| invalid(format!("The sync file is not valid: {error}")))?;
        snapshot.validate().map_err(invalid)?;
        let uploads = snapshot.uploads;
        let mut snapshot = Snapshot::new(
            snapshot.collection,
            snapshot.default_list,
            snapshot.lists,
            snapshot.tasks,
        );
        snapshot.uploads = uploads;
        Ok(snapshot)
    }

    fn validate(&self) -> Result<(), String> {
        if self.collection.is_empty() {
            return Err("The sync file has no collection identifier.".into());
        }
        let in_range = |number: i64| (-MAX_NUMBER..=MAX_NUMBER).contains(&number);
        let mut lists = HashSet::new();
        for list in &self.lists {
            if list.uid.is_empty() || list.name.trim().is_empty() {
                return Err("The sync file has a list without an identifier or name.".into());
            }
            if list.id < 0 || !in_range(list.id) || !in_range(list.position) {
                return Err("The sync file has a list ID or position out of range.".into());
            }
            if !lists.insert(list.uid.as_str()) {
                return Err("The sync file repeats a list identifier.".into());
            }
        }
        let mut tasks = HashSet::new();
        for task in &self.tasks {
            if task.uid.is_empty() || task.title.is_empty() {
                return Err("The sync file has a task without an identifier or title.".into());
            }
            if task.id < 0 || !in_range(task.id) || !in_range(task.position) {
                return Err("The sync file has a task ID or position out of range.".into());
            }
            if !tasks.insert(task.uid.as_str()) {
                return Err("The sync file repeats a task identifier.".into());
            }
        }
        if !lists.contains(self.default_list.as_str()) {
            return Err("The sync file names a default list that it does not hold.".into());
        }
        Ok(())
    }

    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a snapshot serializes to JSON")
    }

    /// Replaces each ID hint with the one `other` holds for the same `uid`.
    ///
    /// Local IDs can differ from the hints in the file. The file's hints never change.
    pub fn with_ids_from(mut self, other: &Snapshot) -> Self {
        let lists: HashMap<&str, i64> = other
            .lists
            .iter()
            .map(|list| (list.uid.as_str(), list.id))
            .collect();
        for list in &mut self.lists {
            if let Some(id) = lists.get(list.uid.as_str()) {
                list.id = *id;
            }
        }
        let tasks: HashMap<&str, i64> = other
            .tasks
            .iter()
            .map(|task| (task.uid.as_str(), task.id))
            .collect();
        for task in &mut self.tasks {
            if let Some(id) = tasks.get(task.uid.as_str()) {
                task.id = *id;
            }
        }
        self
    }

    /// Counts the lists and tasks that `other` adds, changes, or removes compared to `self`.
    pub fn changes_to(&self, other: &Snapshot) -> usize {
        fn count<T: PartialEq>(
            before: &[T],
            after: &[T],
            uid: impl Fn(&T) -> &str + Copy,
        ) -> usize {
            let before: HashMap<&str, &T> = before.iter().map(|item| (uid(item), item)).collect();
            let mut changes = before.len();
            for item in after {
                match before.get(uid(item)) {
                    Some(existing) if *existing == item => changes -= 1,
                    Some(_) => {}
                    None => changes += 1,
                }
            }
            changes
        }
        count(&self.lists, &other.lists, |list| list.uid.as_str())
            + count(&self.tasks, &other.tasks, |task| task.uid.as_str())
    }
}

/// The key that decides whether two list names are the same list name.
pub fn name_key(name: &str) -> String {
    name.trim().to_lowercase()
}
