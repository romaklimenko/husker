//! Read views of a sync file snapshot for the online server.
//!
//! The query and planning functions work on `Task` and `TaskList` with integer
//! IDs. This module numbers the accessible records, calls those functions, and
//! maps the results back to the `uid` of each record. The numbers never leave
//! the server.

use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
use schemars::JsonSchema;
use serde::Serialize;

use super::doc::{Access, DocError, home, version};
use crate::{
    model::{Status, Task, TaskList},
    query::{Query, visible_tasks},
    sync::snapshot::{ListRecord, Snapshot, TaskRecord},
};

/// A task as the online server returns it. `id` and `list_id` are sync identifiers.
#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize)]
pub struct TaskView {
    pub id: String,
    pub list_id: String,
    pub title: String,
    pub description: String,
    pub due: Option<NaiveDate>,
    pub status: Status,
    pub status_changed_on: Option<NaiveDate>,
    pub status_changed_at: Option<DateTime<Utc>>,
    pub deleted: bool,
    pub position: i64,
    pub version: String,
}

impl From<&TaskRecord> for TaskView {
    fn from(task: &TaskRecord) -> Self {
        Self {
            id: task.uid.clone(),
            list_id: task.list.clone(),
            title: task.title.clone(),
            description: task.description.clone(),
            due: task.due,
            status: task.status,
            status_changed_on: task.status_changed_on,
            status_changed_at: task.status_changed_at,
            deleted: task.deleted,
            position: task.position,
            version: version(task),
        }
    }
}

/// A list as the online server returns it. `count` is its open tasks outside Trash.
#[derive(Clone, Debug, JsonSchema, PartialEq, Serialize)]
pub struct ListView {
    pub id: String,
    pub name: String,
    pub count: usize,
}

/// The accessible lists and tasks with the integer IDs the shared functions need.
pub struct Projection<'a> {
    /// Accessible lists in display order: the default list first, then by position.
    pub lists: Vec<TaskList>,
    /// Accessible tasks in ID order.
    pub tasks: Vec<Task>,
    list_records: Vec<&'a ListRecord>,
    task_records: Vec<&'a TaskRecord>,
}

impl<'a> Projection<'a> {
    pub fn new(snapshot: &'a Snapshot, access: &Access) -> Self {
        // Number every list, so a task keeps its list number under any scope.
        // The default list is the lowest, as it is on a device.
        let mut numbered: Vec<&ListRecord> = snapshot.lists.iter().collect();
        numbered.sort_by(|left, right| {
            (left.uid != snapshot.default_list, left.id, &left.uid).cmp(&(
                right.uid != snapshot.default_list,
                right.id,
                &right.uid,
            ))
        });
        let numbers: HashMap<&str, i64> = numbered
            .iter()
            .enumerate()
            .map(|(index, list)| (list.uid.as_str(), index as i64 + 1))
            .collect();
        let default_number = numbers
            .get(snapshot.default_list.as_str())
            .copied()
            .unwrap_or(1);

        // A task whose list is missing counts as a task of the default list, for reads and writes alike.
        let mut task_records: Vec<&TaskRecord> = snapshot
            .tasks
            .iter()
            .filter(|&task| access.covers(home(snapshot, task)))
            .collect();
        task_records.sort_by(|left, right| (left.id, &left.uid).cmp(&(right.id, &right.uid)));
        let tasks: Vec<Task> = task_records
            .iter()
            .enumerate()
            .map(|(index, task)| Task {
                id: index as i64 + 1,
                list_id: numbers
                    .get(task.list.as_str())
                    .copied()
                    .unwrap_or(default_number),
                title: task.title.clone(),
                description: task.description.clone(),
                due: task.due,
                status: task.status,
                status_changed_on: task.status_changed_on,
                status_changed_at: task.status_changed_at,
                deleted: task.deleted,
                position: task.position,
                version: 1,
            })
            .collect();

        let mut list_records: Vec<&ListRecord> = snapshot
            .lists
            .iter()
            .filter(|list| access.covers(&list.uid))
            .collect();
        list_records.sort_by(|left, right| {
            (
                left.uid != snapshot.default_list,
                left.position,
                numbers[left.uid.as_str()],
            )
                .cmp(&(
                    right.uid != snapshot.default_list,
                    right.position,
                    numbers[right.uid.as_str()],
                ))
        });
        let lists = list_records
            .iter()
            .map(|list| {
                let number = numbers[list.uid.as_str()];
                TaskList {
                    id: number,
                    name: list.name.clone(),
                    count: tasks
                        .iter()
                        .filter(|task| {
                            task.list_id == number && task.status == Status::Open && !task.deleted
                        })
                        .count(),
                }
            })
            .collect();

        Self {
            lists,
            tasks,
            list_records,
            task_records,
        }
    }

    /// The record behind a projected task.
    pub fn record(&self, task: &Task) -> &'a TaskRecord {
        self.task_records[(task.id - 1) as usize]
    }

    /// The `uid` of the accessible list with the projected number `number`.
    pub fn list_uid(&self, number: i64) -> Option<&'a str> {
        self.lists
            .iter()
            .position(|list| list.id == number)
            .map(|index| self.list_records[index].uid.as_str())
    }

    /// The view of a projected task. A task of a missing list reports the list it is shown in.
    pub fn task_view(&self, task: &Task) -> TaskView {
        let record = self.record(task);
        let mut view = TaskView::from(record);
        if let Some(list) = self.list_uid(task.list_id) {
            view.list_id = list.to_owned();
        }
        view
    }

    /// The accessible lists in display order.
    pub fn list_views(&self) -> Vec<ListView> {
        self.lists
            .iter()
            .zip(&self.list_records)
            .map(|(list, record)| ListView {
                id: record.uid.clone(),
                name: list.name.clone(),
                count: list.count,
            })
            .collect()
    }

    /// The number of accessible tasks in Trash, of every status.
    pub fn trash_count(&self) -> usize {
        self.tasks.iter().filter(|task| task.deleted).count()
    }
}

/// The accessible lists in display order and the accessible Trash count.
pub fn lists(snapshot: &Snapshot, access: &Access) -> (Vec<ListView>, usize) {
    let projection = Projection::new(snapshot, access);
    (projection.list_views(), projection.trash_count())
}

/// The accessible tasks that match `query`, in planning order.
pub fn tasks(
    snapshot: &Snapshot,
    access: &Access,
    query: &Query,
    trash: bool,
    today: NaiveDate,
) -> Vec<TaskView> {
    let projection = Projection::new(snapshot, access);
    visible_tasks(
        &projection.tasks,
        &projection.lists,
        query,
        None,
        trash,
        today,
    )
    .iter()
    .map(|task| projection.task_view(task))
    .collect()
}

/// One accessible task. A task outside the scope reads like a missing task.
pub fn task(snapshot: &Snapshot, access: &Access, uid: &str) -> Result<TaskView, DocError> {
    let projection = Projection::new(snapshot, access);
    projection
        .tasks
        .iter()
        .find(|task| projection.record(task).uid == uid)
        .map(|task| projection.task_view(task))
        .ok_or_else(|| DocError::TaskNotFound {
            uid: uid.to_owned(),
            scoped: access.list.is_some(),
        })
}

/// The view of a list record with its open-task count.
pub fn list(snapshot: &Snapshot, access: &Access, uid: &str) -> Option<ListView> {
    lists(snapshot, access)
        .0
        .into_iter()
        .find(|list| list.id == uid)
}
