## MODIFIED Requirements

### Requirement: Use keyboard actions

Husker SHALL provide the following default shortcuts in task navigation and the list picker and resolve configurable action shortcuts through the selected configuration.

| Key | Action |
| --- | --- |
| Up or `k` | Select the previous task or picker entry |
| Down or `j` | Select the next task or picker entry |
| Left, Home, or `g` | Select the first visible task or picker entry |
| Right, End, or `G` | Select the last visible task or picker entry |
| PageUp or PageDown | Move the selection one displayed page up or down |
| `l` | Open or cancel the list picker |
| `h` | Open History from ordinary task navigation, or return from History |
| `t` | Set the selected task due today |
| `T` | Set the selected task due tomorrow |
| `w` | Set the selected task due next week on Monday |
| `x` | Clear the selected task's due date |
| `m` | Move the selected task to another list |
| `u` | Undo the last recorded task action |
| `/` | Edit the task filter |
| Enter | Choose the picker entry or edit the selected task |
| Esc | Cancel the picker, clear an active filter, or return from a scoped task view to All tasks |
| `n` | Create a task from a task view, or create a list from the picker |
| `N` | Create a list from the picker |
| `r` | Restore the selected task from Trash, reopen it from History, or rename the selected ordinary list from the picker |
| `R` | Rename the selected ordinary list from the picker |
| Space | Toggle the selected task between open and done |
| `c` | Toggle the selected task between open and cancelled |
| `d` or Delete | Move a task to Trash. In Trash, start deleting the selected task forever. In the picker, start deleting the selected list, or start emptying Trash when the Trash entry is selected |
| `D` | Start emptying Trash while Trash is open |
| Alt+Up or `K` | Move an open task before its eligible neighbor, or move the selected list up |
| Alt+Down or `J` | Move an open task after its eligible neighbor, or move the selected list down |
| F2 | Toggle mouse capture |
| F3 | Choose and save the interface language |
| F4 | Show the selected task's plain text for copying |
| F5 | Refresh database contents |
| `?` | Show help with the effective shortcuts |
| `q` or Ctrl+C | Quit |

Keyboard Up/Down wrap, First/Last jump, and PageUp/PageDown page according to the selection requirement below.
Ctrl+C quits only in task navigation, History, and the list picker. Text inputs keep Ctrl+C for Copy selection.
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

### Requirement: Keep the display usable

Husker SHALL keep the selected row visible, handle terminal resizing, and show an explicit empty state when no lists or tasks match the current view.

Task navigation uses the available content width without a side description preview.
The interface shows its current view and active filter.
Action results and errors follow the requirement to expire action messages after five seconds.
Enter opens the task editor at Description. F4 opens plain task text for copying.
A small terminal receives a readable size message instead of causing a panic.

The footer lists hint entries in priority order and packs them into at most two lines at the current width, dropping entries that do not fit. Each entry shows the first configured key and a short label.
Task views list Open, Toggle done, Toggle cancelled, Trash, the four presets as one entry, Undo, Move task, Filter, Lists, New task, History, Back outside All tasks, Help, and Quit.
Trash lists Restore, Delete forever, Empty Trash, and Copy text instead of the task edits. History lists Reopen beside the task edits.
In the task-view section, Help also lists Delete forever with the `trash` keys and Empty Trash with the `empty_trash` keys.
In the list-picker section, Help also lists Empty Trash with the `delete_list` keys.
Help shows the available actions and effective shortcuts for the current configuration in sections: task views, list picker, task editor, text editing, filter and list-name forms, confirmations, and everywhere.
The section for the screen that opened Help appears first. Built-in text-editing keys appear with fixed labels.
Help shows every configured key of an action. Esc closes help without changing stored data.
The configured Up, Down, PageUp, PageDown, First, and Last actions and the mouse wheel scroll help. Mouse clicks in help do nothing.

#### Scenario: Use the full task area

- **WHEN** a user selects a task in a wide terminal
- **THEN** the task rows occupy the available navigation width and no description preview appears beside them

#### Scenario: Scroll through tasks

- **WHEN** a user moves selection below the last visible task
- **THEN** the task list scrolls so the selected task remains visible

#### Scenario: Show an empty filter result

- **WHEN** an applied filter matches no tasks
- **THEN** Husker shows an empty-result message and no task selection

#### Scenario: Show the most-used keys

- **WHEN** All tasks is open in an 80-column English terminal with default bindings
- **THEN** the footer shows `Enter`, `Space`, `c`, `d`, `t/T/w/x`, `u`, `/`, `l`, `n`, `m`, `h`, `?`, and `q` with their labels on two lines

#### Scenario: Show Trash keys

- **WHEN** Trash is open in an 80-column English terminal with default bindings
- **THEN** the footer shows `r Restore`, `d Delete forever`, `D Empty Trash`, and `F4 Copy`
- **AND** the footer does not show `Space Done` or `m Move`

#### Scenario: Open help from the picker

- **WHEN** a user presses `?` in the list picker
- **THEN** the list picker section appears first and the text-editing section lists Ctrl+A with Select the whole field

### Requirement: Display the list picker over task navigation

Husker SHALL display a centered, bounded modal picker over the current task view without replacing that task view with a full-screen list view.
The picker contains All tasks first, ordinary lists in their persistent display order, and Trash last.
All tasks is a virtual entry without a count. Ordinary lists show open-task counts, and Trash shows its deleted-task count.
The lowest-ID default list has a localized default marker, even after renaming.
Picker hints and help make New list, Rename list, and Delete list discoverable with their configured shortcuts.
While the Trash entry is selected, the Delete list hint reads Empty Trash.
Ordinary task navigation shows the configured History shortcut.
Virtual entries have identities separate from ordinary list IDs. An ordinary list named All tasks remains a separate, valid list with its own count.
Opening the picker does not modify task data or the underlying task view's filter, selection, or scroll position.
Picker selection and scrolling are independent from task selection and scrolling.
The picker keeps its selected entry visible and fits the terminal's available area. The existing small-terminal message applies when the interface cannot fit.
Refresh updates picker entries and counts while preserving the selected entry by identity where possible.
Help opened from the picker returns to the picker with its prior selection and scroll position.
All popup labels, hints, and help entries use the selected language and effective configured shortcuts.
The All tasks entry does not add a stored list or change CLI or MCP list results.

#### Scenario: Show the current scope in a popup

- **WHEN** the user opens the picker from Work
- **THEN** Work's entry is selected in the popup while Work's tasks remain visible behind it

#### Scenario: Return from help

- **WHEN** a user opens help from a scrolled picker and closes help
- **THEN** the picker returns to the same selected entry and scroll position

#### Scenario: Keep an ordinary list with the same name

- **WHEN** an ordinary list is named All tasks
- **THEN** the picker shows the virtual All tasks entry first without a count and the ordinary list separately with its count
- **AND** choosing the ordinary entry scopes tasks to that list's ID

#### Scenario: Identify a renamed default list

- **WHEN** the default Inbox list has been renamed to Personal
- **THEN** Personal keeps the localized default marker and cannot be deleted

### Requirement: Confirm list removal while preserving its tasks

Husker SHALL confirm deletion of an ordinary nondefault list before moving its tasks to the default list and deleting only its list metadata.
The picker uses the configurable `delete_list` action, with `d` and Delete as defaults.
All tasks and the lowest-ID default list are protected and never open a deletion confirmation.
The `delete_list` action on the Trash entry opens the Empty Trash confirmation, which the requirement Empty Trash after confirmation defines. The action never deletes Trash as a list.
The confirmation shows the source name, destination name, and separate counts of open, done, cancelled, and Trash tasks from a consistent store preview.
The confirmation explicitly warns when unfinished tasks exist and states that every task moves to the destination.
No is selected by default. Arrow keys, Tab, and Shift+Tab change the choice. Enter accepts the selected choice.
`y` confirms the current preview immediately and `n` keeps the list immediately, in both languages.
Configured Back cancels to the picker. Other typing, paste, save, and task-action keys do not confirm or change task data.
Configured Refresh is available. A changed preview resets the choice to No and explains that the updated operation requires confirmation again.
The write revalidates the preview atomically according to task storage. A changed result displays the fresh preview and requires a new explicit Yes.
A missing list returns to the picker without moving tasks. A failed write preserves data and shows an error without reporting deletion.
Success keeps the picker open and selects the default destination by ID.
If the removed list was the current task scope, the underlying view returns to All tasks using the saved root state.
Removing another list preserves the underlying task scope. Existing editor drafts retain their text and originally read task versions.
The task-storage requirement owns exact field preservation, version increments, and rollback behavior.
This terminal action adds no CLI command or MCP tool.

#### Scenario: Keep a list by default

- **WHEN** Work has three open tasks and two done tasks and the user starts deleting Work
- **THEN** the confirmation names Work and the default destination and shows the task counts with No selected
- **AND** Enter leaves Work and every task unchanged and returns to the picker

#### Scenario: Preserve every task when removing a list

- **WHEN** the user explicitly chooses Yes for the current confirmed Work preview
- **THEN** every Work task moves to the default list, including completed, cancelled, and Trash tasks
- **AND** only Work's list metadata is deleted and the picker selects the default list

#### Scenario: Reconfirm after an external edit

- **WHEN** another connection adds or changes a source task after the deletion preview was read
- **THEN** attempting confirmation makes no deletion, displays a fresh preview, and selects No
- **AND** only a new explicit Yes for that preview can complete deletion

#### Scenario: Protect the default list and All tasks

- **WHEN** the user invokes Delete on All tasks or the default list
- **THEN** no confirmation opens and no stored data changes

#### Scenario: Confirm with a letter

- **WHEN** the deletion confirmation shows the current Work preview and the user presses `y`
- **THEN** Work is deleted as an explicit Yes would delete it
- **AND** pressing `n` on another confirmation returns to the picker without changes

### Requirement: Undo the last task action

Husker SHALL provide a configurable `undo` action, bound to `u` by default, in ordinary task navigation and History, that reverses the most recent recorded task action of the running process.
Recorded actions are done and cancelled toggles, reopen, Trash, restore, a due-date preset that changed the date, a manual reorder, a move to another list, and an editor save.
A new recorded action replaces the previous entry. Undo consumes the entry. There is no redo and no second level.
Undo targets the task by ID and requires the task's current version to equal the version produced by the recorded action.
A different version shows the error `Task {id} changed since the last action. Nothing was undone.` and discards the entry.
A status undo restores the previous status with its previous status-change date and timestamp through the status-restore mutation. A due undo restores the previous date. A Trash or restore undo restores the previous Trash state. A move undo restores the previous list. A reorder undo swaps the two tasks again and requires both of their versions to be current.
An editor-save undo restores the previous title, description, planned date, and list of an existing task. Undoing the creation of a task moves the new task to Trash.
After a successful undo the task remains selected if visible, the message shows Undone, and any hold on that task ends. Without an entry the message shows Nothing to undo.
A successful list deletion, Delete forever, or Empty Trash discards the entry. Permanent deletion is not recorded, because a deleted task cannot be restored.
List creation, renaming, reordering, language changes, and filter changes are not recorded and leave the entry in place.

#### Scenario: Undo a cleared date

- **WHEN** a task planned for `2026-11-10` receives `x` and then `u`
- **THEN** its planned date is `2026-11-10` again and the message shows Undone

#### Scenario: Undo a reopen with the original closure time

- **WHEN** a task completed on `2026-11-01` is reopened from History and the user presses `u`
- **THEN** the task is done with `status_changed_on` `2026-11-01` and its original timestamp

#### Scenario: Refuse undo after an external change

- **WHEN** the user completes task 4, another connection edits task 4, and the user presses `u`
- **THEN** task 4 keeps the external edit and the message names task 4 as changed

#### Scenario: Nothing to undo

- **WHEN** the user presses `u` before any recorded action
- **THEN** no task changes and the message shows Nothing to undo

#### Scenario: Undo an edit and a creation

- **WHEN** the user changes a task's title from `Buy milk` to `Buy bread`, saves, and presses `u`
- **THEN** the stored title is `Buy milk` again with the other fields unchanged
- **AND** creating a task with `n` and pressing `u` moves that new task to Trash

#### Scenario: Nothing to undo after a permanent deletion

- **WHEN** the user moves task 4 to Trash, opens Trash, deletes task 9 forever, returns to All tasks, and presses `u`
- **THEN** task 4 stays in Trash and the message shows Nothing to undo

### Requirement: Apply task actions

Husker SHALL persist a task action before reporting success and refresh visible tasks after the write.

Done toggles `done` to `open` and changes any other status to `done`.
Cancel toggles `cancelled` to `open` and changes any other status to `cancelled`.
The task-storage spec owns status dates and the fields preserved by Trash and restore.
After an open task becomes done or cancelled, selection moves to the next remaining open task in the previous result order.
If no next open task remains, selection moves to the previous open task.
If no open task remains, selection follows the changed task when visible, or clamps the prior index.
Other actions that remove a task from the results clamp the prior selection index.
An empty result has no selected task.
A task closed from an ordinary task view keeps its row according to the hold requirement.
Each successful action in this requirement records an undo entry and shows a localized result: Marked done, Marked cancelled, Reopened, Moved to Trash, Restored, or Updated for a reorder.
Delete forever and Empty Trash follow their own requirements below. They record no undo entry.

#### Scenario: Complete a planned task

- **WHEN** a user selects an open task and presses Space with default shortcuts
- **THEN** its stored status becomes `done` and its status-change date becomes today
- **AND** the task appears in Done today for the rest of the local day after the view changes
- **AND** selection moves to a nearby open task when one remains

#### Scenario: Complete a task under an open-only filter

- **WHEN** a user completes a task while the active filter is `s:open`
- **THEN** the task leaves the results and the next open row is selected, or the previous open row when no next row remains

#### Scenario: Cancel an undated task

- **WHEN** a user cancels an open task in Sometime
- **THEN** the task moves to Cancelled today with its due date still empty after the view changes
- **AND** selection moves to a nearby open task when one remains

#### Scenario: Report the action

- **WHEN** a user presses `d` on an open task and then opens Trash and presses `r`
- **THEN** the messages show Moved to Trash and then Restored


### Requirement: Change language without leaving the current screen

Husker SHALL provide a modal language picker through the configured `language` action, which defaults to F3.
The picker is available from task navigation, History, the navigation list picker, the editor list picker, task editors, filter editors, list-name editors, and Help.
It is unavailable from task-discard confirmation, list-deletion confirmation, Delete forever confirmation, Empty Trash confirmation, and Copy text.
The choices are English and Dansk. The current language is selected when the picker opens.
Configured Up and Down wrap between English and Dansk. First selects English and Last selects Dansk. Apply saves the choice, and Back cancels without saving.
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

## ADDED Requirements

### Requirement: Delete a Trash task forever

While Trash is open, Husker SHALL open a Delete forever confirmation when the user invokes the `trash` action on the selected task.
The `trash` action defaults to `d` and Delete. If no task is selected, the `trash` action does nothing.
The confirmation shows the task title with line breaks replaced by spaces, followed by the task ID, such as `Buy milk #7`. A title wraps inside the dialog. A title that needs more than three lines is shortened with `…` before the ID, so the ID, warning, and choices stay visible.
The confirmation also shows the warning `This cannot be undone.`
No is selected by default. Arrow keys, Tab, and Shift+Tab change the choice. Enter accepts the selected choice.
`y` confirms immediately and `n` cancels immediately, in both languages. Configured Back cancels.
Other typing, paste, save, and task-action keys do not confirm or change task data. Configured Refresh is available.
Cancelling returns to Trash with the same filter and selection.
Yes deletes the task through the task-storage requirement Delete one Trash task permanently. The request uses the task version that the confirmation shows.

| Outcome | Interface result |
| --- | --- |
| `Deleted` | Returns to Trash, clamps the prior selection index, and shows `Deleted forever`. |
| `Changed` | Keeps the confirmation open with the current task, selects No, and shows `The task changed. Review it and confirm again.` |
| `NotInTrash` | Returns to Trash and shows the error `This task is no longer in Trash.` |

A refresh while the confirmation is open deletes no data. If the task is still in Trash with another version, the refresh applies the `Changed` row.
If the task is outside Trash or no longer exists, the refresh applies the `NotInTrash` row.
A failed write preserves data and shows an error without reporting deletion.

#### Scenario: Keep a task by default

- **WHEN** a user selects task 7 in Trash, presses `d`, and presses Enter
- **THEN** the confirmation closes and task 7 remains in Trash

#### Scenario: Delete a task forever

- **WHEN** a user selects task 7 in Trash, presses `d`, and presses `y`
- **THEN** task 7 no longer exists, Trash no longer lists task 7, and the message shows `Deleted forever`

#### Scenario: Reconfirm after an external edit

- **WHEN** the confirmation for task 7 is open and another connection changes task 7's title in Trash
- **THEN** after the next refresh the confirmation shows the new title with No selected
- **AND** only a new explicit Yes deletes task 7

#### Scenario: Close after an external restore

- **WHEN** the confirmation for task 7 is open and another connection restores task 7
- **THEN** after the next refresh the confirmation closes, task 7 keeps every field, and the message shows `This task is no longer in Trash.`

### Requirement: Empty Trash after confirmation

Husker SHALL provide a configurable `empty_trash` action, bound to `D` by default, that opens an Empty Trash confirmation while Trash is open.
The `delete_list` action on the Trash entry of the list picker opens the same confirmation.
The `empty_trash` action does nothing in other task views.
If Trash is empty, `empty_trash` in Trash and `delete_list` on the Trash entry show `Trash is empty` and open no confirmation.
The Empty Trash confirmation shows the number of tasks in Trash from a Trash preview, as defined by the task-storage requirement Empty Trash from a confirmed preview.
The confirmation states that every task in Trash is deleted, including tasks that the current filter hides. It also shows the warning `This cannot be undone.`
Choice keys, Back, Refresh, and ignored keys work as in the requirement Delete a Trash task forever.
Cancelling returns to the screen that opened the confirmation: Trash with its filter and selection, or the picker with Trash selected.
Yes empties Trash through the task-storage operation. The request uses the Trash preview that the confirmation shows.

| Outcome | Interface result |
| --- | --- |
| `Emptied` | Returns to the screen that opened the confirmation and shows `Trash emptied`. The picker keeps Trash selected. |
| `Changed` with tasks in Trash | Keeps the confirmation open with the new count, selects No, and shows `Trash changed. Review the count and confirm again.` |
| `Changed` with an empty Trash | Returns to the screen that opened the confirmation and shows `Trash is empty` as an error, so the external update notice does not hide it. |

A refresh while the confirmation is open deletes no data. If Trash no longer matches the Trash preview that the confirmation shows, the refresh applies the matching `Changed` row.
A failed write preserves data and shows an error without reporting deletion.

#### Scenario: Empty Trash from Trash

- **WHEN** Trash is open and holds three tasks, and the user presses `D` and then `y`
- **THEN** none of the three tasks exists, Trash shows no tasks, and the message shows `Trash emptied`

#### Scenario: Empty Trash from the picker

- **WHEN** the picker has Trash selected, Trash holds two tasks, and the user presses `d` and chooses Yes
- **THEN** both tasks no longer exist and the picker stays open with Trash selected

#### Scenario: Count tasks hidden by a filter

- **WHEN** Trash holds five tasks, the filter `milk` shows two of them, and the user presses `D`
- **THEN** the confirmation shows 5 tasks

#### Scenario: Reconfirm a changed Trash

- **WHEN** the Empty Trash confirmation shows 2 tasks and another connection moves a task to Trash
- **THEN** after the next refresh the confirmation shows 3 tasks with No selected
- **AND** no task is deleted until the user confirms again

#### Scenario: Ignore Empty Trash outside Trash

- **WHEN** a user presses `D` in All tasks
- **THEN** no confirmation opens and no task changes

#### Scenario: Report an empty Trash

- **WHEN** Trash is empty and the user presses `D` in Trash or `d` on the Trash picker entry
- **THEN** no confirmation opens and the message shows `Trash is empty`

