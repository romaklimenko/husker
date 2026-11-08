## MODIFIED Requirements

### Requirement: List operations

Husker SHALL provide `lists`, `list-add NAME`, and `list-rename ID NAME` commands.
`lists` SHALL use the persistent display order defined by task storage, include open task counts, and place virtual Trash last.
Creating a list appends it before Trash. Renaming a list preserves its position.

#### Scenario: Rename a list

- **WHEN** a script creates list `Private` and renames its ID to `Home`
- **THEN** `husker lists` shows `Home` with the same ID and display position
- **AND** tasks keep their list membership
