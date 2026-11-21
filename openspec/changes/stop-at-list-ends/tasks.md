## 1. Stop at the ends

- [x] 1.1 In `src/ui.rs`, make `navigation_selection` keep the first item for Up on the first item and the last item for Down on the last item. Make `task_selection` use the start task only for First and the end task only for Last. Update the comments of `first_row_from_today` and `last_open_row`.
- [x] 1.2 Update the UI tests in `src/ui.rs` that expect a wrap: filtered task rows in All tasks, a list view, History, and Trash; First and Down with tasks under Earlier; Last and Up with done and cancelled tasks; a list view without Earlier and one with only Earlier; the navigation picker; the editor picker; and the language picker. Each expects Up on the first item and Down on the last item to keep the selection.
- [x] 1.3 Add a UI test for the user's case: six tasks under Earlier, one under Today, one under Sometime. Six Up presses from `Pay rent` select the first Earlier task, a seventh keeps it, Left selects `Pay rent`, Right selects the Sometime task, and Down keeps it.

## 2. Documentation and version

- [x] 2.1 Update `docs/usage.md` and `README.md` for the new Up and Down behavior in task views, History, Trash, the list pickers, and the language picker.
- [x] 2.2 Raise the version to 0.23.0 in `Cargo.toml` and `Cargo.lock`.
- [x] 2.3 Record the checks for version 0.23.0 in `docs/development.md`.

## 3. Verification

- [x] 3.1 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, and `openspec validate stop-at-list-ends --strict`.
- [x] 3.2 Confirm that the new test fails with the 0.22.0 `navigation_selection` and `task_selection`.
- [x] 3.3 Run the release executable in a pseudo-terminal against a temporary profile with six tasks under Earlier, one under Today, one under Sometime, and one under Done today. Confirm that Up stops at the first task, Down stops at the last task, Left and Right still jump, and the list picker stops at both ends.
- [ ] 3.4 Obtain an independent review, fix its findings, and rerun the affected checks.
