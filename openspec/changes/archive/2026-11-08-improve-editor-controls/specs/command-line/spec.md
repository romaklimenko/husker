## MODIFIED Requirements

### Requirement: Validate dates before writing

Husker SHALL accept due dates only in `YYYY-MM-DD` format when the date exists.
`edit ID --clear-due` SHALL remove the date and place the task in the planning group for tasks without due dates.
The terminal names that group Sometime in English and Engang in Danish. Plain CLI output retains the stable `undated` date token.
Supplying both `--due` and `--clear-due` is an argument error.

#### Scenario: Invalid calendar date

- **WHEN** `husker add "Buy milk" --due 2026-02-30` runs
- **THEN** Husker reports an invalid date and creates no task

#### Scenario: Remove a due date

- **WHEN** `husker edit ID --clear-due` runs for a dated task
- **THEN** the task has no due date and appears under Sometime in the English terminal interface
- **AND** plain command output uses `undated` and JSON output contains a null `due` value
