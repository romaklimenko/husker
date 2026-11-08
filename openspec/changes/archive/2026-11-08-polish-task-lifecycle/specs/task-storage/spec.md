## MODIFIED Requirements

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

### Requirement: Trash preserves task information

Husker SHALL move a task to Trash by setting `deleted` to true and restore a task by setting `deleted` to false.
Both operations preserve the task's list, status, status-change date, due date, description, and title.
Husker does not permanently delete tasks.
Deleting an ordinary list transfers its tasks according to the list-deletion requirement below.

#### Scenario: Restore a completed task

- **WHEN** task 7 in list 2 has status `done`, moves to Trash, and is restored
- **THEN** task 7 belongs to list 2, has status `done`, and is outside Trash
- **AND** its original `status_changed_on` date is unchanged

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

## ADDED Requirements

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
