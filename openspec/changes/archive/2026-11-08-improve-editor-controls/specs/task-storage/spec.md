## MODIFIED Requirements

### Requirement: Manual reordering preserves dates and groups

Husker SHALL swap two task positions atomically only when both tasks are outside Trash, have the same due date, and share the open or closed status category.
Two undated tasks have the same due date for this rule.
Open tasks form one category. Done and cancelled tasks share the closed category.
The store checks both tasks' current categories and due dates inside the write transaction.
The swap preserves all task fields except `position` and `version`.

#### Scenario: Refuse a move across dates

- **WHEN** task 7 is due `2026-11-08` and task 8 is due `2026-11-09`
- **THEN** swapping their positions fails and both dates and positions remain unchanged

#### Scenario: Refuse a move across status categories

- **WHEN** task 7 is open and task 8 is done and both share a due date
- **THEN** swapping their positions fails without changing either task's position or version

#### Scenario: Reorder two closed tasks

- **WHEN** task 7 is done and task 8 is cancelled and both share a due date outside Trash
- **THEN** swapping their positions succeeds when both supplied versions are current
- **AND** both tasks retain their statuses and status-change dates

#### Scenario: Reject a stale pair atomically

- **WHEN** task 7 and task 8 share a due date but task 8's version changed after the caller read it
- **THEN** the swap fails without changing either task's position
