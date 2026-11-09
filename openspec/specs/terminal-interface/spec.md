# Terminal interface

## Purpose

Define keyboard and mouse navigation, task editing, copying, list management, and display behavior for terminal users.
Planning and search owns filtering and date groups. Profiles and localization owns language and shortcut settings.
## Requirements
### Requirement: Use keyboard actions

Husker SHALL provide the following default shortcuts in task navigation and the list picker and resolve configurable action shortcuts through the selected configuration.

| Key | Action |
| --- | --- |
| Up or `k` | Select the previous task or picker entry |
| Down or `j` | Select the next task or picker entry |
| Home or `g` | Select the first visible task or picker entry |
| End or `G` | Select the last visible task or picker entry |
| `l` | Open or cancel the list picker |
| `h` | Open History from ordinary task navigation, or return from History |
| `t` | Set the selected task due today |
| `T` | Set the selected task due tomorrow |
| `w` | Set the selected task due next week on Monday |
| `x` | Clear the selected task's due date |
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

Keyboard Up/Down wrap and First/Last jump according to the selection requirement below.
Actions that require a selected task do nothing when no task is selected.
Creating, renaming, moving, or deleting a list requires the picker. All tasks and Trash cannot be renamed, moved, or deleted.
The default ordinary list cannot move or be deleted. Other list moves follow the pinned boundaries defined by task storage.
Task creation, editing, completion, cancellation, deletion, and task reordering are unavailable in Trash. Restore is available only in Trash.
Task actions do not affect background tasks while the picker is open.
Due-date presets are unavailable in Trash and use the navigation and editor contracts below.
The `profiles-localization` spec owns configuration syntax and binding validation. The optional `all_tasks` action and legacy `planned` alias follow that spec's compatibility rules.
The `all_tasks` action has no default shortcut. Back and the All tasks picker entry provide normal root navigation.
History is unavailable from Trash and the picker.
History supports opening, filtering, copying, due-date presets, status changes, moving tasks to Trash, reopening, help, refreshing, and navigation.
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

### Requirement: Edit task fields

Husker SHALL provide a task editor with these fields and save all valid changes together when the user invokes Save.
Default Save shortcuts are Ctrl+Enter and Ctrl+S. Esc and Ctrl+G invoke Discard with the confirmation rules defined below.

| Field | Input |
| --- | --- |
| Title | A nonempty task title |
| Description | Plain text with multiple lines |
| Planned date | `YYYY-MM-DD`, `YYYYMMDD`, `YYMMDD`, `MMDD`, or empty for no date |
| List | A read-only selector for an existing ordinary list ID, displayed by its current name |

Tab selects the next field. Shift+Tab selects the previous field. Enter inserts a newline only in Description and opens the picker when List is focused.
Opening an existing task focuses Description. Creating a task focuses Title.
Ordinary text input does not invoke task-navigation writes.
The Planned date field also accepts the preset actions defined below as draft edits. Title and Description retain those characters as ordinary text. List uses its selector controls.
Title, Description, and Planned date text editing follows the field navigation, selection, and history requirements below.
List changes use the picker and its choice history. Saving resolves the selected list by ID, never by name.
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
- **THEN** the next navigation refresh places yesterday's due open tasks in Earlier
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
Clicking a text editor field focuses that field and positions its cursor. Clicking the task editor's List selector focuses List and opens its picker.
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

Husker SHALL support the following built-in editing keys in Title, Description, Planned date, the filter input, and list-name inputs.
The task editor's List selector does not accept text editing.
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

Husker SHALL support keyboard text selection within the active text input field without selecting another field.
Text inputs are Title, Description, Planned date, filter inputs, and list-name inputs. The List selector does not accept text selection or replacement.
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
Title, Description, Planned date, each filter input, and each list-name input keep separate text histories for their buffer lifetimes.
The List selector keeps a separate history of chosen list IDs and supports the same Undo and Redo keys.
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

Husker SHALL ask for confirmation when Discard would close a task editor whose text fields or selected list ID differ from their initial values.
The rule applies to existing tasks and unsaved new tasks. Cursor movement, selection, and a change fully undone back to the initial text and list ID do not make the editor changed.
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

Husker SHALL provide a configurable `copy_selection` action, bound to Ctrl+C by default, in Title, Description, Planned date, the filter input, and list-name inputs.
Copy selection is unavailable in the task editor's List selector and editor list picker.
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

### Requirement: Review retained completed and cancelled tasks

Husker SHALL provide History for the current ordinary list or All tasks through the configured `history` action, which defaults to `h`.
History includes all currently done or cancelled non-Trash tasks in that scope, including today and unknown status dates.
History follows the date, timestamp, and ID ordering defined by planning-search.
Tasks completed later on the same local day appear first when their timestamps are known. Unknown completion times are not reconstructed.
History groups tasks under localized Today, Yesterday, calendar-date, or Unknown date headings based on `status_changed_on`.
The display date remains the local calendar date even though same-day ordering uses a UTC timestamp.
The headings identify each task's recorded status date.
History rows do not append the stored planned date as metadata. Task titles, including dates typed into titles, remain unchanged.
The planned date remains available in the editor and retains its row suffix in other task views, including Trash.
Unknown closure dates remain Unknown date even when a task has a known planned date.
History uses breadcrumbs such as `All tasks > History` and `All tasks > Work > History`.
History is a view of retained task records. It does not claim to preserve every status transition, edit, or previous task text.
Reopening clears the task's status date and timestamp according to task storage and removes the task from History after the write succeeds.
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
The picker is available from task navigation, History, the navigation list picker, the editor list picker, task editors, filter editors, list-name editors, and Help.
It is unavailable from task-discard confirmation, list-deletion confirmation, and Copy text.
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
Errors and action results use the selected language and existing message-lifetime rules.
The four actions appear in Help with configured shortcuts. They add no persistent navigation-header hint.

#### Scenario: Move an open task to Tomorrow

- **WHEN** an undated open task is selected on `2026-11-08` and the user presses `T` with default bindings
- **THEN** the stored due date becomes `2026-11-09` and the task moves to Tomorrow
- **AND** that same task remains selected

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

### Requirement: Apply due-date presets as whole-field draft edits

Husker SHALL apply a preset to the entire Planned date editor field when that field is focused, without saving the task or changing the focused field.
A dated result replaces the field with its `YYYY-MM-DD` value. Clear due date replaces it with an empty string.
A changed replacement is one undoable edit, regardless of the previous cursor position, selection, or invalid partial input.
The replacement places the cursor at the end and clears the text selection.
Undo restores the original text, cursor, and selection. Redo restores the replacement and its resulting cursor and selection.
An identical replacement changes no buffer state, undo history, task version, or draft dirtiness.
Other editor fields remain unchanged. Existing Save and dirty-discard behavior remains in force.
Today is resolved at the keypress using the current local day. A resolution error leaves the field unchanged.
Manual date entry in the supported editor formats, ordinary text editing, clipboard copying, language selection, and field navigation remain available.
The default letters `t`, `T`, `w`, and `x` remain ordinary text in Title, Description, filter inputs, and list-name inputs.
While Planned date is focused, its localized configured preset hints replace the editor's existing editing-hint row without adding screen height.
Other fields keep their existing editing hints. Preset hints do not appear as new header rows on other screens.

#### Scenario: Replace an incomplete date and undo it

- **WHEN** Planned date contains `2026-` with a partial selection and the user invokes Tomorrow on `2026-11-08`
- **THEN** the field becomes `2026-11-09`, the cursor moves to its end, and selection clears
- **AND** one Undo restores `2026-` with the original cursor and selection
- **AND** the stored task has not changed

#### Scenario: Clear a draft without saving

- **WHEN** Planned date contains `2026-11-09` and the user invokes Clear due date
- **THEN** the field becomes empty and remains focused
- **AND** one Undo restores the prior field state while the stored due date stays unchanged until Save

#### Scenario: Keep other fields literal

- **WHEN** a user types `tTwx` into Description, Title, a filter, or a list-name form with default bindings
- **THEN** the field receives those characters as text and no preset is invoked

#### Scenario: Keep a repeated draft preset unchanged

- **WHEN** Planned date already contains today's exact ISO date and the user invokes Today
- **THEN** its text, cursor, selection, undo history, and draft dirtiness remain unchanged

#### Scenario: Keep editor controls available

- **WHEN** Planned date is focused after using a preset
- **THEN** Ctrl+S saves through the existing validation rules, Tab changes field, Ctrl+Z undoes, and Ctrl+C copies selected input with default bindings

### Requirement: Identify task lists in all-list views

Husker SHALL display the current list name in brackets immediately before the task title in All tasks and all-list History rows.
For example, an open task titled `Buy milk` in Work displays `[ ] [Work] Buy milk` before any existing date text.
An `l:` query filter does not remove the prefix while the containing view remains All tasks or all-list History.
Ordinary list-scoped views, list-scoped History, and Trash do not add this prefix.
The label uses the current list name after refresh, including after renaming or moving tasks between lists.
Embedded carriage returns and newlines in a list name render as spaces, using the same normalization as breadcrumbs.
The prefix occupies the existing task row and adds no extra rows.
This is display metadata only. Stored titles, editor fields, F4 copy text, CLI task titles, and MCP task titles remain unchanged.

#### Scenario: Distinguish matching titles across lists

- **WHEN** All tasks contains tasks titled `Review` in Work and Home
- **THEN** their rows display `[Work] Review` and `[Home] Review` after the status marker
- **AND** editing or copying either task still uses the stored title `Review`

#### Scenario: Keep labels after filtering all-list History

- **WHEN** all-list History applies `l:Work`
- **THEN** each matching task still shows its `[Work]` prefix
- **AND** opening Work's scoped History shows the same tasks without that prefix

#### Scenario: Refresh a renamed list label

- **WHEN** Work is renamed to Office while All tasks is open
- **THEN** the next refresh renders `[Office]` on its task rows without changing their stored titles

### Requirement: Show actual dates on task headings

Husker SHALL show one heading per exact due date for open tasks classified as Soon or Next in ordinary task views.
These headings use the calendar-label format from profiles-localization and replace the visible Soon and Next headings.
Today and Tomorrow headings include their actual due dates alongside their localized labels.
Done today and Cancelled today headings include the actual local status date alongside their localized labels.
History's Today and Yesterday headings include their actual recorded status dates alongside their localized labels.
Other known History dates retain calendar-date headings.
Earlier, Sometime, older aggregate Done and Cancelled, and Unknown date remain aggregate or unknown headings without an invented single date.
Headings do not change sorting, due-filter meanings, manual-reorder eligibility, or stored task data.
Heading rows remain unselectable, unhighlighted, and inert to mouse clicks.

#### Scenario: Split both future buckets by date

- **WHEN** today is `2026-11-08` and open tasks are due on November 10, 15, and 16
- **THEN** the tasks appear under three exact-date headings in that order
- **AND** no Soon or Next heading is displayed while their due-filter buckets remain unchanged

#### Scenario: Add dates to relative labels

- **WHEN** today is `2026-11-08` and the English interface shows Tomorrow and Done today
- **THEN** their headings include `2026-11-09 Mon W46` and `2026-11-08 Sun W45` respectively
- **AND** Done today uses the status date regardless of the task's due date

#### Scenario: Date relative History headings

- **WHEN** today is `2026-11-08` and History includes tasks with status dates November 8 and 7
- **THEN** Today includes the November 8 calendar label and Yesterday includes the November 7 calendar label

#### Scenario: Preserve aggregate headings

- **WHEN** Earlier contains tasks due on different past dates and Done contains older completions
- **THEN** neither aggregate heading claims one date for all of its tasks

### Requirement: Choose a list for a draft by stable identity

Husker SHALL provide a read-only List selector in the task editor backed by a stable ordinary list ID.
The selector displays the current name of that ID. It never exposes an internal ID as editable or copied input.
Tab focuses List without opening the picker. Open or Lists opens the picker, with Enter and `l` as their defaults.
Clicking the List selector focuses it and opens the picker.
Typing, paste, text selection, and Copy selection do nothing in the selector.
Choosing a different list changes only the unsaved selected ID and adds one choice-history step.
Choosing the same ID is a no-op. Ctrl+Z undoes, and Ctrl+Y or Ctrl+Shift+Z redoes, the choice.
The selector retains at most 100 undo snapshots. A new choice after undo clears redo.
Title, Description, Planned date, their cursors, selections, histories, and the originally read task version remain intact.
Save uses the selected ID and the existing task-version guard. A missing selected list prevents Save until a valid replacement is explicitly chosen.
External renames update the displayed name without changing the selected ID or draft dirtiness.
Undo or redo to a deleted list ID retains that unavailable ID until the user explicitly chooses a valid list.
A missing selected ID remains missing. Husker MUST NOT silently match another list by the old name or switch the draft to another list.

#### Scenario: Choose and undo a draft destination

- **WHEN** an unsaved draft belongs to Inbox and the user chooses Work from the editor picker
- **THEN** only the draft's selected list ID changes and the database stays unchanged
- **AND** Ctrl+Z in List restores Inbox while Ctrl+Y restores Work without changing other fields

#### Scenario: Preserve identity across a rename

- **WHEN** Work is selected in a draft and another connection renames that same list to Office
- **THEN** refresh displays Office while preserving the selected ID, draft state, and originally read task version

#### Scenario: Do not retarget a missing list

- **WHEN** a draft's selected list is deleted and another list has the same name
- **THEN** Save does not use the other list and requires an explicit valid choice

#### Scenario: Undo to a deleted destination

- **WHEN** the draft moves from Work to Inbox, Work is deleted externally, and the user undoes the list choice
- **THEN** the draft retains Work's missing ID and Save requires an explicit valid replacement
- **AND** no list is substituted by name or default

### Requirement: Keep editor list picking modal

Husker SHALL display a modal editor list picker containing only ordinary lists in their persistent order with the default-list marker.
Virtual All tasks and Trash entries and all list-management actions are unavailable.
The current draft list is selected when present. Opening the picker does not change the draft if that ID is unavailable.
Configured Up and Down wrap selection. First and Last jump to the corresponding boundary. Open chooses the selected list and returns to the List selector.
Configured Back or Lists cancels and returns to the unchanged List selector, with Esc and `l` as defaults.
Single-click selects a visible row. Double-click on the same row within 400 milliseconds chooses it.
Wheel events move picker selection only inside the popup. Borders, outside clicks, and outside wheel events do nothing.
Background editor fields do not receive popup input.
Refresh preserves the picker selection by ID when present and otherwise clamps its previous index.
Refresh does not replace the draft's chosen ID or text fields.
Help and language-picker round trips preserve the editor picker selection, scroll, and underlying draft state.

#### Scenario: Cancel a destination choice

- **WHEN** a user opens the editor picker, selects Work, and presses Esc or `l` with default bindings
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

### Requirement: Show empty-date guidance without inserting text

Husker SHALL display `YYYY-MM-DD` as a placeholder only when the Planned date buffer is empty.
The placeholder is not stored text, selected text, copied text, an undo step, or a due date.
The Planned date field title or hint lists `YYYY-MM-DD`, `YYYYMMDD`, `YYMMDD`, and `MMDD` without adding header rows.
Entering text hides the placeholder and renders the actual input.
Saving an empty field stores no due date.

#### Scenario: Save an empty date

- **WHEN** Planned date displays its placeholder and the user saves an otherwise valid task
- **THEN** the stored due date is absent and no placeholder text is stored

#### Scenario: Type into the empty field

- **WHEN** Planned date is empty and the user types `1109`
- **THEN** the buffer contains only `1109`, without placeholder characters

### Requirement: Accept compact dates only in the task editor

Husker SHALL accept strict `YYYY-MM-DD` and ASCII digit forms `YYYYMMDD`, `YYMMDD`, and `MMDD` in the task editor's Planned date field.
Parsing trims surrounding whitespace. Empty trimmed input means no due date.
Eight digits contain the full year, month, and day. Six digits use year 2000 plus the two-digit year.
Four digits use the month and day in the injected current local year at normalization or Save, without guessing a future occurrence.
Keyboard and mouse normalization use the local day of that action. A previously normalized explicit year stays fixed across New Year.
Every form must identify a real calendar date. Other lengths, non-ASCII digits, malformed ISO dates, and impossible dates are invalid.
The English error is `Invalid date '{value}'. Use YYYY-MM-DD, YYYYMMDD, YYMMDD, or MMDD.` A Danish equivalent is provided.
CLI, MCP, and query date inputs remain strict `YYYY-MM-DD` and reject compact forms.
No typing mask inserts characters or normalizes partially entered dates.

#### Scenario: Parse the supported forms

- **WHEN** the current local date is `2026-11-08`
- **THEN** `2026-11-09`, `20261109`, `261109`, and `1109` each resolve to `2026-11-09`
- **AND** `991109` resolves to `2099-11-09` and `0101` resolves to `2026-01-01`

#### Scenario: Validate leap dates

- **WHEN** `0229` is parsed in local year 2028 or 2026
- **THEN** it resolves to `2028-02-29` in 2028 and fails in 2026

#### Scenario: Keep external date syntax strict

- **WHEN** compact `20261109` is supplied as a CLI or MCP due date or a query due-date value
- **THEN** the existing strict parser rejects it without writing task data

### Requirement: Normalize valid dates when leaving the field

Husker SHALL normalize valid Planned date input to `YYYY-MM-DD` when Tab, Shift+Tab, or a click on another editor field moves focus away from Planned date.
Trimmed empty input normalizes to an empty buffer.
A changed normalization is one whole-field undo step, places the cursor at the end, and clears selection.
Undo restores the exact prior text, cursor, and selection. Redo restores the normalized result.
An identical replacement changes no cursor, selection, history, or draft state.
Invalid input stays exact when focus changes and shows an invalid-date error only when Save is attempted.
Typing alone never normalizes the field, including after four digits.
Save parses the current raw input directly and writes the parsed date with the other valid task fields.
A failed parse or write preserves the entered draft for correction.

#### Scenario: Normalize and undo compact input

- **WHEN** today is `2026-11-08`, Planned date contains ` 1109 ` with a selection, and the user presses Tab
- **THEN** Planned date becomes `2026-11-09` and focus advances
- **AND** one Undo in Planned date restores ` 1109 ` and its prior cursor and selection
- **AND** no task write occurs before Save

#### Scenario: Keep partial and invalid input intact

- **WHEN** a user types `2026` and continues typing without leaving Planned date
- **THEN** no automatic four-digit normalization occurs
- **AND** leaving the field with `0230` preserves that invalid text until Save reports its error

#### Scenario: Save without leaving the field

- **WHEN** today is `2026-11-08` and the user enters `261109` in Planned date and presses Ctrl+S
- **THEN** the valid task saves with due date `2026-11-09`

#### Scenario: Resolve the year at the action

- **WHEN** raw `0101` stays in Planned date across the transition from December 31, 2026 to January 1, 2027 and is then saved or normalized
- **THEN** it resolves to `2027-01-01`
- **AND** a value already normalized to `2026-01-01` before that transition keeps its explicit 2026 year

### Requirement: Wrap keyboard selection and jump to boundaries

Husker SHALL wrap configured Up and Down selection and support configured First and Last actions in the following selection modes.

| Mode | Selectable items |
| --- | --- |
| Ordinary task view | Tasks in the current visible query results. |
| History | Closed tasks in the current visible History results. |
| Trash | Tasks in the current visible Trash results. |
| Navigation list picker | All tasks, ordinary lists, and Trash in their existing order. |
| Editor list picker | Ordinary destination lists in their existing order. |
| Language picker | English, then Dansk. |

Up on the first item selects the last item. Down on the last item selects the first item.
First selects the first item and Last selects the last item without changing result order or applying a picker choice.
Default keys are Up or `k`, Down or `j`, Home or `g` for First, and End or `G` for Last.
An empty result has no selection and each movement action is a no-op. One item stays selected after every movement action.
Group headings remain unselectable and do not count as items.
Selection uses the filtered visible results. After wrapping or jumping, the selected target is visible within the existing scrollable layout.
Selection does not save tasks, choose a list, change the interface language, or alter task and list positions.
Refresh retains its existing identity-preservation and missing-selection clamping rules. It does not wrap a removed selection.
Mouse wheels remain bounded. Text cursor movement, Help and Copy text scrolling, and Alt+Up/Alt+Down reordering retain their bounded behavior.
First and Last have no selection action in text fields, the closed List selector, Help, Copy text, or confirmation screens.

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

- **WHEN** a selection view has zero or one item and receives Up, Down, First, or Last
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
