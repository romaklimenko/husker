# Terminal interface

## Purpose

Define keyboard and mouse navigation, task editing, copying, list management, and display behavior for terminal users.
Planning and search owns filtering and date groups. Profiles and localization owns language and shortcut settings.
## Requirements
### Requirement: Navigate lists and planned tasks

Husker SHALL open the global planned task view and provide navigation between lists, global planned tasks, and planned tasks restricted to one list.

The `planned` action opens the global planned view. The `lists` action opens the list chooser.
Selecting a list opens its planned view. Back from that view returns to the list chooser with the same list selected by ID.
The list chooser uses the persistent display order defined by task storage and shows each list's open task count.
The default list stays first. Trash appears last and opens deleted tasks.
Planned groups, task ordering, and filter matching follow the `planning-search` spec.

#### Scenario: Open a list and return

- **WHEN** a user presses `l`, selects `Private`, and presses Enter with default shortcuts
- **THEN** Husker shows planned tasks from `Private`
- **AND** pressing Esc returns to the list chooser with `Private` selected
- **AND** pressing `p` opens planned tasks from all lists

#### Scenario: Open Trash

- **WHEN** a user opens the list chooser and selects its last entry, Trash
- **THEN** Husker shows deleted tasks without placing them in planned groups

### Requirement: Use keyboard actions

Husker SHALL provide the following default shortcuts in navigation mode and resolve configurable action shortcuts through the selected configuration.

| Key | Action |
| --- | --- |
| Up or `k` | Select the previous task or list |
| Down or `j` | Select the next task or list |
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
| Alt+Up | Move the selected task before its eligible neighbor, or move the selected list up |
| Alt+Down | Move the selected task after its eligible neighbor, or move the selected list down |
| F2 | Toggle mouse capture |
| F4 | Show the selected task's plain text for copying |
| F5 | Refresh database contents |
| `?` | Show help with the effective shortcuts |
| `q` | Quit |

Actions that require a selected task do nothing when no task is selected.
Creating, renaming, or moving a list requires the list chooser. Trash cannot be renamed or moved.
The default list cannot move. Other list moves follow the pinned boundaries defined by task storage.
Task editing, completion, cancellation, deletion, and task reordering are unavailable in Trash. Restore is available only in Trash.
The `profiles-localization` spec owns configuration syntax and binding validation.

#### Scenario: Rebind an action

- **WHEN** configuration binds `quit` to `Ctrl+Q`
- **THEN** Ctrl+Q exits from navigation mode
- **AND** the help view displays Ctrl+Q as the effective quit shortcut

#### Scenario: Ignore an unavailable action

- **WHEN** a user presses Space while Trash is open
- **THEN** the deleted task keeps its stored status

### Requirement: Edit task fields

Husker SHALL provide a task editor with these fields and save all valid changes together when the user invokes Save.
Default Save shortcuts are Ctrl+Enter and Ctrl+S. Esc and Ctrl+G discard the editor's changes.

| Field | Input |
| --- | --- |
| Title | A nonempty task title |
| Description | Plain text with multiple lines |
| Due date | `YYYY-MM-DD`, or empty for no date |
| List | An existing list name |

Tab selects the next field. Shift+Tab selects the previous field. Enter inserts a newline only in Description.
Opening an existing task focuses Description. Creating a task focuses Title.
Ordinary text input does not invoke navigation actions. Cursor movement, Backspace, and Delete edit the current field.
Field labels and hints use the selected language.
Validation follows the `task-storage` spec. Invalid input leaves the editor open with its entered text and an error message.
A failed write does not show a saved message.

#### Scenario: Save a description and due date

- **WHEN** a user opens a task, enters two description lines, sets its due date to `2026-11-09`, and presses Ctrl+S
- **THEN** both changes persist together
- **AND** Husker returns to navigation and shows the task in the applicable planned group

#### Scenario: Keep invalid input for correction

- **WHEN** a user enters `2026-02-30` as a due date and presses Ctrl+Enter
- **THEN** Husker displays an invalid-date message and keeps the editor open
- **AND** the stored task remains unchanged

#### Scenario: Discard edits

- **WHEN** a user changes a title and presses Esc or Ctrl+G
- **THEN** Husker returns to navigation without changing the stored task

### Requirement: Create tasks and lists

Husker SHALL create tasks and lists through input forms without requiring the user to leave the terminal interface.

The new-task action opens the editor with an empty title, description, and due date.
A list-scoped view supplies its list. A global view supplies the lowest-ID ordinary list.
Saving an empty or invalid title leaves the editor open. Discarding a new task creates no stored task.
A valid new list appears after existing ordinary lists and before Trash.
Renaming a list preserves its position. The created or renamed list remains selected by ID.

#### Scenario: Capture a task in a list

- **WHEN** a user opens `Private`, presses `n`, enters `Buy milk`, and presses Ctrl+S
- **THEN** Husker creates `Buy milk` in `Private`
- **AND** the task has no due date

#### Scenario: Create and rename a list

- **WHEN** a user presses `N` in the list chooser, enters `Errands`, and saves
- **THEN** `Errands` appears immediately before Trash and is selected
- **AND** pressing `R`, entering `Shopping`, and saving changes its name without moving it

### Requirement: Apply task actions

Husker SHALL persist a task action before reporting success and refresh visible tasks after the write.

Done toggles `done` to `open` and changes any other status to `done`.
Cancel toggles `cancelled` to `open` and changes any other status to `cancelled`.
The task-storage spec owns status dates and the fields preserved by Trash and restore.
When an action removes the selected task from current results, selection moves to the closest remaining row.
An empty result has no selected task.

#### Scenario: Complete a planned task

- **WHEN** a user selects an open task and presses Space with default shortcuts
- **THEN** its stored status becomes `done` and its status-change date becomes today
- **AND** the task remains visible in default results for the rest of the local day

#### Scenario: Complete a task under an open-only filter

- **WHEN** a user completes a task while the active filter is `s:open`
- **THEN** the task leaves the results and the nearest remaining row is selected

### Requirement: Filter tasks interactively

Husker SHALL let the user enter a task filter without leaving the current task view.

The filter action opens a single-line input. Enter applies a valid filter and returns to navigation. Esc discards filter edits.
An empty filter removes text restrictions while preserving any list scope and the default status rules.
A list-scoped view combines its list scope with the filter.
Invalid syntax keeps the input open with an error and leaves previous results unchanged.
The `planning-search` spec defines accepted syntax.

#### Scenario: Combine list and text filters

- **WHEN** a user applies `l:private milk #shopping` in the global planned view
- **THEN** Husker shows only matching tasks according to the `planning-search` spec

#### Scenario: Correct an invalid filter

- **WHEN** a user applies a filter with an unterminated quoted value
- **THEN** Husker displays a filter error and keeps the entered filter available for editing
- **AND** the previously applied results remain unchanged

### Requirement: Reorder comparable tasks

Husker SHALL reorder a selected task only within the ordering group defined by the `planning-search` spec.

Dated tasks can move only relative to tasks with the same exact due date. Undated tasks can move relative to other undated tasks.
Moving a task does not change its due date or list.
The operation uses adjacent matching rows in the current results.
At the beginning or end of eligible rows, the move leaves the order unchanged.

#### Scenario: Keep dates unchanged

- **WHEN** the selected task is due `2026-11-09` and its preceding visible task is due `2026-11-08`
- **THEN** Alt+Up leaves the selected task's date and order unchanged

### Requirement: Keep the display usable

Husker SHALL keep the selected row visible, handle terminal resizing, and show an explicit empty state when no lists or tasks match the current view.

Task navigation uses the available content width without a side description preview.
The interface shows its current view and active filter.
Action results and errors follow the requirement to expire action messages after five seconds.
Enter opens the task editor at Description. F4 opens plain task text for copying.
A small terminal receives a readable size message instead of causing a panic.
Help shows the available actions and effective shortcuts for the current configuration. Esc closes help without changing stored data.
The configured Up and Down actions scroll help when its entries do not fit in the terminal.

#### Scenario: Use the full task area

- **WHEN** a user selects a task in a wide terminal
- **THEN** the task rows occupy the available navigation width and no description preview appears beside them

#### Scenario: Scroll through tasks

- **WHEN** a user moves selection below the last visible task
- **THEN** the task list scrolls so the selected task remains visible

#### Scenario: Show an empty filter result

- **WHEN** an applied filter matches no tasks
- **THEN** Husker shows an empty-result message and no task selection

### Requirement: Refresh dates and external changes

Husker SHALL check for external database commits at least once per second and provide an F5 refresh action in every screen.
The interface SHALL show a localized notice when another database connection changes data.

Navigation results refresh automatically. Local date changes recalculate planned groups and default status visibility.
Refresh preserves selection by task or list ID where possible.
Refresh never replaces text being entered in an editor, filter, or list-name form.
An external update during editing shows a notice while preserving the draft and its originally read task version.
F5 acknowledges a notice that was already visible. A newly detected external update still produces a notice.
After leaving an input form, Husker refreshes stored tasks before drawing navigation results.

#### Scenario: Observe a command-line change

- **WHEN** another Husker command adds a matching task while the interface is in navigation mode
- **THEN** the task appears within one refresh interval
- **AND** a notice identifies that another connection changed the database

#### Scenario: Refresh while editing

- **WHEN** another connection changes a task while the user edits its description and the user presses F5
- **THEN** the entered description and original version remain in the editor
- **AND** the interface shows the external-update notice

#### Scenario: Cross local midnight

- **WHEN** the local date advances to the next day
- **THEN** the next navigation refresh places yesterday's due open tasks in Overdue
- **AND** tasks completed yesterday leave the default results

### Requirement: Restore the terminal

Husker SHALL restore cooked input and the main screen and disable mouse capture when the interface exits normally, returns an error, or unwinds after a panic.

Husker refuses to start the interface without an interactive terminal.
A failed database operation displays an error without reporting success or discarding editor text.

#### Scenario: Quit the interface

- **WHEN** a user invokes the quit action from navigation
- **THEN** Husker leaves the alternate screen, disables raw input and mouse capture, and returns control to the shell

#### Scenario: Reject redirected input

- **WHEN** Husker starts the interface with noninteractive standard input or output
- **THEN** Husker returns a nonzero exit status with an explanatory error

### Requirement: Localize visible interface text

Husker SHALL obtain terminal labels, field names, help text, action results, and application error messages from the selected English or Danish catalog.

Task text and list names remain unchanged. Date formatting, language selection, fallback behavior, and stable filter tokens follow the `profiles-localization` spec.

#### Scenario: Use Danish labels

- **WHEN** a user starts Husker with Danish selected and opens the task editor
- **THEN** the editor's labels and hints appear in Danish
- **AND** the task's existing title and description remain unchanged

### Requirement: Use mouse navigation

Husker SHALL enable mouse capture by default and let users select navigation rows, open rows, scroll, and focus editor fields with the mouse.

A left click selects a task or list row. A second click on the same row within 400 milliseconds opens it.
The mouse wheel moves selection through navigation rows and scrolls help where applicable.
Clicking an editor field focuses that field and positions its cursor.
Mouse hit testing follows the actual visible rows after scrolling, including planning-group headings.
Clicks on headings or outside actionable content do not select another row.
F2 toggles mouse capture so users can use terminal-native selection.

#### Scenario: Open a scrolled task

- **WHEN** the task list has scrolled and the user double-clicks a visible task row
- **THEN** Husker opens the task drawn on that row

#### Scenario: Select text with the terminal

- **WHEN** the user presses F2 to disable mouse capture
- **THEN** Husker stops handling mouse navigation and the terminal receives mouse selection gestures

### Requirement: Copy plain task text

Husker SHALL provide an F4 copy-text view from task navigation and the task editor.
The view displays the task title, a blank line, and description without borders or inserted wrapping characters.

The editor opens this view with its current draft text, including unsaved changes.
Mouse capture is disabled while the view is open so terminal-native selection can copy multiline text without border glyphs.
Arrow keys scroll vertically and horizontally without changing task data.
Esc returns to the previous screen, preserves the editor draft when present, and restores the earlier mouse-capture setting.

#### Scenario: Copy multiline description text

- **WHEN** a task description contains `First line` and `Second line` separated by a newline and the user opens F4
- **THEN** the copy view contains both lines without a vertical border character at either line edge

#### Scenario: Preserve an unsaved draft

- **WHEN** a user opens F4 from an edited description and presses Esc
- **THEN** the editor returns with the same unsaved text and no database write occurs

### Requirement: Wrap descriptions without changing text

Husker SHALL display Description editor text with visual word wrapping at the available field width without inserting or removing stored characters.

Explicit newlines remain line breaks. Wrapping prefers whitespace boundaries.
A word wider than the field breaks across visual rows at grapheme boundaries.
Unicode display width determines how much text fits. A combining sequence is not split into separate editable characters by wrapping.
The cursor, vertical scrolling, mouse hit regions, and Up and Down movement use the same visual rows as the displayed text.
Up and Down move through visual rows and preserve the nearest available display column.
Resizing recalculates wrapping while preserving the text and cursor's position in that text.
The F4 copy view stays unwrapped and displays the original text.

#### Scenario: Wrap and save a long paragraph

- **WHEN** a description contains one paragraph longer than the field width and the user saves without adding newlines
- **THEN** the editor displays multiple visual rows and the stored description remains the original single paragraph

#### Scenario: Navigate a wrapped line

- **WHEN** a paragraph spans three visual rows and the cursor is on the first row
- **THEN** Down moves the cursor to the second visual row at the nearest available display column
- **AND** clicking the third visual row positions the cursor in the text drawn there

#### Scenario: Preserve Unicode text while resizing

- **WHEN** a description contains wide characters and combining sequences and the terminal becomes narrower
- **THEN** wrapping recalculates without changing the text or splitting a grapheme between visual rows
- **AND** the cursor remains aligned with its text position

### Requirement: Planning headings are separate from selection

Husker SHALL render planning-group headings independently from task rows.
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

### Requirement: Reorder lists from the chooser

Husker SHALL let users move a selected nondefault list with Alt+Up and Alt+Down in the list chooser.
The move uses the atomic operation defined by task storage and retains selection by list ID.
The default list remains first. Trash remains last.
Unavailable moves leave the order and selection unchanged.

#### Scenario: Keep a moved list selected

- **WHEN** the chooser shows Inbox, Work, Home, Trash and the user selects Home and presses Alt+Up
- **THEN** the chooser shows Inbox, Home, Work, Trash with Home selected
- **AND** reopening the app keeps that order

#### Scenario: Keep fixed rows in place

- **WHEN** the user presses Alt+Down on Inbox or Alt+Up on Trash
- **THEN** neither row moves and no task data changes

### Requirement: Expire action messages after five seconds

Husker SHALL remove a transient action result or error five seconds after setting that message, on the next interface update.
Setting a new message starts a new five-second interval.
Navigation does not extend the interval. Expiration occurs while the user is idle and does not require a key or mouse event.
Removing a message does not alter task data, editor fields, selection, or the current view.
The external database update notice does not expire. It follows the existing refresh and acknowledgement rules.

#### Scenario: Clear a saved result after navigation

- **WHEN** a task save sets the message `Saved` and the user navigates to another list
- **THEN** the message disappears five seconds after the save, even if the user stops typing

#### Scenario: Give a new message its own interval

- **WHEN** an action sets a new message four seconds after the previous message
- **THEN** the new message remains eligible for display for five seconds after the new action

#### Scenario: Expire an editor error without losing text

- **WHEN** an invalid date produces an error in the task editor and five seconds pass
- **THEN** the error disappears and the editor retains every unsaved field

#### Scenario: Keep an external update notice

- **WHEN** another connection changes the database and the external update notice appears
- **THEN** the notice remains after five seconds and through navigation
- **AND** expiring a temporary error does not dismiss the external update notice
