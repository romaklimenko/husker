## Why

Add a local MCP server so agents can manage Husker tasks within a list selected by the user.
A server-side list restriction keeps unrelated tasks out of the agent's tool results and prevents changes to them.

## What Changes

The change adds three behaviors:

| Behavior | Result |
| --- | --- |
| `husker mcp` | Serve task and list tools over standard input and output. |
| `husker mcp --list NAME` | Limit every tool to one existing list for the lifetime of the server. |
| `husker mcp --read-only` | Expose only tools that read task data. |

The server tells the agent its scope during initialization. The store enforces that scope even when the agent supplies another list name or task ID.
Users can combine both options and select a profile with the existing `--profile` option.

## Capabilities

### New Capabilities

- `mcp-server`: MCP stdio transport, tool definitions, structured results, list restrictions, read-only access, and agent instructions.

### Modified Capabilities

- `task-storage`: Add an optional list restriction to a store connection and enforce it within task reads and write transactions.

## Impact

The change affects these four areas:

| Area | Change |
| --- | --- |
| Application | Add `src/mcp.rs` and the `mcp` subcommand. |
| Storage | Add scoped reads and transaction checks in `src/store.rs`. |
| Dependencies | Use the official Rust MCP SDK and Tokio for stdio service handling. |
| Documentation and checks | Add client setup examples and real stdio tests for access restrictions. |

Existing terminal and CLI behavior stays the same. Existing databases need no migration.
The MCP server does not expose an HTTP listener or change installed MCP client settings.
List selection limits this MCP connection. It does not restrict an agent that separately has shell or filesystem access to the same profile.
