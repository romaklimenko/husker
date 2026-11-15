## MODIFIED Requirements

### Requirement: Lists and tasks have sync identifiers

Husker SHALL give every list and task a `uid` of 32 lowercase hexadecimal characters taken from 16 random bytes when it creates the row.
A `uid` is unique within its table. A task's `uid` never changes.
A list's `uid` changes only when a sync merges the local list with a list from the sync file.
The device-sync requirements `A first sync joins two collections without deleting` and `List names stay unique after a merge` define those two merges.
A row that an older running Husker inserted without a `uid` receives one at the start of the next sync.
The `uid` does not appear in the output or JSON of the task commands, or in the results of the local MCP server.
The `uid` appears in two places. The online server returns the `uid` as the ID of a list or task, as the online-mcp requirement `Online IDs are sync identifiers` defines. `husker token --list` stores the `uid` of a list in the access token, as the command-line requirement `Create access tokens` defines.

#### Scenario: Create a task with an identifier

- **WHEN** the user creates two tasks
- **THEN** each stored task has a different 32-character `uid`
- **AND** `husker --json show ID` prints no `uid` member

#### Scenario: Read a task through the online server

- **WHEN** a task with `uid` `0123456789abcdef0123456789abcdef` is in the sync file and a client of the online server reads the task
- **THEN** the result's `id` is `0123456789abcdef0123456789abcdef`
