## MODIFIED Requirements

### Requirement: Partial task updates

Husker SHALL retain fields omitted from `update_task` and validate all supplied changes before saving them together.
Task creation and editing accept title, description, list ID, and an optional due date.
`clear_due = true` removes the date without changing the task's status, status-change date, or status-change timestamp.
Open tasks then use the no-date display group. Closed tasks retain their status sections, as defined by planning-search.
Supplying a date with `clear_due = true` is an error.
The server does not accept `tags` or `readiness` arguments.
Task results contain nullable `status_changed_on` and `status_changed_at` fields and do not contain structured tags or readiness.
Known timestamps use RFC 3339 UTC strings. Unknown timestamps are null.
Tool request arguments and existing task fields retain their meanings.
The task-storage spec owns status-date and timestamp changes.

#### Scenario: Remove a date

- **WHEN** `update_task` supplies `clear_due = true` for an open task
- **THEN** the task has no date and belongs to the group displayed as Sometime in the English terminal interface
- **AND** omitted title, description, and list fields retain their values

#### Scenario: Reject a removed field

- **WHEN** `add_task` supplies `tags` or `readiness`
- **THEN** the call returns an argument error and creates no task

#### Scenario: Remove a completed task's due date

- **WHEN** `update_task` supplies `clear_due = true` for a task completed today
- **THEN** the task's due date becomes null and its status, status-change date, and status-change timestamp remain unchanged
- **AND** the terminal shows it in Done today and omitted task fields retain their values

#### Scenario: Expose a recorded completion time

- **WHEN** `set_status` changes an open task to done
- **THEN** its result contains the local status date and UTC completion timestamp
- **AND** subsequent reads retain both values until another status transition changes them
