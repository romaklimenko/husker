## 1. Specification

- [x] 1.1 Complete and strictly validate the six capability deltas before implementation.

## 2. Implementation

- [x] 2.1 Add checked DuePreset calendar calculations in planning.rs with boundary and overflow tests.
- [x] 2.2 Add Store::set_due with tests for exact field retention, version checks before no-op, scopes, read-only rejection, and rollback.
- [x] 2.3 Add TextBuffer::replace_all with whole-field undo/redo and exact no-op tests.
- [x] 2.4 Add DueField mode, configurable preset actions, conflict checks, and English/Danish labels in config.rs and i18n.rs.
- [x] 2.5 Apply presets in UI navigation, History, and the Due date field with selection, filter, midnight, undo, literal-input, and contextual-hint tests.
- [x] 2.6 Update README and guides for version 0.9.0 and verify runnable examples with isolated profiles.
- [x] 2.7 Add current-list prefixes to All tasks and all-list History rows with tests for scoped omissions, filters, renames, Unicode names, and unchanged copy/editor text.
- [x] 2.8 Add nullable UTC status_changed_at and atomic schema-4 migration with tests for earlier migration chains, transition timestamps, same-status preservation, reopening, read-only rejection, rollback, and exact data retention.
- [x] 2.9 Use date/time/ID ordering for closed groups and History, and update CLI/MCP JSON schemas and integration tests while retaining the prohibition on closed manual reordering.

## 3. Verification

- [x] 3.1 Run formatting, Clippy, all tests, a locked release build, and isolated migration, CLI, and terminal checks.
- [x] 3.2 Complete independent review and resolve findings.
- [x] 3.3 Archive the change and strictly validate the canonical specs.
