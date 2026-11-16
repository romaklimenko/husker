## Context

Husker keeps tasks in a local SQLite database on each computer. Computers sync through one JSON file in Azure Blob Storage, the sync file. Each computer merges the sync file with its database against the copy it stored at its last sync.

Agents reach tasks through `husker mcp`, a local process that speaks MCP over standard input and output. On Windows a running process locks its executable, so `cargo install` fails while an agent is connected.

The user wants an MCP server that needs no local process and can be limited to one list. A later change adds a phone page to the same server. The setup must cost close to nothing. Computers must keep their local database and background sync.

Facts this design rests on:

| Fact | Evidence |
| --- | --- |
| Records in the sync file are matched by `uid`. The integer `id` is a hint from the first uploading device. | Read from `src/sync/snapshot.rs` and `src/store/sync.rs`. |
| A fresh database assigns IDs in apply order, so its IDs change as the sync file changes. | Read from `apply` in `src/store/sync.rs`. |
| Task versions are local counters and are not in the sync file. | Read from `src/sync/snapshot.rs`. |
| `uploads` is read only for lost-answer recovery. A device looks for its last upload identifier among at most 20. | Read from `Snapshot::includes_upload` and `Store::sync_merge`. |
| A device that finds no sync file uploads its database under a new `collection`. | Read from `Store::sync_merge`. |
| `Snapshot::parse` does not check that a task's list exists, that list names are unique, or that positions are unique. | Read from `Snapshot::validate`. |
| The Azure Functions host forwards a proxied request with `Host` set to the loopback address and the address the client used in `X-Forwarded-Host`. | Read from the azure-functions-host and YARP sources. Measured on a computer with Core Tools 4.16.0 and host 4.1054.250 by `deploy/azure/check-local.mjs`. Not measured in Azure, where a front end sits before the host. |
| A custom handler on the Flex Consumption plan starts in a few seconds. | Assumed. The first deployment measures the start time. |

## Goals / Non-Goals

**Goals:**

- Serve the Husker MCP tools over HTTP from a process that scales to zero.
- Change the sync file safely next to syncing computers, with no change to the computers.
- Limit a client to one list and to reading with a signed access token.
- Keep one implementation of the task and list rules.
- Lay the base for the phone page: the same server, access tokens, and sync file editing.

**Non-Goals:**

- The phone page and its JSON API. A later change adds them.
- OAuth, Claude apps as MCP clients, and Microsoft sign-in.
- Permanent deletion, list deletion, and reordering through the online server.
- Managed identity for the online server's access to the sync file.
- More than one user.
- A new release for computers. They keep syncing unchanged.

## Decisions

### 1. Edit the sync file directly, one record per request

The online server downloads the sync file with its ETag, changes one list or task in memory, and uploads with `If-Match`. A refused upload means a device replaced the sync file. The online server then downloads again and applies the same change again. One request makes at most five attempts.

Reason: the online server has no state to merge. Applying the requested change to the newest sync file gives the same result as a merge for a single record. It also leaves every other record byte for byte as a device wrote it.

Rejected: run the online server as a device with a temporary profile, using `Store` and `sync_once`. `Store::sync_merge` adds an upload identifier to every upload and gives a missing sync file a new `collection`. A server built that way uploads an export of the whole database after a fresh join on every write. It still needs a `uid` lookup and a content version outside `Store`, because the IDs and versions of a fresh database are not stable.

### 2. Guard every upload

Before an upload, the online server compares the new snapshot with the downloaded snapshot. The upload goes ahead only when all four conditions hold.

1. `format`, `collection`, `default_list`, and `uploads` are equal.
2. No record is removed.
3. At most one record is added or changed.
4. No existing record changed its `id` or `position`.

The online server also parses its own upload with `Snapshot::parse`.

Reason: `Snapshot::parse` accepts files a device would repair and upload again. The guard limits a bug in the online server to one record.

### 3. Never create the sync file, never add an upload identifier

A missing sync file is an error for every request. The online server leaves `uploads` untouched.

Reason: an online server with a wrong URL must not start a new `collection`. Twenty uploads by the online server would push a device's identifier out of `uploads`. That device would then merge against an older base and lose edits made after a lost upload answer. Only that recovery reads `uploads`.

### 4. Online IDs are the `uid`

On the online server, the ID of a task or list is its `uid`. The MCP results keep the member names `id` and `list_id`.

Reason: the `uid` is the only identifier that is equal on every device and stable across processes of the online server.

Rejected: the integer IDs of a database built from the sync file. They change when a record with an earlier `uid` arrives. Rejected: integers derived from the `uid`. They would look like local IDs without matching them.

### 5. Online versions are content tokens

The online server writes a task's `uid`, `list`, `title`, `description`, `due`, `status`, `status_changed_on`, `status_changed_at`, and `deleted` as a JSON array. The version of the task is the first 16 hexadecimal characters of the SHA-256 of that array.

Reason: the online server keeps no counter, and devices do not sync their counters. The content token changes exactly when a device or the online server changes a field that an agent can see or set. `position` is left out, because a merge renumbers positions.

Rejected: adding a version counter to the sync file. That needs a new format and a new release on every computer.

### 6. One set of rules for the store and the online server

The title rule, the status stamp rule, and the list name rule move from `src/store.rs` to `src/rules.rs`. The store and the online server both call the moved rules. The error sentences stay as they are.

Reason: AGENTS.md asks for one behavior across interfaces. `i18n::danish_error` matches the existing sentences.

### 7. Read through the existing query and planning functions

`src/online/view.rs` turns the accessible records into `Task` and `TaskList` values and calls `query::visible_tasks`. The query and planning functions need integer IDs for sorting. The view therefore numbers tasks by `(id hint, uid)`. It numbers lists with the default list first, then by `(position, id hint, uid)`. Results map back to `uid`.

Reason: a device with a fresh database assigns IDs in the same order, so ties sort as they would on that device. The numbers never leave the online server.

### 8. Cache the file by ETag, serialize writes

The online server keeps the last downloaded snapshot with its ETag and sends `If-None-Match`. One mutex serializes writes within a process. After an upload request that got no answer, the online server downloads once and treats a sync file equal to its upload as success.

Reason: a read then costs one small request. Two writes in one process would otherwise refuse each other.

### 9. Access tokens are signed on the user's computer with Ed25519

`husker token` signs an access token with a private key in `online.toml` in the profile. The online server holds only public keys in `HUSKER_ONLINE_KEYS`. An access token is `hsk1_<payload>.<signature>`. The payload carries a token ID, a name, the public address of the online server, a list `uid` or null, a write flag, and the times of signing and expiry. The two times are at most 400 days apart.

Reason: nothing in the cloud can create an access token, and no secret crosses a deployment. The access token has two parts, so the Azure Functions host does not read it as a JWT (read from the host source).

Rejected: HMAC with a key in the app settings. A settings leak could mint access tokens. Rejected: random tokens with their hashes in an app setting. Each new access token would need a deployment.

Revocation: a token ID in `HUSKER_ONLINE_REVOKED` revokes one access token. Replacing the public key revokes every access token. Deleting the stored access policy of the online server's SAS cuts the online server off from the sync file.

### 10. The access token sets the list scope, the path can narrow it

`/mcp` uses the list scope of the access token. `/mcp/list/NAME` limits an access token for every list to the named list. An access token for one list is refused on a path that names another list or no list. Both cases get the same answer, so the answer does not show which list names exist.

Reason: one access token can then serve several projects, each with its own URL in `.mcp.json`, as `husker mcp --list NAME` does locally. The access token remains the access boundary.

Rejected: a query parameter. Some MCP clients treat the query string specially, and a later OAuth resource identifier should not carry one.

### 11. "Today" comes from a time zone setting

MCP calls take the current date from `HUSKER_ONLINE_TZ`, an IANA time zone name. The default is `UTC`.

Reason: the online server runs in UTC, and the Flex Consumption plan ignores `TZ` (read from Microsoft Learn). Planned dates and status dates are local calendar dates.

### 12. rmcp over HTTP, one service per request, no sessions

For each request, `/mcp` checks the access token, resolves the list scope, and builds one `StreamableHttpService`. The service uses `NeverSessionManager`, `legacy_session_mode = false`, and `json_response = true`. The service gets a handler whose tool list and instructions match the access of the request.

Reason: `get_info` and `get_tool` have no request context in rmcp 3.9 (read from source), so the access must be fixed when the handler is built. With sessions off, rmcp answers `initialize` clients and 2026-07-28 clients from any process (read from source, proven by our tests).

### 13. Check the request before the sync file

A request without a valid access token gets 401 before any storage request. The online server refuses an `X-Forwarded-Host` or `Origin` that differs from `HUSKER_ONLINE_ORIGIN`. The online server removes the `Authorization` header before it calls rmcp, because rmcp hands the request parts to tool code.

### 14. Host on Azure Functions, Flex Consumption plan

The online server runs as a custom handler with request proxying, one catch-all HTTP function, an empty route prefix, 512 MB, and at most one instance.

Reason: the plan scales to zero and has a free monthly grant of 250,000 executions and 100,000 GB-s (read from Microsoft Learn). The plan sits in the same region and resource group as the sync file.

Rejected: Azure Container Apps, with reported cold starts of 15 to 37 seconds. Rejected: Vercel, whose Rust runtime is in beta and which would hold the SAS in a second cloud. Fallback: the Windows Consumption plan with the Windows build.

### 15. The online server reaches the sync file with its own SAS

The Bicep template creates a SAS for the one blob with `listServiceSas` from a stored access policy named `husker-online`. The template writes the SAS into the app settings.

Reason: a SAS from the template needs no new code and no role assignment, and the secret never passes through a terminal. Deleting the policy revokes the online server's SAS and leaves the computers' policy untouched.

Rejected for now: managed identity. `BlobRemote` knows only the VM metadata endpoint, and Azure Functions uses another one.

### 16. Build a static Linux binary with bundled root certificates

The deployed binary targets `x86_64-unknown-linux-musl`, built with `cargo zigbuild`. `husker serve` checks certificates against the bundled Mozilla roots.

Reason: the glibc version and the CA bundle of the Flex Consumption image are not documented. Microsoft's Rust quickstart also builds for musl.

### 17. No cargo feature

The online server code is part of every build.

Reason: the project's checks run with default features. Code behind a feature would not be linted or tested by those checks.

## Risks / Trade-offs

- [The Azure Functions proxy changes a header or the route] → `deploy/azure/check-local.mjs` runs the host on a computer and checks the route, the headers, the cookie, and MCP through it. The Azure front end before the host stays unmeasured, so the first deployment checks `/api/health`, the MCP headers, and the bearer token before anything else. The online server is a plain HTTP program, so another host needs only new deployment files.
- [A bug in the online server damages the sync file] → One record per upload, the guard, `If-Match`, tests against test doubles only, a read-only first deployment, and blob versions on the storage account.
- [The sync file wins a conflict] → A change through the online server beats a device's unsent change to the same field. For a title or description, the device keeps a conflict copy. This is the existing rule for any two writers.
- [The rules drift between the store and the online server] → Shared rule functions, and parity tests that run each operation through both.
- [A new sync file format reaches the online server late] → The online server refuses the sync file and changes nothing. The release notes say to deploy the online server first.
- [A scanner wakes the app] → Requests without a valid access token end before any storage request. One instance limits the cost.
- [An access token leaks] → Its list scope, write permission, and expiry limit the damage. Its token ID goes into `HUSKER_ONLINE_REVOKED`.
- [A write burst refuses a device five times] → The device's next sync trigger retries. Read from `sync_once`.

## Migration Plan

1. Create the stored access policy `husker-online` on the container. Creating the policy is a data plane command, so the template cannot create it.
2. Run `deploy/azure/deploy.ps1`. The script builds the Linux binary, packages the binary, and deploys the template with `HUSKER_ONLINE_READ_ONLY=true`.
3. Check `/api/health`. Then read a list through the MCP endpoint of the online server and compare the result with the same list on a computer.
4. Deploy again without the read-only setting.
5. Point Claude Code and Codex at the online server.

Rollback: stop the function app, or delete the policy `husker-online`. Computers keep syncing. To undo a bad write, promote an earlier version of the blob and sync a computer.

## Open Questions

None. Step 3 of the migration plan measures the cold start time, and `docs/development.md` records the result.
