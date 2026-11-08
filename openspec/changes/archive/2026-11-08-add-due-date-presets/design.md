## Context

Read from source: tasks store an optional due date separately from status and status_changed_on.
Read from source: ordinary task ordering uses open due groups and closed status sections. History uses status-change dates.
Read from source: the task editor holds four TextBuffer fields, with Due date at index 2.
Read from source: update_task normalizes editable task text and writes multiple fields together.

## Decisions

### Resolve calendar presets from the local day at the keypress

Add DuePreset with Today, Tomorrow, NextMonday, and Clear variants.
Its resolve(today) helper returns a due date or None for Clear and accepts an injected date for tests.
Today uses the supplied date. Tomorrow adds one calendar day. NextMonday adds enough days to reach the next strictly future Monday.
A Monday therefore advances seven days. Use checked date arithmetic and return an error on overflow.
Do not interpret a calendar date as an elapsed 24-hour duration.

Before key dispatch, update the runtime date but retain the originally selected task identity and version.
Resolve a preset against that current date and apply it to that original task.
Refresh and clamp visible results afterward if the day changed.
Refreshing away the old selection before applying its action is rejected because it could update a different task at midnight.

### Use a due-only store operation for navigation

Add Store::set_due(id, version, Option<NaiveDate>) returning the current or updated task.
Check writable access, list scope, and the expected version inside the normal immediate transaction before considering a no-op.
If due is already equal, return the task without any write or version increment.
Otherwise update only due and increment version once. Preserve every other field exactly.
Reusing update_task with copied text is rejected because its normalization could change stored text during a date-only action.

Navigation and History apply the mutation immediately outside Trash.
After success, keep the same task selected by ID if it still matches. Otherwise clamp the previous index.
A conflict reports failure without overwriting newer data. No selected task means no action.
Closed tasks keep their status dates, timestamps, and status-group placement unless the active filter excludes their changed due date.

### Add a focused editor mode without changing other text input

Add DueField mode when a task editor's active field is index 2.
DueField inherits all normal editor actions and adds the four preset actions.
Keep the default characters literal in Title, Description, List, filter inputs, and list-name inputs.
Manual YYYY-MM-DD entry remains available in DueField.

TextBuffer::replace_all replaces the complete due field as one undo step, sets the cursor at the end, and clears selection.
Undo restores the previous text, cursor, and selection. Redo restores the replacement.
An identical replacement changes no buffer state or history.
Applying a draft preset does not save, advance focus, or change other fields. Existing Save and dirty-discard behavior remains in force.

### Keep shortcuts discoverable without adding a navigation header

Defaults are due_today=t, due_tomorrow=T, due_next_week=w, and clear_due=x.
All four appear in Help with their configured bindings and localized names.
While Due date is focused, replace the editor's existing second-row editing hint with the due-preset hint.
Other fields keep the existing editing hint. Do not add rows or persistent preset hints to other screens.

### Label task origins in all-list views

Prepend the current list name as [Name] before each rendered title in All tasks and History(None).
Keep the prefix when a query narrows those views to one list. Omit it in List(id), History(Some(id)), and Trash.
Read names from the refreshed list collection and normalize embedded line breaks as the breadcrumbs already do.
Render the prefix within the existing task row. Do not change stored text, editor contents, copy output, CLI output, or MCP output.

### Record the latest status transition time for same-day ordering

Add nullable UTC status_changed_at to Task and schema version 4.
Keep status_changed_on as the local calendar date used for headings, default visibility, and the first sorting key.
On an actual transition to done or cancelled, record the current UTC instant and current local date.
Changing done to cancelled or cancelled to done records a new date and time. Repeating the current closed status preserves both values.
Reopening clears both. Field edits, presets, Trash, restore, and list transfer preserve both.

Within a closed section and in History, sort local date descending, timestamp descending, then task ID descending.
Unknown days follow known days. Unknown times follow known times within the same day.
Keep store and UI manual reordering restricted to open tasks. Do not use manual positions to order closed tasks.
Task IDs remain only a deterministic tie-breaker because they do not reveal completion order.

The schema 3-to-4 migration adds the nullable timestamp with null values and changes no existing data or task versions.
Chain earlier migrations and this addition in the same writable transaction. Read-only access accepts only schema 4.
Reject newer databases. Older executable versions must stay closed during upgrade and cannot reopen the upgraded profile.
Expose the field as nullable RFC 3339 UTC in CLI/MCP JSON. No new command or tool argument is added.
This stores only the latest current closed-status transition. It does not create an immutable event log.

## Non-goals

No My Day property, new status value, CLI or MCP shortcut syntax, event log, or installed-app replacement is included.
Presets do not act from Trash, list pickers, confirmations, Help, Copy text, or the language picker.

## Verification

Test local calendar boundaries, Monday behavior, leap days, and overflow with injected dates.
Test exact due-only mutation, current-version no-ops, stale no-ops, scope, read-only rejection, and rollback.
Test navigation selection after ordering and filtering, History status preservation, and midnight task identity.
Test whole-field undo/redo, literal input elsewhere, inherited editor actions, configured shortcuts, and localized contextual hints.
Test timestamp transitions, no-op status requests, migration preservation and rollback, read-only upgrade errors, same-day ordering, legacy unknown times, and additive CLI/MCP JSON.
Run formatting, Clippy, all tests, release build, isolated migration/CLI/terminal checks, and strict OpenSpec validation before archiving.
