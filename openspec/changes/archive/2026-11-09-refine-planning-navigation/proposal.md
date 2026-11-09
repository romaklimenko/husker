## Why

Use planning language for task dates and make long task lists quicker to navigate.
A planned day does not always represent a deadline. History needs completion dates without competing task-date suffixes.
Users need keyboard movement that wraps and direct jumps to the first or last selectable row.

## What Changes

- Show Planned date and Earlier in English, and Planlagt dato and Tidligere in Danish.
- Keep History grouped by completion or cancellation day and omit the planned-date suffix from its task rows.
- Wrap keyboard Up and Down at the ends of task views, both list pickers, and the language picker.
- Add configurable First and Last actions with Home or `g`, and End or `G`, as defaults in those selection modes.
- Keep mouse wheels, text editing, Help and Copy text scrolling, and manual reordering bounded.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `terminal-interface`: Define wrapping selection, first/last jumps, planning labels, and History row presentation.
- `profiles-localization`: Add configurable First and Last actions and the English/Danish planning labels.
- `planning-search`: Clarify the visible Earlier label while preserving existing due buckets and filters.

## Impact

Release 0.11.0 retains database schema 4 and all task fields. Updating from 0.9.0 or 0.10.0 requires no migration.
The stored `due` field, CLI flags, JSON fields, MCP arguments, `due:` filters, and existing `due_*` action names remain unchanged.
Existing `history` and `lists` actions retain `h` and `l` and the interface keeps list terminology.
New default First and Last keys can conflict with custom bindings in the same mode. Existing conflict validation reports those cases before startup.
Rebind the conflicting action, or disable the new default with `first = []` or `last = []` under `[keys]`.
No installed executable or personal task data changes during development.

## Scope

No new planning property, deadline field, sort order, status history, task operation, or list operation is included.
