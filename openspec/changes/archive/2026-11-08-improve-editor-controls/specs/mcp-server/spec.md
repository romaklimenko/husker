## MODIFIED Requirements

### Requirement: Partial task updates

Husker SHALL retain fields omitted from `update_task` and validate all supplied changes before saving them together.
Task creation and editing accept title, description, list ID, and an optional due date.
`clear_due = true` removes the date. Supplying a date with `clear_due = true` is an error.
The server does not accept `tags` or `readiness` arguments.
Task results contain `status_changed_on` and do not contain structured tags or readiness.
The task-storage spec owns status-date changes.

#### Scenario: Remove a date

- **WHEN** `update_task` supplies `clear_due = true`
- **THEN** the task has no date and belongs to the group displayed as Sometime in the English terminal interface
- **AND** omitted title, description, and list fields retain their values

#### Scenario: Reject a removed field

- **WHEN** `add_task` supplies `tags` or `readiness`
- **THEN** the call returns an argument error and creates no task
