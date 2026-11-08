## Why

Build a local Rust todo app with a keyboard-driven terminal interface and commands for scripts.
Husker keeps task capture, planning, and completion available without leaving the terminal.

## What Changes

The first version adds five capabilities:

| Capability | User-visible result |
| --- | --- |
| Task storage | Lists, task titles, descriptions, tags, dates, statuses, and recoverable Trash persist locally. |
| Planning and search | Seven date groups and combined list, text, tag, status, and date filters find tasks. |
| Terminal interface | Keyboard commands create, edit, complete, cancel, reorder, and restore tasks. |
| Profiles and localization | Separate profiles, configurable shortcuts, and English or Danish interface text. |
| Command line automation | Stable task IDs and JSON output support scripts without opening the terminal interface. |

## Capabilities

### New Capabilities

- `task-storage`: Persist lists and tasks, validate edits, and preserve deleted tasks in Trash.
- `planning-search`: Group and order tasks by local dates and evaluate combined filters.
- `terminal-interface`: Navigate lists and tasks, edit fields, and execute keyboard actions.
- `profiles-localization`: Select storage and configuration paths, language, and key bindings.
- `command-line`: Expose task and list operations through noninteractive commands and JSON output.

### Modified Capabilities

None. The repository has no existing application or stored-data format.

## Impact

The implementation adds these five areas:

| Area | Files or dependencies |
| --- | --- |
| Application | `Cargo.toml`, `Cargo.lock`, and Rust modules under `src/` |
| Storage | SQLite through `rusqlite`, with a database in the selected profile |
| Terminal | Ratatui and Crossterm |
| Input and configuration | Clap, Chrono, Serde, JSON, and TOML |
| Verification and guidance | Rust tests, CLI smoke checks, README, and configuration examples |

No existing data needs migration. The application creates schema version 1 for a new profile and refuses unsupported newer schemas.
No files are committed or pushed during this work.

Recurrence, reminders, subtasks, priorities, synchronization, permanent deletion, and MCP are outside this version.
Commands with JSON output provide an initial path for automation. MCP can reuse the same task operations later.
