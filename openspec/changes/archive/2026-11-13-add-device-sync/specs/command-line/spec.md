## ADDED Requirements

### Requirement: Sync on request

Husker SHALL provide `husker sync` to run one sync of the selected profile and report its result.
The text output is `Synced. Received N, sent M.`, followed by ` Kept both versions of K.` when K is greater than zero.
N is the number of local lists and tasks the sync inserted, changed, or deleted.
M is the number of lists and tasks the upload added, changed, or removed in the sync file.
K is the number of conflict copies the sync created.
With `--json`, the output is an object with the integer members `received` for N, `sent` for M, and `conflicts` for K.
Without `sync.toml` in the profile, the command fails and names the missing file.
A failed sync exits unsuccessfully with a message on stderr that does not contain the URL.
No other command syncs or reads `sync.toml`, except `husker mcp`. The mcp-server spec defines when `husker mcp` syncs.
The terminal interface, which starts without a command, syncs as the terminal-interface spec defines.

#### Scenario: Send a task created by a script

- **WHEN** a profile has synced before, and a script then runs `husker add "Buy milk"` and `husker sync`
- **THEN** the second command prints `Synced. Received 0, sent 1.` and the task is in the sync file

#### Scenario: Report a profile without sync settings

- **WHEN** the profile has no `sync.toml` and the user runs `husker sync`
- **THEN** the command exits unsuccessfully with a message that names `sync.toml`
- **AND** it makes no network request
