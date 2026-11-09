## Context

Trash is the `deleted` flag on a task row. The store has no operation that removes a task row (read from source: `src/store.rs`).
List deletion already uses a preview, a No-by-default confirmation, and revalidation inside an immediate transaction. It reports `Deleted`, `Changed`, or `Missing` instead of failing on a stale preview (read from source).
The `tasks` table declares `id INTEGER PRIMARY KEY AUTOINCREMENT`, so SQLite never reuses a deleted task ID (read from source).
Trash uses the task-navigation key mode (read from source: `src/config.rs`). The `trash` action does nothing on a task that is already in Trash (read from source: `task_action` returns early for deleted tasks).

## Goals / Non-Goals

**Goals:**

- Delete one task in Trash permanently after confirmation.
- Delete every task in Trash permanently after confirmation, from Trash or from the list picker.
- Delete nothing when the task or Trash changed after the confirmation showed it. Ask the user to confirm again.

**Non-Goals:**

- No CLI command and no MCP tool for permanent deletion.
- No automatic Trash expiry.
- No `VACUUM` and no `secure_delete`. SQLite may keep deleted bytes in free pages or the write-ahead log (WAL) until it reuses that space (assumed from SQLite documentation, not measured).
- No undo for permanent deletion.

## Decisions

### Reuse the `trash` action inside Trash

`d` or Delete on a task in Trash opens the Delete forever confirmation.
The key already means "delete" and did nothing in Trash before this change, so no binding changes.
Alternative rejected: a separate `delete_forever` action. It needs its own key in task-navigation mode, because Trash shares that mode with ordinary views.

### Add `empty_trash` bound to `D`

Add an `empty_trash` action with the default `D` in task-navigation mode. The action works only while Trash is open.
`D` is unused in task-navigation mode (read from source: `src/config.rs`) and reads as a stronger `d`.
Alternative rejected: a separate Trash key mode. It would change conflict validation for `restore` and every navigation action for a single new key.

### Delete on the Trash picker entry empties Trash

The `delete_list` action on the Trash entry opens the Empty Trash confirmation.
The picker already uses `delete_list` for removal and protects Trash from list deletion. Empty Trash is the only removal that makes sense for the Trash entry.
All tasks and the default list stay protected.

### Return outcomes for stale confirmations

`purge_task(id, version)` returns `Deleted`, `Changed(task)`, or `NotInTrash`. `empty_trash(preview)` returns `Emptied(count)` or `Changed(preview)`.
The terminal shows a changed confirmation again with No selected, as list deletion does. A stale confirmation never deletes data.
Alternative rejected: deleting the matching subset of a changed Trash. The user confirmed a count, so any difference needs a new confirmation.

### Reject scoped and read-only stores

Only an unrestricted writable store can delete permanently. MCP stores are scoped or read-only in normal use, and no tool exposes the operations.
The store checks access before reading Trash, as list deletion does.

### Share one confirmation mode and renderer

List deletion, Delete forever, and Empty Trash use one `DeleteConfirm` key mode, one key handler, and one dialog renderer.
Before this change, the mode was named `DeleteListConfirm`. The rename is internal. Every action in this mode is also active in task navigation, which validation checks first, so no key-conflict error names this mode.

## Risks / Trade-offs

- [A user deletes the wrong task] → No is selected by default, and the dialog shows the task title and says the deletion cannot be undone. A task must already be in Trash before it can be deleted forever.
- [Empty Trash deletes tasks hidden by a filter] → The dialog shows the full Trash count and says that it deletes every task in Trash, including hidden tasks.
- [A custom `D` binding conflicts] → Validation names both actions before startup. `docs/usage.md` shows how to rebind or disable `empty_trash`.

## Evidence

- `src/store.rs`: `CREATE TABLE tasks (id INTEGER PRIMARY KEY AUTOINCREMENT, ...)`.
- `src/store.rs`: `delete_list` revalidates `ListDeletionPreview` inside `TransactionBehavior::Immediate`.
- `src/ui.rs`: `task_action` ignores every action except `Restore` on a deleted task.
- `src/config.rs`: `Action::Trash` and `Action::Restore` are available in `Mode::Tasks`.
