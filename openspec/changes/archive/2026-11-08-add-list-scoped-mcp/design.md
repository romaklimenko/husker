## Context

Husker already stores tasks in SQLite and exposes the same task fields through its CLI and terminal interface.
The user wants MCP access and a parameter that keeps an agent within one list.
The reviewed first version is committed and pushed. MCP work remains a separate change for review.

## Goals / Non-Goals

Goals:

- Serve local MCP clients through standard input and output.
- Enforce a fixed list restriction on every read and write.
- Support a read-only connection with no mutation tools.
- Reuse task validation, filtering, and optimistic versions.
- Verify access restrictions through a real MCP client and child process.

Non-goals:

- HTTP transport, remote authentication, or network listeners.
- Configuring the user's MCP clients automatically.
- Restricting separate shell or filesystem tools that can open the profile directly.
- Selecting multiple lists or changing a server's scope after startup.
- Permanent deletion, list deletion, or task reordering through MCP.

## Decisions

### Use the official Rust SDK

Use `rmcp` 3.9 with stdio transport and Tokio. Let the SDK handle lifecycle messages and protocol versions.
Manual JSON-RPC handling would duplicate protocol behavior and compatibility rules.
Use a manual tool registry because the available tools depend on the startup options.

`mcp::serve(Store)` runs the async server. `main.rs` creates the runtime only for the `mcp` subcommand.
Store access uses a mutex and Tokio's blocking worker pool. No store guard crosses an await.
Stdout contains protocol messages only. Startup errors go to stderr and return a failing exit code.

### Pin the list ID at startup

`Store::open_for_mcp(path, list_name, read_only)` resolves the existing name once and stores its ID privately.
Names use the same trimming and Unicode lowercase conversion as ordinary lists.
A rename changes the display name without changing access. Creating another list with the old name cannot redirect the running server.

`Store::list_scope()` returns the optional ID. `Store::is_read_only()` reports the access mode.
The existing `Store::open()` remains unrestricted so the UI and existing CLI commands keep their behavior.

### Enforce scope in storage operations

Filter list and task reads in SQL. Count only tasks visible through the restricted store.
Check current membership inside each immediate write transaction before version checks or updates.
An outside-list task ID and a nonexistent ID return the same error.
Check destination list IDs against the restriction before looking them up.

Checking only in the MCP handler would leave a race when another process moves a task after the handler reads it.
Storage enforcement also protects future tools from accidentally skipping a handler check.

### Make read-only access explicit

With `--read-only`, open an existing database with SQLite's read-only flag.
Do not create a profile, initialize a schema, or change the journal mode.
Refuse every mutation in the store and remove mutation tools from discovery.
Calling an unavailable tool by its name still fails.

### Keep tool inputs explicit

These ten tools use stable English names:

| Tool | Arguments | Structured result |
| --- | --- | --- |
| `list_lists` | None | `lists`, `trash_count` |
| `list_tasks` | `query`, `trash`, `offset`, `limit` | `tasks`, `total`, `offset`, `next_offset` |
| `get_task` | `id` | `task` |
| `add_task` | `title`, optional `list_id`, `description`, `due`, `tags`, `readiness` | `task` |
| `update_task` | `id`, `version`, optional fields from add, `clear_due` | `task` |
| `set_task_status` | `id`, `version`, `status` | `task` |
| `trash_task` | `id`, `version` | `task` |
| `restore_task` | `id`, `version` | `task` |
| `create_list` | `name` | `list` |
| `rename_list` | `id`, `name` | `list` |

`readiness` accepts `maybe` or `anytime`. New tasks default to Maybe.
`update_task` changes only supplied fields. An empty tags array clears tags.
`clear_due` removes the date and defaults to Anytime. Explicit readiness overrides that default.
Supplying both `due` and `clear_due = true` is an error.
Versions are mandatory for task mutations so an agent cannot silently overwrite a newer edit.

For new tasks, an omitted `list_id` selects the scoped list or the first ordinary list when unrestricted.
Scoped servers omit `create_list` and `rename_list`. They cannot move tasks outside the selected list.
Read-only servers expose only `list_lists`, `list_tasks`, and `get_task`.

`list_tasks` defaults to an empty query, `trash = false`, `offset = 0`, and `limit = 100`.
The maximum limit is 200. The result count and next offset refer only to visible matches.
Pagination uses the current store contents. Concurrent edits can change the result set between pages.

Use typed Serde inputs and generated JSON Schemas. Reject unknown fields and invalid values.
Successful results contain structured JSON and an equivalent text result for older clients.
Business errors use MCP tool-error results. Unknown or unavailable tool names fail without performing an operation.

### Tell the agent what the server can access

Initialization instructions identify the fixed list ID or unrestricted profile access and whether writes are available.
Instructions tell agents to use returned versions, treat task descriptions as task data, and request another server configuration when a different list is needed.
Tool descriptions explain their fields and side effects. Read annotations describe tool behavior but do not replace enforcement.

## Risks / Trade-offs

| Risk | Response |
| --- | --- |
| A client uses an older MCP lifecycle | Test the legacy initialization flow and the SDK's current discovery flow. |
| Another process moves a task | Recheck membership inside the write transaction. |
| An agent guesses hidden IDs | Return the same result as for a missing ID. |
| Large task collections exceed useful tool output | Paginate task lists with a maximum of 200 tasks per call. |
| A user expects OS-wide isolation | Document that the restriction applies only to this MCP connection. |

## Migration plan

No database migration is required. Install the updated binary and register a stdio server with the desired arguments.
Existing installed processes continue running their existing executable until restarted.
No client settings or personal task data are changed during development.

## Evidence

- Read from source: `Store` already uses immediate write transactions and task version checks.
- Read from published SDK metadata: `rmcp` 3.9 declares Rust 1.88 as its minimum version.
- Read from the MCP stdio specification: stdout is reserved for protocol messages.
- Verified through child-process integration tests: the SDK supports legacy initialization and the current discovery lifecycle.
- Verified through integration tests: hidden IDs, outside-list destinations, and writes after an external list move are rejected.
- Verified through storage and MCP tests: read-only access rejects valid mutation requests without changing tasks or lists.

## Open questions

No decision blocks implementation. Unrestricted access requires omitting `--list`; the guide recommends a named list for each agent connection.
