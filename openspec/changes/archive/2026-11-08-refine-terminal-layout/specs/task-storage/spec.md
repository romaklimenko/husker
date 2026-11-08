## MODIFIED Requirements

### Requirement: Schema versions protect stored data

Husker SHALL initialize new databases at schema version 3 and migrate schema versions 1 and 2 atomically when opening a writable database.
Husker rejects an existing database with a newer schema version or unversioned user tables.

The version 1 migration removes readiness and structured tags before applying the version 2 migration.
Each existing tag becomes literal `#tag` text appended to the task description.
Existing description text remains intact. Each task whose description changes receives one version increment.
Tasks without appended tags keep their versions. Existing list and task IDs, list membership, titles, due dates, statuses, Trash state, and task positions are preserved.
Version 1 tasks receive a null `status_changed_on` because their completion date is unknown.

The version 2 migration adds a persistent position for each list.
The lowest-ID list becomes the pinned default list. Remaining lists receive initial positions in Unicode lowercase name order, with list ID breaking ties.
The version 2 migration does not change task fields or task versions.
Both migration stages run in one transaction when starting from version 1.
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
- **AND** every task field and task version is unchanged

#### Scenario: Refuse a database from a newer version

- **WHEN** `tasks.db` reports schema version 4
- **THEN** opening the profile fails without modifying its schema or task data

### Requirement: Scoped stores do not change lists

A list-scoped store SHALL reject list creation, list renaming, and list reordering.
These operations remain available through unrestricted writable stores.
Rejecting a reorder does not change list positions or task data.

#### Scenario: Reject renaming the allowed list

- **WHEN** a store restricted to list 2 attempts to rename list 2
- **THEN** the operation fails and the stored name remains unchanged

#### Scenario: Reject reordering the allowed list

- **WHEN** a store restricted to list 2 attempts to move list 2
- **THEN** the operation fails and every list retains its position

### Requirement: Read-only stores open existing databases

A read-only MCP store SHALL open an existing SQLite database with read-only access and verify schema version 3 without initializing tables, migrating data, or changing journal mode.
A missing database, an unsupported schema, or an unreadable database causes startup to fail.
Version 1 and version 2 databases require a writable Husker launch before a read-only MCP connection can open them.

#### Scenario: Preserve a missing profile

- **WHEN** a read-only MCP store opens a path with no database
- **THEN** opening fails and no database is created

#### Scenario: Preserve an unsupported schema

- **WHEN** a read-only MCP store opens a database with schema version 1, 2, or 4
- **THEN** opening fails without changing its schema or task data

## ADDED Requirements

### Requirement: Lists have a persistent display order

Husker SHALL return the lowest-ID ordinary list first, followed by other lists in saved position order with list ID breaking ties.
The lowest-ID list is the default list and stays first even after a rename.
Virtual Trash appears after ordinary lists where a caller includes it.
New lists append after existing ordinary lists. Renaming a list preserves its position.
Default task creation uses the selected list, or the default list when no list is selected.
Saved positions are internal storage fields and do not change the `TaskList` JSON fields.

#### Scenario: Keep a renamed default list pinned

- **WHEN** list 1 Inbox is renamed to Personal
- **THEN** Personal remains first and remains the default destination for task creation without a selected list

#### Scenario: Append a new list

- **WHEN** the displayed ordinary lists are Inbox, Work, and Home and a user creates Admin
- **THEN** the order becomes Inbox, Work, Home, Admin, followed by virtual Trash where present

#### Scenario: Preserve a renamed list's position

- **WHEN** the displayed ordinary lists are Inbox, Work, and Home and Work is renamed to Zebra
- **THEN** the order becomes Inbox, Zebra, Home

### Requirement: List reordering uses current stored neighbors

Husker SHALL move a nondefault list up or down by swapping its saved position with the adjacent nondefault list inside one immediate write transaction.
The operation reads the current neighbor inside that transaction.
Moving the default list, moving the first movable list upward, or moving the last movable list downward is a no-op.
The operation rejects a missing list. It never changes task fields, task versions, list IDs, or list names.
Read-only and list-scoped stores reject the operation before changing data.

#### Scenario: Move a list and preserve tasks

- **WHEN** the stored order is Inbox, Work, Home and the user moves Home up
- **THEN** the stored order becomes Inbox, Home, Work
- **AND** reopening the database returns that order with every task unchanged

#### Scenario: Preserve pinned boundaries

- **WHEN** the stored order is Inbox, Work, Home and the user moves Work up or Home down
- **THEN** the order remains Inbox, Work, Home

#### Scenario: Use an externally changed order

- **WHEN** one connection changes the order before a second connection requests another move
- **THEN** the second connection moves relative to the current stored neighbor, not an earlier displayed neighbor

## REMOVED Requirements

### Requirement: Lists are sorted by name

**Reason:** Users control list order while the default list stays first and Trash stays last.
**Migration:** Schema version 3 initializes the other lists in their previous case-insensitive alphabetical order.
