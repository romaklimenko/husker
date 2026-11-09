## Decisions

### Change visible language without changing data

Use Planned date for the task date field and related visible hints and action labels.
Use Earlier for the aggregate heading containing open tasks planned before today.
Use Planlagt dato and Tidligere in Danish.
Retain the existing stored due field and date arithmetic. Keep external flags, JSON names, MCP inputs, filters, and existing action identifiers.
This avoids a schema migration for a terminology change.

Read from source: the English and Danish strings are centralized in src/i18n.rs.
Read from source: the due field and overdue bucket already serve CLI, MCP, query matching, and terminal planning.

### Show the recorded closure day in History

Keep History headings based on status_changed_on and ordering based on the existing status date, timestamp, and ID rules.
Hide the appended planned-date metadata from History rows only. Keep task titles, status markers, list prefixes, and all stored fields unchanged.
A task title that contains a date remains unchanged.
Keep the planned date in the editor and other task views, including Trash.
Unknown closure dates remain Unknown date and are never inferred from the planned date.

Read from source: History already groups by local completion or cancellation date.
Read from source: task-row rendering currently appends the stored due-date label in every view.

### Separate keyboard selection from bounded movement

Use wraparound selection for configured Up and Down in ordinary task views, History, Trash, both list pickers, and the language picker.
The first item followed by Up becomes the last item. The last item followed by Down becomes the first item.
Add First and Last actions for immediate boundary selection. Defaults are Home or g, and End or G.
Selection operates on the current visible results and excludes heading rows.
Refresh preserves identities and clamps missing selections as before. Keyboard wrapping does not change refresh recovery. Existing scroll state makes the selected target visible.
Empty collections do nothing. A single-item collection keeps that item selected.

Keep wheel movement bounded even where keyboard movement wraps. Keep all text cursor movement, Help and Copy text scrolling, and Alt+Up/Alt+Down reordering bounded.
Do not reuse a wrapping helper for these bounded operations.
The language picker retains English then Dansk as its choice order and requires Apply to save.

Read from source: task and picker selections use indices with ListState for scrolling.
Read from source: wheel handling and keyboard selection currently use related bounded movement paths.

### Configure First and Last only in selection modes

Add first and last action names with the existing override, disabled-array, hint, and conflict-validation rules.
Enable them in Task navigation, History, List picker, Editor list picker, and Language picker modes.
Do not enable them in the List field selector itself, text inputs, confirmations, Help, or Copy text.
Home and End keep their text-editing meaning in input fields. g and G remain ordinary input text.
Preserve History h and Lists l.

## Non-goals

No database change, new date field, sorting change, mouse-wheel wrapping, text-input wrapping, or History event log is included.
No installed executable replacement or personal-profile testing is included.

## Verification

Test default, overridden, disabled, and conflicting first/last bindings and mode availability.
Test wrapping and jumps for zero, one, and several rows, filtered tasks, heading rows, and scroll visibility.
Cover ordinary tasks, History, Trash, both list pickers, and the two language choices.
Verify bounded wheels, text editing, Help, Copy text, and manual reordering remain unchanged.
Verify both languages, History closure headings without planned-date suffixes, and unknown closure dates.
Run the standard full checks, isolated CLI/terminal checks, independent review, and strict OpenSpec validation before archiving.
