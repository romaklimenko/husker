# Planning and search

## Purpose

Define the date groups, ordering, and query syntax used to find tasks.
The terminal interface and command line use these same rules. Task storage owns saved fields.
## Requirements
### Requirement: Date groups

Husker SHALL assign every task to exactly one planning group from its due date and the current local calendar date.

| Group | Filter value | Condition |
| --- | --- | --- |
| Overdue | `overdue` | The due date is before today. |
| Today | `today` | The due date is today. |
| Tomorrow | `tomorrow` | The due date is tomorrow. |
| Soon | `soon` | The due date is between today plus two days and today plus seven days, inclusive. |
| Next | `next` | The due date is at least eight days after today. |
| Undated | `undated` | The task has no due date. |

Status and Trash visibility follow the combined-filter rules and selected view.
Date arithmetic uses calendar days, including across months and years.

#### Scenario: Classify the week boundary

- **WHEN** today is `2026-11-08` and tasks are due on `2026-11-09`, `2026-11-10`, `2026-11-15`, and `2026-11-16`
- **THEN** their groups are Tomorrow, Soon, Soon, and Next respectively

#### Scenario: Classify undated tasks

- **WHEN** a task has no due date
- **THEN** the task belongs to Undated

### Requirement: Planning order

Husker SHALL sort dated tasks by ascending due date and place all undated tasks after dated tasks.
Saved manual position orders tasks with the same due date, including undated tasks.
Task IDs break remaining ties.
Status does not change this ordering.

#### Scenario: Keep dates ahead of manual positions

- **WHEN** task 1 is due on `2026-11-10` and task 2 is due on `2026-11-11`
- **THEN** task 1 appears before task 2 regardless of their saved manual positions

#### Scenario: Put undated tasks last

- **WHEN** task 1 is undated and task 2 is due on `2027-01-01`
- **THEN** task 2 appears before task 1 regardless of their statuses or manual positions

### Requirement: Combined filters

Husker SHALL require every filter term to match a task.

| Syntax | Matching rule |
| --- | --- |
| `l:private` | The list name equals `private`. |
| `milk` | The title or description contains `milk`. |
| `#shopping` | The title or description contains the literal text `#shopping`. |
| `due:today` | The task belongs to Today. |
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

Without `s:`, results outside Trash include open tasks and tasks whose current done or cancelled status has `status_changed_on` equal to today.
An unknown status-change date does not count as today.
Without `s:`, results inside Trash include every status.
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
