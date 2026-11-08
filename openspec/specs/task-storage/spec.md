# Task storage

## Purpose

Define how Husker stores lists and tasks, preserves deleted tasks, and handles conflicting writes.
These rules apply to terminal users, scripts, and MCP clients.
Storage enforces MCP access restrictions. Planning and search owns task ordering and search filters.
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
| `status_changed_on` | Local date of the most recent change to done or cancelled, or null |
| `deleted` | Whether the task belongs to Trash |
| `position` | Persistent manual sort position |
| `version` | Integer used to detect stale edits |

A new task has status `open`, `deleted` set to false, and `status_changed_on` set to null.
Tasks do not have structured tags or readiness fields.

#### Scenario: Save a task with a multiline description

- **WHEN** the user saves a task titled `Buy milk` with description `Two cartons` followed by a newline and `Oat milk #shopping`
- **THEN** reopening the database returns the same task ID, title, description, and list ID

### Requirement: Invalid task input does not change stored tasks

Husker SHALL reject an empty title or a task assignment to a nonexistent list before changing the stored task.
The storage operation reports an error to its caller.

#### Scenario: Reject an empty title

- **WHEN** the user changes task 7's title from `Buy milk` to whitespace
- **THEN** the save fails and task 7 remains titled `Buy milk`

### Requirement: Trash preserves task information

Husker SHALL move a task to Trash by setting `deleted` to true and restore a task by setting `deleted` to false.
Both operations preserve the task's list, status, status-change date, due date, description, and title.
Husker does not permanently delete tasks.
Deleting an ordinary list transfers its tasks according to the list-deletion requirement below.

#### Scenario: Restore a completed task

- **WHEN** task 7 in list 2 has status `done`, moves to Trash, and is restored
- **THEN** task 7 belongs to list 2, has status `done`, and is outside Trash
- **AND** its original `status_changed_on` date is unchanged

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

Husker SHALL swap two task positions atomically only when both tasks are open, outside Trash, and have the same due date.
Two undated tasks have the same due date for this rule.
Done and cancelled tasks cannot be reordered because their display order uses status dates and task IDs.
The store checks both tasks' current statuses and due dates inside the write transaction.
The swap preserves all task fields except `position` and `version`.

#### Scenario: Refuse a move across dates

- **WHEN** open task 7 is due `2026-11-08` and open task 8 is due `2026-11-09`
- **THEN** swapping their positions fails and both dates and positions remain unchanged

#### Scenario: Refuse a move involving a closed task

- **WHEN** task 7 is open and task 8 is done and both share a due date
- **THEN** swapping their positions fails without changing either task's position or version

#### Scenario: Refuse reordering two closed tasks

- **WHEN** task 7 is done and task 8 is cancelled and both share a due date outside Trash
- **THEN** swapping their positions fails even when both supplied versions are current
- **AND** both tasks retain every field and version

#### Scenario: Reorder two open tasks

- **WHEN** two open tasks share a due date outside Trash and both supplied versions are current
- **THEN** swapping their positions succeeds and increments both versions
- **AND** every field other than position and version remains unchanged

#### Scenario: Reject a stale pair atomically

- **WHEN** open task 7 and open task 8 share a due date but task 8's version changed after the caller read it
- **THEN** the swap fails without changing either task's position

### Requirement: Database writes are atomic

Husker SHALL apply each operation in a SQLite transaction and report storage failures without replacing or resetting an existing database.
A failed operation preserves the database state from before that operation.

#### Scenario: Preserve a database that cannot be opened

- **WHEN** an existing `tasks.db` cannot be read as a SQLite database
- **THEN** opening the profile fails and Husker does not replace `tasks.db`

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

### Requirement: Store access settings remain fixed

An MCP store SHALL retain its selected list ID and read-only setting for the lifetime of the connection.
An ordinary CLI or terminal store remains unrestricted.

A selected list name is trimmed and compared after Unicode lowercase conversion.
The store resolves that name to an existing list ID before serving requests.
An unknown list causes startup to fail without falling back to unrestricted access.
Deleting the selected list does not change the connection's selected ID.
The connection returns no accessible lists or tasks after that deletion and cannot create tasks in another list.
Recreating the same list name does not grant access to the new list ID.

#### Scenario: Keep access after a list rename

- **WHEN** an MCP store selects list 2 named `Private`, another connection renames list 2 to `Home`, and a new list is named `Private`
- **THEN** the MCP store still selects list 2
- **AND** the new list is outside its scope

#### Scenario: Reject an unknown list

- **WHEN** an MCP store selects `Missing` and no list has that name
- **THEN** opening the MCP store fails

#### Scenario: Keep a removed list inaccessible

- **WHEN** a connection selects list 2 named `Private`, another connection deletes list 2, and a new list 3 is named `Private`
- **THEN** the original connection remains restricted to list 2 and returns no accessible lists or tasks
- **AND** requests for transferred tasks and list 3's tasks return the same not-found error as nonexistent task IDs
- **AND** task creation fails without choosing another destination

### Requirement: Scoped reads exclude other lists

A list-scoped store SHALL return only the selected list and its tasks from every read operation.
The restriction applies to tasks of every status and to deleted tasks.
List counts include only the selected list's open tasks outside Trash.

An outside-scope task ID produces the same error as a nonexistent task ID.
Neither error identifies another list or returns task fields.

#### Scenario: Read one list and its Trash

- **WHEN** a store is restricted to list 2 and lists 2 and 3 each contain open, done, and deleted tasks
- **THEN** list reads return only list 2 and its open-task count
- **AND** task reads return only tasks whose `list_id` is 2

#### Scenario: Hide an outside-scope task

- **WHEN** a caller requests task 7 in another list and then requests nonexistent task 999
- **THEN** both requests return the same not-found error
- **AND** neither request returns task fields

### Requirement: Scoped task writes check current membership

A list-scoped store SHALL check the stored task's current list inside the write transaction before checking its version or changing its fields.
An outside-scope task produces the same not-found error as a nonexistent task.

The check applies to these operations:

| Operation | Membership checked |
| --- | --- |
| Update task fields | The existing task |
| Set task status | The existing task |
| Move a task to Trash | The existing task |
| Restore a task | The existing task |
| Reorder tasks | Both existing tasks |

#### Scenario: Refuse a task moved out of scope

- **WHEN** an MCP store reads task 7 in list 2 and another connection moves task 7 to list 3
- **THEN** a later scoped write to task 7 fails without changing the task
- **AND** the write fails even when the caller supplies task 7's current version

#### Scenario: Preserve both positions when one task is outside scope

- **WHEN** a caller reorders an allowed task with a task from another list
- **THEN** the operation fails and neither task's position or version changes

### Requirement: Scoped task destinations stay in the selected list

A list-scoped store SHALL reject a task creation or update whose destination `list_id` differs from the selected list ID.
The store checks this restriction before checking whether the destination list exists.
The failed operation does not create or change a task.

#### Scenario: Reject another destination without exposing it

- **WHEN** a store restricted to list 2 receives task creation requests for list 3 and nonexistent list 999
- **THEN** both requests fail with the same scope error
- **AND** no task is created

### Requirement: Scoped stores do not change lists

A list-scoped store SHALL reject list creation, list renaming, list reordering, and list deletion.
These operations remain available through unrestricted writable stores, subject to the default-list restriction on deletion.
Rejecting an operation does not change list positions, list metadata, or task data.
List-deletion previews also reject a list-scoped store before reading the source or destination list.

#### Scenario: Reject renaming the allowed list

- **WHEN** a store restricted to list 2 attempts to rename list 2
- **THEN** the operation fails and the stored name remains unchanged

#### Scenario: Reject reordering the allowed list

- **WHEN** a store restricted to list 2 attempts to move list 2
- **THEN** the operation fails and every list retains its position

#### Scenario: Reject deletion through a scoped store

- **WHEN** a store restricted to list 2 requests a deletion preview or submits a deletion preview obtained from an unrestricted store
- **THEN** the request fails before reading source or destination details
- **AND** every list and task remains unchanged

### Requirement: Read-only stores reject writes

A read-only MCP store SHALL reject every operation that changes lists or tasks.
The rejection applies even when the requested change would leave the same field values.
Combining read-only access with a list restriction preserves both restrictions.

#### Scenario: Reject a guessed mutation

- **WHEN** a read-only store receives a status change for an accessible task
- **THEN** the operation fails and the task's status and version remain unchanged

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

### Requirement: External changes can be detected

Husker SHALL expose SQLite's data-version value to the terminal interface so the interface can detect commits made by another database connection.
Reading that value does not change task data.

#### Scenario: Observe a separate writer

- **WHEN** another connection commits a task change
- **THEN** the terminal's next data-version read differs from its earlier read

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

### Requirement: Preview list deletion consistently

Husker SHALL provide a consistent deletion preview for an existing nondefault list through an unrestricted writable store.
The preview reads its source list, default destination list, and source tasks within one database transaction without changing data.
The preview identifies both lists by ID and current name.
The preview includes these separate task counts.

| Count | Included source tasks |
| --- | --- |
| `open_count` | Open tasks outside Trash. |
| `done_count` | Done tasks outside Trash. |
| `cancelled_count` | Cancelled tasks outside Trash. |
| `trashed_count` | Tasks in Trash, regardless of status. |

The preview retains a private snapshot of every source task's ID and version, ordered by task ID.
The snapshot includes tasks in Trash.
The lowest-ID list remains the default destination, including after a rename.
A preview request for the default list or a nonexistent list fails.
A read-only store rejects preview and deletion requests before reading the source or destination list.
The existing scoped-store restriction also applies.

#### Scenario: Count every retained source task once

- **WHEN** Work contains two open tasks, one done task, one cancelled task, and three tasks in Trash
- **THEN** its preview reports counts 2, 1, 1, and 3 respectively
- **AND** the preview identifies Work and the default list by ID and current name without changing any task

#### Scenario: Protect the renamed default list

- **WHEN** list 1 Inbox is renamed to Personal and a deletion preview is requested for list 1
- **THEN** the preview fails and Personal remains the default list

#### Scenario: Reject deletion through a read-only store

- **WHEN** a read-only store requests a deletion preview or submits a preview obtained from a writable store
- **THEN** the request fails and every list and task remains unchanged

### Requirement: Revalidate list deletion before changing data

Husker SHALL revalidate a supplied list-deletion preview inside an immediate write transaction before changing any task or list.
Revalidation compares the source ID and name, default destination ID and name, task counts, and complete source-task ID/version snapshot.
Revalidation does not depend on the ordinary lists' display positions.
The default list cannot be deleted, including when a submitted preview identifies it as the source.
Read-only and scoped-store restrictions are checked before the transaction reads list details.

If the source list no longer exists, the operation returns `Missing` without changing data.
If the current preview differs, the operation returns `Changed` with the fresh preview without changing data.
The caller must obtain confirmation of that fresh preview before retrying deletion.
An unchanged valid preview permits the atomic transfer defined below.

#### Scenario: Reconfirm an added task

- **WHEN** the user previews deletion of Work and another connection creates a task in Work before confirmation
- **THEN** deletion returns `Changed` with the new task counts and snapshot
- **AND** Work and all its tasks remain unchanged by the deletion attempt

#### Scenario: Detect changes without a count difference

- **WHEN** another connection edits a source task or replaces one source task with another after a deletion preview
- **THEN** deletion returns `Changed` even if every displayed task count is unchanged
- **AND** the deletion attempt changes no data

#### Scenario: Reconfirm a renamed source or destination

- **WHEN** another connection renames the source list or default destination after a deletion preview
- **THEN** deletion returns `Changed` with the current names and changes no data

#### Scenario: Handle a missing source

- **WHEN** another connection deletes the source list after a deletion preview
- **THEN** applying the old preview returns `Missing` and changes no data

### Requirement: Delete a list without deleting its tasks

Husker SHALL delete a nondefault list only by transferring every source task to the default list and deleting the source list metadata within the same immediate write transaction.
The operation requires a successfully revalidated deletion preview.
Every transferred task keeps its ID, title, description, due date, status, status-change date, Trash state, and saved position.
Every transferred task receives exactly one version increment because its list membership changes.
Tasks already in Trash remain in Trash and later restore to the default list.
Tasks outside the source list and all other list IDs, names, and positions remain unchanged.
Successful deletion returns `Deleted`. Any storage failure rolls back both the transfer and list deletion.
An empty nondefault list can be deleted through the same preview and confirmation process.
Deleted list IDs are not reused. Recreating its name creates a new list ID.
List deletion does not change database schema version 3 or task JSON fields.

#### Scenario: Preserve completed and trashed tasks

- **WHEN** a confirmed Work deletion transfers done task 7 and trashed open task 8 to default list 1
- **THEN** Work no longer exists and both tasks belong to list 1 with their versions incremented by one
- **AND** task 7 retains its completion date and task 8 remains in Trash
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
