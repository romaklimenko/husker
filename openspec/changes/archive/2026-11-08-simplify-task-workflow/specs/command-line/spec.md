## MODIFIED Requirements

### Requirement: Scriptable task operations

Husker SHALL provide noninteractive commands for these task operations.

| Command | Result |
| --- | --- |
| `add TITLE` | Create an open task with optional list, description, and due date. |
| `tasks [QUERY]` | Print matching tasks in planning order. |
| `show ID` | Print one task, including its description, status-change date, and current version. |
| `edit ID` | Update supplied task fields and retain fields not supplied. |
| `done ID` | Set status to done. |
| `cancel ID` | Set status to cancelled. |
| `reopen ID` | Set status to open. |
| `trash ID` | Move the task to Trash. |
| `restore ID` | Restore the task to its original list and status. |

The task-storage spec owns validation and persistence.
The planning-search spec owns query behavior. `tasks --trash` selects Trash.
Structured tags and readiness are not command options.
The removed options `--tag`, `--clear-tags`, `--anytime`, and `--readiness` produce argument errors without changing data.
Default creation without `--list` uses the lowest-ID ordinary list.

#### Scenario: Create and complete a task

- **WHEN** a script runs `husker add "Buy milk #shopping" --list Inbox`
- **THEN** Husker prints the created task's stable ID
- **AND** `husker done ID` persists the done status and current local date for that task

#### Scenario: Reject a removed option

- **WHEN** a script runs `husker add "Buy milk" --tag shopping`
- **THEN** Husker reports an unrecognized argument and creates no task

### Requirement: List operations

Husker SHALL provide `lists`, `list-add NAME`, and `list-rename ID NAME` commands.
`lists` SHALL use the alphabetical order defined by task storage, include open task counts, and place virtual Trash last.

#### Scenario: Rename a list

- **WHEN** a script creates list `Private` and renames its ID to `Home`
- **THEN** `husker lists` shows `Home` with the same ID in alphabetical order
- **AND** tasks keep their list membership

### Requirement: Validate dates before writing

Husker SHALL accept due dates only in `YYYY-MM-DD` format when the date exists.
`edit ID --clear-due` SHALL remove the date and place the task in Undated.
Supplying both `--due` and `--clear-due` is an argument error.

#### Scenario: Invalid calendar date

- **WHEN** `husker add "Buy milk" --due 2026-02-30` runs
- **THEN** Husker reports an invalid date and creates no task

#### Scenario: Remove a due date

- **WHEN** `husker edit ID --clear-due` runs for a dated task
- **THEN** the task has no due date and belongs to Undated
