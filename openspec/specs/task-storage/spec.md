# Task storage

## Purpose

Define how Husker stores lists and tasks, preserves deleted tasks, and handles conflicting writes.
These rules apply to terminal users and scripts. Planning and search owns task ordering and visibility.

## Requirements

### Requirement: Lists have persistent identities

Husker SHALL assign each list a persistent positive integer ID and create an `Inbox` list when initializing a new database.
Renaming a list preserves the list ID and its tasks.

#### Scenario: Rename a list with tasks

- **WHEN** list 2 named `Private` contains task 7 and the user renames list 2 to `Home`
- **THEN** list 2 is named `Home` and task 7 still belongs to list 2

### Requirement: List names are unambiguous

Husker SHALL trim list names and reject empty names, duplicate names after Unicode lowercase conversion, and the reserved names `Trash` and `Papirkurv` after the same conversion.
Rejected changes preserve the stored list name.

#### Scenario: Reject a duplicate Danish name

- **WHEN** a list named `Ærinder` exists and the user creates a list named `  ÆRINDER  `
- **THEN** creation fails and the existing list remains unchanged

### Requirement: Tasks preserve their fields

Husker SHALL persist each task with a stable positive integer ID and the fields in this table.

| Field | Stored value |
| --- | --- |
| `list_id` | ID of an existing list |
| `title` | Trimmed, nonempty text |
| `description` | Plain text, including newlines |
| `due` | An optional calendar date |
| `status` | `open`, `done`, or `cancelled` |
| `ready` | Whether an undated task belongs to Anytime |
| `tags` | Normalized tag names |
| `deleted` | Whether the task belongs to Trash |
| `position` | Persistent manual sort position |
| `version` | Integer used to detect stale edits |

A new task has status `open` and `deleted` set to false.
A new undated task has `ready` set to false unless the user requests Anytime.

#### Scenario: Save a task with a multiline description

- **WHEN** the user saves a task titled `Buy milk` with description `Two cartons` followed by a newline and `Oat milk`
- **THEN** reopening the database returns the same task ID, title, description, and list ID

### Requirement: Invalid task input does not change stored tasks

Husker SHALL reject an empty title or a task assignment to a nonexistent list before changing the stored task.
The storage operation reports an error to its caller.

#### Scenario: Reject an empty title

- **WHEN** the user changes task 7's title from `Buy milk` to whitespace
- **THEN** the save fails and task 7 remains titled `Buy milk`

### Requirement: Tags have a consistent stored form

Husker SHALL trim tag names, remove leading `#` characters, convert names to Unicode lowercase, remove empty tags, and remove duplicates.
Tags containing whitespace are rejected.

#### Scenario: Normalize repeated tags

- **WHEN** a task is saved with tags `#Shopping`, `shopping`, and ` Ærinder `
- **THEN** the stored tags contain `shopping` and `ærinder` once each

### Requirement: Trash preserves task information

Husker SHALL move a task to Trash by setting `deleted` to true and restore a task by setting `deleted` to false.
Both operations preserve the task's list, status, dates, description, and tags.
Husker does not permanently delete tasks or lists in this version.

#### Scenario: Restore a completed task

- **WHEN** task 7 in list 2 has status `done`, moves to Trash, and is restored
- **THEN** task 7 belongs to list 2, has status `done`, and is outside Trash

### Requirement: List counts describe open tasks

Husker SHALL count only tasks with status `open` and `deleted` set to false in ordinary list counts.
The Trash count includes deleted tasks of every status.

#### Scenario: Count mixed task statuses

- **WHEN** a list contains two open tasks, one done task, one cancelled task, and one deleted open task
- **THEN** the ordinary list count is 2 and the deleted task contributes 1 to the Trash count

### Requirement: Saves detect stale task versions

Husker SHALL require the task version read by the caller for an update, status change, Trash operation, or reorder.
A successful change increments the task version.
A mismatched version causes an error without overwriting the stored task.

#### Scenario: Reject an edit after another process saves

- **WHEN** two callers read task 7 at version 1 and the first caller saves version 2
- **THEN** the second caller's save using version 1 fails and the first caller's saved fields remain unchanged

### Requirement: Manual reordering preserves dates and groups

Husker SHALL swap two task positions atomically only when both tasks are outside Trash and have the same due date.
For two undated tasks, both tasks must also have the same `ready` value.
The swap preserves all task fields except `position` and `version`.

#### Scenario: Refuse a move across dates

- **WHEN** task 7 is due `2026-11-08` and task 8 is due `2026-11-09`
- **THEN** swapping their positions fails and both dates and positions remain unchanged

#### Scenario: Reject a stale pair atomically

- **WHEN** task 7 and task 8 share a due date but task 8's version changed after the caller read it
- **THEN** the swap fails without changing either task's position

### Requirement: Database writes are atomic

Husker SHALL apply each operation in a SQLite transaction and report storage failures without replacing or resetting an existing database.
A failed operation preserves the database state from before that operation.

#### Scenario: Preserve a database that cannot be opened

- **WHEN** an existing `tasks.db` cannot be read as a SQLite database
- **THEN** opening the profile fails and Husker does not replace `tasks.db`

### Requirement: Schema versions protect stored data

Husker SHALL initialize new databases at schema version 1 and reject an existing database with a newer schema version.
An unversioned database containing existing user tables is rejected.

#### Scenario: Refuse a database from a newer version

- **WHEN** `tasks.db` reports schema version 2
- **THEN** opening the profile fails without modifying its schema or task data
