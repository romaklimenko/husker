## MODIFIED Requirements

### Requirement: Task search pagination

Husker SHALL apply the planning-search query rules within the server's accessible tasks before paginating `list_tasks` results.

| Argument | Default and limits |
| --- | --- |
| `query` | Empty text; uses the default status rules from planning-search unless a status filter is supplied. |
| `trash` | `false`; `true` searches accessible Trash and includes every status unless a status filter is supplied. |
| `offset` | `0`; a nonnegative integer. |
| `limit` | `100`; an integer from 1 through 200. |

Default results outside Trash include open tasks and tasks completed or cancelled today.
`s:open` requests only open tasks.
`#words` are literal text terms in titles and descriptions.
Results contain `tasks`, `total`, `offset`, and `next_offset`. `next_offset` is null when no further page exists.
An offset beyond the current matches returns an empty page. Concurrent edits can change later pages.

#### Scenario: A filter cannot broaden list access

- **WHEN** a server restricted to Private receives `list_tasks` with query `l:Work s:all` and `trash = true`
- **THEN** the result is empty regardless of whether Work exists

#### Scenario: Page through matches

- **WHEN** 101 accessible tasks match and the client requests the default first page
- **THEN** the result contains 100 tasks, `total = 101`, and `next_offset = 100`

### Requirement: Partial task updates

Husker SHALL retain fields omitted from `update_task` and validate all supplied changes before saving them together.
Task creation and editing accept title, description, list ID, and an optional due date.
`clear_due = true` removes the date. Supplying a date with `clear_due = true` is an error.
The server does not accept `tags` or `readiness` arguments.
Task results contain `status_changed_on` and do not contain structured tags or readiness.
The task-storage spec owns status-date changes.

#### Scenario: Remove a date

- **WHEN** `update_task` supplies `clear_due = true`
- **THEN** the task has no date and belongs to Undated
- **AND** omitted title, description, and list fields retain their values

#### Scenario: Reject a removed field

- **WHEN** `add_task` supplies `tags` or `readiness`
- **THEN** the call returns an argument error and creates no task
