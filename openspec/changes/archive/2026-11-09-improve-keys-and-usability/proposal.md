## Why

The most-used keys are not shown on screen, Help lists every action in configuration order, and several frequent actions need long key sequences.
Users asked for a completed task to stay visible briefly, for Esc to clear a filter, and for fewer Shift and Alt combinations.

## What Changes

1. Print key names in display form in every hint and in Help: Ctrl+S, Esc, F4, Space, PageUp.
2. Group Help by screen, show the current screen first, and list the built-in text-editing keys.
3. Show the most-used keys in the footer of every task view on up to two hint lines.
4. Accept lowercase `n` and `r` in the list picker beside `N` and `R`.
5. Add `page_up` and `page_down` actions, and let First and Last scroll Help.
6. Add `K` and `J` as default alternates for `move_up` and `move_down`.
7. Accept `y` and `n` in the discard and list-deletion confirmations.
8. Add Ctrl+C as a default `quit` shortcut in navigation modes.
9. Document that Esc belongs to `back`, `discard`, and `close`.
10. Hold a task that becomes done or cancelled in its row until the view changes.
11. Make Back clear an active filter before leaving a scoped view.
12. Make Enter save the task from Title and Planned date.
13. Add a `move_task` action, bound to `m`, that moves the selected task to another list.
14. Add an `undo` action, bound to `u`, for the last task action, and report cleared dates.
15. Preselect the most recently visited list when the picker opens from All tasks.
16. Omit the planned-date row suffix under headings that already show that date.
17. Use Created, Marked done, Marked cancelled, Moved to Trash, Restored, and Planned for messages, and show the filter syntax hint.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `terminal-interface`: Keyboard defaults, footer hints, Help layout, Enter in the editor, filter clearing, held closed tasks, task moves, undo, confirmation keys, picker preselection, row suffixes, and result messages.
- `profiles-localization`: New `page_up`, `page_down`, `move_task`, and `undo` actions, changed defaults, key display names, and the modes that accept the new actions.
- `task-storage`: Guarded list-only and status-restore mutations used by task moves and undo.

## Impact

Release 0.12.0 keeps database schema 4. Updating from 0.9.0 through 0.11.1 requires no migration.
Existing custom bindings for `n`, `r`, `K`, `J`, `m`, `u`, Ctrl+C, PageUp, or PageDown in an affected mode become conflicts. The existing validation reports them before startup, and the guide names the override or disabling fix.
Footer and Help text changes. Automated checks that match raw key tokens such as `ctrl+s` must expect `Ctrl+S`.
CLI commands, JSON fields, MCP tools, and filter syntax remain unchanged.
