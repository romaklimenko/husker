## MODIFIED Requirements

### Requirement: Use keyboard actions

Husker SHALL provide the following default shortcuts in task navigation, History, and Trash and resolve configurable action shortcuts through the selected configuration.

| Key | Action |
| --- | --- |
| Up or `k` | Select the previous task |
| Down or `j` | Select the next task |
| Left, Home, or `g` | Select the first visible task, skipping the rows under Earlier |
| Right, End, or `G` | Select the last visible task. In All tasks and list views, skip done and cancelled tasks while any task is open |
| PageUp or PageDown | Move the selection one displayed page up or down |
| `l` | Open the list picker |
| `h` | Open History from ordinary task navigation, or return from History |
| `t` | Set the selected task due today |
| `T` | Set the selected task due tomorrow |
| `w` | Set the selected task due next week on Monday |
| `x` | Clear the selected task's due date |
| `m` | Move the selected task to another list |
| `u` | Undo the last recorded task action |
| `/` | Edit the task filter |
| Enter | Edit the selected task |
| Esc | Clear an active filter, or return from a scoped task view to All tasks |
| `n` | Create a task from All tasks or a list view |
| `r` | Refresh database contents in All tasks and list views, restore the selected task in Trash, or reopen it in History |
| Space | Toggle the selected task between open and done |
| `c` | Toggle the selected task between open and cancelled |
| `d` or Delete | Move a task to Trash. In Trash, start deleting the selected task forever |
| `D` | Start emptying Trash while Trash is open |
| Alt+Up or `K` | Move an open task before its eligible neighbor |
| Alt+Down or `J` | Move an open task after its eligible neighbor |
| F1 or `?` | Show help with the effective shortcuts |
| F2 | Toggle mouse capture |
| F3 | Choose and save the interface language |
| F4 | Show the selected task's plain text for copying |
| F5 | Refresh database contents on every screen |
| `q` or Ctrl+C | Quit |

The list picker types printable characters into its filter, as the requirement Filter list pickers by typing defines. It uses these default shortcuts.

| Key | Action in the list picker |
| --- | --- |
| Up or Down | Select the previous or next picker entry |
| Left or Home | Select the first picker entry |
| Right or End | Select the last picker entry |
| PageUp or PageDown | Move the selection one displayed page up or down |
| Enter | Choose the selected entry |
| Esc | Clear the picker filter, or cancel the picker when the filter is empty |
| Ctrl+N | Create a list |
| Ctrl+R | Rename the selected ordinary list |
| Ctrl+D or Delete | Start deleting the selected list, or start emptying Trash when the Trash entry is selected |
| Alt+Up or Alt+Down | Move the selected list up or down |
| F1 | Show help |
| F2, F3, or F5 | Toggle mouse capture, choose the language, or refresh |
| Ctrl+C | Quit |

Keyboard Up/Down wrap, First/Last jump, and PageUp/PageDown page according to the selection requirement below.
Ctrl+C quits only in task navigation, History, Trash, and the list picker. Text inputs keep Ctrl+C for Copy selection.
Actions that require a selected task do nothing when no task is selected.
Creating, renaming, moving, or deleting a list requires the picker. All tasks and Trash cannot be renamed, moved, or deleted.
The default ordinary list cannot move or be deleted. Other list moves follow the pinned boundaries defined by task storage.
Task creation, editing, completion, cancellation, moving to Trash, task reordering, moving to another list, and undo are unavailable in Trash.
Restore and Delete forever are available only in Trash. Empty Trash is available in Trash and from the Trash entry of the list picker.
The requirements Delete a Trash task forever and Empty Trash after confirmation define both confirmations.
Task actions do not affect background tasks while the picker is open.
Due-date presets are unavailable in Trash and use the navigation and editor contracts below.
The `profiles-localization` spec owns configuration syntax and binding validation. The optional `all_tasks` action and legacy `planned` alias follow that spec's compatibility rules.
The `all_tasks` action has no default shortcut. Back and the All tasks picker entry provide normal root navigation.
History is unavailable from Trash and the picker.
History supports opening, filtering, copying, due-date presets, status changes, moving tasks to Trash, moving tasks to another list, undo, reopening, help, refreshing, and navigation.
New-task creation and manual reordering are unavailable in History.

#### Scenario: Rebind an action

- **WHEN** configuration binds `quit` to `Ctrl+Q`
- **THEN** Ctrl+Q exits from task navigation or the list picker
- **AND** the help view displays Ctrl+Q as the effective quit shortcut

#### Scenario: Ignore an unavailable action

- **WHEN** a user presses Space while Trash is open
- **THEN** the deleted task keeps its stored status

#### Scenario: Use All tasks while the picker is open

- **WHEN** the picker is open over Work and the user chooses All tasks
- **THEN** the picker closes and All tasks returns with its saved task state

#### Scenario: Quit with Ctrl+C

- **WHEN** a user presses Ctrl+C in task navigation with default bindings
- **THEN** Husker quits and restores the terminal
- **AND** Ctrl+C in a Title field still copies the selection

#### Scenario: Reorder with Shift letters

- **WHEN** two open undated tasks are listed and the user presses `J` on the first
- **THEN** the tasks swap positions as Alt+Down would

#### Scenario: Refresh with r

- **WHEN** All tasks is open with default bindings and the user presses `r`
- **THEN** Husker refreshes database contents exactly as F5 does

#### Scenario: Keep r for Restore and Reopen

- **WHEN** Trash is open and the user presses `r` on a deleted task
- **THEN** the task leaves Trash and no refresh message appears
- **AND** `r` in History reopens the selected task

### Requirement: Wrap keyboard selection and jump to boundaries

Husker SHALL wrap configured Up and Down selection and support configured First and Last actions in the following selection modes.

| Mode | Selectable items |
| --- | --- |
| Ordinary task view | Tasks in the current visible query results. |
| History | Closed tasks in the current visible History results. |
| Trash | Tasks in the current visible Trash results. |
| Navigation list picker | All tasks, ordinary lists, and Trash in their existing order. |
| Editor list picker | Ordinary destination lists in their existing order. |
| Move-task picker | Ordinary destination lists in their existing order. |
| Language picker | English, then Dansk. |

Up on the first item selects the end item. Down on the last item selects the start item.
First selects the start item and Last selects the end item without changing result order or applying a picker choice.
The ordinary task view is All tasks or a list view. Its start item is the first task whose row is not under the Earlier heading.
When every visible task in the ordinary task view is under Earlier, its start item is the first task.
A row is under Earlier when the nearest heading above it is Earlier, or Tidligere in Danish. A held row counts under the heading it shows. The `planning-search` spec owns the date groups.
The end item of the ordinary task view is the last visible task whose status is open. While any visible task is open, a done or cancelled task is never the end item, even in a held row.
When no visible task in the ordinary task view is open, its end item is the last task.
In every other mode, the start item is the first item and the end item is the last item. History and Trash have no Earlier heading.
When the last item is also the start item, Down on it keeps it selected. When the first item is also the end item, Up on it keeps it selected.
Up and Down reach every item. When tasks under Earlier precede the start item, Up from the start item selects the last task under Earlier. When done or cancelled tasks follow the end item, Down from the end item selects the first of them.
Default keys are Up or `k` and Down or `j`.
First defaults to Home, `g`, and Left. Last defaults to End, `G`, and Right.
Configured PageUp and PageDown move the selection by the number of selectable rows last displayed in that selection area, not counting heading rows, clamped at the first or last item without wrapping. They are unavailable in the language picker.
In Help, Up, Down, PageUp, PageDown, First, and Last scroll the text by one row, one viewport, or to an end. In Copy text, PageUp and PageDown scroll by one viewport.
An empty result has no selection and each movement action is a no-op. One item stays selected after every movement action.
Group headings remain unselectable and do not count as items.
Selection uses the filtered visible results. After wrapping, jumping, or paging, the selected target is visible within the existing scrollable layout.
Selection does not save tasks, choose a list, change the interface language, or alter task and list positions.
Refresh retains its existing identity-preservation and missing-selection clamping rules. It does not wrap a removed selection.
The requirement Select the start item when no task is known defines when Husker selects the start item without a movement key.
Mouse wheels remain bounded. Text cursor movement, Help and Copy text scrolling, and reordering retain their bounded behavior.
First and Last have no selection action in text fields, the closed List selector, Copy text, or confirmation screens.
Left and Right retain text-cursor movement in text inputs and horizontal scrolling in Copy text.

#### Scenario: Wrap a filtered task list without selecting headings

- **WHEN** a query shows task 3 under Today and task 8 under Tomorrow and task 3 is selected
- **THEN** Up selects task 8 and makes its row visible
- **AND** Down returns to task 3 without selecting either heading

#### Scenario: Jump through a long task view

- **WHEN** a task view with no task under Earlier and no done or cancelled task has more rows than fit on screen and the user invokes Last then First
- **THEN** the last visible-result task and then the first visible-result task are selected and scrolled into view

#### Scenario: Skip Earlier with First and with Down from the last task

- **WHEN** All tasks shows `Old report` under Earlier, `Pay rent` under Today, and `Call the bank` under Tomorrow, and `Old report` is selected
- **THEN** Left selects `Pay rent`
- **AND** Right selects `Call the bank`, and Down then selects `Pay rent`
- **AND** Up then selects `Old report`

#### Scenario: Start after Earlier when nothing is planned today

- **WHEN** a list view shows two tasks under Earlier and one task under Sometime
- **THEN** Home selects the task under Sometime

#### Scenario: Use the first task when every task is under Earlier

- **WHEN** the filter `due:overdue` shows three tasks, all under Earlier, and the third is selected
- **THEN** Down selects the first task, and `g` keeps it selected

#### Scenario: Count a held row under its heading

- **WHEN** All tasks shows one task under Earlier and one under Today, and the user marks the Earlier task done
- **THEN** the done task stays held under Earlier
- **AND** Left selects the task under Today

#### Scenario: Keep the last task when it is the start item

- **WHEN** All tasks shows two tasks under Earlier and one task under Today, and the task under Today is selected
- **THEN** Down keeps the task under Today selected

#### Scenario: Skip closed tasks with Last and with Up from the first task

- **WHEN** All tasks shows `Old report` under Earlier, `Pay rent` under Today, `Call the bank` under Tomorrow, and `Filed taxes` under Done today, and `Old report` is selected
- **THEN** Up selects `Call the bank`
- **AND** Down then selects `Filed taxes`, and Right then selects `Call the bank`

#### Scenario: Use the last task when no task is open

- **WHEN** the filter `s:done` shows three done tasks and the first is selected
- **THEN** Up selects the third task, and `G` keeps it selected

#### Scenario: Skip a held row with Last

- **WHEN** All tasks shows `Pay rent` and `Call mom` under Today and nothing else, and the user marks `Call mom` done
- **THEN** the done task stays held under Today
- **AND** Down selects `Call mom`, and End then selects `Pay rent`

#### Scenario: Place the end item above the start item

- **WHEN** a list view shows `Older report` and `Old report` under Earlier, and `Work done` under Done today
- **THEN** Home selects `Work done` and End selects `Old report`
- **AND** Up on `Older report` selects `Old report`, and Down on `Work done` keeps it selected

#### Scenario: Keep the first task when it is the end item

- **WHEN** a list view shows one task under Today and two tasks under Done today, and the task under Today is selected
- **THEN** Up keeps the task under Today selected

#### Scenario: Wrap History and Trash

- **WHEN** the last visible task is selected in History or Trash and the user invokes Down
- **THEN** the first visible task becomes selected without changing any task data
- **AND** Up then selects the last visible task, even when it is done or cancelled

#### Scenario: Keep empty and single-item results stable

- **WHEN** a selection view has zero or one item and receives Up, Down, First, Last, PageUp, or PageDown
- **THEN** zero items remain unselected and one item remains selected without an error

#### Scenario: Navigate both list pickers

- **WHEN** a navigation list picker or editor list picker receives Up on its first item
- **THEN** its last item is selected without choosing it
- **AND** First returns to the first item and Last selects the last item

#### Scenario: Navigate language choices without applying them

- **WHEN** English is selected in the language picker and the user invokes Up
- **THEN** Dansk becomes selected without saving or changing the active language
- **AND** First selects English and Last selects Dansk

#### Scenario: Keep wheel and non-selection controls bounded

- **WHEN** the user wheels past the last task or picker item
- **THEN** selection remains at that last item
- **AND** text editing, Help/Copy text scrolling, and manual reordering do not wrap at their boundaries

#### Scenario: Page through a long task view

- **WHEN** a task view shows 10 rows, 30 tasks match, and the first task is selected
- **THEN** PageDown selects a task about ten rows further and scrolls it into view
- **AND** repeated PageDown stops at the last task and PageUp returns toward the first

