## ADDED Requirements

### Requirement: Navigate lists and planned tasks

Husker SHALL open the global planned task view and provide navigation between lists, global planned tasks, and planned tasks restricted to one list.

The `planned` action opens the global planned view. The `lists` action opens the list chooser. Selecting a list opens its planned view. Back from a list's planned view returns to the list chooser.
The list chooser shows each list's open task count. Trash appears last and opens deleted tasks. Planned groups, task ordering, and filter matching follow the `planning-search` spec.

#### Scenario: Open a list and return

- **WHEN** a user presses `l`, selects `Private`, and presses Enter with default shortcuts
- **THEN** Husker shows planned tasks from `Private`
- **AND** pressing Esc returns to the list chooser
- **AND** pressing `p` opens planned tasks from all lists

#### Scenario: Open Trash

- **WHEN** a user opens the list chooser and selects its last entry, Trash
- **THEN** Husker shows deleted tasks without placing them in planned groups

### Requirement: Use keyboard actions

Husker SHALL provide the following default shortcuts in navigation mode and resolve configurable action shortcuts through the selected configuration.

| Key | Action |
| --- | --- |
| Up or `k` | Select the previous row |
| Down or `j` | Select the next row |
| `l` | Open the list chooser |
| `p` | Open global planned tasks |
| `/` | Edit the task filter |
| Enter | Open the selected list or edit the selected task |
| Esc | Return from a list's tasks to the list chooser |
| `n` | Create a task |
| `N` | Create a list from the list chooser |
| `R` | Rename the selected list from the list chooser |
| Space | Toggle the selected task between open and done |
| `c` | Toggle the selected task between open and cancelled |
| `d` or Delete | Move the selected task to Trash |
| `r` | Restore the selected task from Trash |
| `a` | Mark the selected task ready for Anytime |
| `m` | Mark the selected task for Maybe |
| Alt+Up | Move the selected task before its previous eligible neighbor |
| Alt+Down | Move the selected task after its next eligible neighbor |
| `?` | Show help with the effective shortcuts |
| `q` | Quit |

Actions that require a selected task do nothing when no task is selected. Creating or renaming a list requires the list chooser and cannot rename Trash.
Task editing, completion, cancellation, deletion, readiness changes, and reordering are unavailable in Trash. Restore is available only in Trash.
The `profiles-localization` spec owns configuration syntax and binding validation.

#### Scenario: Rebind an action

- **WHEN** configuration binds `quit` to `Ctrl+Q`
- **THEN** Ctrl+Q exits from navigation mode
- **AND** the help view displays Ctrl+Q as the effective quit shortcut

#### Scenario: Ignore an unavailable action

- **WHEN** a user presses Space while Trash is open
- **THEN** the deleted task keeps its stored status

### Requirement: Edit task fields

Husker SHALL provide a task editor with the following fields and save all valid field changes together when the user presses Esc.

| Field | Input |
| --- | --- |
| Title | A nonempty task title |
| Description | Plain text with multiple lines |
| Due date | `YYYY-MM-DD`, or empty for no date |
| List | An existing list name |
| Tags | Space-separated tags, with optional `#` prefixes |
| Readiness | `maybe` or `anytime` |

Tab selects the next field. Shift+Tab selects the previous field. Enter inserts a newline only in Description. Ctrl+G discards the editor's changes and returns to navigation.
Opening an existing task focuses Description. Creating a task focuses Title.
Ordinary text input in the editor does not invoke navigation actions. Cursor movement, Backspace, and Delete edit the current field. Field labels and hints use the selected language. Readiness values remain the stable tokens `maybe` and `anytime`.
Validation follows the `task-storage` spec. An invalid value leaves the editor open with the entered text and an error message. A failed write does not show a saved message.

#### Scenario: Save a description and due date

- **WHEN** a user opens a task, enters two description lines, sets its due date to `2026-11-09`, and presses Esc
- **THEN** both changes persist together
- **AND** Husker returns to navigation and shows the task in the applicable planned group

#### Scenario: Keep invalid input for correction

- **WHEN** a user enters `2026-02-30` as a due date and presses Esc
- **THEN** Husker displays an invalid-date message and keeps the editor open
- **AND** the stored task remains unchanged

#### Scenario: Discard edits

- **WHEN** a user changes a title and presses Ctrl+G
- **THEN** Husker returns to navigation without changing the stored task

### Requirement: Create tasks and lists

Husker SHALL create tasks and lists through input forms without requiring the user to leave the terminal interface.

The new-task action opens the task editor with an empty title, description, due date, and tags. Readiness starts as `maybe`. A list-scoped view supplies its list. A global view supplies the first ordinary list.
Saving an empty or invalid title leaves the editor open. Discarding a new task creates no stored task. A valid new list or renamed list appears in the list chooser after saving.

#### Scenario: Capture a task in a list

- **WHEN** a user opens `Private`, presses `n`, enters `Buy milk`, and presses Esc
- **THEN** Husker creates `Buy milk` in `Private` with readiness `maybe`
- **AND** the task has no due date

#### Scenario: Create and rename a list

- **WHEN** a user presses `N` in the list chooser, enters `Errands`, and saves
- **THEN** `Errands` appears before Trash
- **AND** selecting it, pressing `R`, entering `Shopping`, and saving changes its name to `Shopping`

### Requirement: Apply task actions

Husker SHALL persist a task action before reporting success and refresh the visible tasks after the write.

Done toggles `done` to `open` and changes any other status to `done`. Cancel toggles `cancelled` to `open` and changes any other status to `cancelled`.
The Anytime and Maybe actions set readiness and clear the due date. Trash and restore preserve the task's other stored fields according to the `task-storage` spec.
When an action removes the selected task from the current results, selection moves to the closest remaining row. An empty result has no selected task.

#### Scenario: Complete a planned task

- **WHEN** a user selects an open task and presses Space with default shortcuts
- **THEN** the stored status becomes `done`
- **AND** the task disappears from the default open-task results

#### Scenario: Move a dated task to Anytime

- **WHEN** a user selects a dated task and presses `a` with default shortcuts
- **THEN** the task's readiness becomes `anytime` and its due date becomes empty
- **AND** the task appears in Anytime when it matches the active filter

### Requirement: Filter tasks interactively

Husker SHALL let the user enter a task filter without leaving the current task view.

The filter action opens a single-line input. Enter applies a valid filter and returns to navigation. Esc discards filter edits. An empty filter removes the text filter while preserving any list scope.
A list-scoped view combines its list scope with the filter. Invalid syntax keeps the input open with an error and leaves the previous results unchanged. The `planning-search` spec defines accepted syntax.

#### Scenario: Combine list, text, and tag filters

- **WHEN** a user applies `l:private milk #shopping` in the global planned view
- **THEN** Husker shows only matching tasks according to the `planning-search` spec

#### Scenario: Correct an invalid filter

- **WHEN** a user applies a filter with an unterminated quoted value
- **THEN** Husker displays a filter error and keeps the entered filter available for editing
- **AND** the previously applied results remain unchanged

### Requirement: Reorder comparable tasks

Husker SHALL reorder a selected task only within the ordering group defined by the `planning-search` spec.

Dated tasks can move only relative to tasks with the same exact due date. Undated tasks can move only within the same readiness group. Moving a task does not change its due date, readiness, or list.
The operation uses adjacent matching rows in the current results. At the beginning or end of the eligible rows, the move leaves the order unchanged.

#### Scenario: Keep dates unchanged

- **WHEN** the selected task is due `2026-11-09` and its preceding visible task is due `2026-11-08`
- **THEN** Alt+Up leaves the selected task's date and order unchanged

### Requirement: Keep the display usable

Husker SHALL keep the selected row visible, handle terminal resizing, and show an explicit empty state when no lists or tasks match the current view.

The interface shows its current view, active filter, selected task details when space permits, and the latest action result or error. A small terminal receives a readable size message instead of causing a panic.
Help shows the available actions and effective shortcuts for the current configuration. Esc closes help without changing stored data.
The configured Up and Down actions scroll help when its entries do not fit in the terminal.

#### Scenario: Scroll through tasks

- **WHEN** a user moves selection below the last visible task
- **THEN** the task list scrolls so the selected task remains visible

#### Scenario: Show an empty filter result

- **WHEN** an applied filter matches no tasks
- **THEN** Husker shows an empty-result message and no task selection

### Requirement: Refresh dates and external changes

Husker SHALL refresh task results during navigation at least once per second and recalculate planned groups when the local calendar date changes.

Refresh preserves selection by task ID where possible. Refresh does not replace text being entered in an editor or filter input. After leaving an input form, Husker refreshes stored tasks before drawing navigation results.

#### Scenario: Observe a command-line change

- **WHEN** another Husker command adds a matching task while the terminal interface is in navigation mode
- **THEN** the task appears within one refresh interval

#### Scenario: Cross local midnight

- **WHEN** a task is due today and the local date advances to the next day
- **THEN** the next navigation refresh places that open task in Overdue

### Requirement: Restore the terminal

Husker SHALL restore cooked input and the main screen when the terminal interface exits normally, returns an error, or unwinds after a panic.

Husker refuses to start the terminal interface without an interactive terminal. A failed database operation displays an error without reporting success or discarding editor text.

#### Scenario: Quit the interface

- **WHEN** a user invokes the quit action from navigation
- **THEN** Husker leaves the alternate screen, disables raw input, and returns control to the shell

#### Scenario: Reject redirected input

- **WHEN** Husker starts the terminal interface with noninteractive standard input or output
- **THEN** Husker returns a nonzero exit status with an explanatory error

### Requirement: Localize visible interface text

Husker SHALL obtain terminal labels, field names, help text, action results, and application error messages from the selected English or Danish catalog.

Task text and list names remain unchanged. Date formatting, language selection, fallback behavior, and stable filter tokens follow the `profiles-localization` spec.

#### Scenario: Use Danish labels

- **WHEN** a user starts Husker with Danish selected and opens the task editor
- **THEN** the editor's labels and hints appear in Danish
- **AND** the task's existing title and description remain unchanged
