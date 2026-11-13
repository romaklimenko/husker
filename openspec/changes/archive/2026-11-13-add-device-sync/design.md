## Context

A profile is one SQLite database with integer list and task IDs from `AUTOINCREMENT` columns (read from source: `src/store.rs`).
Each task has a local `version` counter that guards writes from other processes (read from source).
The terminal reads SQLite's `data_version` once per second to notice commits from other connections (read from source: `src/ui.rs`).
The user's profile holds 59 tasks in 6 lists after one week. Exported as JSON they take about 23 KB (measured with `husker --json tasks`).
The user asked for sync through Azure storage, a local SQLite database, sync at start, at exit, one minute after a change, and on return from idle, and a conflict rule that loses no data.
The user's agent runs on a VM and should not hold a credential that reaches other data.

## Goals / Non-Goals

**Goals:**

- Show the same lists and tasks on every device that uses one sync file.
- Keep every read and write local. A slow or missing network never blocks the terminal.
- Lose no text. A conflict keeps both versions.
- Make the result independent of device clocks.
- Let an Azure VM sync without a stored secret.

**Non-Goals:**

- No sync of `config.toml`, shortcuts, language, filters, or undo history.
- No sync from CLI commands other than `husker sync`.
- No sync from a read-only MCP server.
- No per-list sync. The file holds the whole profile, and every device that syncs receives every list.
- No encryption of the file beyond what the storage service does.
- No storage back end other than Azure Blob Storage.
- No merge inside a text field. Two different descriptions are not combined.
- No proxy settings beyond the client's defaults.

## Decisions

### Use one blob in Azure Blob Storage

The remote state is one JSON blob. Blob Storage replaces a blob atomically and supports `If-Match` and `If-None-Match`, so an upload can require that nobody else uploaded since the download (assumed from the Blob service REST documentation, then verified against a real storage account in task 7.3).
The data is small, so storage and uploads cost well under one cent per month. A conditional download that answers 304 costs one read operation, and a process that polls every 60 seconds makes about 43,000 of them per month, which is one to two cents (assumed from the published prices, not measured on a bill).
Blob versioning on the storage account keeps every earlier version of the file as a backup.
Alternative rejected: Table Storage. It costs about the same but needs one entity per task, request signing, and a batch protocol. Changes that span tasks, such as a reorder, would not be atomic.
Alternative rejected: Cosmos DB. It needs an SDK or signed requests and a provisioned or serverless account for 23 KB of data.
Alternative rejected: uploading `tasks.db`. Two devices that both changed data could only overwrite each other.

### Merge three states: last synced, local, and remote

Each device stores the snapshot it last agreed on with the file, called the base, in the same SQLite database as the tasks.
A sync merges the local snapshot and the remote snapshot against the base, one list or task at a time, one field at a time.
A field that differs from the base on one side takes that side's value. A record missing on one side and unchanged on the other is deleted. A record missing on one side and changed on the other is kept.
The base is written in the same transaction as the merged tasks, so the two cannot disagree after a crash or a restored backup.
Alternative rejected: last write wins by timestamp. A wrong clock on one device would silently discard the other device's edit.
Alternative rejected: tombstones for deleted tasks. The base already shows what was deleted, and tombstones would need an expiry rule.

### Keep both versions when text conflicts

When both sides changed `title` or `description` of one task to different values, the task keeps the remote text and Husker creates a new task from the local version with ` #conflict` appended to its title.
The remote text wins on the original because other devices already hold it. The copy sits next to the original in the same list and date group.
The copy's `uid` is derived from the task's `uid` and the kept text, so two devices that keep the same text create the same copy and the merge stays free of random input.
`#conflict` is ordinary task text that search finds in both languages.
Other fields that conflict take the remote value without a copy. They hold one small value, and the user can set it again.
Alternative rejected: a copy for every conflicting field. A reschedule on two devices would produce duplicate tasks.

### Identify records by a random identifier, and carry the ID as a hint

Lists and tasks get a `uid` of 32 hexadecimal characters from SQLite's `randomblob(16)`. The merge matches records by `uid`.
The snapshot also carries each record's integer ID. A device that inserts a record from the file uses that ID when the ID is higher than every ID the device has used in that table. Otherwise the record gets the next local ID.
A device therefore never reuses an ID for a different record, so an ID that a script or agent holds cannot start to mean another task.
In the common case, where devices sync between creations, IDs match on every device.
Alternative rejected: renumbering an unsent local task whose ID is taken in the file. An agent holding the old ID and version 1 could then change the wrong task.
Alternative rejected: IDs that differ on every device by design. The user refers to tasks by ID across devices.

### Join by merging the default lists

On a device's first sync with a file, the local default list takes the identity, name, and position of the file's default list. The file names its default list in `default_list`.
Alternative rejected: treating the list with the lowest ID hint as the default. A list created on a joined device can carry a lower hint than the default list of the profile that created the file.
Other local lists that have the same name as a list in the file, after trimming and lowercase conversion, also merge with that list. Every local task is kept.
The default list therefore has the lowest local ID on every device, which is how the store finds it (read from source: `default_list_id`).
Two lists that still share a name after a merge are told apart by a suffix such as `Work (2)` on the list with the greater `uid`.

### Guard deletions with a collection identifier

The file carries a random `collection` value created with the file. The base remembers it.
When the file is missing or carries another `collection`, the merge deletes nothing. A deleted or replaced blob therefore cannot delete local tasks.
The base is still used to compare fields, so a change this device had not sent survives. Text that the file replaces is kept as a conflict copy, because a file written again by a device that was behind can hold older text.
Alternative rejected: writing a deleted file again with the old `collection`. Tasks that other devices created after the writing device last synced would look deleted to them.

### Remember each upload until its answer arrives

The merge transaction stores the snapshot it is about to upload. Each upload adds a random identifier to the file's `uploads` list, which keeps the last 20.
When a later download lists the identifier, the stored snapshot is the base for that merge.
Without this, an upload that reached the storage while its answer was lost would come back as another device's change. A date edited again in between would revert, and a title would produce a conflict copy (found in review, reproduced with one device).
The identifier list also covers the case where another device uploaded on top before the retry.

### Let a permanent deletion win over a reorder

A record deleted on one device stays deleted when the other device changed only its `position`. Positions also change when a neighbour moves or a merge renumbers them, so they do not show that the user edited the task.

### Normalize positions after a merge

Two devices that each create a task give both tasks the same `position`. Swapping equal positions does nothing, so the user could not reorder them.
After a merge, Husker orders tasks by `position` and then `uid` and raises any position that does not exceed the previous one. Lists get the same treatment.
Every device computes the same result from the same merged state.

### Sign in with a SAS token or a managed identity

With `auth = "sas"`, the default, the URL in `sync.toml` carries a SAS token and Husker sends it unchanged.
With `auth = "managed-identity"`, Husker requests a token for `https://storage.azure.com/` from the Azure Instance Metadata Service at `169.254.169.254` and sends it as a bearer token (assumed from the Azure documentation; verified only against a local test double, not on a VM).
A SAS token for the one blob and a role on the one container both keep a device from reaching other data. The managed identity stores no secret on the VM.
Failure messages never include the URL, because its query string is a secret.
Alternative rejected: the storage account key. It grants every operation on the account and needs request signing.

### Run sync on a worker thread with its own connection

A worker thread opens its own connection to the database and does all network requests. The terminal thread never waits for the network, except for one bounded attempt at exit.
The worker notices local changes through `data_version`, which changes when any other connection commits. No store call site needs a hook.
Every sync transaction that writes increments a `commits` counter in the sync-state table. The terminal reads the counter when it sees an external commit and skips the external-update notice for a sync commit.
A sync that finds nothing to change writes nothing, so the once-a-minute timer does not make the terminal reload.
Processes that share a profile share the base and the ETag through the database. A merge is refused when the stored ETag moved after the download, because merging an older file against a newer base would delete tasks.
Alternative rejected: skipping a timer sync when another process synced recently. It needs a timestamp write on every sync, and each such write looks like an external change to the terminal.
Alternative rejected: applying the merge on the terminal thread. The MCP server and the command would need a second driver for the same steps.

### Use `ureq` with the platform certificate verifier

`ureq` is a blocking client with few dependencies, which fits a worker thread. The platform verifier accepts the certificates the operating system trusts, including a company proxy's (measured: a request from the development laptop to `blob.core.windows.net` completed the TLS handshake).
Requests time out after 10 seconds.

## Risks / Trade-offs

- [Every device holds every list] → The MCP `--list` restriction limits an agent's tools, not the files on its machine. Documented in the guide.
- [The base is parsed on every sync] → At 23 KB this takes well under a millisecond (assumed, not measured). A profile with 10,000 tasks would parse about 3 MB per sync.
- [Old executables cannot open schema version 5] → The upgrade notes tell the user to update every executable together.
- [An old process that stays open across the migration inserts rows without a `uid`] → Every sync first gives such rows a `uid`.
- [A CLI change waits for the next sync] → `husker sync` sends it at once. Documented.
- [After the sync file is deleted, a device that was behind can write it again] → Nothing is deleted and replaced text is kept as a copy, but an older date, status, list, or Trash state from that device replaces newer values on the others. Blob versioning lets the user restore the file instead of deleting it.
- [A sync commit and another program's commit in the same second] → The terminal compares the loaded lists and tasks to tell them apart, but only in its once-per-second check. A commit that lands in the moment of the user's own action is not reported.
- [Managed identity is not tested on a real VM] → A test double covers the token request and the bearer header. Reported as not verified.
- [An exit without network waits up to 10 seconds] → The terminal shows that it is syncing and exits when the attempt ends.

## Migration Plan

1. Back up the profile directory.
2. Close every Husker process and MCP client that uses the profile.
3. Install 0.15.0. The first writable start migrates the database to schema version 5.
4. To roll back, restore the backup and reinstall 0.14.0. A migrated database cannot be opened by 0.14.0.
