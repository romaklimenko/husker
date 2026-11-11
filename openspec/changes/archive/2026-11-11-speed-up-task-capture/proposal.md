## Why

Creating a task outside a list view usually takes a title, a list, and sometimes a planned date. The editor puts Description second, so most captures need an extra Tab.
New tasks in All tasks always start in the default list, although most new tasks belong to another list. Users must open the picker for almost every task.

## What Changes

1. Order the editor fields as Title, List, Planned date, Description. Tab and Shift+Tab follow this order.
2. Suggest the list for a new task created from All tasks. The suggestion is the most common list among the 10 most recently created tasks outside Trash.
3. Show the localized Sometime label as the empty Planned date placeholder instead of `YYYY-MM-DD`. The field title still lists every accepted date form.

A new task still starts with an empty title, description, and planned date. A list view still supplies its own list.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `terminal-interface`: Editor field order, Tab order, the list of a new task created from All tasks, and the empty Planned date placeholder.
- `task-storage`: The default-list rule for task creation now names CLI `add` and MCP `add_task` and refers to the terminal suggestion for All tasks.

## Impact

Release 0.14.0 keeps database schema 4. No migration is needed.
The rule reads task IDs and list IDs that Husker already stores. Task IDs increase with creation and are never reused.
CLI `add` and MCP `add_task` keep their current list defaults. Automation results do not depend on recent terminal activity.
Configuration files and key bindings stay unchanged. Tests or scripts that drive the editor with a fixed number of Tab presses must follow the new order.
