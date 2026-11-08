## MODIFIED Requirements

### Requirement: Tasks preserve their fields

Husker SHALL persist each task with a stable positive integer ID and the fields in this table.

| Field | Stored value |
| --- | --- |
| `list_id` | ID of an existing list |
| `title` | Trimmed, nonempty text |
| `description` | Plain text, including newlines |
| `due` | An optional calendar date |
| `status` | `open`, `done`, or `cancelled` |
| `status_changed_on` | Local date of the most recent change to done or cancelled, or null |
| `deleted` | Whether the task belongs to Trash |
| `position` | Persistent manual sort position |
| `version` | Integer used to detect stale edits |

A new task has status `open`, `deleted` set to false, and `status_changed_on` set to null.
Tasks do not have structured tags or readiness fields.

#### Scenario: Save a task with a multiline description

- **WHEN** the user saves a task titled `Buy milk` with description `Two cartons` followed by a newline and `Oat milk #shopping`
- **THEN** reopening the database returns the same task ID, title, description, and list ID

### Requirement: Trash preserves task information

Husker SHALL move a task to Trash by setting `deleted` to true and restore a task by setting `deleted` to false.
Both operations preserve the task's list, status, status-change date, due date, description, and title.
Husker does not permanently delete tasks or lists in this version.

#### Scenario: Restore a completed task

- **WHEN** task 7 in list 2 has status `done`, moves to Trash, and is restored
- **THEN** task 7 belongs to list 2, has status `done`, and is outside Trash
- **AND** its original `status_changed_on` date is unchanged

### Requirement: Manual reordering preserves dates and groups

Husker SHALL swap two task positions atomically only when both tasks are outside Trash and have the same due date.
Two undated tasks have the same due date for this rule.
The swap preserves all task fields except `position` and `version`.

#### Scenario: Refuse a move across dates

- **WHEN** task 7 is due `2026-11-08` and task 8 is due `2026-11-09`
- **THEN** swapping their positions fails and both dates and positions remain unchanged

#### Scenario: Reject a stale pair atomically

- **WHEN** task 7 and task 8 share a due date but task 8's version changed after the caller read it
- **THEN** the swap fails without changing either task's position

### Requirement: Schema versions protect stored data

Husker SHALL initialize new databases at schema version 2 and migrate schema version 1 atomically when opening a writable database.
Husker rejects an existing database with a newer schema version or unversioned user tables.

Migration preserves list IDs, task IDs, list membership, titles, due dates, statuses, Trash state, and manual positions.
Migration removes readiness and structured tags.
Each existing tag is preserved as literal `#tag` text appended to the task description.
Existing description text remains intact.
Migration increments the version of each task whose description changes.
Tasks without appended tags keep their versions.
Tasks migrated from version 1 have `status_changed_on` set to null because their completion date is unknown.
A failed migration preserves the original schema and data.
Invalid legacy tag data causes migration to fail without discarding that data.

#### Scenario: Preserve old tags as searchable text

- **WHEN** a version 1 task has description `Buy oat milk` and tag `shopping`
- **THEN** migration preserves `Buy oat milk` and appends `#shopping` to its description
- **AND** the task keeps its ID, status, and due date and its version increases by one

#### Scenario: Reject an edit read before tag conversion

- **WHEN** a client reads a tagged task at version 3 before migration and later submits that version to the upgraded server
- **THEN** the edit fails because migration changed the task's description and version
- **AND** the appended hashtag text remains intact

#### Scenario: Do not invent a completion date

- **WHEN** a version 1 task has status `done`
- **THEN** migration preserves `done` and sets `status_changed_on` to null

#### Scenario: Refuse a database from a newer version

- **WHEN** `tasks.db` reports schema version 3
- **THEN** opening the profile fails without modifying its schema or task data

### Requirement: Read-only stores open existing databases

A read-only MCP store SHALL open an existing SQLite database with read-only access and verify schema version 2 without initializing tables, migrating data, or changing journal mode.
A missing database, an unsupported schema, or an unreadable database causes startup to fail.
A version 1 database requires a writable Husker launch before a read-only MCP connection can open it.

#### Scenario: Preserve a missing profile

- **WHEN** a read-only MCP store opens a path with no database
- **THEN** opening fails and no database is created

#### Scenario: Preserve an unsupported schema

- **WHEN** a read-only MCP store opens a database with schema version 1 or 3
- **THEN** opening fails without changing its schema or task data

## ADDED Requirements

### Requirement: Status changes record their local date

Husker SHALL set `status_changed_on` to the current local calendar date when a task changes to done or cancelled.
Reopening a task clears `status_changed_on`.
Setting an already-done task to done, or an already-cancelled task to cancelled, preserves the existing date.
Editing other task fields, reordering, moving to Trash, and restoring do not change this date.

#### Scenario: Record completion and reopening

- **WHEN** an open task becomes done on `2026-11-08`
- **THEN** its `status_changed_on` is `2026-11-08`
- **AND** reopening the task sets that field to null

#### Scenario: Keep an earlier completion date

- **WHEN** a task became done on `2026-11-07` and receives another done request on `2026-11-08`
- **THEN** its `status_changed_on` remains `2026-11-07`

### Requirement: Lists are sorted by name

Husker SHALL return ordinary lists sorted by their Unicode lowercase names, then by list ID for ties.
Virtual Trash appears after ordinary lists where a caller includes it.
Default task creation uses the selected list, or the lowest-ID ordinary list when no list is selected.
Alphabetical display order does not change that default destination.

#### Scenario: Sort names without changing the default list

- **WHEN** list 1 is Inbox and list 2 is Admin
- **THEN** list reads return Admin before Inbox
- **AND** task creation without a selected list uses Inbox

### Requirement: External changes can be detected

Husker SHALL expose SQLite's data-version value to the terminal interface so the interface can detect commits made by another database connection.
Reading that value does not change task data.

#### Scenario: Observe a separate writer

- **WHEN** another connection commits a task change
- **THEN** the terminal's next data-version read differs from its earlier read

## REMOVED Requirements

### Requirement: Tags have a consistent stored form

**Reason:** Tasks use plain text for `#words` instead of a separate tag collection.
**Migration:** Schema version 1 tags become literal text in task descriptions during migration to version 2.
