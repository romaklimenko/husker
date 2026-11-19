## Why

In All tasks and list views, Left, Home, and `g` select the first task that is not under the Earlier heading. Down on the last task wraps to that same task.
Up and Down still move one row at a time, so Up from that task reaches the tasks under Earlier.

The user plans the day from Today down. Tasks under Earlier are planned for a past day and are often left for later.
In version 0.20.1, Left and Down on the last task both selected the first row, which is often under Earlier. The user then had to step down past those tasks to reach today's work.
The user asked for Left, and for Down on the last task, to land on the first task of today.

## What changes

1. In All tasks and list views, the First action selects the start task. The start task, which the spec calls the start item, is the first visible task whose row is not under the Earlier heading. First defaults to Left, Home, and `g`.
2. Down on the last task selects the start task instead of the first task.
3. When every visible task is under Earlier, the start task is the first task, as before.
4. When the start task is also the last task, Down on it keeps it selected.
5. A held row counts under the heading it shows. A task just marked done or cancelled under Earlier stays held under Earlier, so First still skips it.
6. Up, Last, PageUp, PageDown, and the mouse wheel do not change. History, Trash, the list pickers, and the language picker do not change.
7. Opening a view and typing a filter keep their selection rules. They select the first task, or a remembered task, as before.
8. Help names the First action `Select first item, skipping Earlier`, or `Vælg første punkt, spring Tidligere over` in Danish.

## Capabilities

### New capabilities

None.

### Modified capabilities

- `terminal-interface`: the requirement Wrap keyboard selection and jump to boundaries defines the start item. The key table of the requirement Use keyboard actions names the new First behavior.

## Impact

| File | Change |
| --- | --- |
| `src/ui.rs` | Task navigation finds the start task from the displayed date groups. First and Down on the last task select it. |
| `src/i18n.rs` | The help label of the First action in English and Danish |
| `docs/usage.md`, `README.md` | The new First and Down behavior |
| `docs/development.md` | The checks for version 0.21.0 |
| `Cargo.toml`, `Cargo.lock` | Version 0.21.0 |

Compatibility effects:

1. A user who relied on Left, Home, or `g` to reach the tasks under Earlier now reaches them with Up from the start task, or with PageUp.
2. A custom key for the `first` action follows the new rule. The rule belongs to the `first` action, so every key bound to it behaves the same.
3. No configuration, database, sync format, CLI, MCP, online server, or phone page change is needed. No migration is needed.
