## Why

Make Husker easier to use while people and agents work on the same task list.
The user requested mouse control, simpler task fields, predictable save keys, visible refresh feedback, and recently completed tasks that remain visible.

## What Changes

| Area | Result |
| --- | --- |
| Input | Mouse selection, opening, scrolling, and editor focus. Escape cancels edits. Ctrl+Enter or Ctrl+S saves. |
| Copying | A borderless text view supports native terminal selection without decorative vertical lines. |
| Refresh | F5 refreshes the database. External writes produce a visible notice without replacing unsaved text. |
| Task fields | Remove readiness and structured tags. Undated tasks share one group. Hashtags are ordinary searchable text. |
| Visibility | Keep tasks completed or cancelled today in default results. Explicit status filters retain their meaning. |
| Ordering | Sort lists by case-insensitive name. Sort tasks by due date, followed by undated tasks. |
| Workflow | Use the Husker project list as the development inbox. Preserve legacy draft files. |

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `task-storage`: Schema migration, completion dates, alphabetical lists, and change detection.
- `planning-search`: One undated group, literal hashtags, and today's completed tasks.
- `terminal-interface`: Mouse control, copy view, refresh feedback, save keys, and list selection.
- `profiles-localization`: Updated default shortcuts and localized labels.
- `command-line`: Remove readiness and tag arguments and expose the simplified task fields.
- `mcp-server`: Remove readiness and tag inputs and document the new search defaults.

## Impact

Writable startup migrates schema version 1 to version 2 in one transaction.
The migration preserves task identities, list membership, status, Trash, and ordering.
It increments versions for tasks whose descriptions gain hashtags so pre-upgrade edits are rejected.
Tasks with unchanged descriptions keep their versions.
Existing tag values become literal hashtags in the description. Existing description text remains unchanged.
Older completion dates are unknown and remain empty.

Close older Husker processes and MCP connections before using the upgraded profile.
Older binaries cannot use schema version 2. Read-only startup does not migrate a database.
Removed CLI, MCP, and key configuration names are rejected instead of silently ignored.
The current installed executable and personal profile are not upgraded during development.
