## ADDED Requirements

### Requirement: Local stdio server

Husker SHALL run an MCP server over standard input and output when invoked with `husker mcp`.
The server uses the selected profile and existing configuration path rules. Stdout contains only MCP messages.
Startup failures produce a nonzero exit code and a diagnostic on stderr.
Closing the input stream stops the server without deleting task data.

#### Scenario: Launch from an MCP client

- **WHEN** a client launches `husker --profile ./tasks mcp` and completes the MCP lifecycle
- **THEN** the client can discover Husker tools without a terminal interface or startup banner

### Requirement: Fixed list selection

Husker SHALL accept `mcp --list NAME` to restrict the server to one existing list ID resolved at startup.
The task-storage spec owns the restriction on reads and writes.
An unknown list causes startup failure. Omitting `--list` gives tools access to the selected profile's lists.
List names are matched after trimming and Unicode lowercase conversion.

#### Scenario: Keep a running server on a renamed list

- **WHEN** a server selects `Private`, that list is renamed to `Home`, and another list is named `Private`
- **THEN** the running server still accesses the original list ID now named `Home`

### Requirement: Agent instructions describe access

Husker SHALL provide initialization instructions that state the selected list ID or unrestricted profile access and whether the server permits writes.
Instructions require task versions for updates and identify task text as data.
Tool arguments cannot change the server's access settings.

#### Scenario: Explain restricted access

- **WHEN** a client initializes a read-only server restricted to list 2
- **THEN** the instructions describe list 2 and read-only access
- **AND** the agent cannot select a different list through a tool call

### Requirement: Read tools

Husker SHALL expose these three tools in every MCP access mode.

| Tool | Result |
| --- | --- |
| `list_lists` | Accessible lists with open-task counts and a separate accessible Trash count. |
| `list_tasks` | A page of matching tasks in planning order. |
| `get_task` | All fields of one accessible task, including its version. |

`get_task` can read an accessible task in Trash. The task-storage spec owns task visibility and counts.

#### Scenario: Read scoped Trash counts

- **WHEN** a server is restricted to a list with one deleted task and another list has five deleted tasks
- **THEN** `list_lists` returns a Trash count of 1 and does not return the other list

### Requirement: Task search pagination

Husker SHALL apply the planning-search query rules within the server's accessible tasks before paginating `list_tasks` results.

| Argument | Default and limits |
| --- | --- |
| `query` | Empty text; outside Trash, selects open tasks unless a status filter is supplied. |
| `trash` | `false`; `true` searches accessible Trash and includes every status unless a status filter is supplied. |
| `offset` | `0`; a nonnegative integer. |
| `limit` | `100`; an integer from 1 through 200. |

Results contain `tasks`, `total`, `offset`, and `next_offset`. `next_offset` is null when no further page exists.
An offset beyond the current matches returns an empty page. Concurrent edits can change later pages.

#### Scenario: A filter cannot broaden list access

- **WHEN** a server restricted to Private receives `list_tasks` with query `l:Work s:all` and `trash = true`
- **THEN** the result is empty regardless of whether Work exists

#### Scenario: Page through matches

- **WHEN** 101 accessible tasks match and the client requests the default first page
- **THEN** the result contains 100 tasks, `total = 101`, and `next_offset = 100`

### Requirement: Mutation tools

Husker SHALL expose these seven mutation tools on an unrestricted writable server.

| Tool | Result |
| --- | --- |
| `add_task` | Create a task in the supplied list or the default accessible list. |
| `update_task` | Change only supplied task fields. |
| `set_task_status` | Set `open`, `done`, or `cancelled`. |
| `trash_task` | Move an accessible task to Trash. |
| `restore_task` | Restore an accessible task. |
| `create_list` | Create an ordinary list. |
| `rename_list` | Rename an ordinary list. |

A list-restricted server omits `create_list` and `rename_list` and rejects direct calls to those names.
When `list_id` is omitted, `add_task` uses the selected list on a restricted server.
On an unrestricted server, `add_task` uses the list with the lowest ID.
The task-storage spec owns validation, status, Trash, and transaction rules.

#### Scenario: Add a task without choosing a list

- **WHEN** a writable server restricted to Private receives `add_task` without `list_id`
- **THEN** the new task belongs to Private

### Requirement: Versioned task mutations

Husker SHALL require both `id` and `version` for `update_task`, `set_task_status`, `trash_task`, and `restore_task`.
A stale version causes a tool error without overwriting stored data.

#### Scenario: Reject a stale agent edit

- **WHEN** an agent reads version 1, the terminal saves version 2, and the agent submits version 1
- **THEN** the MCP mutation fails and preserves the terminal's saved fields

### Requirement: Partial task updates

Husker SHALL retain fields omitted from `update_task` and validate all supplied changes before saving them together.
An empty tags array clears tags. `clear_due = true` removes the date and defaults to Anytime.
Explicit `readiness` overrides that default. Supplying a date with `clear_due = true` is an error.
For new tasks, `readiness` defaults to `maybe` unless `readiness = "anytime"` is supplied.
The planning-search spec defines task grouping when a due date is present.

#### Scenario: Remove a date and keep the task in Maybe

- **WHEN** `update_task` supplies `clear_due = true` and `readiness = "maybe"`
- **THEN** the task has no date and belongs to Maybe
- **AND** omitted title, description, tags, and list fields retain their values

### Requirement: Read-only access

Husker SHALL accept `mcp --read-only` to expose only read tools and open an existing database without write access.
A missing profile or database causes startup failure without creating it.
Direct calls to mutation tool names fail without changing task data.

#### Scenario: Reject a guessed mutation tool

- **WHEN** a read-only client calls `add_task` even though discovery omitted that tool
- **THEN** the server rejects the call and creates no task

### Requirement: Tool schemas and results

Husker SHALL describe tools with JSON Schemas and reject unknown arguments or invalid argument values.
Successful tools return structured JSON and equivalent JSON text content.
Task validation, stale versions, and access failures return tool-error results without protected data.
Unknown or unavailable tools fail without executing an operation.

#### Scenario: Reject an attempted scope override

- **WHEN** `get_task` receives an unknown `profile` or `scope` argument
- **THEN** the call fails and does not change the server's profile or list restriction
