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

The terminal displays the `overdue` bucket as Earlier in English and Tidligere in Danish.
The accepted filter remains `due:overdue`; no `due:earlier` or `due:tidligere` alias is added.
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

### Requirement: Resolve common due-date presets

Husker SHALL resolve due-date presets from the current local calendar date using these rules.

| Preset | Stable action | Result |
| --- | --- | --- |
| Today | `due_today` | The current local calendar date. |
| Tomorrow | `due_tomorrow` | The next calendar date. |
| Next week (Monday) | `due_next_week` | The first Monday strictly after the current date. |
| Clear due date | `clear_due` | No due date. |

The same calculation applies to navigation and the Planned date editor field.
Tests supply the current date explicitly. Runtime actions use the local day at the keypress, including after crossing midnight.
Monday advances seven days for Next week. Sunday advances one day.
Calendar arithmetic handles month, year, and leap-day boundaries without treating a day as an elapsed 24-hour interval.
If the requested result exceeds the supported date range, the preset returns an error without changing a task or editor field.
Presets change no status or status-change date. Due buckets, result grouping, and filters retain their existing rules.

#### Scenario: Resolve Today and Tomorrow

- **WHEN** the current local date is `2026-11-08`
- **THEN** Today resolves to `2026-11-08` and Tomorrow resolves to `2026-11-09`

#### Scenario: Advance to the next Monday

- **WHEN** Next week is invoked on Sunday `2026-11-08` or Monday `2026-11-09`
- **THEN** the respective results are `2026-11-09` and `2026-11-16`

#### Scenario: Cross year and leap-day boundaries

- **WHEN** Next week is invoked on `2026-12-31` and Tomorrow is invoked on `2028-02-28`
- **THEN** the respective results are `2027-01-04` and `2028-02-29`

#### Scenario: Reject date overflow

- **WHEN** Tomorrow is invoked with the maximum supported date as the supplied current date
- **THEN** resolution fails and no task or draft value changes
