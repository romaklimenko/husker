## RENAMED Requirements

- FROM: `### Requirement: Navigate lists and planned tasks`
- TO: `### Requirement: Navigate All tasks and lists`
- FROM: `### Requirement: Restore Planned after visiting a scoped view`
- TO: `### Requirement: Restore All tasks after visiting a scoped view`

## MODIFIED Requirements

### Requirement: Navigate All tasks and lists

Husker SHALL open All tasks as the top-level task view and use a modal list picker to choose an ordinary list or Trash.
The global view heading is `All tasks`. A selected list uses a breadcrumb such as `All tasks > Work`; Trash uses `All tasks > Trash`.
The `lists` action opens the picker from any task view. Invoking `lists` again or Back while the picker is open cancels it.
Canceling the picker preserves the underlying view, filter, selected task, and scroll position.
The initial picker selection matches the current scope: All tasks, the ordinary list by ID, or Trash.
Choosing the current scope closes the picker without resetting task state.
Choosing a different ordinary list or Trash starts that view with an empty filter and its first task selected, if present.
Back from an ordinary list or Trash returns to All tasks. Back at All tasks leaves it unchanged.
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

### Requirement: Use keyboard actions

Husker SHALL provide the following default shortcuts in task navigation and the list picker and resolve configurable action shortcuts through the selected configuration.

| Key | Action |
| --- | --- |
| Up or `k` | Select the previous task or picker entry |
| Down or `j` | Select the next task or picker entry |
| `l` | Open or cancel the list picker |
| `h` | Open History from ordinary task navigation, or return from History |
| `/` | Edit the task filter |
| Enter | Choose the picker entry or edit the selected task |
| Esc | Cancel the picker, or return from a scoped task view to All tasks |
| `n` | Create a task from a task view |
| `N` | Create a list from the picker |
| `R` | Rename the selected ordinary list from the picker |
| Space | Toggle the selected task between open and done |
| `c` | Toggle the selected task between open and cancelled |
| `d` or Delete | Move a task to Trash, or start deleting the selected list in the picker |
| `r` | Restore the selected task from Trash, or reopen it from History |
| Alt+Up | Move an open task before its eligible neighbor, or move the selected list up |
| Alt+Down | Move an open task after its eligible neighbor, or move the selected list down |
| F2 | Toggle mouse capture |
| F3 | Choose and save the interface language |
| F4 | Show the selected task's plain text for copying |
| F5 | Refresh database contents |
| `?` | Show help with the effective shortcuts |
| `q` | Quit |

Actions that require a selected task do nothing when no task is selected.
Creating, renaming, moving, or deleting a list requires the picker. All tasks and Trash cannot be renamed, moved, or deleted.
The default ordinary list cannot move or be deleted. Other list moves follow the pinned boundaries defined by task storage.
Task creation, editing, completion, cancellation, deletion, and task reordering are unavailable in Trash. Restore is available only in Trash.
Task actions do not affect background tasks while the picker is open.
The `profiles-localization` spec owns configuration syntax and binding validation. The optional `all_tasks` action and legacy `planned` alias follow that spec's compatibility rules.
The `all_tasks` action has no default shortcut. Back and the All tasks picker entry provide normal root navigation.
History is unavailable from Trash and the picker.
History supports opening, filtering, copying, status changes, moving tasks to Trash, reopening, help, refreshing, and navigation.
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

### Requirement: Create tasks and lists

Husker SHALL create tasks and lists through input forms without requiring the user to leave the terminal interface.

The new-task action opens the editor with an empty title, description, and due date. New-task actions are unavailable in History and Trash.
A list-scoped task view supplies its list. All tasks supplies the lowest-ID ordinary list.
Saving an empty or invalid title leaves the editor open. Discarding a new task creates no stored task.
A valid new list appears after existing ordinary lists and before Trash in the picker.
Renaming a list preserves its position. A successful create or rename returns to the picker with the saved list selected by ID.
Canceling the list-name form returns to the picker with its prior selected entry and scroll position.
Creating or renaming a list does not change the task view beneath the picker.

#### Scenario: Capture a task in a list

- **WHEN** a user chooses Private, presses `n`, enters `Buy milk`, and presses Ctrl+S
- **THEN** Husker creates `Buy milk` in Private
- **AND** the task has no due date

#### Scenario: Create and rename a list

- **WHEN** a user presses `N` in the picker, enters `Errands`, and saves
- **THEN** the picker reappears with Errands immediately before Trash and selected
- **AND** pressing `R`, entering `Shopping`, and saving returns to the picker with that same list selected in the same position

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

#### Scenario: Complete a planned task

- **WHEN** a user selects an open task and presses Space with default shortcuts
- **THEN** its stored status becomes `done` and its status-change date becomes today
- **AND** the task appears in Done today for the rest of the local day
- **AND** selection moves to a nearby open task when one remains

#### Scenario: Complete a task under an open-only filter

- **WHEN** a user completes a task while the active filter is `s:open`
- **THEN** the task leaves the results and the next open row is selected, or the previous open row when no next row remains

#### Scenario: Cancel an undated task

- **WHEN** a user cancels an open task in Sometime
- **THEN** the task moves to Cancelled today with its due date still empty
- **AND** selection moves to a nearby open task when one remains

### Requirement: Filter tasks interactively

Husker SHALL let the user enter a task filter without leaving the current task view.

The filter action opens a single-line input. Enter applies a valid filter and returns to navigation. Esc discards filter edits.
An empty filter removes text restrictions while preserving any list scope and the default status rules.
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

### Requirement: Reorder comparable tasks

Husker SHALL reorder a selected task only within the ordering group defined by the `planning-search` spec.

Manual reordering is available only for open tasks outside History and Trash.
Dated tasks can move only relative to other open tasks with the same exact due date.
Undated open tasks can move relative to other undated open tasks.
Moving a task does not change its due date, list, or status.
The operation uses adjacent matching rows in the current results.
At the beginning or end of eligible rows, the move leaves the order unchanged.

#### Scenario: Keep dates unchanged

- **WHEN** the selected task is due `2026-11-09` and its preceding visible task is due `2026-11-08`
- **THEN** Alt+Up leaves the selected task's date and order unchanged

#### Scenario: Keep open and closed tasks separate

- **WHEN** a selected task is done or cancelled, including in History
- **THEN** Alt+Up leaves the selected task's position and status unchanged

### Requirement: Planning headings are separate from selection

Husker SHALL render open-task date headings and finished-task section headings independently from task rows.
Selecting the first task in a group does not highlight the heading or draw a selection marker on it.
Keyboard movement visits task rows only. Mouse clicks on a heading do nothing.
Scrolling and mouse hit testing use the same rendered heading and task rows.

#### Scenario: Select the first task in a group

- **WHEN** Today contains one task and that task is selected
- **THEN** the task row receives selection styling and the Today heading keeps its heading styling

#### Scenario: Skip a group heading

- **WHEN** selection moves from the last task in Today to the first task in Tomorrow
- **THEN** the Tomorrow heading is never selected
- **AND** clicking the Tomorrow heading does not change selection

#### Scenario: Keep a finished section heading inert

- **WHEN** the first task under Done today is selected
- **THEN** only the task receives selection styling
- **AND** clicking Done today does not change selection

### Requirement: Display the list picker over task navigation

Husker SHALL display a centered, bounded modal picker over the current task view without replacing that task view with a full-screen list view.
The picker contains All tasks first, ordinary lists in their persistent display order, and Trash last.
All tasks is a virtual entry without a count. Ordinary lists show open-task counts, and Trash shows its deleted-task count.
The lowest-ID default list has a localized default marker, even after renaming.
Picker hints and help make New list, Rename list, and Delete list discoverable with their configured shortcuts.
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

### Requirement: Restore All tasks after visiting a scoped view

Husker SHALL retain the All tasks filter, parsed query, selected task ID, selection index, and task scroll state when leaving All tasks for an ordinary list or Trash.
Switching between ordinary lists or Trash preserves that original All tasks state.
Back from a scoped view, the `all_tasks` action, and choosing All tasks restore that state against fresh database contents.
If the saved selected task is still visible, it becomes selected again by ID.
If the saved task is no longer visible, Husker clamps the saved selection index to the current result count.
An empty result has no selected task. Rendering clamps the restored scroll position to the available rows and keeps the selected task visible.
Invoking `all_tasks` while already at All tasks preserves the current filter, selection, and scroll position.
The saved All tasks state lasts only for the running process.

#### Scenario: Restore a filtered and scrolled All tasks view

- **WHEN** task 42 is selected in a scrolled All tasks view filtered by `milk`, the user visits Work, and then presses Esc
- **THEN** All tasks returns with filter `milk`, task 42 selected if still visible, and its saved scroll position within current row bounds

#### Scenario: Keep the original parent across list switches

- **WHEN** the user leaves All tasks for Work, switches from Work to Home through the picker, and presses Esc
- **THEN** Husker restores the original All tasks state from before Work was opened

#### Scenario: Recover after the selected task leaves the results

- **WHEN** the saved All tasks task is removed from its results while a scoped view is open
- **THEN** returning to All tasks selects the nearest valid saved index or no task when the results are empty

## ADDED Requirements

### Requirement: Review retained completed and cancelled tasks

Husker SHALL provide History for the current ordinary list or All tasks through the configured `history` action, which defaults to `h`.
History includes all currently done or cancelled non-Trash tasks in that scope, including today and unknown status dates.
History orders known `status_changed_on` dates newest first, with descending task ID for ties. Unknown dates appear last, also by descending task ID.
History groups tasks under localized Today, Yesterday, calendar-date, or Unknown date headings based on `status_changed_on`.
The headings identify each task's recorded status date. Stored due dates remain available in the editor.
History uses breadcrumbs such as `All tasks > History` and `All tasks > Work > History`.
History is a view of retained task records. It does not claim to preserve every status transition, edit, or previous task text.
Reopening clears the task's status date according to task storage and removes the task from History after the write succeeds.
History starts with an empty filter and remembers the origin view, filter, parsed query, task ID, selection index, and scroll state.
Back or the `history` action restores that origin against fresh data, preserving task identity when present and clamping the prior index otherwise.
The `all_tasks` action returns to the saved root task state. Choosing a picker entry follows the navigation requirement.
History supports filtering, editing, copying, status changes, task Trash, reopening, and reading task descriptions.
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
- **THEN** the task becomes open, its status date is cleared, and it leaves History
- **AND** Back restores the originating task view with current database contents

#### Scenario: Return to a filtered task view

- **WHEN** task 42 is selected in a scrolled Work view filtered by `milk`, the user opens History, and then presses Esc or `h`
- **THEN** Work returns with filter `milk`, task 42 selected if still visible, and the saved scroll position within current bounds

### Requirement: Confirm list removal while preserving its tasks

Husker SHALL confirm deletion of an ordinary nondefault list before moving its tasks to the default list and deleting only its list metadata.
The picker uses the configurable `delete_list` action, with `d` and Delete as defaults.
All tasks, Trash, and the lowest-ID default list are protected and never open a deletion confirmation.
The confirmation shows the source name, destination name, and separate counts of open, done, cancelled, and Trash tasks from a consistent store preview.
The confirmation explicitly warns when unfinished tasks exist and states that every task moves to the destination.
No is selected by default. Arrow keys, Tab, and Shift+Tab change the choice. Enter accepts the selected choice.
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


### Requirement: Change language without leaving the current screen

Husker SHALL provide a modal language picker through the configured `language` action, which defaults to F3.
The picker is available from task navigation, History, the list picker, task editors, filter editors, list-name editors, and Help.
It is unavailable from task-discard confirmation, list-deletion confirmation, and Copy text.
The choices are English and Dansk. The current language is selected when the picker opens.
Configured Up and Down select a choice. Apply saves the choice, and Back cancels without saving.
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
