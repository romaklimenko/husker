## ADDED Requirements

### Requirement: Hold a just-closed task in its row

Husker SHALL keep a task that the user marks done or cancelled from an ordinary task view in its current row and under its previous heading until the view changes.
The held row shows the new status marker and is drawn dimmed. Selection moves as defined by the task-action requirement.
Several held rows keep their relative order, including a row held directly below another held row.
The heading is the one the task would have while open. A held task under Earlier keeps its planned-date suffix. Other held rows omit it.
The hold ends when the user changes the task view, applies or clears a filter, opens History, opens the task editor, creates a task, refreshes manually, or when the local date changes.
The hold also ends when the task's version changes again, the task becomes open, or the task leaves the visible results.
Opening the list picker, the move-task picker, Help, the language picker, or the copy view does not end the hold.
Holding changes no stored data, no ordering outside the current terminal view, and no CLI or MCP result.
History and Trash never hold rows.

#### Scenario: Keep a completed task under Today

- **WHEN** Today lists tasks 3, 4, and 5 and the user presses Space on task 4
- **THEN** task 4 stays between tasks 3 and 5 under Today with an `[x]` marker and task 5 becomes selected
- **AND** the stored task is done and a CLI search lists it in Done today

#### Scenario: Release the hold when leaving the view

- **WHEN** task 4 is held under Today and the user opens Work from the picker and returns to All tasks
- **THEN** task 4 appears under Done today

#### Scenario: Release the hold under an open-only filter

- **WHEN** the filter is `s:open` and the user presses Space on task 4
- **THEN** task 4 leaves the results immediately

#### Scenario: Release the hold after another change

- **WHEN** task 4 is held and another connection edits task 4
- **THEN** the next refresh shows task 4 under Done today

### Requirement: Move the selected task to another list

Husker SHALL provide a configurable `move_task` action, bound to `m` by default, in ordinary task navigation and History outside Trash.
The action opens a modal picker over the current task view containing only ordinary lists in their persistent order with the default marker. The task's current list is selected.
The picker uses the editor list picker key mode. Up, Down, First, Last, PageUp, and PageDown move its selection. Open chooses. Back or Lists cancels without a write.
Help, Language, Refresh, and mouse behavior follow the editor list picker requirement. The underlying task view receives no popup input.
Choosing the task's current list closes the picker without a write.
Choosing another list changes only the task's list membership through the list-only mutation defined by task storage, using the originally read version.
After a successful move the message shows Moved to and the destination name. The same task remains selected if it is still visible. Otherwise selection clamps to the prior index.
The move records an undo entry with the previous list. A stale version returns an error without changing data.
No selected task, or a task in Trash, means no action.

#### Scenario: Move a task from All tasks

- **WHEN** task 4 in Inbox is selected in All tasks and the user presses `m`, selects Work, and presses Enter
- **THEN** task 4 belongs to Work with its other fields unchanged and its row shows `[Work]`
- **AND** the message shows Moved to Work and task 4 remains selected

#### Scenario: Cancel a move

- **WHEN** the move-task picker is open and the user presses Esc or `l`
- **THEN** the picker closes and the task's list and version remain unchanged

#### Scenario: Move a task out of its scoped view

- **WHEN** the user moves the only task in Work to Home while viewing Work
- **THEN** Work shows no tasks and the task appears in Home

### Requirement: Undo the last task action

Husker SHALL provide a configurable `undo` action, bound to `u` by default, in ordinary task navigation and History, that reverses the most recent recorded task action of the running process.
Recorded actions are done and cancelled toggles, reopen, Trash, restore, a due-date preset that changed the date, a manual reorder, a move to another list, and an editor save.
A new recorded action replaces the previous entry. Undo consumes the entry. There is no redo and no second level.
Undo targets the task by ID and requires the task's current version to equal the version produced by the recorded action.
A different version shows the error `Task {id} changed since the last action. Nothing was undone.` and discards the entry.
A status undo restores the previous status with its previous status-change date and timestamp through the status-restore mutation. A due undo restores the previous date. A Trash or restore undo restores the previous Trash state. A move undo restores the previous list. A reorder undo swaps the two tasks again and requires both of their versions to be current.
An editor-save undo restores the previous title, description, planned date, and list of an existing task. Undoing the creation of a task moves the new task to Trash.
After a successful undo the task remains selected if visible, the message shows Undone, and any hold on that task ends. Without an entry the message shows Nothing to undo.
A successful list deletion discards the entry. List creation, renaming, reordering, language changes, and filter changes are not recorded and leave the entry in place.

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

### Requirement: Omit planned-date suffixes under dated headings

Husker SHALL append the planned-date label to a task row only where the row's heading does not already state that date.
Rows show the suffix in Trash, under Earlier, and in the Done today, Cancelled today, Done, and Cancelled sections.
Rows omit the suffix under Today, Tomorrow, exact-date headings, and Sometime. History rows omit it as defined by the History requirement.
A held row follows the heading it keeps. The stored due date, filters, editor fields, and copy text are unchanged.

#### Scenario: Hide a repeated date

- **WHEN** an open task is planned for `2026-11-10` and today is `2026-11-08`
- **THEN** its row under the `2026-11-10 Tue W46` heading shows the title without a date suffix

#### Scenario: Keep informative dates

- **WHEN** an open task planned for `2026-11-01` appears under Earlier, and a task planned for `2026-11-10` is completed today
- **THEN** both rows end with their planned-date labels

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
| `d` or Delete | Move a task to Trash, or start deleting the selected list in the picker |
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
Task creation, editing, completion, cancellation, deletion, task reordering, moving to another list, and undo are unavailable in Trash. Restore is available only in Trash.
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

### Requirement: Edit task fields

Husker SHALL provide a task editor with these fields and save all valid changes together when the user invokes Save.
Default Save shortcuts are Ctrl+S and Ctrl+Enter. Esc and Ctrl+G invoke Discard with the confirmation rules defined below.

| Field | Input |
| --- | --- |
| Title | A nonempty task title |
| Description | Plain text with multiple lines |
| Planned date | `YYYY-MM-DD`, `YYYYMMDD`, `YYMMDD`, `MMDD`, or empty for no date |
| List | A read-only selector for an existing ordinary list ID, displayed by its current name |

Tab selects the next field. Shift+Tab selects the previous field.
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

### Requirement: Create tasks and lists

Husker SHALL create tasks and lists through input forms without requiring the user to leave the terminal interface.

The new-task action opens the editor with an empty title, description, and due date. New-task actions are unavailable in History and Trash.
A list-scoped task view supplies its list. All tasks supplies the lowest-ID ordinary list.
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
Each successful action records an undo entry and shows a localized result: Marked done, Marked cancelled, Reopened, Moved to Trash, Restored, or Updated for a reorder.

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

### Requirement: Filter tasks interactively

Husker SHALL let the user enter a task filter without leaving the current task view.

The filter action opens a single-line input. Enter applies a valid filter and returns to navigation. Esc discards filter edits.
While the input is open, the footer shows the configured Apply, Back, and Copy selection keys and the filter syntax hint.
An empty filter removes text restrictions while preserving any list scope and the default status rules.
Back in navigation with a nonempty applied filter clears that filter, keeps the current view, and keeps the selected task when it is still visible. It does not leave the view.
A list-scoped view combines its list scope with the filter.
History combines its retained list scope and closed-task restriction with the filter.
An empty History filter includes completed and cancelled tasks from every date.
An explicit `s:done` or `s:cancelled` narrows History. `s:open` matches nothing there, and `s:all` does not add open tasks.
Due filters always test stored due dates, including in finished sections and History.
Invalid syntax keeps the input open with an error and leaves previous results unchanged.
The `planning-search` spec defines accepted syntax.

#### Scenario: Combine list and text filters

- **WHEN** a user applies `l:private milk #shopping` in the All tasks
- **THEN** Husker shows only matching tasks according to the `planning-search` spec

#### Scenario: Correct an invalid filter

- **WHEN** a user applies a filter with an unterminated quoted value
- **THEN** Husker displays a filter error and keeps the entered filter available for editing
- **AND** the previously applied results remain unchanged

#### Scenario: Filter History without adding open tasks

- **WHEN** a user applies `s:all milk` in Work's History
- **THEN** results contain only Work's non-Trash done or cancelled tasks matching `milk`, regardless of completion date

#### Scenario: Clear a filter with Back

- **WHEN** Work is filtered by `milk` with task 4 selected and the user presses Esc
- **THEN** Work shows all its tasks with task 4 selected and the filter box is gone
- **AND** a second Esc returns to All tasks

### Requirement: Reorder comparable tasks

Husker SHALL reorder a selected task only within the ordering group defined by the `planning-search` spec.

Manual reordering is available only for open tasks outside History and Trash.
The default shortcuts are Alt+Up or `K` to move before the neighbor and Alt+Down or `J` to move after it.
Dated tasks can move only relative to other open tasks with the same exact due date.
Undated open tasks can move relative to other undated open tasks.
Moving a task does not change its due date, list, or status.
The operation uses adjacent matching rows in the current results. A held closed row is skipped when looking for the neighbor.
At the beginning or end of eligible rows, the move leaves the order unchanged.

#### Scenario: Keep dates unchanged

- **WHEN** the selected task is due `2026-11-09` and its preceding visible task is due `2026-11-08`
- **THEN** Alt+Up leaves the selected task's date and order unchanged

#### Scenario: Keep open and closed tasks separate

- **WHEN** a selected task is done or cancelled, including in History
- **THEN** Alt+Up leaves the selected task's position and status unchanged

### Requirement: Keep the display usable

Husker SHALL keep the selected row visible, handle terminal resizing, and show an explicit empty state when no lists or tasks match the current view.

Task navigation uses the available content width without a side description preview.
The interface shows its current view and active filter.
Action results and errors follow the requirement to expire action messages after five seconds.
Enter opens the task editor at Description. F4 opens plain task text for copying.
A small terminal receives a readable size message instead of causing a panic.

The footer lists hint entries in priority order and packs them into at most two lines at the current width, dropping entries that do not fit. Each entry shows the first configured key and a short label.
Task views list Open, Toggle done, Toggle cancelled, Trash, the four presets as one entry, Undo, Move task, Filter, Lists, New task, History, Back outside All tasks, Help, and Quit. Trash lists Restore and Copy text instead of the task edits. History lists Reopen beside the task edits.
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

#### Scenario: Open help from the picker

- **WHEN** a user presses `?` in the list picker
- **THEN** the list picker section appears first and the text-editing section lists Ctrl+A with Select the whole field

### Requirement: Confirm discarding changed task fields

Husker SHALL ask for confirmation when Discard would close a task editor whose text fields or selected list ID differ from their initial values.
The rule applies to existing tasks and unsaved new tasks. Cursor movement, selection, and a change fully undone back to the initial text and list ID do not make the editor changed.
An unchanged editor closes immediately when Discard is invoked.
The confirmation initially selects No, which returns to editing without losing text, cursor positions, selections, or histories.
Arrow keys, Tab, or Shift+Tab change the selected No or Yes choice. Enter applies that choice.
`y` discards immediately and `n` returns to editing immediately, in both languages.
The configured Discard action, including Esc and Ctrl+G by default, closes the prompt and returns to the unchanged draft.
Only an explicit Yes closes the changed editor and discards its draft.
Typing, pasting, and Save do not change or save the draft while the prompt is open.
Refresh remains available and preserves the draft, its initially read version, and the confirmation choice.
Filter and list-name forms keep their existing immediate cancellation behavior.

#### Scenario: Keep a changed task by default

- **WHEN** the user edits Description, presses Esc, and presses Enter without changing the confirmation choice
- **THEN** the editor returns with the complete draft and no database write

#### Scenario: Discard a new task explicitly

- **WHEN** the user enters a new title, invokes Discard, selects Yes, and presses Enter
- **THEN** the editor closes without creating a task

#### Scenario: Leave an unchanged editor immediately

- **WHEN** the user only moves the cursor or undoes all text changes and invokes Discard
- **THEN** the editor closes without a confirmation prompt

#### Scenario: Preserve a draft during an external change

- **WHEN** a discard prompt is open and another connection changes the same task before a refresh
- **THEN** the prompt and draft remain intact with the originally read task version
- **AND** choosing No returns to that draft without bypassing the later save's version check

#### Scenario: Answer with a letter

- **WHEN** the discard prompt is open and the user presses `n`, then invokes Discard again and presses `y`
- **THEN** the first key returns to the unchanged draft and the second key discards it

### Requirement: Confirm list removal while preserving its tasks

Husker SHALL confirm deletion of an ordinary nondefault list before moving its tasks to the default list and deleting only its list metadata.
The picker uses the configurable `delete_list` action, with `d` and Delete as defaults.
All tasks, Trash, and the lowest-ID default list are protected and never open a deletion confirmation.
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

#### Scenario: Protect default and virtual entries

- **WHEN** the user invokes Delete on All tasks, Trash, or the default list
- **THEN** no confirmation opens and no stored data changes

#### Scenario: Confirm with a letter

- **WHEN** the deletion confirmation shows the current Work preview and the user presses `y`
- **THEN** Work is deleted as an explicit Yes would delete it
- **AND** pressing `n` on another confirmation returns to the picker without changes

### Requirement: Navigate All tasks and lists

Husker SHALL open All tasks as the top-level task view and use a modal list picker to choose an ordinary list or Trash.
The global view heading is `All tasks`. A selected list uses a breadcrumb such as `All tasks > Work`; Trash uses `All tasks > Trash`.
The `lists` action opens the picker from any task view. Invoking `lists` again or Back while the picker is open cancels it.
Canceling the picker preserves the underlying view, filter, selected task, and scroll position.
The initial picker selection matches the current scope: the ordinary list by ID, or Trash.
From All tasks or all-list History, the picker selects the ordinary list most recently visited in the running process when it still exists, and All tasks otherwise.
Choosing the current scope closes the picker without resetting task state.
Choosing a different ordinary list or Trash starts that view with an empty filter and its first task selected, if present.
Back with an active filter clears the filter as defined by the filter requirement. Back without a filter from an ordinary list or Trash returns to All tasks. Back at All tasks without a filter leaves it unchanged.
The `all_tasks` action and choosing All tasks also return to All tasks with the saved parent state defined below.
All tasks groups, task ordering, and filter matching follow the `planning-search` spec. Trash shows deleted tasks without planned groups.
Editor save and dirty-discard confirmation keep their existing behavior and return to the task view from which the editor opened.

#### Scenario: Open a list and return to All tasks

- **WHEN** a user in All tasks presses `l`, selects `Work`, and presses Enter with default shortcuts
- **THEN** Husker closes the picker and shows `All tasks > Work` with tasks from Work
- **AND** pressing Esc returns to All tasks instead of opening the picker

#### Scenario: Cancel a list choice

- **WHEN** a user opens the picker from a filtered list view, moves its selection, and presses Esc or `l`
- **THEN** the picker closes and the original task filter, selection, and scroll position remain unchanged

#### Scenario: Keep state when choosing the current list

- **WHEN** a user opens the picker from Work and chooses Work again
- **THEN** the picker closes without resetting Work's filter, selected task, or scroll position

#### Scenario: Open Trash

- **WHEN** a user chooses the last picker entry, Trash
- **THEN** Husker shows `All tasks > Trash` with deleted tasks, no planning-group headings, and an empty filter
- **AND** Esc returns to All tasks

#### Scenario: Preselect the last visited list

- **WHEN** a user visits Work, presses Esc to return to All tasks, and presses `l`
- **THEN** the picker opens with Work selected
- **AND** after Work is deleted elsewhere the picker opens with All tasks selected

History is a separate task view defined below. Choosing its underlying task scope in the picker restores its saved origin state.
Choosing another ordinary list exits History and starts that list with an empty filter and its first task selected.
Choosing All tasks exits History and restores the saved root state.
If the current ordinary list is deleted locally or externally, Husker returns to All tasks using the saved root state.
An open task editor keeps its draft when its list is deleted.
A later stale save cannot overwrite the transferred task. List-name validation or task-version validation can reject that save.

#### Scenario: Leave History through its current list

- **WHEN** History was opened from Work and the user chooses Work in the picker
- **THEN** Husker leaves History and restores Work's original filter, task selection, and scroll position

#### Scenario: Recover when the current list is removed

- **WHEN** Work is the current list and another connection deletes Work
- **THEN** Husker returns to All tasks with the saved root state after refreshing
- **AND** any open task editor keeps its entered text and originally read version

### Requirement: Review retained completed and cancelled tasks

Husker SHALL provide History for the current ordinary list or All tasks through the configured `history` action, which defaults to `h`.
History includes all currently done or cancelled non-Trash tasks in that scope, including today and unknown status dates.
History follows the date, timestamp, and ID ordering defined by planning-search.
Tasks completed later on the same local day appear first when their timestamps are known. Unknown completion times are not reconstructed.
History groups tasks under localized Today, Yesterday, calendar-date, or Unknown date headings based on `status_changed_on`.
The display date remains the local calendar date even though same-day ordering uses a UTC timestamp.
The headings identify each task's recorded status date.
History rows do not append the stored planned date as metadata. Task titles, including dates typed into titles, remain unchanged.
The planned date remains available in the editor and retains its row suffix in other task views where the row-suffix requirement shows it.
Unknown closure dates remain Unknown date even when a task has a known planned date.
History uses breadcrumbs such as `All tasks > History` and `All tasks > Work > History`.
History is a view of retained task records. It does not claim to preserve every status transition, edit, or previous task text.
Reopening clears the task's status date and timestamp according to task storage and removes the task from History after the write succeeds.
History starts with an empty filter and remembers the origin view, filter, parsed query, task ID, selection index, and scroll state.
The `history` action restores that origin against fresh data, preserving task identity when present and clamping the prior index otherwise.
Back with an active History filter clears the filter first. Back without a filter restores the origin in the same way as the `history` action.
The `all_tasks` action returns to the saved root task state. Choosing a picker entry follows the navigation requirement.
History supports filtering, editing, copying, status changes, task Trash, moving tasks to another list, undo, reopening, and reading task descriptions.
Status changes refresh History after the write. Tasks that become open or move to Trash leave its results.
Its empty state explains that no completed or cancelled tasks match and identifies the configured return shortcut.

#### Scenario: Find an old completion

- **WHEN** Work contains a non-Trash task completed last month and the user presses `h` from Work
- **THEN** History shows that task with its recorded status date
- **AND** unrelated lists, open tasks, and Trash tasks are absent

#### Scenario: Keep legacy tasks with unknown dates

- **WHEN** a migrated completed task has no status-change date
- **THEN** History includes it after all tasks with recorded dates and labels the date as unknown

#### Scenario: Reopen from History

- **WHEN** the user selects a done or cancelled task in History and invokes `reopen`
- **THEN** the task becomes open, its status date and timestamp are cleared, and it leaves History
- **AND** Back restores the originating task view with current database contents

#### Scenario: Return to a filtered task view

- **WHEN** task 42 is selected in a scrolled Work view filtered by `milk`, the user opens History, and then presses Esc or `h`
- **THEN** Work returns with filter `milk`, task 42 selected if still visible, and the saved scroll position within current bounds

#### Scenario: Distinguish the completion day from the planned day

- **WHEN** a task planned for November 12 was completed on November 9 and is shown in History
- **THEN** its heading uses the November 9 status date and its row has no November 12 planned-date suffix
- **AND** opening the editor still shows November 12 as Planned date

#### Scenario: Keep an unknown closure date unknown

- **WHEN** a completed task has a planned date but no recorded status date
- **THEN** History places it under Unknown date without appending the planned-date suffix or inventing a closure day

#### Scenario: Clear a History filter with Back

- **WHEN** History is filtered by `milk` and the user presses Esc
- **THEN** History shows all its tasks and remains open
- **AND** a second Esc restores the origin view

### Requirement: Apply due-date presets to the selected task

Husker SHALL apply a due-date preset immediately to the originally selected task in ordinary task navigation or History, outside Trash.
The planning-search spec defines the resolved date. The task-storage due-only mutation owns access checks, version validation, field preservation, and no-op behavior.
The action uses the selected task's ID and originally read version. No selected task means no action.
A stale task returns an error without overwriting newer data. A no-op makes no database write or version increment.
After success, the same task remains selected by ID if it still matches the current view and filter.
If it no longer matches, selection clamps the prior index to remaining results. An empty result has no selection.
Presets change no title, description, list, status, status-change date and timestamp, Trash state, or saved position.
Closed tasks remain in their existing status group or History unless a filter excludes their changed due date.
At midnight, the preset uses the new local date but retains the task identity and version from before result refresh.
A changed date shows Planned for followed by the new date label. A cleared date shows Planned date cleared and the previous date label. A no-op shows no message.
A changed date records an undo entry with the previous date.
Errors and action results use the selected language and existing message-lifetime rules.
The four actions appear in Help with configured shortcuts and as one footer entry in task views.

#### Scenario: Move an open task to Tomorrow

- **WHEN** an undated open task is selected on `2026-11-08` and the user presses `T` with default bindings
- **THEN** the stored due date becomes `2026-11-09` and the task moves to Tomorrow
- **AND** that same task remains selected and the message shows Planned for `2026-11-09 Mon W46`

#### Scenario: Leave a due-date filter

- **WHEN** a selected task matches `due:none` and the user invokes Today
- **THEN** its date changes and it leaves the filtered results
- **AND** selection clamps the previous index or becomes empty when no results remain

#### Scenario: Change a date in History

- **WHEN** a done task in History receives Tomorrow
- **THEN** only its due date and version change
- **AND** its status date, timestamp, and History ordering remain unchanged when the active filter still matches

#### Scenario: Keep the original target across midnight

- **WHEN** a selected completed task would leave default results at midnight and the user invokes Today after midnight
- **THEN** the preset targets that task's original ID and version using the new day's date
- **AND** it does not change another task that occupies the same row after refresh

#### Scenario: Ignore a preset in Trash

- **WHEN** a user presses `t`, `T`, `w`, or `x` while Trash is open with default bindings
- **THEN** no task date, status, or version changes

#### Scenario: Report a cleared date

- **WHEN** a task planned for `2026-11-10` receives `x`
- **THEN** the message shows Planned date cleared with `2026-11-10 Tue W46`
- **AND** pressing `x` on an undated task shows no message and writes nothing

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

Up on the first item selects the last item. Down on the last item selects the first item.
First selects the first item and Last selects the last item without changing result order or applying a picker choice.
Default keys are Up or `k` and Down or `j`.
First defaults to Home, `g`, and Left. Last defaults to End, `G`, and Right.
Configured PageUp and PageDown move the selection by the number of selectable rows last displayed in that selection area, not counting heading rows, clamped at the first or last item without wrapping. They are unavailable in the language picker.
In Help, Up, Down, PageUp, PageDown, First, and Last scroll the text by one row, one viewport, or to an end. In Copy text, PageUp and PageDown scroll by one viewport.
An empty result has no selection and each movement action is a no-op. One item stays selected after every movement action.
Group headings remain unselectable and do not count as items.
Selection uses the filtered visible results. After wrapping, jumping, or paging, the selected target is visible within the existing scrollable layout.
Selection does not save tasks, choose a list, change the interface language, or alter task and list positions.
Refresh retains its existing identity-preservation and missing-selection clamping rules. It does not wrap a removed selection.
Mouse wheels remain bounded. Text cursor movement, Help and Copy text scrolling, and reordering retain their bounded behavior.
First and Last have no selection action in text fields, the closed List selector, Copy text, or confirmation screens.
Left and Right retain text-cursor movement in text inputs and horizontal scrolling in Copy text.

#### Scenario: Wrap a filtered task list without selecting headings

- **WHEN** a query shows tasks 3 and 8 under separate date headings and task 3 is selected
- **THEN** Up selects task 8 and makes its row visible
- **AND** Down returns to task 3 without selecting either heading

#### Scenario: Jump through a long task view

- **WHEN** a task view has more rows than fit on screen and the user invokes Last then First
- **THEN** the last visible-result task and then the first visible-result task are selected and scrolled into view

#### Scenario: Wrap History and Trash

- **WHEN** the last visible task is selected in History or Trash and the user invokes Down
- **THEN** the first visible task becomes selected without changing any task data

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

### Requirement: Copy plain task text

Husker SHALL provide an F4 copy-text view from task navigation and the task editor.
The view displays the task title, a blank line, and description without borders or inserted wrapping characters.

The editor opens this view with its current draft text, including unsaved changes.
Mouse capture is disabled while the view is open so terminal-native selection can copy multiline text without border glyphs.
Arrow keys scroll vertically and horizontally without changing task data. PageUp and PageDown scroll vertically by one viewport.
Esc returns to the previous screen, preserves the editor draft when present, and restores the earlier mouse-capture setting.

#### Scenario: Copy multiline description text

- **WHEN** a task description contains `First line` and `Second line` separated by a newline and the user opens F4
- **THEN** the copy view contains both lines without a vertical border character at either line edge

#### Scenario: Preserve an unsaved draft

- **WHEN** a user opens F4 from an edited description and presses Esc
- **THEN** the editor returns with the same unsaved text and no database write occurs
