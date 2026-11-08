## Why

Remove the noneditable description sidebar and make task editing fit the terminal width.
Keep group headings visually separate from selection and let users choose the order of their lists.
Clear transient status messages after five seconds while retaining external-update notices.

## What Changes

| Area | Result |
| --- | --- |
| Task navigation | Use the full terminal width without a description sidebar. Highlight only task rows. |
| Description editor | Wrap words to the field width without inserting newlines into the saved text. |
| Cursor and mouse | Map movement and clicks to wrapped description rows. Keep the cursor visible after typing and resizing. |
| List order | Pin the original default list first and Trash last. Alt+Up and Alt+Down move other lists. |
| New lists | Append new lists before Trash. Renaming keeps their saved position. |
| Status messages | Clear action results and errors after five seconds. Keep external-update notices until the user refreshes. |

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `terminal-interface`: Full-width tasks, separate heading styles, wrapped editing, manual list ordering, and timed status messages.
- `task-storage`: Persistent list positions, atomic list moves, and schema version 3.
- `command-line`: Return saved list order with the default list first and Trash last.
- `mcp-server`: Return the same persistent list order through MCP reads.

## Impact

Version 0.4.0 upgrades schema version 2 to version 3 on writable startup.
Existing ordinary lists start in their previous alphabetical order, after the default list.
The migration does not change task fields or task versions.
Schema version 1 still upgrades through the existing tag-preserving migration in the same transaction.
Read-only connections do not migrate profiles.

Close older Husker processes and MCP clients before upgrading a real profile.
The installed version and personal profile remain unchanged during development.
The MCP tool names, task inputs, and list result fields remain unchanged.
