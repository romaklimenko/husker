## Why

Up and Down stop at the ends of every selection list. Up on the first item keeps it selected, and Down on the last item keeps it selected.
Left and Right still jump: Left to the first task after Earlier, and Right to the last open task.

In version 0.22.0, Up on the first task wraps to the last open task, and Down on the last task wraps to the first task after Earlier.
The user often walks up from today's first task to the oldest task under Earlier. One Up too many then jumps to the bottom of the view.
Left and Right already reach both ends in one key press, so the wrap no longer saves any steps.

## What changes

1. In All tasks and list views, Up on the first task keeps it selected. Down on the last task keeps it selected.
2. History and Trash follow the same rule.
3. The navigation list picker, the editor list picker, and the move-task picker follow the same rule.
4. In the language picker, Up selects English and Down selects Dansk. Neither wraps to the other choice.
5. First and Last do not change. Left, Home, and `g` select the first task after Earlier. Right, End, and `G` select the last open task.
6. PageUp, PageDown, the mouse wheel, and Help scrolling do not change. They already stop at the ends.
7. The selection when a view opens, when the filter changes, or after a refresh does not change.
8. Help labels do not change. They name Up and Down `Move up` and `Move down`.

## Capabilities

### New capabilities

None.

### Modified capabilities

- `terminal-interface`: the requirement Wrap keyboard selection and jump to boundaries becomes Stop keyboard selection at the ends and jump to boundaries. The requirements Use keyboard actions, Change language without leaving the current screen, Keep editor list picking modal, and Select the start item when no task is known follow the new name and rule.

## Impact

| File | Change |
| --- | --- |
| `src/ui.rs` | Up and Down stop at the first and last item in task views and pickers. The UI tests expect the new rule. |
| `docs/usage.md`, `README.md` | The new Up and Down behavior |
| `docs/development.md` | The checks for version 0.23.0 |
| `Cargo.toml`, `Cargo.lock` | Version 0.23.0 |

Compatibility effects:

1. A user who pressed Up on the first task to reach the last open task now presses Right, End, or `G`.
2. A user who pressed Down on the last task to reach today's first task now presses Left, Home, or `g`.
3. Custom keys for the `up` and `down` actions follow the new rule.
4. No configuration, database, sync format, CLI, MCP, online server, or phone page change is needed. No migration is needed.
