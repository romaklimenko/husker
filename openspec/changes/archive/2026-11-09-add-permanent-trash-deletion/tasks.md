## 1. Specification

- [x] 1.1 Write the proposal, design, and task-storage, terminal-interface, and profiles-localization deltas, and run `openspec validate add-permanent-trash-deletion --strict`.

## 2. Storage

- [x] 2.1 Add `purge_task`, `preview_empty_trash`, and `empty_trash` with `TaskPurgeOutcome`, `TrashPreview`, and `EmptyTrashOutcome` in `src/store.rs`. Tests cover deletion, `Changed`, `NotInTrash`, ID reuse, a changed snapshot with an equal count, and untouched tasks outside Trash.
- [x] 2.2 Reject read-only and list-scoped stores before reading Trash, with a Danish translation of the scoped error in `src/i18n.rs`. Tests cover both store kinds.

## 3. Configuration

- [x] 3.1 Add the `empty_trash` action with default `D` in task navigation and rename `Mode::DeleteListConfirm` to `Mode::DeleteConfirm` in `src/config.rs`. Tests cover the default, the example configuration, and a `D` conflict.

## 4. Terminal interface

- [x] 4.1 Add the Delete forever confirmation for the `trash` action in Trash, with outcome handling and refresh revalidation in `src/ui.rs`. Tests cover No by default, `y`, Back, an external edit, and an external restore.
- [x] 4.2 Add the Empty Trash confirmation for `empty_trash` in Trash and for `delete_list` on the Trash picker entry. Tests cover an empty Trash, a filtered count, a changed Trash, the picker return, and `D` outside Trash.
- [x] 4.3 Share one confirmation key handler and dialog renderer between list deletion, Delete forever, and Empty Trash.
- [x] 4.4 Discard the undo entry after a permanent deletion. A test shows Nothing to undo afterward.
- [x] 4.5 Show Delete forever and Empty Trash in Trash footer hints, the picker hint for the Trash entry, and Help, with English and Danish labels. Tests check the rendered footer and Help.

## 5. Documentation and release

- [x] 5.1 Update `README.md`, `docs/usage.md`, `docs/development.md`, and `docs/mcp.md` where they describe Trash, and bump `Cargo.toml` to 0.13.0.

## 6. Verification

- [x] 6.1 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and a locked release build.
- [x] 6.2 Exercise the CLI against a temporary profile and inspect the rendered Trash screens.
- [x] 6.3 Delegate a review, address its findings, and rerun the affected checks.
- [x] 6.4 Archive the change and strictly validate all canonical capabilities.
