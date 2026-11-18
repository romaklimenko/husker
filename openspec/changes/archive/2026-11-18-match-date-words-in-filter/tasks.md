## 1. Day names in the filter

- [x] 1.1 In `src/query.rs`, store text terms with a `literal` flag for wholly quoted and `#` terms, and add `Query::with_group_names`. Pass the view to `Query::matches` as a `GroupLayout`. An unquoted term also matches when a word of the day name starts with it: Earlier, Today, Tomorrow, or Sometime from the planned date in All tasks and list views, Today or Yesterday from `status_changed_on` in History, and no name in Trash.
- [x] 1.2 In `src/ui.rs`, filter task rows in `App::rebuild_visible` with the group names of `App::language`. Rebuild the rows in `App::set_language` and keep the selected task.
- [x] 1.3 Add a query unit test for `to`, `tod`, `ear`, `some`, `soon`, `day`, two terms, quoted terms, no group names, the Danish names, a closed task planned tomorrow, History by closing day in English and Danish, and Trash.
- [x] 1.4 Add UI tests: typing `to`, `tod`, and `tom` narrows the rows, a completed task stays held under `tom`, a quoted `"to"` matches only text, and a language change rebuilds an applied filter and keeps the selected task.

## 2. Picker border

- [x] 2.1 In `draw_list_popup` in `src/ui.rs`, draw the bottom border title only when the picker filter has text.
- [x] 2.2 Change the picker UI tests to check that `Type to filter` and `Skriv for at filtrere` appear once, and that the popup's bottom border row holds no text.

## 3. Documentation and version

- [x] 3.1 Update `README.md`, `docs/usage.md`, and `docs/mcp.md` for the day names and the picker border, and review them with the docs writer.
- [x] 3.2 Raise the version to 0.20.0 in `Cargo.toml` and `Cargo.lock`.
- [x] 3.3 Record the checks for version 0.20.0 in `docs/development.md`.

## 4. Verification

- [x] 4.1 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, and `openspec validate match-date-words-in-filter --strict`.
- [x] 4.2 Run the release executable in a pseudo-terminal against a temporary profile. Confirm `to` and `tod` filter by Today and Tomorrow, and the list picker border is empty until the user types.
- [x] 4.3 Obtain an independent review, fix its findings, and rerun the affected checks.

## Verification evidence

Formatting, Clippy, 395 unit tests, and the CLI, MCP, online, sync, and package integration tests passed.
The 0.20.0 release executable passed 21 checks in an isolated psmux pseudo-terminal against a temporary profile.
An independent review found no code defect. Its finding that History contradicted its headings was fixed: History matches Today and Yesterday from the closing day, and Trash matches no names.
The editor and move-task picker borders in a live terminal, Linux, and macOS were not verified.
