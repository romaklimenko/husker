## 1. Specification

- [x] 1.1 Write the proposal, design, and the device-sync, task-storage, terminal-interface, mcp-server, and command-line deltas, and run `openspec validate add-device-sync --strict`.

## 2. Storage

- [x] 2.1 Move the database to schema version 5 in `src/store.rs`: `uid` columns with unique indexes, the `sync_state` table, a version 4 migration, and `uid` values on every insert. Tests cover the migration, fresh databases, unchanged task versions, and read-only opening.
- [x] 2.2 Add the sync operations in `src/store/sync.rs`: export a snapshot, merge and apply a downloaded snapshot in one immediate transaction, record an upload, report unsent changes, and read the counters. Tests cover scoped and read-only rejection, rows without a `uid`, ID hints, and a stale ETag.

## 3. Merge

- [x] 3.1 Add the snapshot types, validation, and the three-way merge in `src/sync/snapshot.rs` and `src/sync/merge.rs`. Tests cover every row of the record table, field merges, text conflicts and copies, the first sync, same-name lists, missing lists, the kept default list, and position normalization.

## 4. Remote and engine

- [x] 4.1 Add `ureq` and the blob client in `src/sync/remote.rs` with conditional download and upload, SAS and managed-identity sign-in, a 10-second timeout, and failure kinds that hide the URL. Tests run against a local HTTP test double.
- [x] 4.2 Add `sync.toml` loading and `sync_once` in `src/sync.rs` with the retry loop and the report. Tests cover two profiles converging, a refused upload, a missing file, another `collection`, and a newer `format`.
- [x] 4.3 Add the worker and its schedule in `src/sync/worker.rs`. Tests cover each trigger, a failure reported once, and the exit sync.

## 5. Interfaces

- [x] 5.1 Add `husker sync` in `src/main.rs` with text and JSON output. CLI tests cover two profiles, the missing settings file, and a hidden URL in errors.
- [x] 5.2 Sync in `husker mcp`: before resolving `--list`, on the worker, and at exit, and never for `--read-only`. MCP tests cover a list that arrives by sync and an uploaded `add_task`.
- [x] 5.3 Run the worker from `src/ui.rs`: activity, F5, the sync messages in English and Danish in `src/i18n.rs`, no external-update notice for sync commits, and the exit sync. Tests cover the messages, the notice rule, and the failure message shown once.

## 6. Documentation and release

- [x] 6.1 Write `docs/sync.md` with the Azure setup and update `README.md`, `docs/usage.md`, `docs/mcp.md`, `docs/development.md`, and `AGENTS.md`. Bump `Cargo.toml` to 0.15.0.

## 7. Verification

- [x] 7.1 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and a locked release build.
- [x] 7.2 Exercise `husker sync` and the terminal against temporary profiles and a local test double, and inspect the rendered sync messages.
- [x] 7.3 Create the Azure resource group, storage account, and container, and sync two temporary profiles through the real blob with a SAS URL.
- [x] 7.4 Delegate a review, address its findings, and rerun the affected checks.
- [x] 7.5 Archive the change and strictly validate all canonical capabilities.
