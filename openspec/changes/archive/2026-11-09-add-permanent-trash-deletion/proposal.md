## What Changes

1. In Trash, the `trash` action (`d` or Delete) opens a Delete forever confirmation. Yes deletes the selected task permanently.
2. A new `empty_trash` action, bound to `D` by default, works while Trash is open. It opens an Empty Trash confirmation, and Yes permanently deletes every task in Trash. If Trash is empty, the action shows `Trash is empty` and opens no confirmation.
3. In the list picker, the `delete_list` action (`d` or Delete) on the Trash entry opens the same Empty Trash confirmation.
4. Both confirmations select No by default, accept `y` and `n`, and use the keys of the list-deletion confirmation.
5. Task storage adds two guarded operations. Deleting one task requires the task to be in Trash and requires its current version. Emptying Trash revalidates a snapshot of task IDs and versions inside one write transaction.
6. Read-only and list-scoped stores reject both operations and the Trash preview that Empty Trash uses.
7. A successful permanent deletion discards the undo entry, because the deleted task cannot be restored.
8. Trash footer hints, the picker hint for the Trash entry, and Help show Delete forever and Empty Trash in English and Danish.

## Why

Trash only grows. Husker has no way to remove a task from the database, so old tasks stay in Trash and in the Trash count forever.
The user asked for a way to clear Trash and to delete single tasks permanently.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `task-storage`: Permanent deletion of one Trash task and of the whole Trash, with version and snapshot checks and access restrictions. The Trash requirement no longer states that Husker never deletes tasks.
- `terminal-interface`: Delete forever and Empty Trash actions, their confirmations, footer hints, Help entries, the Trash entry in the picker, and undo behavior.
- `profiles-localization`: The new `empty_trash` action and its default. The deletion confirmation mode now covers the two Trash confirmations.

## Impact

Release 0.13.0 keeps database schema 4. No migration is needed.
Permanently deleted tasks cannot be recovered. Task IDs are not reused, because the `tasks` table already uses `AUTOINCREMENT`.
An existing custom binding of `D` to another task-navigation action now conflicts with `empty_trash`. Configuration loading reports the conflict before Husker starts.
To keep that binding, the user rebinds `empty_trash` to another key or disables it with `empty_trash = []`.
CLI commands, JSON fields, MCP tools, and filter syntax remain unchanged. MCP clients can still move tasks to Trash but cannot delete tasks permanently.
