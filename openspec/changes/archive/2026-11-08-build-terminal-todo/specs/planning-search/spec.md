## ADDED Requirements

### Requirement: Date groups

Husker SHALL assign every task to exactly one planning group from its due date, readiness, and the current local calendar date.

| Group | Filter value | Condition |
| --- | --- | --- |
| Overdue | `overdue` | The due date is before today. |
| Today | `today` | The due date is today. |
| Tomorrow | `tomorrow` | The due date is tomorrow. |
| Soon | `soon` | The due date is between today plus two days and today plus seven days, inclusive. |
| Next | `next` | The due date is at least eight days after today. |
| Anytime | `anytime` | The task has no due date and is ready. |
| Maybe | `maybe` | The task has no due date and is not ready. |

Status and Trash visibility are defined by task storage and the selected view.
Date arithmetic uses calendar days, including across months and years.

#### Scenario: Classify the week boundary

- **WHEN** today is `2026-11-08` and tasks are due on `2026-11-09`, `2026-11-10`, `2026-11-15`, and `2026-11-16`
- **THEN** their groups are Tomorrow, Soon, Soon, and Next respectively

#### Scenario: Classify undated tasks

- **WHEN** two tasks have no due date and only the first task is ready
- **THEN** the first task belongs to Anytime and the second belongs to Maybe

### Requirement: Planning order

Husker SHALL sort tasks by planning group, due date, saved manual position, and task ID, in that order.

Planning groups follow the order in the date-groups table.
Manual positions order tasks within one exact due date or one undated group.
Task IDs break remaining ties.

#### Scenario: Keep dates ahead of manual positions

- **WHEN** task 1 is due on `2026-11-10` and task 2 is due on `2026-11-11`
- **THEN** task 1 appears before task 2 regardless of their saved manual positions

### Requirement: Combined filters

Husker SHALL require every filter term to match a task.

| Syntax | Matching rule |
| --- | --- |
| `l:private` | The list name equals `private`. |
| `milk` | The title or description contains `milk`. |
| `#shopping` | One task tag equals `shopping`. |
| `due:today` | The task belongs to Today. |
| `due:2026-11-08` | The due date equals `2026-11-08`. |
| `due:none` | The task has no due date. |
| `s:done` | The task status is done. |

List names, text, and tags compare their Unicode lowercase forms.
Accents remain significant.
Date groups accept the filter values in the date-groups table.

| Status value | Matching rule |
| --- | --- |
| `open` | The task is open. |
| `done` | The task is done. |
| `cancelled` | The task is cancelled. |
| `all` | The task has any status. |

Without `s:`, the filter selects open tasks outside Trash and every status inside Trash.
The `s:all` term removes the status restriction.
Filters never change whether the containing view includes Trash.

#### Scenario: Combine list, text, and tag

- **WHEN** the query is `l:private milk #shopping`
- **THEN** a task matches only if its list is private, its title or description contains milk, and it has the shopping tag

#### Scenario: Match Danish letters without dropping accents

- **WHEN** the query is `ÆBLE` and task titles are `Køb æbler` and `Køb abler`
- **THEN** only `Køb æbler` matches

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

These inputs are invalid:

| Error | Example |
| --- | --- |
| Unknown field | `owner:me` |
| Repeated list field | `l:private l:work` |
| Repeated date field | `due:none due:today` |
| Repeated status field | `s:open s:done` |
| Empty term | `""` |
| Empty field value | `due:` |
| Empty tag | `#` |
| Invalid date | `due:2026-02-30` |
| Unsupported status | `s:paused` |
| Unsupported escape inside quotes | `"milk\n"` |
| Unclosed quotes | `"milk` |

Repeated text and tag terms remain valid and combine with AND.
Date values use the exact `YYYY-MM-DD` form.

#### Scenario: Reject a repeated list field

- **WHEN** the query is `l:private l:work`
- **THEN** parsing returns an error that identifies the repeated `l:` field

#### Scenario: Reject an impossible date

- **WHEN** the query is `due:2026-02-30`
- **THEN** parsing returns an invalid-date error
