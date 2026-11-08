use std::{collections::HashSet, io, path::Path, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension, Row, TransactionBehavior, params, types::Type};

use crate::model::{Status, Task, TaskInput, TaskList};

const TASK_COLUMNS: &str =
    "id, list_id, title, description, due, status, ready, tags, deleted, position, version";

pub struct Store {
    connection: Connection,
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
                        INSERT INTO lists(name, name_key) VALUES ('Inbox', 'inbox');
                        PRAGMA user_version = 1;",
                    )?;
                }
                1 => {}
                _ => bail!(
                    "Unsupported database schema version {version}. This Husker supports version 1."
                ),
            }
            transaction.commit()?;
        }

        connection.pragma_update(None, "journal_mode", "WAL")?;
        Ok(Self { connection })
    }

    pub fn lists(&self) -> Result<Vec<TaskList>> {
        let mut statement = self.connection.prepare(
            "SELECT lists.id, lists.name,
                    COUNT(CASE WHEN tasks.status = 'open' AND tasks.deleted = 0 THEN 1 END)
             FROM lists LEFT JOIN tasks ON tasks.list_id = lists.id
             GROUP BY lists.id ORDER BY lists.id",
        )?;
        let lists = statement
            .query_map([], |row| {
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
        let mut statement = self
            .connection
            .prepare(&format!("SELECT {TASK_COLUMNS} FROM tasks ORDER BY id"))?;
        let tasks = statement
            .query_map([], read_task)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(tasks)
    }

    pub fn task(&self, id: i64) -> Result<Task> {
        find_task(&self.connection, id)
    }

    pub fn create_list(&mut self, name: &str) -> Result<TaskList> {
        let (name, name_key) = normalize_list_name(name)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_unique_list_name(&transaction, &name_key, None)?;
        transaction.execute(
            "INSERT INTO lists(name, name_key) VALUES (?1, ?2)",
            params![name, name_key],
        )?;
        let id = transaction.last_insert_rowid();
        transaction.commit()?;
        Ok(TaskList { id, name, count: 0 })
    }

    pub fn rename_list(&mut self, id: i64, name: &str) -> Result<TaskList> {
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

    pub fn create_task(&mut self, input: &TaskInput) -> Result<Task> {
        let input = normalize_input(input)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_list(&transaction, input.list_id)?;
        transaction.execute(
            "INSERT INTO tasks
                (list_id, title, description, due, status, ready, tags, deleted, position, version)
             VALUES (?1, ?2, ?3, ?4, 'open', ?5, ?6, 0,
                (SELECT COALESCE(MAX(position), 0) + 1 FROM tasks), 1)",
            params![
                input.list_id,
                input.title,
                input.description,
                input.due.map(|date| date.to_string()),
                input.ready,
                serde_json::to_string(&input.tags)?,
            ],
        )?;
        let task = find_task(&transaction, transaction.last_insert_rowid())?;
        transaction.commit()?;
        Ok(task)
    }

    pub fn update_task(&mut self, id: i64, version: i64, input: &TaskInput) -> Result<Task> {
        let input = normalize_input(input)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_version(&find_task(&transaction, id)?, version)?;
        require_list(&transaction, input.list_id)?;
        transaction.execute(
            "UPDATE tasks SET list_id = ?1, title = ?2, description = ?3, due = ?4,
                ready = ?5, tags = ?6, version = version + 1 WHERE id = ?7 AND version = ?8",
            params![
                input.list_id,
                input.title,
                input.description,
                input.due.map(|date| date.to_string()),
                input.ready,
                serde_json::to_string(&input.tags)?,
                id,
                version,
            ],
        )?;
        let task = find_task(&transaction, id)?;
        transaction.commit()?;
        Ok(task)
    }

    pub fn set_status(&mut self, id: i64, version: i64, status: Status) -> Result<Task> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_version(&find_task(&transaction, id)?, version)?;
        transaction.execute(
            "UPDATE tasks SET status = ?1, version = version + 1 WHERE id = ?2 AND version = ?3",
            params![status.as_str(), id, version],
        )?;
        let task = find_task(&transaction, id)?;
        transaction.commit()?;
        Ok(task)
    }

    pub fn set_deleted(&mut self, id: i64, version: i64, deleted: bool) -> Result<Task> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_version(&find_task(&transaction, id)?, version)?;
        transaction.execute(
            "UPDATE tasks SET deleted = ?1, version = version + 1 WHERE id = ?2 AND version = ?3",
            params![deleted, id, version],
        )?;
        let task = find_task(&transaction, id)?;
        transaction.commit()?;
        Ok(task)
    }

    pub fn swap_positions(&mut self, first: &Task, second: &Task) -> Result<()> {
        ensure!(
            first.id != second.id,
            "Select two different tasks to reorder."
        );
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let stored_first = find_task(&transaction, first.id)?;
        let stored_second = find_task(&transaction, second.id)?;
        require_version(&stored_first, first.version)?;
        require_version(&stored_second, second.version)?;
        ensure!(
            !stored_first.deleted && !stored_second.deleted,
            "Tasks in Trash cannot be reordered."
        );
        ensure!(
            stored_first.due == stored_second.due
                && (stored_first.due.is_some() || stored_first.ready == stored_second.ready),
            "Reorder tasks within the same date or undated group."
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

fn normalize_input(input: &TaskInput) -> Result<TaskInput> {
    let mut input = input.clone();
    input.title = input.title.trim().to_owned();
    ensure!(!input.title.is_empty(), "A task title cannot be empty.");
    let mut tags = Vec::new();
    let mut seen = HashSet::new();
    for tag in &input.tags {
        let tag = tag.trim().trim_start_matches('#').to_lowercase();
        if tag.is_empty() {
            continue;
        }
        ensure!(
            !tag.chars().any(char::is_whitespace),
            "Tags cannot contain whitespace: {tag}"
        );
        if seen.insert(tag.clone()) {
            tags.push(tag);
        }
    }
    input.tags = tags;
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

fn find_task(connection: &Connection, id: i64) -> Result<Task> {
    connection
        .query_row(
            &format!("SELECT {TASK_COLUMNS} FROM tasks WHERE id = ?1"),
            [id],
            read_task,
        )
        .optional()?
        .with_context(|| format!("Task {id} does not exist."))
}

fn read_task(row: &Row<'_>) -> rusqlite::Result<Task> {
    let due: Option<String> = row.get(4)?;
    let due = due
        .map(|value| {
            NaiveDate::parse_from_str(&value, "%Y-%m-%d").map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(4, Type::Text, Box::new(error))
            })
        })
        .transpose()?;
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
    let tags: String = row.get(7)?;
    let tags = serde_json::from_str(&tags).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(7, Type::Text, Box::new(error))
    })?;
    Ok(Task {
        id: row.get(0)?,
        list_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        due,
        status,
        ready: row.get(6)?,
        tags,
        deleted: row.get(8)?,
        position: row.get(9)?,
        version: row.get(10)?,
    })
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
            ready: false,
            tags: Vec::new(),
        }
    }

    fn memory_store() -> Store {
        Store::open(Path::new(":memory:")).unwrap()
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
        fields.description = "Two cartons\nOat milk".into();
        fields.due = NaiveDate::from_ymd_opt(2026, 11, 8);
        fields.tags = vec!["Shopping".into()];
        let task = store.create_task(&fields).unwrap();
        store.rename_list(list.id, "Home").unwrap();
        drop(store);
        let store = Store::open(&path).unwrap();
        let saved = store.task(task.id).unwrap();
        assert_eq!(saved.title, "Buy milk");
        assert_eq!(saved.description, fields.description);
        assert_eq!(saved.due, fields.due);
        assert_eq!(saved.list_id, list.id);
        assert_eq!(saved.tags, ["shopping"]);
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
    fn tags_are_normalized_without_duplicate_names() {
        let mut store = memory_store();
        let mut fields = input("Buy milk");
        fields.tags = vec![
            "#Shopping".into(),
            "shopping".into(),
            " Ærinder ".into(),
            "".into(),
            "#".into(),
        ];
        let task = store.create_task(&fields).unwrap();
        assert_eq!(task.tags, ["shopping", "ærinder"]);
        fields.tags = vec!["two words".into()];
        assert!(store.create_task(&fields).is_err());
        assert_eq!(store.tasks().unwrap().len(), 1);
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
        let maybe = store.create_task(&fields).unwrap();
        fields.ready = true;
        let anytime = store.create_task(&fields).unwrap();
        assert!(store.swap_positions(&maybe, &anytime).is_err());
        assert!(store.swap_positions(&first, &maybe).is_err());
        assert!(store.swap_positions(&first, &first).is_err());
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
        assert_eq!(moved.ready, first.ready);
        let second = store.task(second.id).unwrap();
        let deleted = store.set_deleted(second.id, second.version, true).unwrap();
        assert!(store.swap_positions(&moved, &deleted).is_err());
    }

    #[test]
    fn storage_preserves_explicit_readiness_when_clearing_a_date() {
        let mut store = memory_store();
        let mut fields = input("Dated");
        fields.due = NaiveDate::from_ymd_opt(2026, 11, 8);
        let task = store.create_task(&fields).unwrap();
        fields.due = None;
        fields.ready = false;
        let task = store.update_task(task.id, task.version, &fields).unwrap();
        assert!(!task.ready);
        fields.ready = true;
        assert!(
            store
                .update_task(task.id, task.version, &fields)
                .unwrap()
                .ready
        );
    }

    #[test]
    fn newer_and_unversioned_databases_are_not_changed() {
        let directory = tempfile::tempdir().unwrap();
        for (name, version) in [("newer.db", 2), ("unversioned.db", 0)] {
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
}
