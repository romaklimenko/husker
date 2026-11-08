## Why

Make completed work and list removal clear during daily use.
Completed tasks remain stored, but the default view hides older records and groups today's completions under their due dates.
A list showing zero open tasks can still contain completed or trashed records.

## What Changes

- Rename the root view to All tasks and use Esc or the picker for root navigation.
- Retain existing `planned` configuration overrides through an alias for `all_tasks`.
- Keep open tasks in due-date groups and show completed work in separate status sections.
- Add a searchable History view for current done and cancelled records, with their status dates.
- Keep selection near remaining open work after completing or cancelling a task.
- Delete a nondefault list only after confirmation, moving all its tasks to the default list.
- Recheck the deletion preview before writing and preserve drafts when another process removes a list.
- Show the default list and relevant History, rename, and delete shortcuts in the interface.
- Add an F3 language picker that saves English or Danish without replacing custom shortcuts or losing an open draft.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `planning-search`: Separate due classification from display groups and define closed-task history ordering.
- `task-storage`: Preview and atomically remove a list while retaining every task.
- `terminal-interface`: Add History, list deletion, completion selection, the All tasks navigation label, and a language picker.
- `profiles-localization`: Add the new actions, preserve old root-navigation overrides, and save language choices to the active configuration.
- `command-line`: Clarify open and closed task grouping after removing a due date.
- `mcp-server`: Clarify the same grouping for partial task updates.

## Impact

Release 0.8.0 retains database schema 3. Existing records require no migration.
Ordinary search ordering changes consistently in the terminal, CLI, and MCP.
Due filters, task fields, CLI commands, and MCP tool schemas retain their meanings.
No CLI or MCP list-deletion command is added in this batch.
No installed executable or personal task data is changed during development.

## Scope

History lists current closed records. It is not an immutable activity log.
Repeated completions, previous edits, archived lists, and list restoration are outside this change.
