## Why

Set common due dates without opening an editor or typing a date.
Show the most recently completed tasks first within the same day and identify task lists in all-list views.
The same shortcuts should also help while entering the editor's Due date field.
These actions change the existing due date. They do not add a separate daily-planning property.

## What Changes

- Add configurable Today, Tomorrow, Next week (Monday), and Clear due date actions with defaults `t`, `T`, `w`, and `x`.
- Apply a selected task's preset immediately in ordinary task navigation and History, outside Trash.
- Replace the Due date editor field as one undoable draft edit without saving or changing fields.
- Keep those letters as ordinary text in every other input field.
- Calculate dates from the local calendar day at the keypress, with next Monday always strictly in the future.
- Show the four actions in Help and show their configured hints only while the Due date field is focused.
- Show each task's list name before its title in All tasks and all-list History.
- Record the UTC time of status transitions so same-day completions sort newest first. Keep completed and cancelled tasks ineligible for manual reordering.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `planning-search`: Define local-calendar presets and order closed tasks by their recorded date and time.
- `task-storage`: Add guarded due-only writes and migrate completion timestamps without inventing old times.
- `terminal-interface`: Apply presets, preserve selection and drafts, display task-list prefixes, and use timestamp-aware History ordering.
- `profiles-localization`: Add configurable preset actions and a Due date field input mode.
- `command-line`: Add nullable completion timestamps to task JSON without changing commands.
- `mcp-server`: Add the same nullable field to task results without changing tool arguments.

## Impact

Release 0.9.0 uses database schema version 4.
Writable launches migrate versions 1, 2, and 3 atomically. Existing tasks receive a null `status_changed_at` because their exact completion times are unknown.
The version 3 migration preserves every existing field and task version. Earlier migrations keep their existing tag-conversion and list-order behavior.
Older binaries cannot open the upgraded database. Read-only connections require a writable upgrade first.
CLI and MCP task JSON gain nullable `status_changed_at`. Existing commands, tool arguments, and other task fields keep their meanings.
Existing ISO date entry remains valid. Custom shortcuts that conflict with a new default must be rebound or disabled.
English and Danish labels use the same stable action names.

## Scope

No My Day property, reminders, recurrence, date-picker calendar, or natural-language date parser is included.
The installed executable and personal task profiles remain unchanged during development.
