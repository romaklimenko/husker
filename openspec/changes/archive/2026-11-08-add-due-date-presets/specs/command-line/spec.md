## MODIFIED Requirements

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
