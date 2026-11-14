## 1. Suggestion rule

- [x] 1.1 Change `suggested_list_id` in `src/planning.rs` to return the current list of the counted task with the highest ID, and remove `LIST_SUGGESTION_WINDOW`.
- [x] 1.2 Replace the unit tests in `src/planning.rs`: the newest task wins over a larger count, input order does not matter, Trash is skipped, closed and moved tasks count by their current list, inaccessible lists are ignored, and the lowest-ID list is the fallback.
- [x] 1.3 Update the editor test in `src/ui.rs` so Private holds more tasks but the newest counted task is in Work, and `n` in All tasks opens Work.

## 2. Documentation and version

- [x] 2.1 Update `README.md` and `docs/usage.md` to describe the newest-task rule, and review them with the docs writer.
- [x] 2.2 Raise the version to 0.16.0 in `Cargo.toml` and `Cargo.lock`.
- [x] 2.3 Record the checks for version 0.16.0 in `docs/development.md`.

## 3. Verification

- [x] 3.1 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `openspec validate suggest-newest-task-list --strict`.
- [x] 3.2 Run the release executable in a terminal against a temporary profile and confirm `n` in All tasks opens the newest task's list.
- [x] 3.3 Obtain an independent review, fix its findings, and rerun the affected checks.

## Verification evidence

Formatting, Clippy, strict OpenSpec validation, 286 unit tests, 9 CLI tests, and 8 MCP tests passed.
Two sync integration tests failed intermittently, the same way as on the unchanged 0.15.1 source.
A Windows pseudo-terminal ran the 0.16.0 release executable against a temporary profile.
All 7 checks passed: the newest task's list won over 9 Labs tasks, a newer Labs task took over, and a Labs task in Trash was skipped.
The installed 0.15.1 suggested Labs in the first check.
An independent review found a weak test and an ambiguous spec sentence. Both were fixed and the checks rerun.
The Danish interface, mouse input, Linux, and macOS were not verified.
