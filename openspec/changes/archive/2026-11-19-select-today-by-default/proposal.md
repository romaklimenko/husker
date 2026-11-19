## Why

Husker selects the start item whenever it shows rows without a known task to select. In All tasks and list views, the start item is the first task after the Earlier heading. Left selects the same task.
That covers startup, opening a list, typing or applying a filter, clearing a filter whose selected task is gone, returning to a view whose saved task is gone, and a refresh that fills an empty view.

Version 0.21.0 made Left, and Down on the last task, skip the tasks under Earlier. Startup, opening a list, and filtering still selected the first row, which is often under Earlier.
The user asked that Husker start on today's first task, and select it in every case where it has no task to keep.

## What changes

1. Startup selects the start item of All tasks.
2. Opening a list view, Trash, or History without saved state selects the start item. In History and Trash that is the first task, as before.
3. Each filter edit that leaves valid text, and Enter in the filter input, select the start item of the matching tasks instead of the first matching task.
4. Back that clears a filter keeps the selected task when it is still visible. Otherwise it selects the start item instead of keeping the row position.
5. Returning to a view whose saved task is no longer visible selects the start item instead of the saved row position. This includes All tasks after the current list is deleted.
6. A refresh that finds rows in a view that had no selected task selects the start item.
7. These do not change: a refresh keeps the selected task, or its row position when the task left the results. Esc in the filter input restores the earlier selection. Returning to a view selects its saved task while it is visible.

## Capabilities

### New capabilities

None.

### Modified capabilities

- `terminal-interface`: a new requirement, Select the start item when no task is known, lists every case. The requirements Filter tasks interactively, Navigate All tasks and lists, and Wrap keyboard selection and jump to boundaries refer to the start item instead of the first task. The requirements Restore All tasks after visiting a scoped view and Review retained completed and cancelled tasks select the start item instead of the saved row index when the saved task is gone.

## Impact

| File | Change |
| --- | --- |
| `src/ui.rs` | `App::refresh` selects the start item when no task is known. Typing and applying a filter, and clearing it without a visible task, select it too. |
| `docs/usage.md`, `README.md` | Describe which task Husker selects without a known task, and add the 0.21.1 update note |
| `docs/development.md` | The checks for version 0.21.1 |
| `Cargo.toml`, `Cargo.lock` | Version 0.21.1 |

Compatibility effects:

1. A user who relied on startup or a filter selecting the first row now presses Up, or PageUp, to reach the tasks under Earlier.
2. Returning to a view whose saved task is gone no longer keeps the saved row position.
3. No configuration, database, sync format, CLI, MCP, online server, or phone page change is needed. No migration is needed.
