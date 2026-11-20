## 1. End task

- [x] 1.1 In `src/ui.rs`, add `App::last_open_row`, which returns the last visible open task in All tasks and list views, and the last row otherwise or when no task is open. Make `task_selection` take that row and select it for Last and for Up on the first row. Use it for task navigation only.
- [x] 1.2 In `src/i18n.rs`, change the help label of the Last action to `Select last item, skipping done and cancelled` and `Vælg sidste punkt, spring udførte og annullerede over`.
- [x] 1.3 Add UI tests in `src/ui.rs`: Right, End, and `G` select the last open task with tasks under Done today and Cancelled today; Up on the first task under Earlier wraps to it; Down from it selects the first done task; with no open task, Last and Up select the last task; a held row after the last open task is skipped; Up on a first row that is the end task keeps it; History and Trash still select the last row; a list view follows the same rule.

## 2. Documentation and version

- [x] 2.1 Update `docs/usage.md` and `README.md` for the new Last and Up behavior.
- [x] 2.2 Raise the version to 0.22.0 in `Cargo.toml` and `Cargo.lock`.
- [x] 2.3 Record the checks for version 0.22.0 in `docs/development.md`.

## 3. Verification

- [x] 3.1 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, and `openspec validate jump-to-last-open-task --strict`.
- [x] 3.2 Run the release executable in a pseudo-terminal against a temporary profile with tasks under Earlier, Today, Tomorrow, and Done today. Confirm that Right and Up on the first task select the last open task, and that Down from it selects the first task under Done today.
- [x] 3.3 Obtain an independent review, fix its findings, and rerun the affected checks.
