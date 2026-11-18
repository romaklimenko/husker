## 1. Phrase terms

- [x] 1.1 In `src/query.rs`, record each text term's position, and add `Query::with_phrases`, which joins the longest run of two or more unquoted text terms at consecutive positions whose joined text starts a day name of the view. Call it once in `visible_tasks` and `visible_history`. Match a term against a day name from the start of any word of the name.
- [x] 1.2 Add query unit tests in `src/query.rs`. In Danish, cover `i mo`, `I  MO`, `i m`, `i morgen`, `i d`, `i` alone, `i mo pay`, `i mo milk`, a quoted `"i" mo`, `mo i`, and `buy milk` against `Buy oat milk`. Cover `i l:home mo` and `l:home i mo`, `i g` in All tasks, English `i mo` and `pay rent`, `i mo` without day names, `i g`, `i går`, `i d`, and `i mo` in History, and the same Danish and English results in Trash.
- [x] 1.3 Add a UI test in `src/ui.rs` that types `i mo` with Danish selected and checks that only the I morgen task stays listed.

## 2. Documentation and version

- [x] 2.1 Update `docs/usage.md` and `README.md` for the phrase rule.
- [x] 2.2 Raise the version to 0.20.1 in `Cargo.toml` and `Cargo.lock`.
- [x] 2.3 Record the checks for version 0.20.1 in `docs/development.md`.

## 3. Verification

- [x] 3.1 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, and `openspec validate match-day-name-phrases --strict`.
- [x] 3.2 Run the release executable in a pseudo-terminal against a temporary profile with Danish selected. Confirm `i mo` lists only the I morgen tasks.
- [x] 3.3 Obtain an independent review, fix its findings, and rerun the affected checks.

## Verification evidence

Formatting, Clippy, 398 unit tests, and the CLI, MCP, online, sync, and package integration tests passed.
The 0.20.1 release executable passed 7 checks in an isolated psmux pseudo-terminal with Dansk selected.
The reviews found that a field term did not separate words, that every view checked all five names, and that the run search was cubic. All three were fixed with tests.
Linux and macOS were not verified.
