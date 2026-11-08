# Terminal interface

## Purpose

Define keyboard and mouse navigation, task editing, copying, list management, and display behavior for terminal users.
Planning and search owns filtering and date groups. Profiles and localization owns language and shortcut settings.
## Requirements
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

### Requirement: Edit task fields

Husker SHALL provide a task editor with these fields and save all valid changes together when the user invokes Save.
Default Save shortcuts are Ctrl+Enter and Ctrl+S. Esc and Ctrl+G invoke Discard with the confirmation rules defined below.

| Field | Input |
| --- | --- |
| Title | A nonempty task title |
| Description | Plain text with multiple lines |
| Due date | `YYYY-MM-DD`, or empty for no date |
| List | An existing list name |

Tab selects the next field. Shift+Tab selects the previous field. Enter inserts a newline only in Description.
Opening an existing task focuses Description. Creating a task focuses Title.
Ordinary text input does not invoke navigation actions. Text editing follows the field navigation, selection, and history requirements below.
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

#### Scenario: Discard confirmed edits

- **WHEN** a user changes a title, presses Esc or Ctrl+G, and confirms Yes in the discard prompt
- **THEN** Husker returns to navigation without changing the stored task

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

Dated tasks can move only relative to tasks with the same exact due date and the same open or closed category.
Undated tasks can move relative to other undated tasks in the same category. Done and cancelled tasks share the closed category.
Moving a task does not change its due date, list, or status.
The operation uses adjacent matching rows in the current results.
At the beginning or end of eligible rows, the move leaves the order unchanged.

#### Scenario: Keep dates unchanged

- **WHEN** the selected task is due `2026-11-09` and its preceding visible task is due `2026-11-08`
- **THEN** Alt+Up leaves the selected task's date and order unchanged

#### Scenario: Keep open and closed tasks separate

- **WHEN** the selected task is the first closed task for its due date and the preceding row is open
- **THEN** Alt+Up leaves the selected task's position and status unchanged

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

### Requirement: Show the existing task ID in the editor

Husker SHALL display the existing task's persistent ID compactly in the task editor heading.
The ID is visible without entering a field or opening the copy view.
The new-task editor has no task ID because no task has been saved.
The ID is a heading label and is not inserted into any task field or the F4 copy text.

#### Scenario: Identify a task while editing

- **WHEN** the user opens task 42
- **THEN** the editor heading includes `#42` while the title and description retain their stored text

#### Scenario: Create a task without a fabricated ID

- **WHEN** the user opens the new-task editor
- **THEN** the heading identifies a new task without displaying a task ID

### Requirement: Navigate and delete field text with standard keys

Husker SHALL support the following built-in editing keys in task fields, the filter input, and list-name inputs.
Configured actions take precedence when their effective shortcuts use the same key in the active input mode.

| Key | Result |
| --- | --- |
| Left or Right | Move one grapheme backward or forward. |
| Up or Down in Description | Move between wrapped visual rows at the nearest available display column. |
| Home or End | Move to the start or end of the current logical line. |
| Ctrl+Home or Ctrl+End | Move to the start or end of the field. |
| Ctrl+Left | Skip preceding whitespace, then move backward over one word or punctuation run. |
| Ctrl+Right | Move forward over the current run, then skip following whitespace. |
| Ctrl+Backspace or Ctrl+Delete | Delete to the boundary selected by Ctrl+Left or Ctrl+Right respectively. |

A logical line ends at an explicit newline, not a visual wrap.
Word runs contain Unicode letters, numbers, and underscores. Whitespace and punctuation or symbols form separate run categories.
Word movement and deletion preserve grapheme boundaries, including combining sequences and joined emoji.
Movement at a field boundary does not change text.

#### Scenario: Move over punctuation separately

- **WHEN** a field contains `alpha,beta` and the cursor is at its end
- **THEN** Ctrl+Left moves to the start of `beta` and a second Ctrl+Left moves before the comma

#### Scenario: Delete a Unicode word

- **WHEN** a field contains `Køb æbler` and its cursor is at the end
- **THEN** Ctrl+Backspace removes `æbler` and preserves `Køb `

#### Scenario: Distinguish field and line boundaries

- **WHEN** Description contains two explicit lines and the cursor is on the second line
- **THEN** Home moves to the second line's start and Ctrl+Home moves to the field's start

### Requirement: Select and replace text within one field

Husker SHALL support keyboard text selection within the active input field without selecting another field.
Shift+Left and Shift+Right extend selection by grapheme. Shift+Up and Shift+Down extend selection through Description's visual rows.
Shift+Home and Shift+End extend selection to the current logical line's boundaries.
Ctrl+Shift+Left and Ctrl+Shift+Right extend selection using the word-movement rules.
Ctrl+Shift+Home and Ctrl+Shift+End extend selection to the field's boundaries. Ctrl+A selects the entire active field.
Selection uses the same source positions as cursor placement and wrapped text drawing.
The selected text is visibly highlighted. Typing, pasting, Backspace, or Delete replaces or removes the selected range.
Mouse clicks keep their existing cursor-placement behavior. Mouse selection is outside this requirement.
The copy-selection action copies a selected range as defined below. F4 remains available for copying plain task text through the terminal.

#### Scenario: Replace the selected title

- **WHEN** the Title field contains `Buy milk` and the user presses Ctrl+A and types `Read`
- **THEN** the Title field contains `Read` and no other field changes

#### Scenario: Select across a soft wrap

- **WHEN** a Description paragraph wraps across rows and the user extends selection with Shift+Down
- **THEN** the highlight follows the displayed text across those rows without inserting newlines

#### Scenario: Replace a selection with pasted text

- **WHEN** one word is selected in Description and a paste event contains `replacement text`
- **THEN** the selected word is replaced by `replacement text` in one edit

### Requirement: Undo and redo field edits

Husker SHALL provide Ctrl+Z to undo and Ctrl+Y or Ctrl+Shift+Z to redo text edits in the active input field.
Each task field, filter input, and list-name input keeps its own history for that buffer's lifetime.
History retains at most 100 undo snapshots per field. A new edit beyond that limit drops the oldest snapshot.
Each printable input event, paste event, or deletion is one undo step. Pure cursor or selection movement creates no undo step.
Undo and redo restore the text, cursor position, and selection anchor from the corresponding snapshot.
A new text edit after undo clears redo history. Undo or redo with no corresponding history does nothing.
Field changes and F4 copy-view round trips preserve the existing field histories.
Saving or discarding an input form ends that form's history. History is not stored in the database.

#### Scenario: Undo in the active field only

- **WHEN** the user edits Title, then Description, and presses Ctrl+Z in Description
- **THEN** only the latest Description edit is undone and the Title edit remains

#### Scenario: Undo and redo a selection replacement

- **WHEN** the user replaces selected text with a paste and presses Ctrl+Z
- **THEN** the previous text and selection return
- **AND** Ctrl+Y restores the pasted replacement and its cursor position

#### Scenario: Start a new edit after undo

- **WHEN** the user undoes an edit and then types different text
- **THEN** Ctrl+Y does not restore the abandoned edit

### Requirement: Confirm discarding changed task fields

Husker SHALL ask for confirmation when Discard would close a task editor whose field text differs from its initial values.
The rule applies to existing tasks and unsaved new tasks. Cursor movement, selection, and a change fully undone back to the initial text do not make the editor changed.
An unchanged editor closes immediately when Discard is invoked.
The confirmation initially selects No, which returns to editing without losing text, cursor positions, selections, or histories.
Arrow keys, Tab, or Shift+Tab change the selected No or Yes choice. Enter applies that choice.
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

### Requirement: Copy selected input text to the system clipboard

Husker SHALL provide a configurable `copy_selection` action, bound to Ctrl+C by default, in task fields, the filter input, and list-name inputs.
The action copies only the selected range in the active input to the system clipboard available to the Husker process.
The copied text comes from the original field text, preserving Unicode and explicit newlines without adding visual-wrap characters, labels, or borders.
Copying does not change field text, cursor position, selection, undo history, redo history, or whether task fields differ from their initial values.
Copying does not save task data or add an undo step.
Without a nonempty selection, the action does nothing and leaves the clipboard untouched.
The action does not access the clipboard from navigation, help, the F4 copy view, or discard confirmation.
Selecting text alone never changes the clipboard. Cut commands, direct clipboard paste, mouse selection, and remote clipboard forwarding are outside this requirement.

#### Scenario: Copy a selected Unicode word

- **WHEN** the Description field contains `Køb æbler` with only `æbler` selected and the user presses Ctrl+C
- **THEN** the clipboard receives `æbler` and the entire editor buffer state remains unchanged

#### Scenario: Copy source text across visual rows

- **WHEN** selected Description text spans a soft wrap and one explicit newline
- **THEN** copying includes the explicit newline and adds no newline at the visual wrap

#### Scenario: Copy from a filter or list-name form

- **WHEN** text is selected in the active filter input or list-name input and Copy selection is invoked
- **THEN** only that input's selected text is copied without applying or changing the form

#### Scenario: Preserve the clipboard without a selection

- **WHEN** the clipboard contains `previous text`, the active input has no selected range, and the user presses Ctrl+C
- **THEN** the clipboard still contains `previous text` and the input remains unchanged

#### Scenario: Preserve clean task fields

- **WHEN** a user opens an unchanged task, selects and copies text, and invokes Discard
- **THEN** the editor closes without a dirty-field confirmation

### Requirement: Report clipboard results without losing selected text

Husker SHALL show a localized success message after a successful clipboard write and a localized error after a failed clipboard attempt.
Clipboard messages follow the existing five-second action-message lifetime.
An unavailable clipboard or a failed native clipboard write leaves the input text, selection, cursor, and histories intact.
Selected text containing a NUL character is rejected before the operating-system clipboard is accessed, preserving its previous contents.
The error identifies that copying failed. The task editor retains its F4 copy-text hint so users can fall back to terminal-native copying.
No success message appears when the clipboard write fails.

#### Scenario: Copy successfully

- **WHEN** the native clipboard accepts the selected text
- **THEN** Husker shows a success message in the selected interface language

#### Scenario: Retain a draft after clipboard failure

- **WHEN** the user copies a selected part of an unsaved task description and the clipboard is unavailable
- **THEN** Husker shows a localized error and preserves the full draft and selected range
- **AND** the user can open F4 to copy the draft through the terminal

#### Scenario: Reject a selection that would be truncated

- **WHEN** selected text contains a NUL character and the clipboard contains `previous text`
- **THEN** copying reports an error without accessing the operating-system clipboard
- **AND** the clipboard retains `previous text` and the selected text remains unchanged

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
