# online-server Specification

## Purpose
Define `husker serve`, the online server that reads and changes the sync file directly, without a database.
Describe its settings, access tokens, request checks, the rules for each upload, failures, and the health endpoint.
The device-sync spec owns the sync file format. The online-mcp spec owns the MCP tools.
## Requirements
### Requirement: Settings come from the environment

The online server SHALL read its settings from environment variables when `husker serve` starts.
The online server reads no profile, no `config.toml`, no `sync.toml`, and no database.

| Variable | Required | Value |
| --- | --- | --- |
| `HUSKER_SYNC_URL` | Yes | The URL of the sync file. The rules for `url` in `sync.toml` apply, as the device-sync requirement `Sync settings live in the profile` defines them. The online server signs in with the SAS token in the URL. |
| `HUSKER_ONLINE_KEYS` | Yes | One or more public keys, separated by commas. Each key is 32 bytes in base64url without padding. |
| `HUSKER_ONLINE_ORIGIN` | Yes | The public address of the online server: scheme, host, and an optional port, without a path. The scheme is `https`. `http` is accepted only for the hosts `localhost`, `127.0.0.1`, and `[::1]`. |
| `HUSKER_ONLINE_TZ` | No | An IANA time zone name. The default is `UTC`. |
| `HUSKER_ONLINE_REVOKED` | No | The token IDs of revoked access tokens, separated by commas. Each token ID is 12 lowercase hexadecimal characters. The default is none. |
| `HUSKER_ONLINE_READ_ONLY` | No | `true` or `false`. The default is `false`. |

When a required variable is missing or a value is invalid, `husker serve` exits unsuccessfully with a message that names the variable.
The message never contains the value of `HUSKER_SYNC_URL`.

#### Scenario: Refuse to start without a key

- **WHEN** `husker serve` starts without `HUSKER_ONLINE_KEYS`
- **THEN** `husker serve` exits unsuccessfully with a message that names `HUSKER_ONLINE_KEYS`
- **AND** `husker serve` makes no network request

#### Scenario: Hide the sync URL in a start error

- **WHEN** `husker serve` starts with `HUSKER_SYNC_URL` set to `http://example.com/tasks.json?sig=S3CRET`
- **THEN** `husker serve` exits unsuccessfully with a message that names `HUSKER_SYNC_URL`
- **AND** the message contains neither `example.com` nor `S3CRET`

### Requirement: One upload changes one list or task

The online server SHALL change the sync file only by these steps.

1. Download the sync file with its ETag. The online server may reuse its last download when the download answers 304 to `If-None-Match`.
2. Apply the requested change to one list or task in a copy of the downloaded snapshot.
3. Skip the upload when the copy equals the download.
4. Check the copy as the table below defines. A failed check ends the request without an upload.
5. Upload the copy with `If-Match` set to the downloaded ETag.
6. When the upload is refused because the sync file changed, start again from step 1.

| Check before an upload |
| --- |
| `format`, `collection`, `default_list`, and `uploads` of the copy equal those of the download. |
| Every list and task of the download is still present in the copy. |
| At most one list or task of the copy is new or differs from the download. |
| No `uid` appears twice among the lists or among the tasks of the copy. |
| A new task, and a task whose `list` changed, names a list of the copy. |
| The copy is not larger than 16 MiB as JSON. A larger copy fails with `The sync file is too large for the online server.` |
| No list or task of the download has another `id` or `position` in the copy. |
| The copy passes the validation that the device-sync requirement `The sync file holds one snapshot of the profile` defines for a downloaded sync file. |

One request makes at most five attempts. After the fifth refused upload, the request fails and reports that the sync file is busy, as the requirement `Storage failures change nothing` defines.
The online server reads no clock and no random source between step 1 and step 5 of a retry. A retry applies the same new `uid`, date, and time as the first attempt.
When an upload request gets no answer, the online server downloads the sync file once. A downloaded sync file that holds the changed list or task exactly as the upload left it counts as a successful upload.
When the downloaded sync file holds the list or task in another state, the request fails with the failure of the upload.
When that download fails, the request fails with `The change may not have been saved. Read the task again before you repeat it.` The sync file can then hold the change.

#### Scenario: Leave every other record untouched

- **WHEN** the sync file holds 3 lists and 40 tasks and a client marks one task done
- **THEN** the uploaded sync file differs from the download only in that task's `status`, `status_changed_on`, and `status_changed_at`

#### Scenario: Apply a change again after a device uploaded first

- **WHEN** a device replaces the sync file between the online server's download and upload, and the device changed another task
- **THEN** the online server's upload is refused, and the online server downloads the device's sync file, applies the requested change again, and uploads
- **AND** the sync file then holds both changes

#### Scenario: Skip an upload that changes nothing

- **WHEN** a client sets the status of a done task to `done`
- **THEN** the online server uploads nothing and returns the task

#### Scenario: Give up when the file keeps changing

- **WHEN** five uploads in one request are refused
- **THEN** the request fails with `The sync file kept changing.` and the sync file holds no change from the request

### Requirement: The server never creates the sync file or records an upload

The online server SHALL fail every request that needs the sync file while the sync file does not exist. The online server SHALL NOT create the sync file.
The online server SHALL NOT add an identifier to `uploads` and SHALL NOT remove an identifier from `uploads`.
The online server leaves `collection`, `default_list`, `format`, and the `id` of every existing list and task unchanged.

#### Scenario: Report a missing sync file

- **WHEN** the sync file does not exist and a client calls `add_task`
- **THEN** the call fails with `The sync file does not exist. Sync a computer first.`
- **AND** the sync file still does not exist

#### Scenario: Keep a device's upload identifier

- **WHEN** a device uploaded the sync file and the online server then changes 25 tasks
- **THEN** `uploads` still holds the identifier of the device's upload

### Requirement: New records get an identifier, an ID hint, and a position

The online server SHALL give a new list or task a `uid` of 32 lowercase hexadecimal characters taken from 16 random bytes.
The online server sets `id`, the ID hint, to one more than the highest `id` among the records of the same kind in the sync file.
The online server sets `position` to one more than the highest `position` among the records of the same kind, or to 1 when that highest `position` is below 0.
A new task has `status` `open` and `deleted` false. Its `status_changed_on` and `status_changed_at` are null.
When the sync file holds no record of the same kind, `id` and `position` are 1.
A request that would push `id` or `position` above 2^53 fails without an upload.

#### Scenario: Add a task after the highest hint

- **WHEN** the highest task `id` in the sync file is 57 and a client adds a task
- **THEN** the new task has `id` 58 and a `position` above the `position` of every other task

### Requirement: Online changes follow the task storage rules

The online server SHALL apply the task-storage rules for task titles, list names, status changes, and Trash to every change.
The task-storage spec owns those rules. The table lists the rules that the online server applies.

| Rule | Result |
| --- | --- |
| A title is trimmed. An empty title is refused. | An empty title fails with `A task title cannot be empty.` |
| A list name is trimmed, is not empty, and is not `Trash` or `Papirkurv` in any letter case. | An empty list name fails with `A list name cannot be empty.` A reserved list name fails with `Trash and Papirkurv are reserved list names.` |
| A list name is unique without regard to letter case. | A duplicate list name fails with `A list with that name already exists.` |
| A task's list exists. | When the list scope is every list, a `list_id` that names no list fails with `List ID does not exist.`, where ID is the `list_id` of the request. |
| A change to `done` or `cancelled` from another status stores the request's date and time. | `status_changed_on` holds the request's date. `status_changed_at` holds the request's time. |
| A change to `open` clears the status date and time. | `status_changed_on` and `status_changed_at` are null. |
| A change to `done` or `cancelled`, when the task already has that status, keeps the status date and time. | `status_changed_on` and `status_changed_at` are unchanged. |
| A task is in Trash when `deleted` is true. A task in Trash keeps its list, status, and dates. | Restoring returns the task as it was. |

The online-mcp requirement `Dates follow the configured time zone` defines the date and time of a request.
The online server offers no permanent deletion, no list deletion, and no reordering.

The online server also limits the length of text, because every upload sends the whole sync file to every device.

| Text | Longest accepted | Failure |
| --- | --- | --- |
| Task title | 1000 characters | `A task title can have at most 1000 characters online.` |
| Task description | 50000 characters | `A task description can have at most 50000 characters online.` |
| List name | 200 characters | `A list name can have at most 200 characters online.` |

A limit applies to text that a request sets. A task that already holds longer text can change in its other members.
A request body for `/mcp` larger than 512 KiB is answered with status 413.

#### Scenario: Refuse an empty title

- **WHEN** a client adds a task whose title is three spaces
- **THEN** the call fails with `A task title cannot be empty.` and the sync file is unchanged

#### Scenario: Refuse a description that is too long

- **WHEN** a client adds a task with a description of 50001 characters
- **THEN** the call fails with `A task description can have at most 50000 characters online.` and the sync file is unchanged

#### Scenario: Stamp a completion

- **WHEN** a client marks an open task done on 2026-11-15 at 09:30 UTC, and the request's date is 2026-11-15
- **THEN** the sync file holds `status_changed_on` 2026-11-15 and `status_changed_at` 2026-11-15T09:30:00Z for the task

### Requirement: Access tokens are signed on the user's computer

The online server SHALL read or change the sync file only for a request with an access token that one of the keys in `HUSKER_ONLINE_KEYS` signed.
The requirement `Health endpoint` names the two answers that need no access token.
An access token is the text `hsk1_`, the payload, a dot, and the signature. The payload and the signature are base64url without padding.
The signature is an Ed25519 signature over the text of the access token before the dot.
The payload is a JSON object with exactly these members.

| Member | Value |
| --- | --- |
| `id` | The token ID: 12 lowercase hexadecimal characters. |
| `name` | A label of 1 to 64 characters. |
| `aud` | The public address of the online server that the access token is for. |
| `list` | Null for every list, or the `uid` of one list. |
| `write` | True when the access token allows changes. |
| `iat` | The time of signing, in seconds since 1970-01-01 UTC. |
| `exp` | The expiry time, in seconds since 1970-01-01 UTC. |

The online server checks the signature before it reads the payload. The online server refuses an access token in each of these cases.

| Refused access token |
| --- |
| The access token does not start with `hsk1_`, has no dot, or has a payload or a signature that is not base64url. |
| No key in `HUSKER_ONLINE_KEYS` verifies the signature. |
| The payload is not a JSON object with exactly the seven members. |
| `id` is not 12 lowercase hexadecimal characters, or `name` does not have 1 to 64 characters. |
| `aud` differs from `HUSKER_ONLINE_ORIGIN` in its stored form: lowercase scheme and host, without a trailing slash. |
| `exp` is not later than the current time. |
| `iat` is more than 300 seconds after the current time. |
| `exp` is more than 400 days after `iat`. |
| `id` is in `HUSKER_ONLINE_REVOKED`. |

The command-line requirement `Create access tokens` defines how `husker token` creates an access token.

#### Scenario: Accept a token from the configured key

- **WHEN** a client sends an access token that the configured key signed, with a matching `aud` and a future `exp`
- **THEN** the request proceeds with the list scope and the write permission of the access token

#### Scenario: Refuse a changed payload

- **WHEN** a client changes `write` to true in the payload of an access token, keeps the signature, and sends the changed access token
- **THEN** the online server answers 401

#### Scenario: Refuse a revoked token

- **WHEN** `HUSKER_ONLINE_REVOKED` holds the `id` of an otherwise valid access token and a client sends that access token
- **THEN** the online server answers 401

### Requirement: A token limits lists and writes

The online server SHALL limit every read and write to the access of the request.
The access has a list scope and a write permission.

| Access | Source |
| --- | --- |
| List scope | The `list` of the access token: every list or one list. A request path can narrow a list scope of every list to one list, as the online-mcp requirement `The path can limit a server to one list` defines. |
| Write permission | The `write` of the access token. `HUSKER_ONLINE_READ_ONLY=true` removes the write permission from every request. |

When the list scope is one list, the online server reads and changes only that list and its tasks.
A task outside the list scope gives the same error as a task that does not exist.
When the list scope is one list, a change that would put a task in another list fails with `This store is restricted to one list.`
That covers `add_task` and `update_task` with a `list_id` other than the list of the list scope, whether or not that `list_id` names a list.
When the list scope is one list, a request cannot create or rename a list.
A list scope whose list is not in the sync file gives access to no list and no task. The online server never falls back to another list.
A task whose `list` names no list of the sync file counts as a task of the default list, for reads and for changes. A device moves such a task to the default list at its next sync.
Without write permission, every change fails with `This store is read-only.` The online server uploads nothing for such a change.
An MCP client never reaches that sentence. The mutation tools are not exposed without write permission, and a direct call fails as the online-mcp requirement `Online tools match the local tools` defines.
The online server checks the access on the snapshot that it uploads. A move by a device between the download and the upload therefore cannot bypass the list scope.

#### Scenario: Hide a task in another list

- **WHEN** the list scope is the list Husker and a client requests a task in the list Private by the task's `uid`
- **THEN** the call fails with `Task not found.`

#### Scenario: Refuse a move out of the scope

- **WHEN** the list scope is the list Husker and a client updates a task with the `uid` of the list Private as `list_id`
- **THEN** the call fails with `This store is restricted to one list.` and the sync file is unchanged

#### Scenario: Recheck the scope after a device moved the task

- **WHEN** the list scope is one list, a device moves a task out of that list after the online server's download, and the online server's upload is refused
- **THEN** the retry fails with `Task not found.` and uploads nothing

#### Scenario: Refuse a write when the server is read-only

- **WHEN** `HUSKER_ONLINE_READ_ONLY` is `true` and a client whose access token has `write` true marks a task done
- **THEN** the call fails and the sync file is unchanged

### Requirement: Requests are checked before the sync file is read

The online server SHALL answer a request to `/mcp` or `/mcp/list/NAME` without a valid access token with status 401 and a `WWW-Authenticate: Bearer` header.
That holds for a `NAME` that is not valid percent-encoded text too. With a valid access token, such a `NAME` is answered with status 404.
For such a request, the online server SHALL NOT send a request to the storage that holds the sync file.
An access token is valid when no refusal case of the requirement `Access tokens are signed on the user's computer` applies to it.
A client sends the access token in the `Authorization` header as `Bearer TOKEN`.
The online server SHALL answer 403 when the request has an `Origin` header that differs from `HUSKER_ONLINE_ORIGIN`.
The online server SHALL answer 403 when the request has an `X-Forwarded-Host` header that differs from the host in `HUSKER_ONLINE_ORIGIN`.
Both header checks apply to every path and come before the access token check. They ignore letter case. A port of 443 or 80 in `X-Forwarded-Host` counts as absent.
When `X-Forwarded-Host` holds several host names, the last one counts, because the proxy next to the online server adds it. The 403 answer names the host that the request was for.
Tool code never receives the `Authorization` header.
No answer and no log line of the online server contains an access token or the value of `HUSKER_SYNC_URL`.

#### Scenario: Answer an unknown caller without touching storage

- **WHEN** a client posts to `/mcp` without an `Authorization` header
- **THEN** the online server answers 401 with `WWW-Authenticate: Bearer`
- **AND** the storage receives no request

#### Scenario: Refuse a request from another web page

- **WHEN** a request carries `Origin: https://evil.example` and a valid access token
- **THEN** the online server answers 403

### Requirement: Storage failures change nothing

The online server SHALL report each failure in this table to the client and leave the sync file unchanged.
The one exception is an upload without an answer, which the requirement `One upload changes one list or task` defines.

| Failure | Report |
| --- | --- |
| The storage that holds the sync file cannot be reached. | `No connection to the sync storage.` |
| The storage refuses the credentials. | `The sync storage refused access.` |
| The storage gives another unexpected answer. | `The sync storage gave an unexpected answer.` |
| The sync file has a `format` other than 1. | `The sync file needs a newer Husker.` |
| The sync file fails validation. | `The sync file is not valid.` |
| The sync file does not exist. | `The sync file does not exist. Sync a computer first.` |
| Five uploads in one request were refused. | `The sync file kept changing.` |

An MCP tool reports the failure as a tool error.
A failure while the online server resolves `NAME` of a path `/mcp/list/NAME` happens before any tool runs. The online server answers it with a JSON error object and status 503 for a missing or busy sync file, and status 502 otherwise.
The report never contains the value of `HUSKER_SYNC_URL`.

#### Scenario: Refuse a newer sync file

- **WHEN** the sync file has `"format": 2` and a client calls `list_tasks`
- **THEN** the call fails with `The sync file needs a newer Husker.`
- **AND** the online server uploads nothing

### Requirement: Health endpoint

The online server SHALL answer `GET /api/health` with status 200 and a JSON object with the members `name`, `version`, and `format`.
`name` is `husker`. `version` is the Husker version. `format` is the sync file format that the online server reads and writes.
`GET /api/health` needs no access token. To answer `GET /api/health`, the online server sends no request to the storage that holds the sync file.
The online server answers a request for any other path that it does not serve with status 404 and a JSON error object. That answer needs no access token either.
A JSON error object has one member, `error`, an object with the members `code` and `message`. `code` is a stable English word, and `message` is a sentence for a person.

#### Scenario: Check a deployed server

- **WHEN** the online server runs Husker 0.17.0 and a client requests `GET /api/health` without an access token
- **THEN** the answer is a JSON object with `name` `husker`, `version` `0.17.0`, and `format` 1

