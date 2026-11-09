## MODIFIED Requirements

### Requirement: Show actual dates on task headings

Husker SHALL show one heading per exact due date for open tasks classified as Soon or Next in ordinary task views.
These headings use the calendar-label format from profiles-localization and replace the visible Soon and Next headings.
Today and Tomorrow headings include their actual due dates alongside their localized labels.
Done today and Cancelled today headings include the actual local status date alongside their localized labels.
History's Today and Yesterday headings include their actual recorded status dates alongside their localized labels.
Other known History dates retain calendar-date headings.
Earlier, Sometime, older aggregate Done and Cancelled, and Unknown date remain aggregate or unknown headings without an invented single date.
Every heading in an ordinary task view and in History ends with the number of task rows displayed under it in parentheses, such as `Today - 2026-11-09 Mon W46 (3)` or `Sometime (1)`.
The count covers the rows under that heading in the current results, so it follows the active filter and includes a held row under the heading it keeps.
Trash has no headings and shows no counts.
Headings do not change sorting, due-filter meanings, manual-reorder eligibility, or stored task data.
Heading rows remain unselectable, unhighlighted, and inert to mouse clicks.

#### Scenario: Split both future buckets by date

- **WHEN** today is `2026-11-08` and open tasks are due on November 10, 15, and 16
- **THEN** the tasks appear under three exact-date headings in that order
- **AND** no Soon or Next heading is displayed while their due-filter buckets remain unchanged

#### Scenario: Add dates to relative labels

- **WHEN** today is `2026-11-08` and the English interface shows Tomorrow and Done today
- **THEN** their headings include `2026-11-09 Mon W46` and `2026-11-08 Sun W45` respectively
- **AND** Done today uses the status date regardless of the task's due date

#### Scenario: Date relative History headings

- **WHEN** today is `2026-11-08` and History includes tasks with status dates November 8 and 7
- **THEN** Today includes the November 8 calendar label and Yesterday includes the November 7 calendar label

#### Scenario: Preserve aggregate headings

- **WHEN** Earlier contains tasks due on different past dates and Done contains older completions
- **THEN** neither aggregate heading claims one date for all of its tasks

#### Scenario: Count the rows under each heading

- **WHEN** today is `2026-11-08`, two open tasks are due today, three are due tomorrow, and one task was completed today
- **THEN** the headings read `Today - 2026-11-08 Sun W45 (2)`, `Tomorrow - 2026-11-09 Mon W46 (3)`, and `Done today - 2026-11-08 Sun W45 (1)`
- **AND** marking one of the tomorrow tasks done keeps `(3)` on Tomorrow while its row is held, and the next view change shows `(2)` there and `(2)` on Done today
