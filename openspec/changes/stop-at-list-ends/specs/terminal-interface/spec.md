## RENAMED Requirements

- FROM: `### Requirement: Wrap keyboard selection and jump to boundaries`
- TO: `### Requirement: Stop keyboard selection at the ends and jump to boundaries`

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

Keyboard Up/Down step, First/Last jump, and PageUp/PageDown page according to the requirement Stop keyboard selection at the ends and jump to boundaries.
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

### Requirement: Change language without leaving the current screen

Husker SHALL provide a modal language picker through the configured `language` action, which defaults to F3.
The picker is available from task navigation, History, the navigation list picker, the editor list picker, task editors, filter editors, list-name editors, and Help.
It is unavailable from task-discard confirmation, list-deletion confirmation, Delete forever confirmation, Empty Trash confirmation, and Copy text.
The choices are English and Dansk. The current language is selected when the picker opens.
Configured Up selects English and configured Down selects Dansk. Neither wraps to the other choice. First selects English and Last selects Dansk. Apply saves the choice, and Back cancels without saving.
Background task actions and text editing are inactive while the picker is open. Mouse events do not change the underlying screen.
The global Refresh action remains available and preserves the language choice and underlying drafts.
Opening and canceling the picker preserves the current view, filter, selection, scroll, editor text, cursor, text selection, undo history, and originally read task version.
Successful saving changes localized text immediately and returns to the same underlying screen with that state preserved.
A save failure keeps the picker open with an error and the previous language active.
The profiles-localization spec owns the saved path, configuration preservation, and atomic replacement.
The picker does not translate task text, list names, or filter queries.
Language changes do not count as task edits or make a clean task draft dirty.

#### Scenario: Change language during an edit

- **WHEN** a user edits a task description, opens the language picker, and successfully saves Dansk
- **THEN** the task editor returns with Danish labels and the exact same unsaved text, cursor, selection, undo history, and task version
- **AND** the task has not been saved or otherwise changed in the database

#### Scenario: Cancel from a scrolled list picker

- **WHEN** a user opens the language picker from a scrolled list picker, changes its choice, and presses Esc with default bindings
- **THEN** the list picker returns with its original language, selected row, and scroll position
- **AND** no configuration file is written

#### Scenario: Keep a clean editor clean

- **WHEN** a user opens an unchanged task, changes interface language successfully, and invokes Discard
- **THEN** the editor closes without a changed-task confirmation

#### Scenario: Preserve a draft when language saving fails

- **WHEN** a configuration write fails while the language picker is open over an editor
- **THEN** the picker shows an error without switching language
- **AND** canceling returns to the unchanged editor draft

### Requirement: Keep editor list picking modal

Husker SHALL display a modal editor list picker containing only ordinary lists in their persistent order with the default-list marker.
Virtual All tasks and Trash entries and all list-management actions are unavailable.
The current draft list is selected when present. Opening the picker does not change the draft if that ID is unavailable.
Configured Up and Down move the selection by one list and stop at the first and last list. First and Last jump to the corresponding boundary. Open chooses the selected list and returns to the List selector.
Configured Back with an empty picker filter cancels and returns to the unchanged List selector, with Esc as the default. A `lists` key that is not a printable character also cancels.
Printable keys type into the picker filter as the requirement Filter list pickers by typing defines.
Single-click selects a visible row. Double-click on the same row within 400 milliseconds chooses it.
Wheel events move picker selection only inside the popup. Borders, outside clicks, and outside wheel events do nothing.
Background editor fields do not receive popup input.
Refresh preserves the picker selection by ID when present and otherwise clamps its previous index.
Refresh does not replace the draft's chosen ID or text fields.
Help and language-picker round trips preserve the editor picker selection, scroll, and underlying draft state.

#### Scenario: Cancel a destination choice

- **WHEN** a user opens the editor picker, selects Work, and presses Esc with default bindings
- **THEN** the picker closes with the draft's original list and all other field state unchanged

#### Scenario: Keep picker scope separate from navigation

- **WHEN** the editor picker is open
- **THEN** it shows ordinary lists and the default marker without All tasks, Trash, or management actions
- **AND** background fields receive no click or wheel action

#### Scenario: Refresh a removed picker row

- **WHEN** an externally deleted list was selected in the editor picker and the user refreshes
- **THEN** the picker clamps selection to an existing row without changing the draft's selected list ID

#### Scenario: Return from Help or language selection

- **WHEN** the user opens Help or the language picker from a scrolled editor list picker and returns
- **THEN** the editor picker retains its selected list ID and scroll within current bounds
- **AND** all underlying draft fields and their histories remain unchanged

### Requirement: Stop keyboard selection at the ends and jump to boundaries

Husker SHALL move configured Up and Down selection by one item without wrapping past either end, and support configured First and Last actions in the following selection modes.

| Mode | Selectable items |
| --- | --- |
| Ordinary task view | Tasks in the current visible query results. |
| History | Closed tasks in the current visible History results. |
| Trash | Tasks in the current visible Trash results. |
| Navigation list picker | All tasks, ordinary lists, and Trash in their existing order. |
| Editor list picker | Ordinary destination lists in their existing order. |
| Move-task picker | Ordinary destination lists in their existing order. |
| Language picker | English, then Dansk. |

Up on the first item keeps the first item selected. Down on the last item keeps the last item selected.
First selects the start item and Last selects the end item without changing result order or applying a picker choice.
The ordinary task view is All tasks or a list view. Its start item is the first task whose row is not under the Earlier heading.
When every visible task in the ordinary task view is under Earlier, its start item is the first task.
A row is under Earlier when the nearest heading above it is Earlier, or Tidligere in Danish. A held row counts under the heading it shows. The `planning-search` spec owns the date groups.
The end item of the ordinary task view is the last visible task whose status is open. While any visible task is open, a done or cancelled task is never the end item, even in a held row.
When no visible task in the ordinary task view is open, its end item is the last task.
In every other mode, the start item is the first item and the end item is the last item. History and Trash have no Earlier heading.
Up and Down reach every item. Up from the start item reaches the tasks under Earlier. Down from the end item reaches the done and cancelled tasks after it.
Default keys are Up or `k` and Down or `j`.
First defaults to Home, `g`, and Left. Last defaults to End, `G`, and Right.
Configured PageUp and PageDown move the selection by the number of selectable rows last displayed in that selection area, not counting heading rows, clamped at the first or last item. They are unavailable in the language picker.
In Help, Up, Down, PageUp, PageDown, First, and Last scroll the text by one row, one viewport, or to an end. In Copy text, PageUp and PageDown scroll by one viewport.
An empty result has no selection and each movement action is a no-op. One item stays selected after every movement action.
Group headings remain unselectable and do not count as items.
Selection uses the filtered visible results. After stepping, jumping, or paging, the selected target is visible within the existing scrollable layout.
Selection does not save tasks, choose a list, change the interface language, or alter task and list positions.
Refresh retains its existing identity-preservation and missing-selection clamping rules.
The requirement Select the start item when no task is known defines when Husker selects the start item without a movement key.
Mouse wheels remain bounded. Text cursor movement, Help and Copy text scrolling, and reordering retain their bounded behavior.
First and Last have no selection action in text fields, the closed List selector, Copy text, or confirmation screens.
Left and Right retain text-cursor movement in text inputs and horizontal scrolling in Copy text.

#### Scenario: Step through a filtered task list without selecting headings

- **WHEN** a query shows task 3 under Today and task 8 under Tomorrow and task 3 is selected
- **THEN** Down selects task 8 and makes its row visible, and a second Down keeps task 8 selected
- **AND** Up returns to task 3 without selecting either heading, and a second Up keeps task 3 selected

#### Scenario: Stop at the first task under Earlier

- **WHEN** All tasks shows six tasks under Earlier, `Pay rent` under Today, and `Someday` under Sometime, and `Pay rent` is selected
- **THEN** six presses of Up select the first task under Earlier
- **AND** a seventh Up keeps the first task under Earlier selected
- **AND** Left then selects `Pay rent`, Right selects `Someday`, and Down keeps `Someday` selected

#### Scenario: Jump through a long task view

- **WHEN** a task view with no task under Earlier and no done or cancelled task has more rows than fit on screen and the user invokes Last then First
- **THEN** the last visible-result task and then the first visible-result task are selected and scrolled into view

#### Scenario: Skip Earlier with First

- **WHEN** All tasks shows `Old report` under Earlier, `Pay rent` under Today, and `Call the bank` under Tomorrow, and `Old report` is selected
- **THEN** Left selects `Pay rent`
- **AND** Right selects `Call the bank`, and Down then keeps `Call the bank` selected
- **AND** two presses of Up then select `Old report`, and a third Up keeps `Old report` selected

#### Scenario: Start after Earlier when nothing is planned today

- **WHEN** a list view shows two tasks under Earlier and one task under Sometime
- **THEN** Home selects the task under Sometime

#### Scenario: Use the first task when every task is under Earlier

- **WHEN** the filter `due:overdue` shows three tasks, all under Earlier, and the third is selected
- **THEN** Down keeps the third task selected
- **AND** `g` selects the first task, and Up keeps it selected

#### Scenario: Count a held row under its heading

- **WHEN** All tasks shows one task under Earlier and one under Today, and the user marks the Earlier task done
- **THEN** the done task stays held under Earlier
- **AND** Left selects the task under Today

#### Scenario: Keep the last task when it is the start item

- **WHEN** All tasks shows two tasks under Earlier and one task under Today, and the task under Today is selected
- **THEN** Down keeps the task under Today selected

#### Scenario: Skip closed tasks with Last

- **WHEN** All tasks shows `Old report` under Earlier, `Pay rent` under Today, `Call the bank` under Tomorrow, and `Filed taxes` under Done today, and `Old report` is selected
- **THEN** Up keeps `Old report` selected
- **AND** Right selects `Call the bank`
- **AND** Down then selects `Filed taxes`, a second Down keeps `Filed taxes` selected, and Right then selects `Call the bank`

#### Scenario: Use the last task when no task is open

- **WHEN** the filter `s:done` shows three done tasks and the first is selected
- **THEN** Up keeps the first task selected
- **AND** `G` selects the third task, and Down keeps it selected

#### Scenario: Skip a held row with Last

- **WHEN** All tasks shows `Pay rent` and `Call mom` under Today and nothing else, and the user marks `Call mom` done
- **THEN** the done task stays held under Today
- **AND** Down selects `Call mom`, and End then selects `Pay rent`

#### Scenario: Place the end item above the start item

- **WHEN** a list view shows `Older report` and `Old report` under Earlier, and `Work done` under Done today
- **THEN** Home selects `Work done` and End selects `Old report`
- **AND** Up on `Older report` keeps it selected, and Down on `Work done` keeps it selected

#### Scenario: Keep the first task when it is the end item

- **WHEN** a list view shows one task under Today and two tasks under Done today, and the task under Today is selected
- **THEN** Up keeps the task under Today selected

#### Scenario: Stop at the ends of History and Trash

- **WHEN** the last visible task is selected in History or Trash and the user invokes Down
- **THEN** the last visible task stays selected without changing any task data
- **AND** First selects the first visible task, Up keeps it selected, and Last selects the last visible task, even when it is done or cancelled

#### Scenario: Keep empty and single-item results stable

- **WHEN** a selection view has zero or one item and receives Up, Down, First, Last, PageUp, or PageDown
- **THEN** zero items remain unselected and one item remains selected without an error

#### Scenario: Navigate both list pickers

- **WHEN** a navigation list picker or editor list picker receives Up on its first item
- **THEN** its first item stays selected without choosing it
- **AND** Down on its last item keeps its last item selected
- **AND** First returns to the first item and Last selects the last item

#### Scenario: Navigate language choices without applying them

- **WHEN** English is selected in the language picker and the user invokes Up
- **THEN** English stays selected
- **AND** Down selects Dansk without saving or changing the active language, and a second Down keeps Dansk selected
- **AND** First selects English and Last selects Dansk

#### Scenario: Keep wheel and non-selection controls bounded

- **WHEN** the user wheels past the last task or picker item
- **THEN** selection remains at that last item
- **AND** text editing, Help/Copy text scrolling, and manual reordering stop at their boundaries

#### Scenario: Page through a long task view

- **WHEN** a task view shows 10 rows, 30 tasks match, and the first task is selected
- **THEN** PageDown selects a task about ten rows further and scrolls it into view
- **AND** repeated PageDown stops at the last task and PageUp returns toward the first

### Requirement: Select the start item when no task is known

Husker SHALL select the start item of a task view whenever it shows rows without a known task to select.

The requirement Stop keyboard selection at the ends and jump to boundaries defines the start item. In All tasks and list views, it is the first task whose row is not under the Earlier heading, or the first task when every visible task is under Earlier. In History and Trash, it is the first task.
A known task is a task that was selected or saved for the view and is still among its visible results.
A filter edit and Enter in the filter input have no known task. They select the start item of the matching tasks even when the selected task still matches.
A refresh, Esc in the filter input, and a language change keep the row position when the selected task leaves the results. They do not select the start item in that case.
A view has saved state in two cases: All tasks after the user leaves it for a list view or Trash, and the view from which History was opened. The requirements Restore All tasks after visiting a scoped view and Review retained completed and cancelled tasks define that state.
The following table lists each situation and the task that Husker selects.

| Situation | Selected task |
| --- | --- |
| Husker starts | The start item of All tasks |
| A list view, Trash, or History opens without saved state | The start item |
| A view returns to its saved state, and the saved task is visible | The saved task |
| A view returns to its saved state, and the saved task is no longer visible | The start item |
| The current list is deleted, and All tasks returns to its saved state | The saved task when visible, otherwise the start item |
| A refresh finds rows in a view that had no selected task | The start item |
| A refresh while a task is selected | The selected task, or the same row position when the task left the results |
| An edit in the filter input leaves valid filter text | The start item of the matching tasks |
| Enter applies valid filter text | The start item of the matching tasks |
| Esc closes the filter input | The task selected before the input opened, or that row position when the task is gone. The start item when no task was selected. |
| The interface language changes | The selected task, or the same row position when the task left the results. The start item when no task was selected. |
| Back clears an applied filter | The selected task when still visible, otherwise the start item |
| Choosing the current scope in the list picker | The selected task stays selected |

An empty view has no selection. Selecting the start item changes no stored data and does not end a held row.

#### Scenario: Start at today's first task

- **WHEN** Husker starts and All tasks shows `Old report` under Earlier, `Pay rent` under Today, and `Someday` under Sometime
- **THEN** `Pay rent` is selected

#### Scenario: Select the start item while typing a filter

- **WHEN** All tasks shows `Work earlier` under Earlier and `Work soon` under a later date, and the user presses `/` and types `work`
- **THEN** `Work soon` is selected after each letter
- **AND** `Work soon` stays selected after Enter

#### Scenario: Replace a saved task that is gone

- **WHEN** All tasks shows `Old report` under Earlier, `Pay rent` under Today, `Call the bank` under Tomorrow, and `Someday` under Sometime. The user leaves All tasks for Work with `Call the bank` selected, `Call the bank` moves to Trash, and the user presses Esc in Work
- **THEN** All tasks returns with `Pay rent`, its start item, selected

#### Scenario: Return to a saved task that is still visible

- **WHEN** the user leaves All tasks for Work with `Someday` selected and presses Esc in Work
- **THEN** All tasks returns with `Someday` selected

#### Scenario: Clear a filter whose selected task is not in the default results

- **WHEN** All tasks holds `Old report` under Earlier and `Pay rent` under Today, the user filters it by `s:done`, and selects `Filed taxes`, which was completed last week
- **THEN** Esc clears the filter and selects `Pay rent`, because the default results do not show `Filed taxes`

#### Scenario: Fill an empty view

- **WHEN** the Later list is empty and open, and a refresh brings `Later old` under Earlier and `Later today` under Today
- **THEN** `Later today` is selected
- **AND** after the user selects `Later old`, a later refresh keeps `Later old` selected

