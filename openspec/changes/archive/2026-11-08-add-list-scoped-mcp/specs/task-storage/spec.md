## ADDED Requirements

### Requirement: Store access settings remain fixed

An MCP store SHALL retain its selected list ID and read-only setting for the lifetime of the connection.
An ordinary CLI or terminal store remains unrestricted.

A selected list name is trimmed and compared after Unicode lowercase conversion.
The store resolves that name to an existing list ID before serving requests.
An unknown list causes startup to fail without falling back to unrestricted access.

#### Scenario: Keep access after a list rename

- **WHEN** an MCP store selects list 2 named `Private`, another connection renames list 2 to `Home`, and a new list is named `Private`
- **THEN** the MCP store still selects list 2
- **AND** the new list is outside its scope

#### Scenario: Reject an unknown list

- **WHEN** an MCP store selects `Missing` and no list has that name
- **THEN** opening the MCP store fails

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

A list-scoped store SHALL reject list creation and list renaming.
These operations remain available through ordinary CLI and terminal stores.

#### Scenario: Reject renaming the allowed list

- **WHEN** a store restricted to list 2 attempts to rename list 2
- **THEN** the operation fails and the stored name remains unchanged

### Requirement: Read-only stores reject writes

A read-only MCP store SHALL reject every operation that changes lists or tasks.
The rejection applies even when the requested change would leave the same field values.
Combining read-only access with a list restriction preserves both restrictions.

#### Scenario: Reject a guessed mutation

- **WHEN** a read-only store receives a status change for an accessible task
- **THEN** the operation fails and the task's status and version remain unchanged

### Requirement: Read-only stores open existing databases

A read-only MCP store SHALL open an existing SQLite database with read-only access and verify schema version 1 without initializing tables or changing journal mode.
A missing database, an unsupported schema, or an unreadable database causes startup to fail.

#### Scenario: Preserve a missing profile

- **WHEN** a read-only MCP store opens a path with no database
- **THEN** opening fails and no database is created

#### Scenario: Preserve an unsupported schema

- **WHEN** a read-only MCP store opens a database with schema version 2
- **THEN** opening fails without changing its schema or task data
