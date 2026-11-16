## 1. Conflict copy order

- [x] 1.1 In `merge` in `src/sync/merge.rs`, give a new conflict copy the merged position of its original, and sort tasks in step 6 by position, then by the original's `uid` for a new copy and the own `uid` otherwise, then the copy after the original.
- [x] 1.2 Add a unit test in `src/sync/merge.rs` that creates conflict copies for many task `uid` values, covers a copy `uid` that sorts before the original, and asserts that the original keeps its position and the copy gets the next one.
- [x] 1.3 Add a unit test in `src/sync/merge.rs`: the copy follows an original that the file moved, and a task the file added at the same position does not come between the original and its copy.
- [x] 1.4 Tighten the copy position assertions in `src/sync/merge.rs` and `src/sync/tests.rs` from "next to" to "directly after".
- [x] 1.5 Change `two_profiles_show_the_same_tasks_after_husker_sync` in `tests/sync.rs` to expect `Received 2, sent 1` and to assert the positions of task 3 and its copy.

## 2. Sync thread start

- [x] 2.1 In `run` in `src/sync/worker.rs`, after the first data version is read, schedule a local change when `sync_at_start` is false and `Store::sync_pending` reports unsent changes or fails.
- [x] 2.2 Add unit tests in `src/sync/worker.rs`: a task committed before the worker starts, with a one-hour timer and no sync at start, reaches the in-memory sync file. A stored base that cannot be read is reported as a failure. A worker without unsent changes makes no request at start.

## 3. Version and documentation

- [x] 3.1 Raise the version to 0.18.1 in `Cargo.toml` and `Cargo.lock`.
- [x] 3.2 Say in `docs/sync.md` that a conflict copy is placed directly after the original.
- [x] 3.3 Record the checks for version 0.18.1 in `docs/development.md`.

## 4. Verification

- [x] 4.1 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `openspec validate fix-flaky-sync --strict`.
- [x] 4.2 Run `cargo test --test sync` 20 times and confirm that every run passes.
- [x] 4.3 Obtain an independent review, fix its findings, and rerun the affected checks.
