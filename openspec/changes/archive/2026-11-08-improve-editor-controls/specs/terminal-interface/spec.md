## MODIFIED Requirements

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

## ADDED Requirements

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
Mouse clicks keep their existing cursor-placement behavior. Mouse selection and system clipboard commands are outside this requirement. F4 remains the copy-text workflow.

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
