## Context

Read from source: View currently separates Lists from task scopes and resets filters, selection, and scroll whenever set_view runs.
The existing list screen shares task navigation state and mouse row targets.
List forms and help return to navigation without remembering an overlay.

## Decisions

### Make the picker an overlay on a task view

Replace View::Lists with a ListPicker screen and keep Mode::Lists for existing action bindings.
The picker owns a separate selection and ListState.
Represent All tasks, ordinary list IDs, and Trash with distinct identities.
Opening the picker selects the current scope. Canceling leaves the task view unchanged.
Invoking the lists action while the picker is open closes it like Esc.
Choosing the current scope closes the picker without resetting its task view.

Draw the current task view behind a centered, bordered picker that fits the terminal.
Long lists scroll inside the picker. Only picker rows receive mouse targets while it is open.
Ignore clicks and wheel events outside the picker. Borders and labels are not actionable rows.
All tasks is a virtual row without an open-task count. Ordinary lists retain their counts and names.

### Restore the Planned parent on upward navigation

Capture a Planned bookmark only when leaving Planned for an ordinary list or Trash.
The bookmark contains the filter, parsed query, selected task ID, fallback task index, and task ListState.
Entering a different child scope starts with an empty filter and its first task selected.
Switching between children keeps the original Planned bookmark.
Esc from a child, the planned action, and choosing All tasks restore the bookmark against current database contents.
If the saved task no longer matches, clamp the saved index to the current results. Drawing clamps scroll to the available rows.
The planned action at Planned leaves the view unchanged. Esc at Planned remains a no-op.
These bookmarks last only for the running process.

### Keep list management inside the picker

List-name forms return to the picker after saving or cancellation.
Saving selects the created or renamed list by ID. Cancellation retains the previous picker selection and scroll.
Help opened from the picker returns to the picker.
Refresh preserves picker identity after insertion, renaming, or reordering and updates the underlying task view.
All tasks and Trash cannot be renamed or moved. The default ordinary list remains pinned before other ordinary lists.
The existing task-storage list-order rules continue to apply.

### Keep input modes and task contracts unchanged

Use existing actions and shortcuts, including lowercase l, p, Esc, N, R, and Alt+Up/Down.
Resolve hints through configuration and localize new labels in English and Danish.
Task editors, discard confirmation, filter editing, copying, and task operations retain their behavior.
All tasks does not appear in CLI or MCP list results and does not restrict ordinary list names.
Trash retains its ungrouped rows and existing restrictions on task actions.

## Non-goals

Persistent navigation history, per-list bookmarks, list search, new storage operations, and CLI or MCP changes are outside this change.

## Verification

Test upward navigation, same-scope selection, canceled picker and form operations, parent filters and scroll, and changed or removed saved tasks.
Test virtual rows, ordinary list identity after refresh, list management, help return, configured actions, and mouse targets after scrolling and resizing.
Run formatting, Clippy, all tests, a release build, strict OpenSpec validation, and an isolated interactive terminal check.
Keep the installed executable and personal profiles unchanged.
