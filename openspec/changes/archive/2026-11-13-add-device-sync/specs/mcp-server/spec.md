## ADDED Requirements

### Requirement: Sync while serving

Husker SHALL sync the selected profile from a writable MCP server when the profile has sync settings.
A profile has sync settings when its directory contains `sync.toml`, as the device-sync spec defines.

| Trigger | Sync starts |
| --- | --- |
| Start | Before the server resolves `--list` and before it answers any request. |
| Local change | 2 seconds after the last commit to the database by any process. |
| Timer | 60 seconds after the previous sync ended. |
| Exit | After the input stream closes, only when the local database holds unsent changes. |

Because the server syncs before it resolves `--list`, a client on a new device can select a list that exists only in the sync file.
A failed sync writes one diagnostic line to stderr and the server continues with local data. Stdout contains only MCP messages.
Sync covers every list of the profile. The `--list` restriction applies to tools only.
A read-only server does not sync and makes no network request.
Sync adds no tool and changes no tool schema or tool result.

#### Scenario: Serve a list from another device

- **WHEN** a new profile has sync settings, the sync file holds a list named `Husker`, and a client launches `husker mcp --list Husker`
- **THEN** the server starts and its tools return the tasks of that list

#### Scenario: Send an agent's task

- **WHEN** an agent calls `add_task` and no process commits to the database for 2 seconds
- **THEN** the task is uploaded to the sync file

#### Scenario: Start without a network

- **WHEN** the network is unavailable and a client launches a writable server for a profile that already holds the selected list
- **THEN** the server writes a sync diagnostic to stderr and serves the local tasks

#### Scenario: Keep a read-only server offline

- **WHEN** a client launches `husker mcp --read-only` for a profile with sync settings
- **THEN** the server makes no network request and does not change the database
