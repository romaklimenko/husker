## MODIFIED Requirements

### Requirement: Date groups

Husker SHALL assign every task to exactly one planning group from its due date and the current local calendar date.

| Group | Filter value | Condition |
| --- | --- | --- |
| Overdue | `overdue` | The due date is before today. |
| Today | `today` | The due date is today. |
| Tomorrow | `tomorrow` | The due date is tomorrow. |
| Soon | `soon` | The due date is between today plus two days and today plus seven days, inclusive. |
| Next | `next` | The due date is at least eight days after today. |
| Sometime | `undated` | The task has no due date. |

The terminal displays the undated group as `Sometime` in English and `Engang` in Danish.
The internal group key remains `undated`. Both `due:none` and `due:undated` match tasks without due dates.
The display names do not add `due:sometime` or `due:engang` filter values.
Status and Trash visibility follow the combined-filter rules and selected view.
Date arithmetic uses calendar days, including across months and years.

#### Scenario: Classify the week boundary

- **WHEN** today is `2026-11-08` and tasks are due on `2026-11-09`, `2026-11-10`, `2026-11-15`, and `2026-11-16`
- **THEN** their groups are Tomorrow, Soon, Soon, and Next respectively

#### Scenario: Classify undated tasks

- **WHEN** a task has no due date
- **THEN** the task belongs to the group displayed as Sometime in English and Engang in Danish
- **AND** both `due:none` and `due:undated` match that task

### Requirement: Planning order

Husker SHALL sort tasks by planning group, then open or closed status category, then ascending due date, saved manual position, and task ID.
Planning groups appear in the order Overdue, Today, Tomorrow, Soon, Next, and Sometime.
Within each group, open tasks precede closed tasks.
Done and cancelled tasks share one closed category and do not receive separate status ordering.
Tasks in the same status category sort by due date before manual position. Task IDs break remaining ties.
All undated tasks remain after dated groups, regardless of status or manual position.
Filtering changes which tasks appear without changing this ordering among the returned tasks.

#### Scenario: Keep open tasks first within Soon

- **WHEN** today is `2026-11-08`, an open task is due `2026-11-15`, and a visible done task is due `2026-11-10`
- **THEN** both tasks appear in Soon with the open task first

#### Scenario: Keep planning groups ahead of status

- **WHEN** today is `2026-11-08`, a visible done task is due `2026-11-07`, and an open task is due today
- **THEN** the done task appears in Overdue before the open task in Today

#### Scenario: Keep dates ahead of manual positions within one category

- **WHEN** two open tasks are due `2026-11-10` and `2026-11-11`
- **THEN** the task due `2026-11-10` appears first regardless of their saved manual positions

#### Scenario: Combine closed statuses

- **WHEN** a cancelled task is due `2026-11-10` and a done task is due `2026-11-11` and both are visible in Soon
- **THEN** the cancelled task appears first because both tasks belong to the closed category

#### Scenario: Put undated tasks last

- **WHEN** task 1 is undated and task 2 is due on `2027-01-01`
- **THEN** task 2 appears before task 1 regardless of their statuses or manual positions
