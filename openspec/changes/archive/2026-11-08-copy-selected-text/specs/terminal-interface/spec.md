## MODIFIED Requirements

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

## ADDED Requirements

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
