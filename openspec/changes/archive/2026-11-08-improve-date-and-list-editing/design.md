## Context

Read from source: Bucket classifies dates for query filters, while planned_group supplies broad task sections.
Read from source: The existing date_label formatter includes a date, localized weekday, and ISO week.
Read from source: The editor currently stores a typed list name and resolves that name when saving.
Read from source: The shared strict date parser serves CLI, MCP, and query inputs.
Read from source: Each editor field retains its own undo history.

## Decisions

### Keep query buckets independent from headings

Retain Bucket, PlannedGroup, ordering, manual-reorder boundaries, and all due-filter ranges.
Render open tasks in Soon and Next under separate headings for each exact stored due date.
Keep Overdue and Sometime as aggregate headings. Keep older Done and Cancelled as aggregate headings.
Use stored due dates for open task headings and status_changed_on for completion and History headings.
Add date_label output to Today, Tomorrow, Done today, Cancelled today, and History Today and Yesterday.
Unknown status dates remain unknown. Do not attach an invented date to any aggregate or unknown heading.
All headings remain inert and use the existing flat heading/task row layout for scrolling and mouse hit testing.

### Parse compact dates only in the editor

Use a separate editor parser with an injected current local date.
Trim surrounding whitespace. Empty input means no due date.
Accept strict ISO YYYY-MM-DD or ASCII digit strings of length 8, 6, or 4.
Interpret eight digits as YYYYMMDD. Interpret six digits as YYMMDD with year 2000 + YY.
Interpret four digits as MMDD in the current local year at normalization or Save, without guessing the next future occurrence.
Refresh the injected date before keyboard and mouse actions. Raw MMDD crossing New Year uses the new year; an already normalized ISO year stays fixed.
Validate real calendar dates, including leap years. Other lengths, characters, and impossible dates fail.
The English error is `Invalid date '{value}'. Use YYYY-MM-DD, YYYYMMDD, YYMMDD, or MMDD.`
Keep CLI, MCP, and query parsing strict ISO. Add regression tests for rejecting compact external dates.

### Normalize only when leaving Due date or saving

Tab, Shift+Tab, or a mouse click to another editor field normalizes valid Due date input to YYYY-MM-DD through one whole-field undoable replacement.
A changed replacement puts the cursor at the end and clears the selection. Undo restores the exact original text, cursor, and selection.
An identical replacement creates no history entry. Empty input remains empty.
Invalid input stays exact when focus changes and produces an error only on Save.
Typing never inserts a mask or normalizes a partial input automatically, including after four digits.
Save parses the current input directly. A failed save keeps the entered text and other drafts intact.
The existing t/T/w/x presets remain available only in Due date and their existing task-navigation modes.
Show a grey YYYY-MM-DD placeholder only when the actual buffer is empty.
Show accepted formats in the existing Due date field title or hint without adding header rows.

### Store a list identity in the draft

Replace the typed List field with a read-only selector backed by the stable list ID.
Render the current list name and never expose an internal ID as editable or copied text.
The selector ignores typing, paste, text selection, and Copy selection.
Tab focuses List. Enter or the configured Lists action opens its picker. Clicking the List field also opens the picker.
Choosing another list is one undoable field change. Ctrl+Z and Ctrl+Y or Ctrl+Shift+Z undo and redo list choices.
Other fields retain their text, cursor, selection, and histories. Choosing a list does not save the task.
Dirty-discard checks include changes to the selected list ID. Undo back to the initial ID clears that list change.
Save uses the chosen ID and the originally read task version. A missing selected list must be replaced explicitly, never matched to another list by name.
External renames show the current name for the same ID. A renamed list does not make the draft dirty.

### Keep the editor picker separate from task navigation

Use a modal popup with ordinary lists in persistent order and the default-list marker.
Exclude virtual All tasks and Trash. Exclude list creation, renaming, deletion, and reordering.
Open with the draft's current list selected when present. Keep an unavailable selected ID in the draft until an explicit choice.
Up and Down select rows. Enter or a double-click chooses. Back or Lists cancels without changing the draft.
Single-click selects a row. Wheel events act only inside the popup. Borders and outside clicks do nothing.
Background fields do not receive mouse or keyboard input while the popup is open.
Refresh preserves the selected picker ID when present and otherwise clamps the previous index.
Help and the language picker return to the editor picker with its selection, scroll, and underlying draft intact.

### Reuse existing action names

Add a List field mode with normal Editor actions except Copy selection, plus Open and Lists.
Add an Editor list picker mode with Up, Down, Open, Lists, Back, Help, Language, Refresh, and Toggle mouse.
Use existing configured bindings and conflict validation. Add no action names or configuration keys.
Keep default Enter and l for opening the List picker, and Esc or l for cancelling it.

## Non-goals

No schema change, new task field, external compact-date syntax, new status order, or installed-app replacement is included.
List choices do not modify list metadata. The navigation list picker retains its management actions.

## Verification

Test English and Danish date headings, year/week boundaries, filter compatibility, and inert heading rows after scrolling.
Test all accepted date forms, current-year and 2000-based short-year rules, leap dates, malformed input, normalization timing, undo, and strict external parsing.
Test picker IDs, cancellation, choice undo, external rename/deletion, refresh, Help/language round trips, and no writes before Save.
Test placeholders without buffer or clipboard changes.
Run formatting, Clippy, all tests, a locked release build, isolated terminal/CLI checks, independent review, and strict OpenSpec validation before archiving.
