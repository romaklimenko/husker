## Why

A conflict copy now always comes directly after its original task in position order, and the original keeps its position.
An MCP server now also sends a change that an agent made before the server's sync thread was ready.

Two sync integration tests fail in about half of the runs on unchanged code.
Both failures come from real behavior, not from the tests.

1. A conflict copy gets the local position of the task. The merge then orders the two by `uid`, which is random. In half of the merges the original task moves down by one, and the upload counts it as a change. When the other device moved the task, the copy can also land before the original or away from it.
2. The sync thread of an MCP server notices local changes when SQLite's data version moves. A commit made before the thread reads its first data version never moves it. An agent's task made right after the server starts then waits for the 60-second timer instead of uploading after 2 seconds.

## What changes

1. A conflict copy gets the position that the merge gives the original task, not the local position.
2. After a merge, a conflict copy that the merge created sorts by its original's position and `uid`, directly after the original. The original task keeps its position, and the copy takes the next one.
3. When the sync thread starts without a sync at start, it compares the database with the last synced file. When they differ, or when the comparison fails, it starts a sync 2 seconds later, as for any other local change.
4. The two integration tests and new unit tests assert the fixed behavior: the copy directly after the original, also after a move, and an upload of a change committed before the thread started.
5. The version becomes 0.18.1.

## Capabilities

### New capabilities

None.

### Modified capabilities

- `device-sync`: the requirements A text conflict keeps both versions and Positions are unique after a merge place the conflict copy directly after the original task.
- `mcp-server`: the requirement Sync while serving also counts a commit made before the sync thread started as a local change.

## Impact

1. `src/sync/merge.rs`: give a new conflict copy the merged position and order it directly after its original, with unit tests.
2. `src/sync/worker.rs`: check for unsent changes when the thread starts, with unit tests.
3. `src/sync/tests.rs` and `tests/sync.rs`: assert the copy position and the deterministic upload count.
4. `docs/sync.md`: say that the copy is placed directly after the original.
5. `docs/development.md`: record the checks for version 0.18.1.
6. `Cargo.toml` and `Cargo.lock`: raise the version to 0.18.1.

No storage migration, sync file format change, configuration change, CLI change, or MCP tool change is required.
A conflict copy created by an earlier version keeps its position. Only new merges use the new order.
The terminal starts its sync thread with a sync at start, so its behavior does not change.
