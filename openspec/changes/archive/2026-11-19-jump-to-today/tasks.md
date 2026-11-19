## 1. Start task

- [x] 1.1 In `src/ui.rs`, add `App::first_row_from_today`, which returns the first row of the first displayed group whose key is not `overdue` in All tasks and list views, and row 0 otherwise. Add `task_selection`, which makes First and Down on the last row select that row. Use it for task navigation only.
- [x] 1.2 In `src/i18n.rs`, change the help label of the First action to `Select first item, skipping Earlier` and `Vælg første punkt, spring Tidligere over`.
- [x] 1.3 Add UI tests in `src/ui.rs`: Left, Home, and `g` select the first task under Today with tasks under Earlier; Down on the last task wraps to it; Up from it selects the last task under Earlier; with no task under Today, First selects the first task under a later heading; with every task under Earlier, First and Down select the first task; a held row under Earlier is skipped; Down on a last row that is the start task keeps it; History and Trash still select row 0; a list view follows the same rule.

## 2. Documentation and version

- [x] 2.1 Update `docs/usage.md` and `README.md` for the new First and Down behavior.
- [x] 2.2 Raise the version to 0.21.0 in `Cargo.toml` and `Cargo.lock`.
- [x] 2.3 Record the checks for version 0.21.0 in `docs/development.md`.

## 3. Verification

- [x] 3.1 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, and `openspec validate jump-to-today --strict`.
- [x] 3.2 Run the release executable in a pseudo-terminal against a temporary profile with tasks under Earlier, Today, and Tomorrow. Confirm that Left and Down on the last task select the first task under Today, and that Up from it selects the last task under Earlier.
- [x] 3.3 Obtain an independent review, fix its findings, and rerun the affected checks.

## Verification evidence

Formatting, Clippy, 399 unit tests, and the CLI, MCP, online, sync, and package integration tests passed.
The 0.21.0 release executable passed the key checks in an isolated psmux pseudo-terminal: Left, Home, `g`, and Down on the last task selected the first task under Today, and Up still reached Earlier.
The code review found no significant issue. Its test notes and the docs review's spec note were fixed.
Linux and macOS were not verified.
