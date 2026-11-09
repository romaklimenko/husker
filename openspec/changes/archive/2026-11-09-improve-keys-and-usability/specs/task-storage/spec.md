## ADDED Requirements

### Requirement: Change only the list with a guarded write

Husker SHALL provide a list-only mutation that validates writable access, fixed list scope, task existence, the expected task version, and the destination list before changing data or returning a no-op.
The operation reads and validates the task inside an immediate transaction.
A scoped store rejects a destination other than its selected list before checking whether the destination exists.
A changed destination updates only `list_id` and increments `version` exactly once.
The task's ID, exact title and description, due date, status, status-change date and timestamp, Trash state, and saved position remain unchanged.
If the destination equals the stored list, the operation returns the task without writing data or incrementing its version.
A stale expected version fails even if the destination equals the stored list. A nonexistent destination fails without changing data.
A failed write rolls back completely. This list-only operation adds no CLI command or MCP tool and does not change the database schema.

#### Scenario: Move a done task between lists

- **WHEN** a list-only mutation moves a done task from Work to Home using its current version
- **THEN** only its `list_id` and version change
- **AND** its exact text, due date, status, completion date and timestamp, Trash state, and position remain unchanged

#### Scenario: Keep the current list as a true no-op

- **WHEN** a task belongs to Work and a current-version mutation requests Work
- **THEN** the operation returns the task with every field and version unchanged and writes no data

#### Scenario: Reject a stale or unknown destination

- **WHEN** another connection increments a task's version before an old-version mutation, or a current-version mutation names list 999
- **THEN** the mutation fails and changes no data

#### Scenario: Retain access restrictions

- **WHEN** a read-only store requests a list mutation, or a store restricted to Work requests Home as the destination
- **THEN** the operation fails without exposing or changing inaccessible data

### Requirement: Restore a previous status with its recorded time

Husker SHALL provide a status-restore mutation that sets `status`, `status_changed_on`, and `status_changed_at` to supplied values after validating writable access, fixed list scope, task existence, and the expected task version inside an immediate transaction.
Restoring `open` stores null in both status-change fields regardless of the supplied values.
Restoring `done` or `cancelled` stores the supplied date and timestamp, including null values for a task whose closure time was unknown.
The task's ID, list membership, exact title and description, due date, Trash state, and saved position remain unchanged. The version increments exactly once.
A stale expected version fails without changing data. A failed write rolls back completely.
The terminal undo action uses this mutation. It adds no CLI command or MCP tool and does not change the database schema.

#### Scenario: Restore a completion with its original time

- **WHEN** a task completed on `2026-11-07` at `2026-11-07T10:00:00Z` was reopened and the restore mutation supplies `done`, `2026-11-07`, and that timestamp
- **THEN** the task is done with `status_changed_on` `2026-11-07` and the original timestamp
- **AND** its version increments once and no other field changes

#### Scenario: Restore an unknown closure time

- **WHEN** the restore mutation supplies `done` with null date and timestamp for a reopened legacy task
- **THEN** the task is done with both status-change fields null

#### Scenario: Reject a stale restore

- **WHEN** another connection changes the task after the caller read it
- **THEN** the restore fails with a version conflict and changes no data

## MODIFIED Requirements

### Requirement: Scoped task writes check current membership

A list-scoped store SHALL check the stored task's current list inside the write transaction before checking its version or changing its fields.
An outside-scope task produces the same not-found error as a nonexistent task.

The check applies to these operations:

| Operation | Membership checked |
| --- | --- |
| Update task fields | The existing task |
| Set task status | The existing task |
| Restore a previous status | The existing task |
| Change only the due date | The existing task |
| Change only the list | The existing task |
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
