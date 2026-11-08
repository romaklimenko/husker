## 1. Specification

- [x] 1.1 Record the confirmed date forms, future headings, and editor list-picker contract.
- [x] 1.2 Complete the affected capability deltas and strictly validate the change before implementation.

## 2. Implementation

- [x] 2.1 Update `src/planning.rs` and `src/ui.rs` for future and known-day heading rendering with tests for labels, sorting, filters, and inert heading rows.
- [x] 2.2 Update `src/ui.rs` with the editor list picker with tests for draft preservation, selection, cancellation, refresh, and eventual Save behavior.
- [x] 2.3 Add the visual empty-date placeholder in `src/ui.rs` without changing stored values or buffer history.
- [x] 2.4 Add `src/editor_date.rs` and editor integration for compact manual date entry with date-boundary, invalid-input, canonical-format, and undo tests.
- [x] 2.5 Update `src/config.rs` and the catalogs in `src/i18n.rs` for focused modes and English/Danish hints without changing existing external date syntax.
- [x] 2.6 Update `README.md` and `docs/` for version 0.10.0 and verify examples using isolated profiles.

## 3. Verification

- [x] 3.1 Run formatting, Clippy, all tests, a locked release build, and isolated CLI/terminal checks.
- [x] 3.2 Complete independent review and address findings.
- [x] 3.3 Archive the change and strictly validate the canonical specs.
