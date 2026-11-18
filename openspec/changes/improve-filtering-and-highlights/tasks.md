## 1. Keys and modes

- [x] 1.1 In `src/config.rs`, add `Mode::Trash`, move `restore` and `empty_trash` to it, and let it permit every task-navigation action. In `src/ui.rs`, return `Mode::Trash` for navigation in Trash.
- [x] 1.2 In `src/config.rs`, add `binding_active` and use it in `action` and `validate`: printable keys are inactive in `Mode::Lists` and `Mode::EditorListPicker`, and a printable `refresh` key is active only in `Mode::Tasks`.
- [x] 1.3 Change the defaults: `refresh` to `r` and F5, `new_list` to Ctrl+N, `rename_list` to Ctrl+R, `delete_list` to Ctrl+D and Delete, and `help` to `?` and F1.
- [x] 1.4 Add `Config::keys_in` and `Config::primary_key_in` for the keys that work in one mode. Use them for picker footers, the external-update notice, and the Help sections for task views, the list picker, and everywhere.
- [x] 1.5 Add config unit tests: `r` resolves to refresh only in task navigation, to restore in Trash, and to reopen in History; printable keys resolve to nothing in both picker modes; `new_task = ["r"]` conflicts with refresh; `new_task = ["D"]` conflicts with `empty_trash` in Trash.

## 2. Live task filter

- [x] 2.1 In `src/ui.rs`, keep the query and selected task from before the filter input opened in `App::filter_origin`. Apply each valid edit at once, keep the last results for text that does not parse, and restore the saved state on Esc.
- [x] 2.2 Draw the filter input border red while its text does not parse.
- [x] 2.3 Add UI tests: typing narrows rows before Enter, an unclosed quote keeps the last valid rows and Enter shows the error, and Esc restores the earlier filter and selected task.

## 3. Group styles

- [x] 3.1 In `draw_navigation`, style headings and task rows of All tasks and list views by group key as the requirement Color planning groups by date and status lists.
- [x] 3.2 Add a UI test that reads the rendered buffer styles for Earlier, Today, Tomorrow, Done today, Cancelled today, a held row, and the selected row.

## 4. Picker filter

- [x] 4.1 Add a filter and displayed entries to `ListPicker`. Route printable keys and Backspace to the filter in the list picker, the editor list picker, and the move-task picker. Map selection, Open, mouse rows, refresh, and Help round trips through the displayed entries.
- [x] 4.2 Make Back clear a nonempty picker filter before it closes the picker. Clear the filter after creating or renaming a list. Show `Clear the filter to reorder lists.` for a reorder while filtering.
- [x] 4.3 Draw the bottom border hint or filter text, `No matching lists` for an empty result, and keep the unfiltered popup height. Add the strings to `src/i18n.rs` in English and Danish.
- [x] 4.4 Add UI tests: `hu` selects Husker and Enter opens it, a prefix match wins over an earlier substring match, shortcut letters type into the filter, Esc clears then closes, no match keeps the picker open on Enter, a click on a filtered row picks that row, and the move-task and editor pickers filter too.
- [x] 4.5 Update existing UI tests that used `n`, `r`, `d`, `l`, `?`, `K`, or `J` in a picker.

## 5. Documentation and version

- [x] 5.1 Update `README.md`, `docs/usage.md`, and `docs/sync.md` for `r`, the live filter, group colors, and picker typing, and review them with the docs writer.
- [x] 5.2 Raise the version to 0.19.0 in `Cargo.toml` and `Cargo.lock`.
- [x] 5.3 Record the checks for version 0.19.0 in `docs/development.md`.

## 6. Verification

- [x] 6.1 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `openspec validate improve-filtering-and-highlights --strict`.
- [x] 6.2 Run the release executable in a pseudo-terminal against a temporary profile. Confirm `r` refreshes in All tasks and restores in Trash, typing filters tasks before Enter, typing `hu` in the list picker opens Husker, and the Today heading renders black on yellow.
- [x] 6.3 Obtain an independent review, fix its findings, and rerun the affected checks.

## Verification evidence

Formatting, Clippy, 392 unit tests, and the CLI, MCP, online, sync, and package integration tests passed.
A Windows pseudo-terminal ran the 0.19.0 release executable against a temporary profile, and all 19 checks passed.
An independent review found three defects: a stale picker scroll offset, a hidden filter after a list deletion during filter input, and lost held rows on Esc. All three were fixed with tests and the checks rerun.
Colors in a real terminal window, mouse input in the pseudo-terminal, Linux, and macOS were not verified.
