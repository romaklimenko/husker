## ADDED Requirements

### Requirement: Suggest a list for a new task

Husker SHALL select a suggested list for a new task created from All tasks. The suggested list is the most common list among the 10 most recently created counted tasks.

A counted task is outside Trash and belongs to an accessible ordinary list. Open, done, and cancelled tasks count.
A higher task ID means a more recently created task. A task counts with its current list, even when it moved after creation.
When two or more lists share the highest count, the tied list whose newest counted task has the highest ID wins.
When no task is counted, the suggested list is the lowest-ID ordinary list.
The suggestion only sets the draft's initial List value. Choosing another list, undo, redo, and Save follow the requirement Choose a list for a draft by stable identity.
List views, the CLI `add` command, and the MCP `add_task` tool do not use the suggestion.

| Case | Suggested list |
| --- | --- |
| No task outside Trash | The lowest-ID ordinary list |
| One list has the highest count among the 10 newest counted tasks | That list |
| Several lists share the highest count | The tied list with the newest counted task |
| More than 10 counted tasks | Only the 10 with the highest IDs count |

#### Scenario: Suggest the most common recent list

- **WHEN** the 10 newest tasks outside Trash are 6 in Work, 3 in Private, and 1 in Inbox, and the user presses `n` in All tasks
- **THEN** the editor opens with Work in List and empty Title, Description, and Planned date

#### Scenario: Break a tie with the newest task

- **WHEN** the newest tasks are, from newest to oldest, one in Private, one in Work, one in Private, and one in Work, and the user presses `n` in All tasks
- **THEN** the editor opens with Private in List

#### Scenario: Ignore Trash and older tasks

- **WHEN** tasks 1 to 6 are in Private, tasks 7 to 10 are in Work, and tasks 11 to 13 are in Errands and in Trash
- **THEN** a new task from All tasks starts in Private
- **AND** Work would win if the Trash tasks used 3 of the 10 places

#### Scenario: Count closed tasks

- **WHEN** tasks 1 to 3 are done tasks in Work and tasks 4 and 5 are open tasks in Private
- **THEN** a new task from All tasks starts in Work

#### Scenario: Fall back to the default list

- **WHEN** every task is in Trash and the user presses `n` in All tasks
- **THEN** the editor opens with the lowest-ID ordinary list in List

#### Scenario: Keep the list of a list view

- **WHEN** most recent tasks belong to Work and the user presses `n` while viewing Private
- **THEN** the editor opens with Private in List

## MODIFIED Requirements

### Requirement: Edit task fields

Husker SHALL provide a task editor with these fields in this order and save all valid changes together when the user invokes Save.
Default Save shortcuts are Ctrl+S and Ctrl+Enter. Esc and Ctrl+G invoke Discard with the confirmation rules defined below.

| Field | Input |
| --- | --- |
| Title | A nonempty task title |
| List | A read-only selector for an existing ordinary list ID, displayed by its current name |
| Planned date | `YYYY-MM-DD`, `YYYYMMDD`, `YYMMDD`, `MMDD`, or empty for no date |
| Description | Plain text with multiple lines |

Tab selects the next field in this order and wraps from Description to Title. Shift+Tab selects the previous field and wraps from Title to Description.
Enter inserts a newline in Description, opens the picker when List is focused, and invokes Save when Title or Planned date is focused.
Opening an existing task focuses Description. Creating a task focuses Title.
Ordinary text input does not invoke task-navigation writes.
The Planned date field also accepts the preset actions defined below as draft edits. Title and Description retain those characters as ordinary text. List uses its selector controls.
Title, Description, and Planned date text editing follows the field navigation, selection, and history requirements below.
List changes use the picker and its choice history. Saving resolves the selected list by ID, never by name.
Field labels and hints use the selected language.
Validation follows the `task-storage` spec. Invalid input leaves the editor open with its entered text and an error message.
A failed write does not show a saved message.
A successful save shows Created for a new task and Saved for an existing task.

#### Scenario: Save a description and due date

- **WHEN** a user opens a task, enters two description lines, sets its due date to `2026-11-09`, and presses Ctrl+S
- **THEN** both changes persist together
- **AND** Husker returns to navigation and shows the task in the applicable planned group

#### Scenario: Keep invalid input for correction

- **WHEN** a user enters `2026-02-30` as a due date and presses Ctrl+Enter
- **THEN** Husker displays an invalid-date message and keeps the editor open
- **AND** the stored task remains unchanged

#### Scenario: Discard confirmed edits

- **WHEN** a user changes a title, presses Esc or Ctrl+G, and confirms Yes in the discard prompt
- **THEN** Husker returns to navigation without changing the stored task

#### Scenario: Capture a task with Enter

- **WHEN** a user presses `n`, types `Buy milk`, and presses Enter while Title is focused
- **THEN** Husker creates `Buy milk`, returns to navigation, and shows Created

#### Scenario: Keep Enter as a newline in Description

- **WHEN** Description is focused and the user presses Enter
- **THEN** the draft gains a line break and no save occurs

#### Scenario: Plan a new task with two Tabs

- **WHEN** a user presses `n`, types `Pay rent`, presses Tab twice, presses `t`, and presses Enter
- **THEN** Husker creates `Pay rent` planned for today in the list shown in List
- **AND** the first Tab focused List and the second focused Planned date

#### Scenario: Wrap field focus

- **WHEN** Description is focused and the user presses Tab
- **THEN** Title is focused
- **AND** pressing Shift+Tab focuses Description again

### Requirement: Create tasks and lists

Husker SHALL create tasks and lists through input forms without requiring the user to leave the terminal interface.

The new-task action opens the editor with an empty title, description, and due date. New-task actions are unavailable in History and Trash.
A list-scoped task view supplies its list. All tasks supplies the list defined by the requirement Suggest a list for a new task.
Saving an empty or invalid title leaves the editor open. Discarding a new task creates no stored task.
The new-list action defaults to `n` and `N` in the picker. The rename-list action defaults to `r` and `R` there.
A valid new list appears after existing ordinary lists and before Trash in the picker.
Renaming a list preserves its position. A successful create or rename returns to the picker with the saved list selected by ID.
Canceling the list-name form returns to the picker with its prior selected entry and scroll position.
Creating or renaming a list does not change the task view beneath the picker.
The list-name form footer shows the configured Apply and Back keys and the list-name hint.

#### Scenario: Capture a task in a list

- **WHEN** a user chooses Private, presses `n`, enters `Buy milk`, and presses Ctrl+S
- **THEN** Husker creates `Buy milk` in Private
- **AND** the task has no due date

#### Scenario: Create and rename a list

- **WHEN** a user presses `n` or `N` in the picker, enters `Errands`, and saves
- **THEN** the picker reappears with Errands immediately before Trash and selected
- **AND** pressing `r` or `R`, entering `Shopping`, and saving returns to the picker with that same list selected in the same position

#### Scenario: Cancel a list-name form

- **WHEN** a user opens a list-name form from a scrolled picker and presses Esc
- **THEN** the picker returns to its previous selected entry and scroll position without changing a list

### Requirement: Show empty-date guidance without inserting text

Husker SHALL display the localized Sometime label as a placeholder only when the Planned date buffer is empty. The label is `Sometime` in English and `Engang` in Danish.
The placeholder is not stored text, selected text, copied text, an undo step, or a due date.
The Planned date field title or hint lists `YYYY-MM-DD`, `YYYYMMDD`, `YYMMDD`, and `MMDD` without adding header rows.
Entering text hides the placeholder and renders the actual input.
Saving an empty field stores no due date. An open task without a due date appears under Sometime.

#### Scenario: Save an empty date

- **WHEN** Planned date displays its placeholder and the user saves an otherwise valid task
- **THEN** the stored due date is absent and no placeholder text is stored

#### Scenario: Type into the empty field

- **WHEN** Planned date is empty and the user types `1109`
- **THEN** the buffer contains only `1109`, without placeholder characters

#### Scenario: Show the Danish placeholder

- **WHEN** the language is Danish and a new task's Planned date is empty
- **THEN** the field shows `Engang` as a placeholder and its title still lists the accepted date forms
