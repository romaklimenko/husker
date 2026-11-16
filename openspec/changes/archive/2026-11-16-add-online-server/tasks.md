## 1. Specification

- [x] 1.1 Write the proposal, design, the `online-server` and `online-mcp` specs, and the `device-sync`, `command-line`, and `task-storage` deltas. Run `openspec validate add-online-server --strict`. The docs-writer agent reviews the prose.

## 2. Shared rules

- [x] 2.1 Add `src/rules.rs` with the title rule, the status stamp rule, and the list name rule, and call them from `src/store.rs`. The error sentences stay the same. The existing store tests pass unchanged.

## 3. Sync file editing

- [x] 3.1 Add `src/online/doc.rs`: `Access`, `Stamp`, `Op`, `apply`, `guard`, `version`, and `resolve_list`. Unit tests cover every operation, each error sentence, the scope checks, no-op changes, ID hints and positions, and the guard cases.
- [x] 3.2 Add `src/online/view.rs`: the projection of accessible records onto `Task` and `TaskList`, and the read functions that call `query::visible_tasks`. Tests cover list order, counts, scope, Trash counts, and tasks whose list is missing.
- [x] 3.3 Add a property test in `src/online/tests.rs`: after any applied operation at most one record differs, the header members are equal, the guard passes, and `Snapshot::parse` accepts the result. A failed operation leaves the snapshot equal.
- [x] 3.4 Add parity tests in `src/online/tests.rs`: each operation through `Store` plus `sync_once` and through `apply` gives the same record and the same error sentence, for unscoped, scoped, read-only, stale, and invalid cases.

## 4. Engine

- [x] 4.1 Make `Remote` `Send + Sync`, add a public `Settings::new` with the URL checks, and add bundled root certificates to `BlobRemote` in `src/sync.rs` and `src/sync/remote.rs`. Existing sync tests pass.
- [x] 4.2 Add `src/online/engine.rs`: the cached read, the write loop with `If-Match`, five attempts, and the lost-answer check. Tests with `MemoryRemote` cover a device sync injected before the upload for the same task and for another task, five refusals, a lost answer, a missing file that stays missing, 25 writes that keep a device's upload identifier, and a device that receives the changes without a conflict copy.

## 5. Tokens

- [x] 5.1 Add `src/online/access.rs`: the key file `online.toml`, signing, verification, and the server settings from the environment. Tests cover each refused-token case, two configured keys, the 400-day limit, revocation, and that errors never hold the key or the sync URL.
- [x] 5.2 Add `husker token` in `src/main.rs` and `Store::list_uid` in `src/store.rs`. CLI tests in `tests/online.rs` cover a token for one list, `--public-key` creating and reusing the key, `--json`, and each error case.

## 6. Online MCP

- [x] 6.1 Add `src/online/mcp.rs`: the handler with the tool sets for the three access kinds, text IDs and versions, and the instructions. Share the tool builder from `src/mcp.rs`. Tests cover each tool, tool counts, the default list of `add_task`, and the time zone date.

## 7. HTTP and the serve command

- [x] 7.1 Add `src/online/http.rs`: the router, the token check before any storage request, the `Origin` and `X-Forwarded-Host` checks, path scope, the per-request rmcp service, and `/api/health`. Add `husker serve` in `src/main.rs`, which skips the profile and `config.toml`.
- [x] 7.2 Add `tests/online.rs` with `FakeBlob`: a real `husker serve` process, the rmcp HTTP client in the `initialize` and discover lifecycles, tool counts per access, path narrowing with 403 and 404, 401 without a storage request, 405 for `GET /mcp`, a laptop CLI profile that receives a server change, and no error text with the sync URL.
- [x] 7.3 Add `examples/fake_blob.rs`, which runs the test double and prints its URL for manual checks.

## 8. Deployment files

- [x] 8.1 Add `deploy/azure/host.json`, `deploy/azure/serve/function.json`, `deploy/azure/main.bicep`, `deploy/azure/code.bicep`, and `deploy/azure/deploy.ps1`. `az bicep build` accepts both templates. The script passes `--subscription` on every `az` call and prints no secret.
- [x] 8.2 Build the static Linux binary with `cargo zigbuild --release --target x86_64-unknown-linux-musl` and confirm that the package stores it with mode 0755.

## 9. Documentation and release

- [x] 9.1 Write `docs/online.md` and update `README.md`, `docs/mcp.md`, `docs/sync.md`, `docs/development.md`, and `AGENTS.md`. Bump `Cargo.toml` to 0.17.0.

## 10. Verification

- [x] 10.1 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and a locked release build.
- [x] 10.2 Run a real `husker serve` process against the local test double with a laptop profile. Change a task through the online MCP, run `husker sync`, and compare. `tests/online.rs` does this. A run by hand with `examples/fake_blob.rs` and `curl` was not allowed in the agent session.
- [x] 10.3 Delegate a code review and a security review of `src/online/access.rs` and `src/online/doc.rs`, address the findings, and rerun the affected checks.

## 11. Deployment

- [x] 11.1 Create the stored access policy `husker-online` and deploy with `HUSKER_ONLINE_READ_ONLY=true`. Check `/api/health`, the headers that reach the handler, and the cold start time. Record the measurements in `docs/development.md`.
- [x] 11.2 Read the Husker list through the online MCP and compare it with a computer. Deploy without the read-only setting. Close a task through the online MCP and see the change on a computer after its sync.
- [x] 11.3 Switch `.mcp.json` to the online server with `${HUSKER_ONLINE_URL}` and `${HUSKER_ONLINE_TOKEN}`, and update the task inbox section of `AGENTS.md`.
- [x] 11.4 Archive the change and strictly validate all canonical capabilities.
