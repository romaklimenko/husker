## What Changes

1. A new command, `husker serve`, runs an HTTP server called the online server. The online server reads and writes the sync file that the user's computers already use.
2. The online server keeps no database. For each change, the online server downloads the sync file and changes one list or task in the downloaded copy. The online server uploads that copy only if nobody replaced the sync file since the download.
3. The online server never creates the sync file and never adds an upload identifier to it.
4. The online server offers the Husker MCP tools over HTTP at `/mcp`. An agent that uses `/mcp` needs no local `husker mcp` process, so the agent no longer holds the installed executable open.
5. On the online server, a task ID and a list ID are the 32-character `uid` from the sync file. A task version is a content token: text computed from the task's fields.
6. Every request that reads or changes tasks needs an access token. A new command, `husker token`, signs access tokens on the user's computer with a private key that never leaves that computer. The online server holds only the public key.
7. An access token gives access to every list or to one list, with or without writes, until the access token expires.
8. The path `/mcp/list/NAME` limits an access token for every list to one list. The path cannot widen an access token for one list.
9. The repository gains the files that deploy the online server to Azure Functions.

## Why

Agents connect through a local `husker mcp` process. On Windows, that running process locks `husker.exe`, so installing a new build fails while an agent is connected.
The user also wants to reach tasks from a phone. A later change adds a phone page to the online server.
The user asked for a setup that costs close to nothing and keeps the computers on their local database with background sync.

## Capabilities

### New Capabilities

- `online-server`: The `husker serve` command and its settings, and how the online server reads and changes the sync file. Access tokens with their list scope and write permission, the HTTP checks, and the health endpoint.
- `online-mcp`: The MCP endpoint over HTTP, its tools, text IDs and versions, the list scope from the access token and the path, and the time zone for dates.

### Modified Capabilities

- `device-sync`: The sync file gains a second kind of writer. The `uploads` member lists uploads by devices only.
- `command-line`: The `serve` and `token` commands.
- `task-storage`: The `uid` appears in the results of the online server.

## Impact

Computers need no change and no new release to keep syncing. The sync file format stays 1.
A computer receives a change made through the online server at its next sync. For a running terminal, the next sync starts within about 60 seconds.
A change made through the online server is in the sync file at once. That change wins over a change to the same field that a computer has not uploaded yet. For a title or description, the computer keeps its own text in a conflict copy. A conflict copy is a new task whose title ends with `#conflict`.
A later change to the sync file format needs the online server deployed first. Until that deployment, the online server refuses the newer sync file and changes nothing.
Task and list IDs from the online server are text and differ from the integers that a computer shows. Agents that switch to the online server see the text IDs.
The local `husker mcp` command, its tools, and its results are unchanged.

New dependencies: `axum`, `chrono-tz`, `tower`, and the `transport-streamable-http-server` feature of `rmcp`. `ring`, `sha2`, and `base64` become direct dependencies. These three crates are already part of the build.
The online server code is part of every `husker` build. `husker serve` binds only to the loopback address.

Affected files:

| File | Change |
| --- | --- |
| `src/online.rs`, `src/online/*.rs` | New. The online server. |
| `src/rules.rs` | New. Task and list rules shared by the store and the online server. |
| `src/store.rs` | Calls the shared rules. Adds a read of a list's `uid`. |
| `src/sync.rs`, `src/sync/remote.rs` | A public constructor for the settings, bundled root certificates, and a `Sync` bound on the remote. |
| `src/mcp.rs` | Shares the tool builder with the online server. |
| `src/main.rs` | The `serve` and `token` commands. |
| `deploy/azure/*` | New. Deployment files for Azure Functions. |
| `tests/online.rs`, `examples/fake_blob.rs` | New. Tests and a helper for manual checks. |
| `README.md`, `docs/online.md`, `docs/mcp.md`, `docs/sync.md`, `docs/development.md`, `AGENTS.md`, `.mcp.json` | Documentation and the agent connection. |
