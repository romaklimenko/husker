# Command line

## Purpose

Define how terminal users and scripts create, find, and change tasks with Husker commands.
Task storage owns persistence and validation. Planning and search owns filter behavior.
## Requirements
### Requirement: Default interactive launch

Husker SHALL open the terminal interface when invoked without a subcommand in an interactive terminal.
Without an interactive terminal, Husker SHALL fail with instructions to use a subcommand.

#### Scenario: Redirected default launch

- **WHEN** `husker` runs with redirected input
- **THEN** Husker exits unsuccessfully without changing terminal settings
- **AND** stderr directs the user to a noninteractive command

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
`lists` SHALL use the persistent display order defined by task storage, include open task counts, and place virtual Trash last.
Creating a list appends it before Trash. Renaming a list preserves its position.

#### Scenario: Rename a list

- **WHEN** a script creates list `Private` and renames its ID to `Home`
- **THEN** `husker lists` shows `Home` with the same ID and display position
- **AND** tasks keep their list membership

### Requirement: Structured output and failures

Husker SHALL accept the global `--json` option for data commands and return JSON with stable English field names and status values.
Task JSON includes nullable `status_changed_at` alongside `status_changed_on`.
A known timestamp is an RFC 3339 UTC string. A new open task or migrated task without a recorded time has a null timestamp.
Existing command names, arguments, and English status values remain unchanged.
Failures SHALL return a nonzero exit code and write a message to stderr without printing success data to stdout.

#### Scenario: Invalid task ID

- **WHEN** `husker --json show 999` requests a missing task
- **THEN** the command exits unsuccessfully
- **AND** stdout contains no task JSON

#### Scenario: Return a completion timestamp

- **WHEN** `husker --json done ID` changes an open task to done
- **THEN** the task JSON includes its local `status_changed_on` date and UTC `status_changed_at` timestamp
- **AND** `husker --json reopen ID` returns null for both fields

### Requirement: Validate dates before writing

Husker SHALL accept due dates only in `YYYY-MM-DD` format when the date exists.
`edit ID --clear-due` SHALL remove the date and assign the undated due bucket.
Open tasks appear in the no-date display group. Done and cancelled tasks remain in their status sections, as defined by planning-search.
The terminal names the no-date group Sometime in English and Engang in Danish. Plain CLI output retains the stable `undated` date token.
Supplying both `--due` and `--clear-due` is an argument error.

#### Scenario: Invalid calendar date

- **WHEN** `husker add "Buy milk" --due 2026-02-30` runs
- **THEN** Husker reports an invalid date and creates no task

#### Scenario: Remove a due date

- **WHEN** `husker edit ID --clear-due` runs for a dated open task
- **THEN** the task has no due date and appears under Sometime in the English terminal interface
- **AND** plain command output uses `undated` and JSON output contains a null `due` value

#### Scenario: Remove a completed task's due date

- **WHEN** `husker edit ID --clear-due` runs for a task completed today
- **THEN** its due date becomes null and its status and status-change date remain unchanged
- **AND** the terminal shows it in Done today while plain CLI output uses the `undated` date token

### Requirement: Explicit configuration inspection

Husker SHALL provide `config --example` to print a complete default TOML configuration without creating a profile or opening its database.

#### Scenario: Inspect shortcuts

- **WHEN** a user runs `husker config --example`
- **THEN** stdout includes the language setting and the default action bindings
