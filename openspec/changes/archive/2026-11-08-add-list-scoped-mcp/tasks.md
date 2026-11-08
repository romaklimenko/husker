## 1. Restricted storage

- [x] 1.1 Add `Store::open_for_mcp`, fixed list scope, and scoped reads in `src/store.rs`; test names, counts, Trash, and rename identity.
- [x] 1.2 Enforce membership and destination checks inside write transactions; test hidden IDs, stale tasks moved by another connection, and all mutation paths.
- [x] 1.3 Open existing databases read-only and reject mutations in `src/store.rs`; test missing profiles, unchanged schema, and rejected writes.

## 2. MCP server

- [x] 2.1 Add the official SDK, Tokio, JSON Schemas, and `husker mcp` options without changing existing CLI startup behavior.
- [x] 2.2 Implement ten tools, typed input validation, structured results, query pagination, and versioned mutations in `src/mcp.rs`.
- [x] 2.3 Add access instructions and filter tool discovery and invocation by list scope and read-only mode.

## 3. Verification and guidance

- [x] 3.1 Test legacy and current MCP lifecycles through real child processes in `tests/mcp.rs`, including stdout purity and shutdown.
- [x] 3.2 Test list restrictions, hidden IDs, paginated search, readonly calls, edits, statuses, and Trash through MCP.
- [x] 3.3 Run formatting, Clippy, the complete test suite, and a release build; preserve the existing installed app and personal profiles.
- [x] 3.4 Have the docs-writer update README and write client registration guidance with `--list` and `--read-only`; verify examples without changing client settings.
- [x] 3.5 Complete independent code and scope reviews, fix findings, and rerun affected checks.
- [x] 3.6 Validate and archive the change, replace generated Purpose text, and validate current specs.
