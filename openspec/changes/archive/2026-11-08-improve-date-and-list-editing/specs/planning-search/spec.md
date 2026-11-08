## MODIFIED Requirements

### Requirement: Date groups

Husker SHALL classify every task into exactly one due-date bucket from its stored due date and the current local calendar date.

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
Open tasks use these buckets for classification and ordering. Terminal headings subdivide Soon and Next by exact due date as defined by terminal-interface.
Closed tasks use the status sections defined by Planning order.
Status and Trash visibility follow the combined-filter rules and selected view.
Date arithmetic uses calendar days, including across months and years.

#### Scenario: Classify the week boundary

- **WHEN** today is `2026-11-08` and tasks are due on `2026-11-09`, `2026-11-10`, `2026-11-15`, and `2026-11-16`
- **THEN** their due buckets are Tomorrow, Soon, Soon, and Next respectively

#### Scenario: Classify undated tasks

- **WHEN** a task has no due date
- **THEN** the task has the undated due bucket, displayed as Sometime in English and Engang in Danish when open
- **AND** both `due:none` and `due:undated` match that task

### Requirement: Planning order

Husker SHALL sort ordinary task results using open due-date groups followed by closed-status groups.
This order is shared by terminal task views, CLI task searches, and MCP task searches.
The group names below identify sorting sections. Terminal headings for Soon and Next show each exact due date without changing this order.

| Group | Contents |
| --- | --- |
| Overdue, Today, Tomorrow, Soon, Next, Sometime | Open tasks, classified by the existing due-date rules. |
| Done today | Done tasks whose status_changed_on is today. |
| Cancelled today | Cancelled tasks whose status_changed_on is today. |
| Done | Other visible done tasks, including unknown status dates. |
| Cancelled | Other visible cancelled tasks, including unknown status dates. |

Open tasks sort by ascending due date, manual position, and task ID within their group.
Closed tasks sort by `status_changed_on` descending, then `status_changed_at` descending, then task ID descending within their group.
Unknown local dates follow known dates. Within one local date, unknown timestamps follow known timestamps.
Closed-task manual positions do not affect this order, and closed tasks remain ineligible for manual reordering.
Grouping never changes a stored due date or the meaning of a due filter.
Filtering changes which tasks appear without changing this order among the returned tasks.
Default visibility still excludes older closed tasks. Explicit status filters can include them.

#### Scenario: Keep open work before completed overdue work

- **WHEN** an undated open task and a task completed today with an overdue due date are visible
- **THEN** the open task appears in Sometime before the completed task in Done today

#### Scenario: Group an undated completion today

- **WHEN** a task without a due date is marked done today
- **THEN** it appears in Done today and still matches due:none

#### Scenario: Keep dates ahead of manual positions for open work

- **WHEN** two open tasks are due on consecutive dates within Soon
- **THEN** the earlier due task appears first regardless of manual position

#### Scenario: Separate closed statuses

- **WHEN** done and cancelled tasks have status dates equal to today
- **THEN** done tasks appear in Done today and cancelled tasks appear in Cancelled today

#### Scenario: Find older and undated completions

- **WHEN** an explicit status filter includes a task completed yesterday and a completed task with an unknown status date
- **THEN** both appear in Done, with the known-date task first

#### Scenario: Show the most recent completion first on the same day

- **WHEN** tasks 12 and 15 have the same local status date and task 12 was completed at 15:30 UTC after task 15 at 09:00 UTC
- **THEN** task 12 appears before task 15 within their closed section, regardless of their manual positions

#### Scenario: Put unknown times after recorded times on the same day

- **WHEN** two done tasks have the same known local date and only one has a recorded timestamp
- **THEN** the task with a recorded timestamp appears first

## ADDED Requirements

### Requirement: Preserve due-filter buckets independently from headings

Husker SHALL preserve the existing due-filter meanings when terminal heading text or future display groups change.
The `due:soon` filter matches stored due dates from today plus two through seven calendar days, inclusive.
The `due:next` filter matches stored due dates at least eight calendar days after today.
All existing exact-date, relative-date, and no-date filters keep their accepted syntax and matching rules.
Display headings do not add new query tokens or change task due dates, status dates, or completion timestamps.

#### Scenario: Keep Soon matching across date headings

- **WHEN** today is `2026-11-08` and the query is `due:soon`
- **THEN** tasks due November 10 through November 15 match regardless of how their terminal headings are displayed
- **AND** tasks due November 9 or November 16 do not match

#### Scenario: Keep Next matching

- **WHEN** today is `2026-11-08` and the query is `due:next`
- **THEN** tasks due November 16 or later match and earlier due dates do not match
