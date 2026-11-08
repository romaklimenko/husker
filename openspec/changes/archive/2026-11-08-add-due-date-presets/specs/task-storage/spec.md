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
| `status_changed_at` | UTC timestamp of that most recent status change, or null when unknown |
| `deleted` | Whether the task belongs to Trash |
| `position` | Persistent manual sort position |
| `version` | Integer used to detect stale edits |

A new task has status `open`, `deleted` set to false, and both status-change fields set to null.
Tasks do not have structured tags or readiness fields.

#### Scenario: Save a task with a multiline description

- **WHEN** the user saves a task titled `Buy milk` with description `Two cartons` followed by a newline and `Oat milk #shopping`
- **THEN** reopening the database returns the same task ID, title, description, and list ID

### Requirement: Schema versions protect stored data

Husker SHALL initialize new databases at schema version 4 and migrate schema versions 1, 2, and 3 atomically when opening a writable database.
Husker rejects an existing database with a newer schema version or unversioned user tables.

The version 1 migration removes readiness and structured tags before applying the version 2 migration.
Each existing tag becomes literal `#tag` text appended to the task description.
Existing description text remains intact. Each task whose description changes receives one version increment.
Tasks without appended tags keep their versions. Existing list and task IDs, list membership, titles, due dates, statuses, Trash state, and task positions are preserved.
Version 1 tasks receive a null `status_changed_on` because their completion date is unknown.

The version 2 migration adds a persistent position for each list.
The lowest-ID list becomes the pinned default list. Remaining lists receive initial positions in Unicode lowercase name order, with list ID breaking ties.
The version 2 migration does not change task fields or task versions.
The version 3 migration adds nullable `status_changed_at` and sets it to null on every existing task.
It preserves every existing task field, task version, list field, and list order. Prior completion times cannot be reconstructed from task IDs or local dates.
All required migration stages run in one transaction when starting from version 1, 2, or 3.
A failed migration preserves the original schema and data. Invalid legacy tag data causes migration to fail without discarding that data.

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

#### Scenario: Initialize list order without changing tasks

- **WHEN** a version 2 database contains lists 1 Inbox, 2 Zebra, and 3 Alpha
- **THEN** migration initializes the list order as Inbox, Alpha, Zebra
- **AND** every existing task field and task version is unchanged

#### Scenario: Refuse a database from a newer version

- **WHEN** `tasks.db` reports schema version 5
- **THEN** opening the profile fails without modifying its schema or task data

#### Scenario: Upgrade a completion without inventing its time

- **WHEN** a schema 3 task has status done and `status_changed_on = "2026-11-08"`
- **THEN** migration preserves that date and every existing field and version while setting `status_changed_at` to null
- **AND** the database becomes schema version 4

#### Scenario: Roll back a failed timestamp migration

- **WHEN** any migration stage fails while opening a schema 1, 2, or 3 database
- **THEN** the database keeps its original schema, fields, and versions without a partial upgrade


### Requirement: Read-only stores open existing databases

A read-only MCP store SHALL open an existing SQLite database with read-only access and verify schema version 4 without initializing tables, migrating data, or changing journal mode.
A missing database, an unsupported schema, or an unreadable database causes startup to fail.
Version 1, 2, and 3 databases require a writable Husker launch before a read-only MCP connection can open them.

#### Scenario: Preserve a missing profile

- **WHEN** a read-only MCP store opens a path with no database
- **THEN** opening fails and no database is created

#### Scenario: Preserve an unsupported schema

- **WHEN** a read-only MCP store opens a database with schema version 1, 2, 3, or 5
- **THEN** opening fails without changing its schema or task data

### Requirement: Status changes record their local date

Husker SHALL set `status_changed_on` to the current local calendar date and `status_changed_at` to the current UTC timestamp when a task changes to done or cancelled.
This includes open-to-closed transitions and changes between done and cancelled.
Reopening a task clears both status-change fields.
Setting an already-done task to done, or an already-cancelled task to cancelled, preserves both existing values, including an unknown timestamp.
Editing other task fields, due-date presets, moving to Trash, restoring, and transferring tasks after list deletion do not change either value.
Tests can inject the current local date and UTC timestamp. Runtime records the current completion instant without adding a separate event log.

#### Scenario: Record completion and reopening

- **WHEN** an open task becomes done on `2026-11-08`
- **THEN** its `status_changed_on` is `2026-11-08`
- **AND** `status_changed_at` records the completion instant in UTC
- **AND** reopening the task sets both fields to null

#### Scenario: Keep an earlier completion date

- **WHEN** a task became done on `2026-11-07` and receives another done request on `2026-11-08`
- **THEN** its `status_changed_on` remains `2026-11-07` and its existing `status_changed_at` remains unchanged

#### Scenario: Record a changed closed status

- **WHEN** a done task becomes cancelled at `2026-11-08T15:30:00Z`
- **THEN** `status_changed_at` becomes that timestamp and `status_changed_on` records the current local date

#### Scenario: Preserve an unknown legacy time

- **WHEN** a migrated done task has a null timestamp and receives another done request
- **THEN** both existing status-change fields remain unchanged and no completion time is invented


### Requirement: Trash preserves task information

Husker SHALL move a task to Trash by setting `deleted` to true and restore a task by setting `deleted` to false.
Both operations preserve the task's list, status, status-change date and timestamp, due date, description, and title.
Husker does not permanently delete tasks.
Deleting an ordinary list transfers its tasks according to the list-deletion requirement below.

#### Scenario: Restore a completed task

- **WHEN** task 7 in list 2 has status `done`, moves to Trash, and is restored
- **THEN** task 7 belongs to list 2, has status `done`, and is outside Trash
- **AND** its original `status_changed_on` date and `status_changed_at` timestamp are unchanged

### Requirement: Delete a list without deleting its tasks

Husker SHALL delete a nondefault list only by transferring every source task to the default list and deleting the source list metadata within the same immediate write transaction.
The operation requires a successfully revalidated deletion preview.
Every transferred task keeps its ID, title, description, due date, status, status-change date and timestamp, Trash state, and saved position.
Every transferred task receives exactly one version increment because its list membership changes.
Tasks already in Trash remain in Trash and later restore to the default list.
Tasks outside the source list and all other list IDs, names, and positions remain unchanged.
Successful deletion returns `Deleted`. Any storage failure rolls back both the transfer and list deletion.
An empty nondefault list can be deleted through the same preview and confirmation process.
Deleted list IDs are not reused. Recreating its name creates a new list ID.
List deletion does not change the database schema or task JSON fields.

#### Scenario: Preserve completed and trashed tasks

- **WHEN** a confirmed Work deletion transfers done task 7 and trashed open task 8 to default list 1
- **THEN** Work no longer exists and both tasks belong to list 1 with their versions incremented by one
- **AND** task 7 retains its completion date and timestamp and task 8 remains in Trash
- **AND** restoring task 8 keeps it in list 1

#### Scenario: Reject an editor read before transfer

- **WHEN** an editor reads task 7 at version 3 before its list is deleted and later attempts to save version 3
- **THEN** the save fails because the transfer incremented task 7's version
- **AND** the transferred task remains unchanged by that save

#### Scenario: Roll back a failed list deletion

- **WHEN** a storage failure occurs after transferring tasks but before deleting the source list
- **THEN** the transaction restores the source list and all original task fields and versions

#### Scenario: Delete an empty list

- **WHEN** a nondefault list has no tasks and its current deletion preview is confirmed
- **THEN** the list is deleted and no task or other list changes

## ADDED Requirements

### Requirement: Change only the due date with a guarded write

Husker SHALL provide a due-only mutation that validates writable access, fixed list scope, task existence, and the expected task version before changing data or returning a no-op.
The operation reads and validates the task inside an immediate transaction.
A changed due value updates only `due` and increments `version` exactly once.
The task's ID, list membership, exact title and description, status, status-change date and timestamp, Trash state, and saved position remain unchanged.
If the requested date already equals the stored date, the operation returns the task without writing data or incrementing its version.
Clearing an already empty date follows the same no-op rule.
A stale expected version fails even if the requested due date equals the current stored date.
A failed write rolls back completely. Scoped and read-only stores retain their existing access restrictions.
This due-only operation adds no CLI command or MCP tool and does not change the database schema.

#### Scenario: Preserve a closed task's exact contents

- **WHEN** a due-only mutation changes a done task's date using its current version
- **THEN** only its due date and version change
- **AND** its exact text, list, completion date and timestamp, status, Trash state, and position remain unchanged

#### Scenario: Keep a repeated date as a true no-op

- **WHEN** a task is due `2026-11-08` and a current-version mutation requests `2026-11-08`
- **THEN** the operation returns the task with every field and version unchanged and writes no data

#### Scenario: Reject a stale request for the current date

- **WHEN** another connection increments a task's version and an old-version mutation requests its already stored due date
- **THEN** the mutation fails with a version conflict and changes no data

#### Scenario: Preserve data after a failed write

- **WHEN** storing a changed due date fails
- **THEN** the task retains every original field and version

#### Scenario: Retain access restrictions

- **WHEN** a read-only store requests a due mutation or a scoped store requests another list's task
- **THEN** the operation fails without exposing or changing inaccessible task data
