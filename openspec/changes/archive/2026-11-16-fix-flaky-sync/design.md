## Context

Measured on November 16, 2026: `cargo test --test sync` failed in 4 of 6 runs on version 0.18.0.
`two_profiles_show_the_same_tasks_after_husker_sync` printed `Received 2, sent 1` where it expects `sent 2`.
`an_agent_on_a_new_machine_reads_and_writes_a_synced_list` timed out after 20 seconds, waiting for the agent's task in the sync file.

Read from source: `merge` in `src/sync/merge.rs` gave a conflict copy the local position of the task.
Step 6 then ordered tasks by `(position, uid)` and raised each position that did not exceed the previous one.
The original's `uid` is random, and the copy's `uid` is a hash. Which of the two moved down was therefore random.
When the sync file had moved the original, the copy kept the old local position and could land before the original or far from it.

Measured with temporary logging in `src/sync/worker.rs`: in each failed run the worker read its first data version and never saw it change.
In each passing run the data version moved once, and the worker synced 2 seconds later.
Read from source: `SyncWorker::spawn` opens its own connection on a new thread. The MCP server opens its connection and answers requests at the same time.
An `add_task` that commits before the worker reads its first data version is part of that first value.
No `Sync failed` line appeared in any run, so the sync itself never failed.

## Goals / Non-goals

Goal: the merge result and the upload count of a text conflict do not depend on random identifiers.
Goal: a conflict copy comes directly after its original, also when the sync file moved the original.
Goal: an MCP server sends every local change about 2 seconds after the last commit, also a commit made while its sync thread starts.

Out of scope:

1. The order of two tasks from different devices with equal positions. It stays ordered by `uid`, as the scenario Reorder two tasks created on two devices requires.
2. Positions of conflict copies created by earlier versions.
3. The list, planned date, and status of a conflict copy. They stay the local values, so a copy can land in another group of the planning view.
4. The terminal's sync thread. It syncs at start, which already covers earlier commits.
5. The online server, which does not merge.

## Decisions

Give a conflict copy the position that the merge gives its original task.
The copy then shares the original's position even when the sync file moved the original.

Order a conflict copy that the merge created by its original's `position` and `uid`, right after the original.
Step 6 sorts by `(position, sort uid, created copy, uid)`. The sort uid is the original's `uid` for a new copy and the task's own `uid` otherwise.
The original keeps its position, and the copy takes the next one. No other task can sort between the two.
Later tasks with consecutive positions move down by one, as they did before when the copy sorted last.
In the planning view the copy is directly below the original when both have the same planned date and status.

Rejected: give the copy the highest position plus one.
No other task would move, but the copy would leave its original and appear at the end of the date group.
The user resolves a conflict by comparing the two tasks, so they belong together.

Rejected: sort a new copy after every task at its position.
Another task with the same position, such as a task that the other device added, could then sit between the original and its copy.

Rejected: change the test to accept `sent 1` or `sent 2`.
The test would pass, but devices would still upload a moved original in half of the conflicts.

When the worker starts without a sync at start, it reads its first data version and then calls `Store::sync_pending`.
If the database holds unsent changes, the worker schedules a sync after the debounce period, as for a local change.
A commit between the two calls is caught by both, and that only schedules the same sync twice.
A commit after them moves the data version, as before.
An error from `sync_pending` also schedules a sync. The sync then fails and reports the error, so the problem does not stay hidden until the 60-second timer.

Rejected: open the worker's connection on the calling thread before `spawn` returns.
This closes the gap inside one process, but not for a terminal on the same profile that commits between the MCP server's start sync and the worker's start.

## Risks / Trade-offs

A server that starts without a network now tries again after 2 seconds instead of 60 seconds.
It writes a second `Sync failed` line to stderr at that point. The start already wrote the first line.

A new conflict copy can still move later tasks with consecutive positions down by one.
That happened before as well. Each moved task counts as one sent change.

The integration test and the device test in `src/sync/tests.rs` use random `uid` values from the store.
Without the fix they fail in about half of the runs, not in every run.
The unit tests in `src/sync/merge.rs` use fixed identifiers and fail every time without the fix.

No migration is needed. Reinstall the prior executable to roll back.
