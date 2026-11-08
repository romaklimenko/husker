## 1. Application foundation

- [x] 1.1 Add `Cargo.toml`, `src/lib.rs`, and shared task types in `src/model.rs`; verify Rust compilation.
- [x] 1.2 Implement versioned SQLite initialization and persistent lists in `src/store.rs`; test reopening, invalid schemas, names, and counts.
- [x] 1.3 Implement transactional task writes, Trash, version conflicts, and position swaps in `src/store.rs`; test persistence and failed writes.

## 2. Planning and configuration

- [x] 2.1 Implement date groups and deterministic ordering in `src/planning.rs`; test month, year, and seven-day boundaries.
- [x] 2.2 Implement filters in `src/query.rs`; test quoted text, Danish case matching, statuses, invalid syntax, and Trash visibility.
- [x] 2.3 Implement TOML settings and per-mode shortcuts in `src/config.rs`; test defaults, overrides, invalid settings, and conflicts.
- [x] 2.4 Add English and Danish catalogs in `src/i18n.rs`; test key parity and date labels.

## 3. Interfaces

- [x] 3.1 Implement paths, list commands, task commands, JSON output, and error handling in `src/main.rs`; test commands against isolated profiles.
- [x] 3.2 Implement list and task navigation, filtering, grouping, and scrolling in `src/ui.rs`; verify rendering and key actions.
- [x] 3.3 Implement task and list editors with save, discard, and retained error buffers in `src/ui.rs`; test Unicode and failed saves.
- [x] 3.4 Implement task actions, reorder, refresh, help, and terminal cleanup in `src/ui.rs`; test action handling and run an interactive smoke check.

## 4. Verification and documentation

- [x] 4.1 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`; resolve every failure.
- [x] 4.2 Exercise a multi-command workflow and a 10,000-task fixture; record measured results and the test environment.
- [x] 4.3 Have the docs-writer review specs and write README plus configuration guidance; execute documented commands.
- [x] 4.4 Review the implementation independently, fix findings, and rerun affected checks.
- [x] 4.5 Validate and archive the completed OpenSpec change; replace generated Purpose placeholders and validate canonical specs.
