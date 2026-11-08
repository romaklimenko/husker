## MODIFIED Requirements

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
