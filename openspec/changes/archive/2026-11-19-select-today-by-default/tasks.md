## 1. Default selection

- [x] 1.1 In `src/ui.rs`, add `App::select_start`, `App::select_task_or_start`, and `App::reselect`. In `App::refresh` and `App::set_language`, select the start item when no task was selected. In `App::refresh`, select the saved task or the start item after the current list is deleted. Remove the unused saved row index from `PlannedBookmark`.
- [x] 1.2 In `src/ui.rs`, select the start item after each valid filter edit and after Enter in the filter input. In `clear_filter`, keep a visible selected task and select the start item otherwise. Skip both when the refresh left a deleted list. In `discard_typed_filter`, select the start item when no task was selected when the input opened.
- [x] 1.3 Add the UI test `default_selection_starts_after_earlier` in `src/ui.rs`. It covers startup, opening a list, each typed letter, Enter, Enter without an edit, clearing a filter with a visible task, with no result, and with a task that is gone, returning to a saved task that is visible and one that is gone, a refresh that keeps the row position of a gone task, Esc in the filter input of an empty view that receives tasks, a refresh that keeps a task, a deleted current list, and Enter that finds the current list deleted. Update the startup assertion in `first_and_down_wrap_skip_tasks_under_earlier` and the gone-task assertion in the planned bookmark test.
- [x] 1.4 Confirm that the new test fails when each of the eight code paths is undone.

## 2. Documentation and version

- [x] 2.1 Update `docs/usage.md` and `README.md` for the new default selection.
- [x] 2.2 Raise the version to 0.21.1 in `Cargo.toml` and `Cargo.lock`.
- [x] 2.3 Record the checks for version 0.21.1 in `docs/development.md`.

## 3. Verification

- [x] 3.1 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, and `openspec validate select-today-by-default --strict`.
- [x] 3.2 Run the release executable in a pseudo-terminal against a temporary profile with tasks under Earlier, Today, and Tomorrow. Confirm that startup, opening a list, and a typed filter select the first task after Earlier.
- [x] 3.3 Obtain an independent review, fix its findings, and rerun the affected checks.

## Verification evidence

Formatting, Clippy, 400 unit tests, and the CLI, MCP, online, sync, and package integration tests passed.
The 0.21.1 release executable selected `Pay rent` under Today at startup, kept the start task while a filter was typed and applied, selected it after opening a list, and selected it after clearing a filter with no result.
The code review found no significant bug. Its four findings were fixed, three with tests. The docs review found two requirements that still kept the saved row index and a scenario that raced the refresh; both were fixed in the spec delta.
Linux and macOS were not verified.
