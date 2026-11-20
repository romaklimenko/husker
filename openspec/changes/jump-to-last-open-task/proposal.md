## Why

In All tasks and list views, Right, End, and `G` select the last open task. Up on the first task wraps to that same task.
Done and cancelled tasks below it stay reachable with Down and PageDown.

The user plans the day from the open tasks. Done today, Cancelled today, and the other status groups follow the open tasks at the bottom of the view.
In version 0.21.1, Right and Up on the first task both selected the last row, which is often a done or cancelled task.
The user asked for Up on the first task under Earlier, and for the Last action, to land on the last task that is not done.
This mirrors version 0.21.0, where First and Down on the last task skip the tasks under Earlier.

## What changes

1. In All tasks and list views, the Last action selects the end task. The end task, which the spec calls the end item, is the last visible open task. Last defaults to Right, End, and `G`.
2. Up on the first task selects the end task instead of the last task.
3. When no visible task is open, the end task is the last task, as before.
4. When the end task is also the first task, Up on it keeps it selected.
5. A held row is a task just marked done or cancelled that keeps its row and heading until the view changes. Last skips it, even though it shows under an open-task heading.
6. Down, First, PageUp, PageDown, and the mouse wheel do not change. History, Trash, the list pickers, and the language picker do not change.
7. The selection when a view opens, when the filter changes, or after a refresh does not change.
8. Help names the Last action `Select last item, skipping done and cancelled`, or `Vælg sidste punkt, spring udførte og annullerede over` in Danish.

## Capabilities

### New capabilities

None.

### Modified capabilities

- `terminal-interface`: the requirement Wrap keyboard selection and jump to boundaries defines the end item. The key table of the requirement Use keyboard actions names the new Last behavior.

## Impact

| File | Change |
| --- | --- |
| `src/ui.rs` | Task navigation finds the end task. Last and Up on the first task select it. |
| `src/i18n.rs` | The help label of the Last action in English and Danish |
| `docs/usage.md`, `README.md` | The new Last and Up behavior |
| `docs/development.md` | The checks for version 0.22.0 |
| `Cargo.toml`, `Cargo.lock` | Version 0.22.0 |

Compatibility effects:

1. A user who relied on Right, End, or `G` to reach the done and cancelled tasks now reaches them with Down from the end task, or with PageDown.
2. A custom key for the `last` action follows the new rule. The rule belongs to the `last` action, so every key bound to it behaves the same.
3. No configuration, database, sync format, CLI, MCP, online server, or phone page change is needed. No migration is needed.
