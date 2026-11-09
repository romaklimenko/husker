## MODIFIED Requirements

### Requirement: Trash preserves task information

Husker SHALL move a task to Trash by setting `deleted` to true and restore a task by setting `deleted` to false.
Both operations preserve the task's list, status, status-change date and timestamp, due date, description, and title.
Moving a task to Trash keeps its row. Only two operations remove a task row: deleting one Trash task permanently and emptying Trash. Their requirements in this spec define both operations.
Deleting an ordinary list transfers its tasks according to the list-deletion requirement below.

#### Scenario: Restore a completed task

- **WHEN** task 7 in list 2 has status `done`, moves to Trash, and is restored
- **THEN** task 7 belongs to list 2, has status `done`, and is outside Trash
- **AND** its original `status_changed_on` date and `status_changed_at` timestamp are unchanged

## ADDED Requirements

### Requirement: Delete one Trash task permanently

Husker SHALL provide a permanent-deletion operation that removes one task row only when the task is in Trash and its stored version equals the version the caller read.
The operation reads and checks the task inside an immediate write transaction.

| Stored task | Result |
| --- | --- |
| In Trash with the expected version | `Deleted`. The row is removed. |
| In Trash with another version | `Changed` with the current task. No data changes. |
| Outside Trash or nonexistent | `NotInTrash`. No data changes. |

Deleting a task changes no other task, list, list position, or list count.
Deleted task IDs are not reused. A task created afterward receives a higher ID.
A failed write rolls back completely. The operation does not change the database schema or task JSON fields.
Access restrictions follow the requirement Permanent deletion requires unrestricted write access.

#### Scenario: Delete a trashed task

- **WHEN** task 7 is in Trash at version 3 and the caller deletes task 7 with version 3
- **THEN** the result is `Deleted` and reading task 7 fails with `Task 7 does not exist.`
- **AND** every other task keeps its fields and version

#### Scenario: Refuse a changed task

- **WHEN** the caller read task 7 in Trash at version 3, another connection then edited task 7 in Trash to version 4, and the caller deletes task 7 with version 3
- **THEN** the result is `Changed` with task 7 at version 4 and task 7 still exists

#### Scenario: Refuse a restored task

- **WHEN** the caller read task 7 in Trash, another connection then restored task 7, and the caller deletes task 7 with the version it read
- **THEN** the result is `NotInTrash` and task 7 keeps every field

#### Scenario: Keep IDs unique

- **WHEN** tasks 1, 2, and 3 exist, task 3 is in Trash and is deleted permanently, and a new task is created
- **THEN** the new task has ID 4

### Requirement: Empty Trash from a confirmed preview

Husker SHALL empty Trash only by revalidating a Trash preview inside an immediate write transaction and then deleting every task in Trash within the same transaction.
A Trash preview holds the number of tasks in Trash and a private snapshot of each task's ID and version, ordered by task ID.
The Trash preview includes tasks of every status and every list.

| Current Trash during the write | Result |
| --- | --- |
| Same task IDs and versions as the preview snapshot | `Emptied` with the number of deleted tasks. Every task in Trash is removed. |
| Any task ID or version differs from the preview snapshot, because a task was added to Trash, restored, removed, or changed | `Changed` with a fresh preview. No data changes. |

The caller must obtain confirmation of a fresh preview before emptying Trash again.
Tasks outside Trash, lists, list positions, and list counts remain unchanged.
A failed write rolls back completely. The operation does not change the database schema or task JSON fields.
Access restrictions follow the requirement Permanent deletion requires unrestricted write access.

#### Scenario: Empty Trash

- **WHEN** Trash holds open task 7 in Work and done task 9 in Inbox, and the caller empties Trash with the current preview
- **THEN** the result is `Emptied` with count 2 and the Trash count becomes 0
- **AND** every task outside Trash keeps its fields and version

#### Scenario: Reconfirm after another connection changes Trash

- **WHEN** the caller previews Trash with tasks 7 and 9, and another connection moves task 4 to Trash before the caller empties Trash with that preview
- **THEN** the result is `Changed` with a preview of tasks 4, 7, and 9 and no task is deleted

#### Scenario: Detect a change without a count difference

- **WHEN** the caller previews Trash with tasks 7 and 9, and another connection then restores task 7 and moves task 4 to Trash
- **THEN** emptying Trash with that preview returns `Changed` even though the count is unchanged

### Requirement: Permanent deletion requires unrestricted write access

Husker SHALL reject deleting one Trash task permanently, requesting a Trash preview, and emptying Trash through a read-only store or a list-scoped store before reading Trash or changing data.
A list-scoped store reports `A list-scoped store cannot delete tasks permanently.`
A read-only store reports the read-only error `This store is read-only.`
A store that is both read-only and list-scoped reports the read-only error.
These operations are available only through unrestricted writable stores. The terminal interface uses an unrestricted writable store.

#### Scenario: Reject a scoped store

- **WHEN** a store restricted to list 2 requests a Trash preview or deletes a Trash task in list 2 permanently
- **THEN** the request fails with `A list-scoped store cannot delete tasks permanently.` and every task remains unchanged

#### Scenario: Reject a read-only store

- **WHEN** a read-only store requests to empty Trash
- **THEN** the request fails and every task remains unchanged
