## MODIFIED Requirements

### Requirement: Navigate lists and planned tasks

Husker SHALL open Planned as the top-level task view and use a modal list picker to choose an ordinary list or Trash.
The global view heading is `Planned`. A selected list uses a breadcrumb such as `Planned > Work`; Trash uses `Planned > Trash`.
The `lists` action opens the picker from any task view. Invoking `lists` again or Back while the picker is open cancels it.
Canceling the picker preserves the underlying view, filter, selected task, and scroll position.
The initial picker selection matches the current scope: All tasks, the ordinary list by ID, or Trash.
Choosing the current scope closes the picker without resetting task state.
Choosing a different ordinary list or Trash starts that view with an empty filter and its first task selected, if present.
Back from an ordinary list or Trash returns to Planned. Back at Planned leaves it unchanged.
The `planned` action and choosing All tasks also return to Planned with the saved parent state defined below.
Planned groups, task ordering, and filter matching follow the `planning-search` spec. Trash shows deleted tasks without planned groups.
Editor save and dirty-discard confirmation keep their existing behavior and return to the task view from which the editor opened.

#### Scenario: Open a list and return to Planned

- **WHEN** a user in Planned presses `l`, selects `Work`, and presses Enter with default shortcuts
- **THEN** Husker closes the picker and shows `Planned > Work` with tasks from Work
- **AND** pressing Esc returns to Planned instead of opening the picker

#### Scenario: Cancel a list choice

- **WHEN** a user opens the picker from a filtered list view, moves its selection, and presses Esc or `l`
- **THEN** the picker closes and the original task filter, selection, and scroll position remain unchanged

#### Scenario: Keep state when choosing the current list

- **WHEN** a user opens the picker from Work and chooses Work again
- **THEN** the picker closes without resetting Work's filter, selected task, or scroll position

#### Scenario: Open Trash

- **WHEN** a user chooses the last picker entry, Trash
- **THEN** Husker shows `Planned > Trash` with deleted tasks, no planning-group headings, and an empty filter
- **AND** Esc returns to Planned

### Requirement: Use keyboard actions

Husker SHALL provide the following default shortcuts in task navigation and the list picker and resolve configurable action shortcuts through the selected configuration.

| Key | Action |
| --- | --- |
| Up or `k` | Select the previous task or picker entry |
| Down or `j` | Select the next task or picker entry |
| `l` | Open or cancel the list picker |
| `p` | Return to Planned with its saved task state |
| `/` | Edit the task filter |
| Enter | Choose the picker entry or edit the selected task |
| Esc | Cancel the picker, or return from a scoped task view to Planned |
| `n` | Create a task from a task view |
| `N` | Create a list from the picker |
| `R` | Rename the selected ordinary list from the picker |
| Space | Toggle the selected task between open and done |
| `c` | Toggle the selected task between open and cancelled |
| `d` or Delete | Move the selected task to Trash |
| `r` | Restore the selected task from Trash |
| Alt+Up | Move the selected task before its eligible neighbor, or move the selected list up |
| Alt+Down | Move the selected task after its eligible neighbor, or move the selected list down |
| F2 | Toggle mouse capture |
| F4 | Show the selected task's plain text for copying |
| F5 | Refresh database contents |
| `?` | Show help with the effective shortcuts |
| `q` | Quit |

Actions that require a selected task do nothing when no task is selected.
Creating, renaming, or moving a list requires the picker. All tasks and Trash cannot be renamed or moved.
The default ordinary list cannot move. Other list moves follow the pinned boundaries defined by task storage.
Task creation, editing, completion, cancellation, deletion, and task reordering are unavailable in Trash. Restore is available only in Trash.
Task actions do not affect background tasks while the picker is open.
The `profiles-localization` spec owns configuration syntax and binding validation. Existing action names and default bindings remain unchanged.

#### Scenario: Rebind an action

- **WHEN** configuration binds `quit` to `Ctrl+Q`
- **THEN** Ctrl+Q exits from task navigation or the list picker
- **AND** the help view displays Ctrl+Q as the effective quit shortcut

#### Scenario: Ignore an unavailable action

- **WHEN** a user presses Space while Trash is open
- **THEN** the deleted task keeps its stored status

#### Scenario: Use Planned while the picker is open

- **WHEN** the picker is open over Work and the user presses `p`
- **THEN** the picker closes and Planned returns with its saved task state

### Requirement: Create tasks and lists

Husker SHALL create tasks and lists through input forms without requiring the user to leave the terminal interface.

The new-task action opens the editor with an empty title, description, and due date.
A list-scoped task view supplies its list. Planned supplies the lowest-ID ordinary list.
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

### Requirement: Use mouse navigation

Husker SHALL enable mouse capture by default and let users select navigation rows, open rows, scroll, and focus editor fields with the mouse.

A left click selects a task or picker entry. A second click on the same row within 400 milliseconds opens it.
The mouse wheel moves selection through task rows or picker entries and scrolls help where applicable.
Clicking an editor field focuses that field and positions its cursor.
Mouse hit testing follows the actual visible rows after scrolling, including planning-group headings.
Clicks on headings or outside actionable content do not select another row.
While the picker is open, only its visible entry rows respond to selection and opening.
Clicks on the popup border or outside the popup do nothing. Wheel events outside the popup do nothing.
No mouse event passes through the popup to change or open a background task.
F2 toggles mouse capture so users can use terminal-native selection.

#### Scenario: Open a scrolled task

- **WHEN** the task list has scrolled and the user double-clicks a visible task row
- **THEN** Husker opens the task drawn on that row

#### Scenario: Select text with the terminal

- **WHEN** the user presses F2 to disable mouse capture
- **THEN** Husker stops handling mouse navigation and the terminal receives mouse selection gestures

#### Scenario: Choose a scrolled picker entry

- **WHEN** a user scrolls the picker and double-clicks a visible ordinary list
- **THEN** the picker closes and Husker shows the list drawn on that row

#### Scenario: Protect background tasks

- **WHEN** the picker is open and a user clicks or scrolls over a visible background task outside the popup
- **THEN** neither the task selection nor the picker selection changes and no task opens

### Requirement: Reorder lists from the chooser

Husker SHALL let users move a selected nondefault ordinary list with Alt+Up and Alt+Down in the modal picker.
The move uses the atomic operation defined by task storage and retains selection by list ID.
All tasks remains the first picker entry. The default list remains first among ordinary lists. Trash remains last.
Unavailable moves leave the order and selection unchanged.

#### Scenario: Keep a moved list selected

- **WHEN** the picker shows All tasks, Inbox, Work, Home, Trash and the user selects Home and presses Alt+Up
- **THEN** the picker shows All tasks, Inbox, Home, Work, Trash with Home selected
- **AND** reopening the app keeps the ordinary lists in that order

#### Scenario: Keep fixed rows in place

- **WHEN** the user attempts to move All tasks, the default Inbox list, or Trash
- **THEN** no fixed row moves and no task data changes

## ADDED Requirements

### Requirement: Display the list picker over task navigation

Husker SHALL display a centered, bounded modal picker over the current task view without replacing that task view with a full-screen list view.
The picker contains All tasks first, ordinary lists in their persistent display order, and Trash last.
All tasks is a virtual entry without a count. Ordinary lists show open-task counts, and Trash shows its deleted-task count.
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

### Requirement: Restore Planned after visiting a scoped view

Husker SHALL retain Planned's filter, parsed query, selected task ID, selection index, and task scroll state when leaving Planned for an ordinary list or Trash.
Switching between ordinary lists or Trash preserves that original Planned state.
Back from a scoped view, the `planned` action, and choosing All tasks restore that state against fresh database contents.
If the saved selected task is still visible, it becomes selected again by ID.
If the saved task is no longer visible, Husker clamps the saved selection index to the current result count.
An empty result has no selected task. Rendering clamps the restored scroll position to the available rows and keeps the selected task visible.
Invoking `planned` while already at Planned preserves the current filter, selection, and scroll position.
The saved Planned state lasts only for the running process.

#### Scenario: Restore a filtered and scrolled Planned view

- **WHEN** task 42 is selected in a scrolled Planned view filtered by `milk`, the user visits Work, and then presses Esc
- **THEN** Planned returns with filter `milk`, task 42 selected if still visible, and its saved scroll position within current row bounds

#### Scenario: Keep the original parent across list switches

- **WHEN** the user leaves Planned for Work, switches from Work to Home through the picker, and presses Esc
- **THEN** Husker restores the original Planned state from before Work was opened

#### Scenario: Recover after the selected task leaves the results

- **WHEN** the saved Planned task is removed from its results while a scoped view is open
- **THEN** returning to Planned selects the nearest valid saved index or no task when the results are empty
