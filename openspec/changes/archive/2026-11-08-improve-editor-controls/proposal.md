## Why

Add familiar keyboard editing controls and protect unsaved task edits from accidental dismissal.
Keep open tasks ahead of closed tasks within each planning group and make task IDs available in the editor.
Rename the visible Undated group to Sometime so its label is easier to distinguish from Updated.

## What Changes

| Area | Result |
| --- | --- |
| Text inputs | Add word navigation and deletion, keyboard selection, replacement, and undo/redo. |
| Task editor | Show the task ID and confirm discarding changed fields. |
| Planning | Sort open tasks first within each group. Use Sometime in English and Engang in Danish. |
| Manual ordering | Prevent moves across the open/closed boundary. |
| Hints | Explain the editing shortcuts in English and Danish. |

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `terminal-interface`: Editing controls, selection display, task ID, and discard confirmation.
- `planning-search`: Open-first ordering and the Sometime display label.
- `task-storage`: Reordering stays within an open or closed category.
- `profiles-localization`: Validate action keys for the discard confirmation mode and preserve configured action precedence.
- `command-line`: Retain the `undated` output token while naming the terminal group Sometime.
- `mcp-server`: Refer to the Sometime terminal label when removing a date.

## Impact

Release version 0.5.0 keeps database schema 3 and all existing task fields.
The shared ordering applies to terminal, CLI, and MCP results.
The existing `due:none` and `due:undated` query values remain valid.
No installed executable or personal profile is upgraded during development.
