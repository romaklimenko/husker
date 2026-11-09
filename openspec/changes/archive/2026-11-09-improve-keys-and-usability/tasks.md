## 1. Specification

- [x] 1.1 Record the 0.12.0 contract in proposal, design, and the terminal-interface, profiles-localization, and task-storage deltas.
- [x] 1.2 Strictly validate improve-keys-and-usability before implementation.

## 2. Configuration and storage

- [x] 2.1 Add `page_up`, `page_down`, `move_task`, and `undo` actions, the changed defaults, key display names, and the primary-key lookup to src/config.rs with tests.
- [x] 2.2 Add the list-only and status-restore guarded mutations to src/store.rs with scope, read-only, no-op, and version tests.
- [x] 2.3 Add the English and Danish strings for Help sections, built-in keys, footer labels, and result messages to src/i18n.rs. Remove unused strings.

## 3. Terminal interface

- [x] 3.1 Rebuild Help as sections with the opening screen first, built-in editing keys, and scrolling by Up, Down, PageUp, PageDown, First, Last, and the wheel.
- [x] 3.2 Pack footer hints by width with the most-used keys for every screen, the filter syntax hint, and the list-name hint.
- [x] 3.3 Add PageUp and PageDown paging to task views, both list pickers, and Copy text.
- [x] 3.4 Accept `y` and `n` in the discard and list-deletion confirmations.
- [x] 3.5 Hold a just-closed task in its row with its previous heading until the view changes.
- [x] 3.6 Make Back clear an active filter before leaving a scoped view or History.
- [x] 3.7 Make Enter save from Title and Planned date.
- [x] 3.8 Add the move-task picker and the undo action with result messages.
- [x] 3.9 Preselect the most recently visited list in the navigation picker.
- [x] 3.10 Omit planned-date suffixes under dated headings and Sometime.
- [x] 3.11 Use Created, Marked done, Marked cancelled, Moved to Trash, Restored, Planned for, and cleared-date messages.

## 4. Documentation and verification

- [x] 4.1 Update Cargo.toml to 0.12.0 and README.md, docs/usage.md, docs/mcp.md, and docs/development.md for the new keys, messages, and the three Esc actions.
- [x] 4.2 Run formatting, Clippy, all tests, a locked release build, and isolated terminal and CLI checks.
- [x] 4.3 Complete an independent review, address findings, and rerun affected checks.
- [x] 4.4 Archive the change and strictly validate all canonical capabilities.
