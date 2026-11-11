## 1. Specification

- [x] 1.1 Record the 0.14.0 contract in proposal, design, and the terminal-interface delta.
- [x] 1.2 Strictly validate speed-up-task-capture before implementation.

## 2. List suggestion

- [x] 2.1 Add the suggested-list function to src/planning.rs with tests for the most common list, ties, the 10-task window, Trash, inaccessible lists, moved tasks, and the empty fallback.
- [x] 2.2 Use the suggestion in `App::new_task` for All tasks and keep the list of a list view, with a terminal test for both.

## 3. Editor order

- [x] 3.1 Name the editor buffer indexes in src/ui.rs and add the Title, List, Planned date, Description display order.
- [x] 3.2 Make Tab and Shift+Tab follow and wrap in the display order, with tests for both directions.
- [x] 3.3 Draw the fields in the display order with Description filling the remaining height, and keep mouse field hits aligned, with a rendering test.
- [x] 3.4 Show the localized Sometime placeholder in an empty Planned date, with English and Danish rendering checks.
- [x] 3.5 Update existing editor tests that depend on Tab order or field positions.

## 4. Documentation and verification

- [x] 4.1 Update Cargo.toml to 0.14.0 and README.md and docs/usage.md for the field order, the suggested list, and the placeholder.
- [x] 4.2 Run formatting, Clippy, all tests, a release build, and isolated terminal and CLI checks.
- [x] 4.3 Complete an independent review, address findings, and rerun affected checks.
- [x] 4.4 Archive the change and strictly validate all canonical capabilities.
