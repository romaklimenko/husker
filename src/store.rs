use std::{io, path::Path, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use chrono::{Local, NaiveDate};
use rusqlite::{
    Connection, OpenFlags, OptionalExtension, Row, TransactionBehavior, params, types::Type,
};

use crate::model::{Status, Task, TaskInput, TaskList};

const TASK_COLUMNS: &str =
    "id, list_id, title, description, due, status, status_changed_on, deleted, position, version";

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
                            deleted INTEGER NOT NULL CHECK(deleted IN (0, 1)),
                            position INTEGER NOT NULL,
                            version INTEGER NOT NULL CHECK(version > 0)
                        );
                        CREATE INDEX tasks_list ON tasks(list_id);
                        CREATE INDEX tasks_planning ON tasks(deleted, status, due, position);
                        INSERT INTO lists(name, name_key) VALUES ('Inbox', 'inbox');
                        PRAGMA user_version = 3;",
                    )?;
                }
                1 => {
                    migrate_schema_one(&transaction)?;
                    migrate_schema_two(&transaction)?;
                }
                2 => migrate_schema_two(&transaction)?,
                3 => {}
                _ => bail!(
                    "Unsupported database schema version {version}. This Husker supports version 3."
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
            "A list-scoped store cannot create, rename, or reorder lists."
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
            "INSERT INTO lists(name, name_key, position)
             VALUES (?1, ?2, (SELECT COALESCE(MAX(position), 0) + 1 FROM lists))",
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
                (list_id, title, description, due, status, status_changed_on, deleted, position, version)
             VALUES (?1, ?2, ?3, ?4, 'open', NULL, 0,
                (SELECT COALESCE(MAX(position), 0) + 1 FROM tasks), 1)",
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

    pub fn set_status(&mut self, id: i64, version: i64, status: Status) -> Result<Task> {
        self.set_status_on(id, version, status, Local::now().date_naive())
    }

    pub fn set_status_on(
        &mut self,
        id: i64,
        version: i64,
        status: Status,
        today: NaiveDate,
    ) -> Result<Task> {
        self.require_writable()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let stored = find_task(&transaction, id, self.list_scope)?;
        require_version(&stored, version)?;
        let status_changed_on = match status {
            Status::Open => None,
            _ if status == stored.status => stored.status_changed_on,
            _ => Some(today),
        };
        transaction.execute(
            "UPDATE tasks SET status = ?1, status_changed_on = ?2,
                 version = version + 1 WHERE id = ?3 AND version = ?4",
            params![
                status.as_str(),
                status_changed_on.map(|date| date.to_string()),
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

fn require_current_schema(version: i64) -> Result<()> {
    if version == 1 || version == 2 {
        bail!(
            "Database schema version {version} needs an upgrade. Open this profile with writable Husker before connecting read-only."
        );
    }
    ensure!(
        version == 3,
        "Unsupported database schema version {version}. This Husker supports version 3."
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

fn find_task(connection: &Connection, id: i64, list_scope: Option<i64>) -> Result<Task> {
    connection
        .query_row(
            &format!(
                "SELECT {TASK_COLUMNS} FROM tasks
                 WHERE id = ?1 AND (?2 IS NULL OR list_id = ?2)"
            ),
            params![id, list_scope],
            read_task,
        )
        .optional()?
        .with_context(|| match list_scope {
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
        assert!(!milk.deleted);
        assert_eq!((milk.position, milk.version), (5, 4));
        let mut before_upgrade = TaskInput::from(&milk);
        before_upgrade.description = "Keep this description".into();
        assert!(store.update_task(milk.id, 3, &before_upgrade).is_err());
        assert_eq!(store.task(milk.id).unwrap(), milk);
        let done = store.task(9).unwrap();
        assert_eq!(done.status, Status::Done);
        assert_eq!(done.status_changed_on, None);
        assert_eq!(done.description, "#archive");
        assert_eq!((done.position, done.version), (8, 5));
        let cancelled = store.task(10).unwrap();
        assert_eq!(cancelled.status, Status::Cancelled);
        assert_eq!(cancelled.status_changed_on, None);
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
        assert_eq!(version, 3);
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
        let before: Vec<Task> = connection
            .prepare(&format!("SELECT {TASK_COLUMNS} FROM tasks ORDER BY id"))
            .unwrap()
            .query_map([], read_task)
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
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
        store.swap_positions(&task, &neighbor).unwrap();
        let task = store.task(task.id).unwrap();
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
        let task = store
            .set_status_on(task.id, task.version, Status::Done, tomorrow)
            .unwrap();
        assert_eq!(task.status_changed_on, Some(tomorrow));
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
        let undated = store.create_task(&fields).unwrap();
        let another_undated = store.create_task(&fields).unwrap();
        assert!(store.swap_positions(&first, &undated).is_err());
        assert!(store.swap_positions(&first, &first).is_err());
        store.swap_positions(&undated, &another_undated).unwrap();
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
    fn newer_and_unversioned_databases_are_not_changed() {
        let directory = tempfile::tempdir().unwrap();
        for (name, version) in [("newer.db", 4), ("unversioned.db", 0)] {
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
        assert_eq!(other.task(first.id).unwrap(), first);
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
        let baseline = other.tasks().unwrap();
        let mut readonly = Store::open_for_mcp(&path, Some("Inbox"), true).unwrap();
        assert!(readonly.is_read_only());
        assert_eq!(readonly.list_scope(), Some(1));
        assert_eq!(readonly.lists().unwrap().len(), 1);
        assert_eq!(readonly.tasks().unwrap().len(), 2);
        assert_eq!(readonly.task(first.id).unwrap(), first);
        let errors = [
            readonly.create_list("New").unwrap_err(),
            readonly.rename_list(1, "Renamed").unwrap_err(),
            readonly.move_list(1, true).unwrap_err(),
            readonly.create_task(&input("New")).unwrap_err(),
            readonly
                .update_task(first.id, first.version, &TaskInput::from(&first))
                .unwrap_err(),
            readonly
                .set_status(first.id, first.version, Status::Open)
                .unwrap_err(),
            readonly
                .set_deleted(first.id, first.version, false)
                .unwrap_err(),
            readonly.swap_positions(&first, &second).unwrap_err(),
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
        for version in [0, 1, 2, 4] {
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
}
