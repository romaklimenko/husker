## MODIFIED Requirements

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
The terminal task filter also matches an unquoted text term against the day name of the task's heading, as the requirement Match day names in the task filter of the `terminal-interface` spec defines.
The `husker tasks` command, the local MCP server, and the online server match every text term against titles and descriptions only.

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

#### Scenario: Keep text matching exact for automation

- **WHEN** today is `2026-11-18`, `Call mom` is due `2026-11-18`, `Fix the toaster` has no due date, and `husker tasks to` runs
- **THEN** only `Fix the toaster` is listed
