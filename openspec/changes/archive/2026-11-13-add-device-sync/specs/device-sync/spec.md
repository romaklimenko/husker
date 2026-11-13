## ADDED Requirements

### Requirement: Sync settings live in the profile

Husker SHALL enable sync for a profile only when the profile directory contains a `sync.toml` file.
Without `sync.toml`, Husker makes no network request.

| Key | Required | Value |
| --- | --- | --- |
| `url` | Yes | The URL of the sync file. It uses `https`. `http` is accepted only for the hosts `localhost`, `127.0.0.1`, and `[::1]`. |
| `auth` | No | `sas` or `managed-identity`. The default is `sas`. |
| `client_id` | No | The client ID of a user-assigned managed identity. Accepted only with `auth = "managed-identity"`. |

Each of these cases is an error.

| Part of `sync.toml` | Error case |
| --- | --- |
| The file | It exists and cannot be read, or it is not valid TOML. |
| Any key | It is not `url`, `auth`, or `client_id`. |
| `url` | It is missing. |
| `url` | Its scheme is not `https`, and it is not an `http` URL for one of the three hosts in the table above. |
| `url` | It has no host name. |
| `auth` | It is not `sas` or `managed-identity`. |
| `client_id` | It is set without `auth = "managed-identity"`. |

The error names `sync.toml` and never includes the URL.
The terminal, a writable MCP server, and `husker sync` report the error at startup and exit unsuccessfully.
A read-only MCP server and every other command do not read `sync.toml`.

#### Scenario: Leave sync off

- **WHEN** a profile has no `sync.toml` and the user starts the terminal
- **THEN** Husker opens the local tasks and makes no network request

#### Scenario: Reject an insecure URL

- **WHEN** `sync.toml` contains `url = "http://example.com/tasks.json"`
- **THEN** `husker sync` fails with an error that names `sync.toml`
- **AND** the error does not contain `example.com`

### Requirement: The sync file holds one snapshot of the profile

Husker SHALL store the synced state of a profile as one UTF-8 JSON object, the sync file, with these members.

| Member | Value |
| --- | --- |
| `format` | The integer 1. |
| `collection` | A random identifier created when the sync file is first written. Later uploads keep it. |
| `default_list` | The `uid` of the default list. |
| `uploads` | The identifiers of the most recent uploads, oldest first, at most 20. Each upload adds one random identifier. |
| `lists` | One object per list with `uid`, `id`, `name`, and `position`. |
| `tasks` | One object per task with `uid`, `id`, `list`, `title`, `description`, `due`, `status`, `status_changed_on`, `status_changed_at`, `deleted`, and `position`. |

These members of a list object or task object have a fixed form.

| Member | Value |
| --- | --- |
| `list` | The `uid` of the task's list. |
| `due`, `status_changed_on` | A `YYYY-MM-DD` string or null. |
| `status_changed_at` | An RFC 3339 UTC string or null. |
| `status` | `open`, `done`, or `cancelled`. |
| `deleted` | True for a task in Trash. |
| `id` | The integer ID the list or task had on the device that first uploaded it. Later uploads keep that value. |

The snapshot includes tasks in Trash and excludes task versions.
Husker rejects a downloaded sync file in each of these cases.

| Rejected sync file |
| --- |
| It is not valid JSON. |
| It has an unknown member. |
| It repeats a `uid`. |
| It has an empty `uid`, an empty task title, or a list name that is empty after trimming. |
| It has an empty `collection`. |
| Its `default_list` is not the `uid` of a list in the file. |
| An `id` is negative or above 2^53, or a `position` is outside -2^53 through 2^53. |
| Its `format` is missing or is not 1. |

A rejected sync file changes no local data, and Husker does not replace it.

#### Scenario: Refuse a file from a newer Husker

- **WHEN** the downloaded sync file has `"format": 2`
- **THEN** the sync fails and tells the user to update Husker
- **AND** local tasks and the sync file are unchanged

### Requirement: Sync downloads and uploads only when needed

Husker SHALL run one sync as the following steps.

1. Download the sync file with `If-None-Match` set to the ETag of the last synced file, if one is stored.
2. Merge the local database with the downloaded snapshot inside one immediate write transaction. A 304 answer means the sync file equals the last synced snapshot.
3. Upload the merged snapshot when it differs from the downloaded snapshot. Skip the upload when it does not differ.
4. Send the upload with `If-Match` set to the downloaded ETag, or `If-None-Match: *` when the sync file did not exist.
5. When the upload is refused because the sync file changed, start again from step 1.

One sync makes at most five attempts. An attempt that ends in a refused upload, or in a download discarded under the requirement `Processes that share a profile share sync state`, counts as one. After the fifth, the sync fails.

The local database keeps the merged lists and tasks whether or not the upload succeeds. The next sync sends an upload that did not succeed.

#### Scenario: Skip both transfers when nothing changed

- **WHEN** neither the sync file nor the local database changed since the last sync
- **THEN** the download answers 304 and Husker uploads nothing

#### Scenario: Retry after another device uploaded first

- **WHEN** devices A and B both download the sync file, and A uploads before B
- **THEN** B's upload is refused, B downloads A's file, merges it, and uploads the result
- **AND** the sync file then holds the changes of both devices

### Requirement: Merge field by field against the last synced snapshot

Husker SHALL merge each list and task by `uid` using three snapshots: the base, the local database, and the downloaded sync file.
The base is the snapshot that Husker stored at the last sync. The two sides are the local database and the downloaded sync file.

| Record state | Result |
| --- | --- |
| Present on one side only and absent from the base | Kept. |
| Present in the base, missing on one side, equal to the base on the other apart from `position` | Deleted. |
| Present in the base, missing on one side, different from the base on the other in another field than `position` | Kept with the changed values. |
| Present on both sides | Merged field by field. |

For a record present on both sides, a field that differs from the base on one side only takes that side's value.
A field that both sides changed to the same value keeps it.
A field that both sides changed to different values takes the value from the sync file.
A record that is present on both sides and absent from the base counts every differing field as changed on both sides.

| Record | Merged fields |
| --- | --- |
| List | `name` and `position`. |
| Task | `list`, `title`, `description`, `due`, `deleted`, `position`, and the status field. |

The status field is `status`, `status_changed_on`, and `status_changed_at` merged as one field.
The merge reads no clock and no random source. Two devices that merge the same three snapshots get the same result.

#### Scenario: Combine edits to different fields

- **WHEN** device A marks task 7 done and device B changes its planned date, both after the same sync
- **THEN** after both devices sync, task 7 is done and has B's planned date on both devices

#### Scenario: Delete a task that another device left alone

- **WHEN** device A deletes task 7 forever and device B did not change it
- **THEN** B's next sync removes task 7 from B

#### Scenario: Keep a task that another device edited

- **WHEN** device A deletes task 7 forever and device B changes its title before syncing
- **THEN** after both devices sync, task 7 exists on both devices with B's title

#### Scenario: Delete a task that another device only reordered

- **WHEN** device A deletes task 7 forever and device B moves task 7 below task 8 before syncing
- **THEN** after both devices sync, task 7 is gone from both devices

### Requirement: A text conflict keeps both versions

Husker SHALL keep both texts when the local database and the sync file changed `title` or `description` of one task to different values.
The task keeps the text from the sync file. Husker also creates a conflict copy, which is a new task.
The `uid` of a conflict copy is derived from the `uid` of the task and the title and description the copy keeps.
Two devices that keep the same text of one task therefore create one conflict copy, not two.
Husker adds no conflict copy when a task with that `uid`, title, and description already exists.
The conflict copy has every field of the local version, with ` #conflict` appended to its title. A title that already ends with `#conflict` stays as it is.
The conflict copy gets the position of the original task.
The requirement `Positions are unique after a merge` then places the conflict copy directly before or after the original task.
A conflict in any other field creates no conflict copy.

#### Scenario: Keep two titles

- **WHEN** device A renames task 7 to `Buy oat milk`, device B renames it to `Buy whole milk`, and A syncs first
- **THEN** after B syncs, task 7 is titled `Buy oat milk` and a new task is titled `Buy whole milk #conflict`
- **AND** after A syncs again, A shows both tasks

#### Scenario: Take the file's date without a copy

- **WHEN** device A plans task 7 for Monday, device B plans it for Friday, and A syncs first
- **THEN** after B syncs, task 7 is planned for Monday on both devices and no task is added

### Requirement: A first sync joins two collections without deleting

Husker SHALL delete no list and no task in a sync when no base is stored, when the sync file is missing, or when the `collection` of the sync file differs from the `collection` of the base.
The base is the snapshot that Husker stored at the last sync.

| Situation | Merge |
| --- | --- |
| No base is stored | Every list and task of both sides is kept. A field that differs takes the value from the sync file. |
| The sync file is missing | Husker uploads the local snapshot with a new `collection` and keeps the stored base until that upload succeeds. |
| The `collection` differs | Every list and task of both sides is kept. Fields merge against the stored base. |

When the `collection` differs, a field that only the local database changed since the base keeps the local value.
Every other differing field takes the value from the sync file. When that replaces a local title or description, Husker keeps the local text as a conflict copy.
In the first and third situation, the local default list takes the `uid`, name, and position of the list that `default_list` names in the sync file.

#### Scenario: Join from a new profile

- **WHEN** a new profile with only an empty `Inbox` syncs with a sync file whose default list is `Private` with ID 1 and 40 tasks
- **THEN** the profile's list 1 is named `Private` and holds those 40 tasks with the IDs they have in the sync file
- **AND** the sync file gains no `Inbox` list

#### Scenario: Survive a deleted file

- **WHEN** a device has synced before and the sync file no longer exists
- **THEN** the next sync keeps every local list and task and uploads them as a new sync file

#### Scenario: Keep an unsent change after the file was written again

- **WHEN** device A marks task 7 done without syncing, the sync file is deleted, and device B writes it again
- **THEN** after A syncs, task 7 is done on both devices and no task is deleted

#### Scenario: Keep newer text when an old device writes the file again

- **WHEN** devices A and B agree on the title `Book flights to Oslo`, the sync file is deleted, and device C writes it again with the older title `Book flights`
- **THEN** after all devices sync, the task is titled `Book flights` and one task is titled `Book flights to Oslo #conflict`

### Requirement: An upload with a lost answer is not mistaken for another device

Husker SHALL store the snapshot it is about to upload in the profile's database, in the same transaction as the merge.
When a later download holds the identifier of that upload in `uploads`, Husker uses the stored snapshot as the base for that merge.
A device that never received the answer to its upload therefore keeps the edits it made afterwards.
A successful upload, and a merge that needs no upload, remove the stored snapshot.

#### Scenario: Edit again after the answer was lost

- **WHEN** a device uploads a planned date of November 16 for task 7, the storage keeps the upload, the answer never arrives, and the user then sets November 20
- **THEN** the next sync keeps November 20, uploads it, and creates no conflict copy

#### Scenario: Recognize the upload under another device's upload

- **WHEN** the answer to a device's upload is lost and another device downloads that upload and uploads again before the first device retries
- **THEN** the first device's next sync still merges against its own upload and keeps its later edits

### Requirement: List names stay unique after a merge

Husker SHALL merge a local list that both the sync file and the base lack with a list from the sync file that the device lacks when both lists have the same name.
The base is the snapshot that Husker stored at the last sync.
Two names are the same when they are equal after trimming and Unicode lowercase conversion.
The local list takes the `uid` of the list from the sync file and keeps its local ID and tasks.
When two lists still share a name after the merge, the list with the greater `uid` gets the suffix ` (2)`, or the next free number.

#### Scenario: Merge two lists created with one name

- **WHEN** devices A and B each create a list named `Work` before syncing
- **THEN** after both sync, each device has one `Work` list that holds the tasks from both devices

### Requirement: Tasks never point to a missing list

Husker SHALL move a task to the local default list when the merge leaves the task's list absent.
The merge always keeps the local default list.

#### Scenario: Add a task to a list deleted elsewhere

- **WHEN** device A deletes list `Errands` and device B adds a task to `Errands` before syncing
- **THEN** after both sync, `Errands` is gone and the task is in the default list on both devices

### Requirement: Positions are unique after a merge

Husker SHALL order tasks by `position` and then `uid` after a merge and raise each position that does not exceed the previous one to the previous position plus one.
Lists receive the same rule. Positions that are already unique do not change.

#### Scenario: Reorder two tasks created on two devices

- **WHEN** devices A and B each create a task with position 60 before syncing
- **THEN** after both sync, the two tasks have positions 60 and 61 on both devices in the same order

### Requirement: Local IDs are never reused for another record

Husker SHALL insert a list or task from the sync file with the `id` from the sync file when that ID is higher than every ID the local table has ever assigned.
Otherwise the list or task gets the next local ID. A sync never changes the ID of an existing local list or task.
A local task changed by a sync gets one version increment. A task inserted by a sync has version 1.

#### Scenario: Keep IDs equal across devices

- **WHEN** device A creates task 61 and syncs, and device B has assigned IDs up to 60
- **THEN** B's sync inserts the task with ID 61

#### Scenario: Protect an ID that is in use locally

- **WHEN** devices A and B each create a task with ID 61 before syncing
- **THEN** each device keeps its own task as 61 and gives the other task a higher ID

### Requirement: Sign in with a SAS token or a managed identity

Husker SHALL send the configured URL unchanged when `auth` is `sas`, so a SAS token in its query string authorizes the request.
When `auth` is `managed-identity`, Husker requests a token for the resource `https://storage.azure.com/` from the Azure Instance Metadata Service and sends it as a bearer token.
The token request includes `client_id` when `sync.toml` sets it. Husker reuses a token until five minutes before it expires.
Husker stores no token on disk.

#### Scenario: Sync from an Azure VM without a secret

- **WHEN** `sync.toml` holds a URL without a query string and `auth = "managed-identity"`
- **THEN** each request carries a bearer token from the metadata service and `sync.toml` holds no secret

### Requirement: A failed sync keeps local data and hides the URL

Husker SHALL leave the local database unchanged when a download fails or a downloaded sync file is rejected.
Every request times out after 10 seconds.

| Failure | Kind |
| --- | --- |
| No connection, DNS failure, or timeout | `offline` |
| HTTP 401 or 403 | `denied` |
| A managed identity token that cannot be obtained | `denied` |
| Any other unexpected HTTP status | `rejected` |
| A sync file with another `format` | `incompatible` |
| A sync file that fails validation | `invalid` |
| Five attempts in one sync without a completed upload | `busy` |
| The local database cannot be read or written | `local` |

No failure message contains the URL or a token.

#### Scenario: Work without a network

- **WHEN** the network is unavailable and the user adds a task
- **THEN** the task is saved locally and the sync reports `offline`
- **AND** the next successful sync uploads the task

### Requirement: Processes that share a profile share sync state

Husker SHALL store the base and its ETag in the profile's database, so every process that uses the profile merges against the same base.
The base is the snapshot that Husker stored at the last sync.
A sync merges a download only when the stored ETag is still the one the download was conditioned on. Otherwise it downloads again.
A sync records an upload as the new base only when the stored ETag is still the one the upload replaced.
Each process syncs on its own schedule. Processes do not coordinate their timers.

#### Scenario: Run the terminal and an MCP server together

- **WHEN** the terminal and an MCP server use one profile, and the MCP server syncs between the terminal's download and merge
- **THEN** the terminal discards its download and downloads again
- **AND** no task is deleted or duplicated
