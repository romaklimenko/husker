## Context

Husker starts as an empty repository and a set of user notes. The user wants a working first version and has forbidden commits before review.
The app runs locally in a terminal. Windows is the available test platform.

## Goals / Non-Goals

Goals:

- Capture and manage tasks without leaving the keyboard.
- Keep data durable across restarts and detect stale edits from another process.
- Use the same storage and planning functions from the UI and CLI.
- Provide English and Danish interface text and configurable shortcuts.

Non-goals:

- Recurrence, reminders, subtasks, and priorities.
- Network synchronization, accounts, or shared profiles on network drives.
- List deletion and permanent deletion of tasks.
- MCP transport. Noninteractive JSON commands are included.
- Full text search indexes or performance claims without measurements.

## Decisions

### Store tasks in SQLite

Use `rusqlite` with bundled SQLite and transactions. Store the schema number in `PRAGMA user_version`.
SQLite gives the CLI and UI one transaction model. Plain text would need a separate locking and conflict protocol.
Keep configuration in TOML so users can change shortcuts with a text editor.

Use `~/.husker/tasks.db` by default. `--profile DIRECTORY` selects a different directory.
Use `<profile>/config.toml` unless `--config FILE` selects an existing configuration file.
Relative arguments resolve from the working directory.

### Separate task state from deletion

Use `open`, `done`, and `cancelled` for status. A separate deletion flag moves a task into virtual Trash without changing its list or status.
An undated task has `ready = false` for Maybe or `ready = true` for Anytime.
Due dates take precedence over readiness when grouping. Removing an existing due date makes the task ready.

### Sort dates before manual position

Use the system's local date and seven nonoverlapping groups. The planning spec owns their boundaries.
Within dated groups, sort by exact date, then manual position, then task ID.
Alt+Up/Down swaps two adjacent visible tasks only if their dates match or their undated readiness matches.
A reorder never changes a date, list, status, or readiness.

### Share Rust modules across interfaces

Use these module boundaries:

| Module | Responsibility |
| --- | --- |
| `model.rs` | Task, task input, list, and status types |
| `store.rs` | SQLite schema, validation, lists, task writes, and optimistic version checks |
| `planning.rs` | Date groups, date labels, and deterministic ordering |
| `query.rs` | Filter parsing and matching |
| `config.rs` | TOML settings, key parsing, per-mode conflict checks |
| `i18n.rs` | English and Danish UI labels |
| `ui.rs` | Ratatui rendering, input modes, and editor buffers |
| `main.rs` | Clap arguments, path resolution, commands, and output |

Keep the internal interfaces small:

```rust
pub struct Task {
    pub id: i64,
    pub list_id: i64,
    pub title: String,
    pub description: String,
    pub due: Option<chrono::NaiveDate>,
    pub status: Status,
    pub ready: bool,
    pub tags: Vec<String>,
    pub deleted: bool,
    pub position: i64,
    pub version: i64,
}
pub struct TaskInput {
    pub list_id: i64,
    pub title: String,
    pub description: String,
    pub due: Option<chrono::NaiveDate>,
    pub ready: bool,
    pub tags: Vec<String>,
}
pub struct TaskList { pub id: i64, pub name: String, pub count: usize }
pub enum Status { Open, Done, Cancelled }
```

`Store::open(&Path)` opens a database file. Writes take `&mut self`.
Read operations are `lists()`, `tasks()`, and `task(id)`.
Write operations are `create_list(name)`, `rename_list(id, name)`, `create_task(&TaskInput)`, and `update_task(id, version, &TaskInput)`.
`set_status(id, version, Status)` and `set_deleted(id, version, bool)` return the updated task.
`swap_positions(&Task, &Task)` validates both versions and the shared date or undated group.
All store operations return `anyhow::Result`.

`Query::parse(&str)` returns a validated query. `Query::default()` selects open tasks.
`query::visible_tasks(&[Task], &[TaskList], &Query, Option<i64>, bool, NaiveDate)` returns sorted cloned tasks.
The optional ID limits the list. The boolean selects Trash. Trash defaults to every status unless a status filter is explicit.

`Bucket::for_task(&Task, NaiveDate)` assigns a group. Each bucket exposes a stable English key for localization.
`planning::date_label(NaiveDate, Language)` formats the date, localized weekday, and ISO week.
Show the ISO week-year when it differs from the calendar year.

`Config::load(&Path, explicit)` loads defaults or a TOML override.
`Config::action(KeyEvent, Mode)` returns an optional `Action`.
`Language` lives in `i18n.rs`; `tr(Language, key)` returns a UI label.
`ui::run(&mut Store, &Config)` runs the interactive application.

### Detect concurrent task edits

Increment a task version on every write. A save updates only the expected version.
Keep an editor buffer open when the version check fails. The user can discard the buffer and reopen the current task.
Do not provide a silent overwrite path. Refresh the navigation view periodically to notice other processes and date changes.

### Keep scripts independent of UI language

Expose stable English command names, filter operators, JSON keys, and status values.
Use `--json` for structured output. Send errors to stderr and return a nonzero exit code.
The terminal interface uses the selected locale. User text is never translated.

## Risks / Trade-offs

| Risk | Response |
| --- | --- |
| Some terminals intercept Alt+arrow | Every action supports configurable keys. |
| Unicode text and narrow terminals | Test Danish input, cursor operations, and a small terminal size. |
| Interrupted writes or concurrent use | Use SQLite transactions and task version checks. |
| Editing the draft inbox at the same time | Capture verbatim notes and remove only unchanged completed blocks. |
| Unknown performance | Measure a fixture and record results without claiming universal latency. |

## Migration plan

Create schema version 1 in a new profile. Refuse a newer schema without modifying user data.
No earlier Husker schema exists. To back up data, close Husker and copy the whole profile directory.
Keep the database when removing the executable. No uninstall command deletes a profile.

## Evidence

- Read from the environment: Rust 1.91.1, Cargo 1.91.1, and OpenSpec 1.5.0 are installed.
- Read from upstream documentation: Ratatui 0.30 uses Crossterm 0.29 and requires Rust 1.88 or newer.
- Assumed before implementation: SQLite and in-memory filtering are sufficient for a personal task collection. Tests and a fixture measurement check this assumption.

## Open questions

No decision blocks this version. The user authorized a working version with reasonable defaults.
