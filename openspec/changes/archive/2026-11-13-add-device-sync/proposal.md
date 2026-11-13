## What Changes

1. Husker syncs the lists and tasks of a profile with one JSON file in Azure Blob Storage. Every device that uses the same file sees the same lists and tasks.
2. Sync is off until the profile contains a `sync.toml` file. The file names the blob URL and how to sign in: a SAS token in the URL, or the managed identity of an Azure VM.
3. Each device keeps its SQLite database. The terminal, the commands, and the MCP tools read and write only the local database, so they work without a network.
4. A sync downloads the file only when it changed, merges it with the local database, and uploads the result only when the local database holds changes the file lacks.
5. The merge compares the local database and the downloaded file with the copy from the last sync. A field changed on one device wins. A task or list deleted on one device stays deleted unless another device edited it.
6. When two devices changed the title or description of one task to different text, Husker keeps both. The task keeps the text that reached the file first. The other text becomes a new task whose title ends with `#conflict`.
7. An upload replaces the file only if nobody else replaced it since the download. Otherwise Husker downloads the newer file and merges again.
8. The terminal syncs at start, 60 seconds after the last local change, every 60 seconds, on input after 60 seconds without input, on F5, and at exit when local changes are unsent.
9. The MCP server syncs before it serves tools, 2 seconds after a local change, every 60 seconds, and at exit. A read-only server does not sync.
10. A new `husker sync` command runs one sync and prints what it received and sent.
11. The terminal shows a message when a sync changed local tasks, when it kept both versions of a task, and when it failed.
12. The database moves to schema version 5. Lists and tasks get a random sync identifier, and a new table holds the sync state.

## Why

The user works on more than one computer and runs an agent on a VM. Today each computer has its own tasks.
The user asked for a simple sync through Azure storage that keeps the local database fast and never loses data in a conflict.

## Capabilities

### New Capabilities

- `device-sync`: The `sync.toml` settings, the sync file format, the merge rules, conflict copies, the conditional download and upload, sign-in with a SAS token or a managed identity, and failure handling.

### Modified Capabilities

- `task-storage`: Schema version 5 with sync identifiers on lists and tasks and a sync-state table. Read-only stores require schema version 5.
- `terminal-interface`: Background sync with its triggers and messages. Changes that a sync wrote do not raise the external-update notice.
- `mcp-server`: The server syncs before serving, after changes, on a timer, and at exit.
- `command-line`: The `sync` command.

## Impact

Release 0.15.0 migrates a profile to schema version 5 on the first writable start. Husker 0.14.0 and older cannot open a migrated profile, so every Husker executable that uses the profile must be updated together.
The migration keeps every list and task field and every task version.
Sync needs outbound HTTPS. The new `ureq` dependency adds an HTTP client with `rustls`. Certificates are checked against the operating system's trust store.
Without `sync.toml`, Husker makes no network request and behaves as before, except for the new schema.
The sync file holds every list and task of the profile as plain JSON. Anyone who can read the blob can read the tasks.
A device that joins an existing sync file keeps its own tasks and receives the others. Its default list merges with the default list in the file.
Task and list IDs normally match across devices. Two tasks created on two devices before either synced can get different IDs on each device.
CLI commands other than `sync` do not sync. Their changes upload at the next sync from the terminal, an MCP server, or `husker sync`.
Existing commands, JSON fields, MCP tools, filter syntax, and configuration settings are unchanged.
