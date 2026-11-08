# Planning and search

## Purpose

Define the date groups, ordering, and query syntax used to find tasks.
The terminal interface and command line use these same rules. Task storage owns saved fields.
## Requirements
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
Open tasks use these buckets as display groups. Closed tasks use the status sections defined by Planning order.
Status and Trash visibility follow the combined-filter rules and selected view.
Date arithmetic uses calendar days, including across months and years.

#### Scenario: Classify the week boundary

- **WHEN** today is `2026-11-08` and tasks are due on `2026-11-09`, `2026-11-10`, `2026-11-15`, and `2026-11-16`
- **THEN** their groups are Tomorrow, Soon, Soon, and Next respectively

#### Scenario: Classify undated tasks

- **WHEN** a task has no due date
- **THEN** the task has the undated due bucket, displayed as Sometime in English and Engang in Danish when open
- **AND** both `due:none` and `due:undated` match that task

### Requirement: Planning order

Husker SHALL sort ordinary task results using open due-date groups followed by closed-status groups.
This order is shared by terminal task views, CLI task searches, and MCP task searches.

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

### Requirement: Combined filters

Husker SHALL require every filter term to match a task.

| Syntax | Matching rule |
| --- | --- |
| `l:private` | The list name equals `private`. |
| `milk` | The title or description contains `milk`. |
| `#shopping` | The title or description contains the literal text `#shopping`. |
| `due:today` | The stored due date is today. |
| `due:2026-11-08` | The due date equals `2026-11-08`. |
| `due:none` or `due:undated` | The task has no due date. |
| `s:done` | The task status is done. |

List names and text compare their Unicode lowercase forms.
Accents remain significant.
Any term beginning with `#` is literal text, including a bare `#` or `#area:work`.
Date groups accept the filter values in the date-groups table.

| Status value | Matching rule |
| --- | --- |
| `open` | The task is open. |
| `done` | The task is done, regardless of completion date. |
| `cancelled` | The task is cancelled, regardless of cancellation date. |
| `all` | The task has any status. |

Without `s:`, ordinary task results outside History and Trash include open tasks and tasks whose current done or cancelled status has `status_changed_on` equal to today.
An unknown status-change date does not count as today.
Without `s:`, results inside Trash include every status. History uses the closed-record default defined below.
Explicit status filters replace these defaults.
Filters never change whether the containing view includes Trash.

#### Scenario: Combine list and text terms

- **WHEN** the query is `l:private milk #shopping`
- **THEN** a task matches only if its list is private and its title or description contains both `milk` and `#shopping`

#### Scenario: Match Danish letters without dropping accents

- **WHEN** the query is `ÆBLE` and task titles are `Køb æbler` and `Køb abler`
- **THEN** only `Køb æbler` matches

#### Scenario: Keep today's completed tasks visible

- **WHEN** today is `2026-11-08` and three done tasks have status-change dates `2026-11-08`, `2026-11-07`, and null
- **THEN** the default non-Trash results include only the first of those done tasks
- **AND** `s:done` includes all three

#### Scenario: Request only open tasks

- **WHEN** the query is `s:open`
- **THEN** done and cancelled tasks are excluded even when their status changed today

#### Scenario: Request completed tasks

- **WHEN** the query is `s:done due:none`
- **THEN** the filter selects only done tasks without due dates

### Requirement: Quoted filter text

Husker SHALL support double-quoted filter values and text phrases with escaped quotes and backslashes.

Whitespace separates terms outside quotes.
The query `l:"Home admin" "electricity bill"` combines an exact list name and a text phrase.
A wholly quoted term is literal text, even when the text starts with a recognized field or `#`.
Inside quotes, `\"` represents a quote and `\\` represents a backslash.

#### Scenario: Search for literal filter syntax

- **WHEN** the query is `"due:today"`
- **THEN** the filter searches the title and description for the literal substring `due:today`

### Requirement: Invalid filters

Husker SHALL reject malformed filters with an error instead of silently ignoring a term.

| Error | Example |
| --- | --- |
| Unknown field | `owner:me` |
| Repeated list field | `l:private l:work` |
| Repeated date field | `due:none due:today` |
| Repeated status field | `s:open s:done` |
| Empty term | `""` |
| Empty field value | `due:` |
| Invalid date | `due:2026-02-30` |
| Unsupported status | `s:paused` |
| Unsupported escape inside quotes | `"milk\n"` |
| Unclosed quotes | `"milk` |
| Removed date groups | `due:maybe` or `due:anytime` |

Repeated text terms remain valid and combine with AND.
A bare `#` is a valid literal text term.
Date values use the exact `YYYY-MM-DD` form.

#### Scenario: Reject a repeated list field

- **WHEN** the query is `l:private l:work`
- **THEN** parsing returns an error that identifies the repeated `l:` field

#### Scenario: Reject an impossible date

- **WHEN** the query is `due:2026-02-30`
- **THEN** parsing returns an invalid-date error

### Requirement: Search current closed-task history

Husker SHALL provide History results containing current done and cancelled tasks outside Trash, restricted to the selected ordinary list when one is selected.
History applies the existing text, list, due, and status filters as additional restrictions.
Without a status filter, History includes both closed statuses on all dates.
Explicit s:all includes both closed statuses; s:open returns no History records.
Due filters continue to inspect stored due dates, not status dates or display headings.
History sorts by `status_changed_on` descending, then `status_changed_at` descending, then task ID descending.
Unknown local dates follow known dates. Within one local date, unknown timestamps follow known timestamps.
History reads current task records and does not invent prior edits or status events.
Reopening removes the task from History because its current status becomes open.

#### Scenario: Retrieve old completions in a list

- **WHEN** History is opened for Work with an empty query
- **THEN** it contains Work's currently done and cancelled non-Trash tasks, including records from earlier dates
- **AND** it excludes open tasks and tasks belonging to other lists

#### Scenario: Combine due and status history filters

- **WHEN** the History query is s:done due:none
- **THEN** only currently done tasks without due dates remain, regardless of completion date

#### Scenario: Preserve unknown completion dates

- **WHEN** a legacy completed task has no status_changed_on value
- **THEN** History places it after known-date records without assigning a fabricated date

#### Scenario: Resolve same-day ties

- **WHEN** tasks 12 and 15 have the same status date and equal or unknown timestamps
- **THEN** History places task 15 before task 12 as a deterministic tie-breaker

#### Scenario: Respect the recorded same-day completion order

- **WHEN** task 12 has a later timestamp than task 15 on the same local status date
- **THEN** History places task 12 first even though its task ID is lower

### Requirement: Resolve common due-date presets

Husker SHALL resolve due-date presets from the current local calendar date using these rules.

| Preset | Stable action | Result |
| --- | --- | --- |
| Today | `due_today` | The current local calendar date. |
| Tomorrow | `due_tomorrow` | The next calendar date. |
| Next week (Monday) | `due_next_week` | The first Monday strictly after the current date. |
| Clear due date | `clear_due` | No due date. |

The same calculation applies to navigation and the Due date editor field.
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
