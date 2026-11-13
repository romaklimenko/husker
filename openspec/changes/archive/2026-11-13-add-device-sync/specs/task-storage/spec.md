## MODIFIED Requirements

### Requirement: Schema versions protect stored data

Husker SHALL initialize new databases at schema version 5 and migrate schema versions 1, 2, 3, and 4 atomically when opening a writable database.
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
The version 4 migration adds a `uid` column to lists and tasks, gives every existing list and task a random `uid`, and creates the empty `sync_state` table.
It preserves every existing list field, task field, and task version.
All required migration stages run in one transaction when starting from version 1, 2, 3, or 4.
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

- **WHEN** `tasks.db` reports schema version 6
- **THEN** opening the profile fails without modifying its schema or task data

#### Scenario: Upgrade a completion without inventing its time

- **WHEN** a schema 3 task has status done and `status_changed_on = "2026-11-08"`
- **THEN** migration preserves that date and every existing field and version while setting `status_changed_at` to null
- **AND** the database becomes schema version 5

#### Scenario: Roll back a failed timestamp migration

- **WHEN** any migration stage fails while opening a schema 1, 2, 3, or 4 database
- **THEN** the database keeps its original schema, fields, and versions without a partial upgrade

#### Scenario: Add sync identifiers without changing tasks

- **WHEN** a schema 4 database holds lists 1 and 2 and task 7 at version 3
- **THEN** migration gives each list and the task a different `uid`
- **AND** task 7 keeps its ID, fields, and version 3, and the database becomes schema version 5

### Requirement: Read-only stores open existing databases

A read-only MCP store SHALL open an existing SQLite database with read-only access and verify schema version 5 without initializing tables, migrating data, or changing journal mode.
A missing database, an unsupported schema, or an unreadable database causes startup to fail.
Version 1, 2, 3, and 4 databases require a writable Husker launch before a read-only MCP connection can open them.

#### Scenario: Preserve a missing profile

- **WHEN** a read-only MCP store opens a path with no database
- **THEN** opening fails and no database is created

#### Scenario: Preserve an unsupported schema

- **WHEN** a read-only MCP store opens a database with schema version 1, 2, 3, 4, or 6
- **THEN** opening fails without changing its schema or task data

## ADDED Requirements

### Requirement: Lists and tasks have sync identifiers

Husker SHALL give every list and task a `uid` of 32 lowercase hexadecimal characters taken from 16 random bytes when it creates the row.
A `uid` is unique within its table. A task's `uid` never changes.
A list's `uid` changes only when a sync merges the local list with a list from the sync file.
The device-sync requirements `A first sync joins two collections without deleting` and `List names stay unique after a merge` define those two merges.
A row that an older running Husker inserted without a `uid` receives one at the start of the next sync.
The `uid` does not appear in command output, JSON, or MCP results.

#### Scenario: Create a task with an identifier

- **WHEN** the user creates two tasks
- **THEN** each stored task has a different 32-character `uid`
- **AND** `husker --json show ID` prints no `uid` member

### Requirement: Sync state is stored with the tasks

Husker SHALL keep sync state in the `sync_state` table of the profile's database and write it in the same transaction as the lists and tasks a sync changes.

| Key | Value |
| --- | --- |
| `base` | The snapshot that the local database and the sync file agreed on at the last sync, as JSON. |
| `etag` | The ETag the sync file had at that sync. |
| `sent` | The snapshot of an upload whose answer has not arrived, as JSON. |
| `commits` | A counter that increases by one in each sync transaction that writes anything. |
| `applied` | A counter that increases by one in each sync transaction that changes a list or task. |
| `conflicts` | The number of conflict copies created so far. |

A sync that changes no list, no task, and no sync state writes nothing, so other connections see no new commit.
The sync operations that read the snapshot or change data require an unrestricted writable store. A list-scoped or read-only store rejects them without reading or changing data.
Any store can read the `commits`, `applied`, and `conflicts` counters.

#### Scenario: Keep the base consistent with a restored backup

- **WHEN** the user restores an older copy of `tasks.db` and syncs
- **THEN** the merge uses the base stored in that copy and brings the profile up to date without duplicating tasks

#### Scenario: Reject sync on a restricted store

- **WHEN** a store restricted to list 2 is asked to merge a snapshot
- **THEN** the operation fails and no list, task, or sync state changes
