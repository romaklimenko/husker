# Command line

## Purpose

Define how terminal users and scripts create, find, and change tasks with Husker commands.
Task storage owns persistence and validation. Planning and search owns filter behavior.
## Requirements
### Requirement: Default interactive launch

Husker SHALL open the terminal interface when invoked without a subcommand in an interactive terminal.
Without an interactive terminal, Husker SHALL fail with instructions to use a subcommand.

#### Scenario: Redirected default launch

- **WHEN** `husker` runs with redirected input
- **THEN** Husker exits unsuccessfully without changing terminal settings
- **AND** stderr directs the user to a noninteractive command

### Requirement: Scriptable task operations

Husker SHALL provide noninteractive commands for these task operations.

| Command | Result |
| --- | --- |
| `add TITLE` | Create an open task with optional list, description, and due date. |
| `tasks [QUERY]` | Print matching tasks in planning order. |
| `show ID` | Print one task, including its description, status-change date, and current version. |
| `edit ID` | Update supplied task fields and retain fields not supplied. |
| `done ID` | Set status to done. |
| `cancel ID` | Set status to cancelled. |
| `reopen ID` | Set status to open. |
| `trash ID` | Move the task to Trash. |
| `restore ID` | Restore the task to its original list and status. |

The task-storage spec owns validation and persistence.
The planning-search spec owns query behavior. `tasks --trash` selects Trash.
Structured tags and readiness are not command options.
The removed options `--tag`, `--clear-tags`, `--anytime`, and `--readiness` produce argument errors without changing data.
Default creation without `--list` uses the lowest-ID ordinary list.

#### Scenario: Create and complete a task

- **WHEN** a script runs `husker add "Buy milk #shopping" --list Inbox`
- **THEN** Husker prints the created task's stable ID
- **AND** `husker done ID` persists the done status and current local date for that task

#### Scenario: Reject a removed option

- **WHEN** a script runs `husker add "Buy milk" --tag shopping`
- **THEN** Husker reports an unrecognized argument and creates no task

### Requirement: List operations

Husker SHALL provide `lists`, `list-add NAME`, and `list-rename ID NAME` commands.
`lists` SHALL use the persistent display order defined by task storage, include open task counts, and place virtual Trash last.
Creating a list appends it before Trash. Renaming a list preserves its position.

#### Scenario: Rename a list

- **WHEN** a script creates list `Private` and renames its ID to `Home`
- **THEN** `husker lists` shows `Home` with the same ID and display position
- **AND** tasks keep their list membership

### Requirement: Structured output and failures

Husker SHALL accept the global `--json` option for data commands and return JSON with stable English field names and status values.
Task JSON includes nullable `status_changed_at` alongside `status_changed_on`.
A known timestamp is an RFC 3339 UTC string. A new open task or migrated task without a recorded time has a null timestamp.
Existing command names, arguments, and English status values remain unchanged.
Failures SHALL return a nonzero exit code and write a message to stderr without printing success data to stdout.

#### Scenario: Invalid task ID

- **WHEN** `husker --json show 999` requests a missing task
- **THEN** the command exits unsuccessfully
- **AND** stdout contains no task JSON

#### Scenario: Return a completion timestamp

- **WHEN** `husker --json done ID` changes an open task to done
- **THEN** the task JSON includes its local `status_changed_on` date and UTC `status_changed_at` timestamp
- **AND** `husker --json reopen ID` returns null for both fields

### Requirement: Validate dates before writing

Husker SHALL accept due dates only in `YYYY-MM-DD` format when the date exists.
`edit ID --clear-due` SHALL remove the date and assign the undated due bucket.
Open tasks appear in the no-date display group. Done and cancelled tasks remain in their status sections, as defined by planning-search.
The terminal names the no-date group Sometime in English and Engang in Danish. Plain CLI output retains the stable `undated` date token.
Supplying both `--due` and `--clear-due` is an argument error.

#### Scenario: Invalid calendar date

- **WHEN** `husker add "Buy milk" --due 2026-02-30` runs
- **THEN** Husker reports an invalid date and creates no task

#### Scenario: Remove a due date

- **WHEN** `husker edit ID --clear-due` runs for a dated open task
- **THEN** the task has no due date and appears under Sometime in the English terminal interface
- **AND** plain command output uses `undated` and JSON output contains a null `due` value

#### Scenario: Remove a completed task's due date

- **WHEN** `husker edit ID --clear-due` runs for a task completed today
- **THEN** its due date becomes null and its status and status-change date remain unchanged
- **AND** the terminal shows it in Done today while plain CLI output uses the `undated` date token

### Requirement: Explicit configuration inspection

Husker SHALL provide `config --example` to print a complete default TOML configuration without creating a profile or opening its database.

#### Scenario: Inspect shortcuts

- **WHEN** a user runs `husker config --example`
- **THEN** stdout includes the language setting and the default action bindings

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

### Requirement: Run the online server

Husker SHALL provide `husker serve` to run the online server until the process is stopped.
`husker serve` listens only on the loopback address `127.0.0.1`.
`husker serve` takes the port from the first source in this table that is set.

| Port source | Port |
| --- | --- |
| `--port PORT` | The given port. |
| The environment variable `FUNCTIONS_CUSTOMHANDLER_PORT` | The value of the variable. |
| Neither | 8787. |

`husker serve` ignores `--profile`, `--config`, and `--json`. `husker serve` creates no profile and opens no database.
The online-server spec defines the settings and the behavior of the online server.
When the online server cannot start, `husker serve` exits unsuccessfully with a message on stderr.

#### Scenario: Start on a given port

- **WHEN** the settings are valid and the user runs `husker serve --port 9000`
- **THEN** `GET http://127.0.0.1:9000/api/health` answers 200

#### Scenario: Start without a profile

- **WHEN** `husker serve` starts on a machine without a `~/.husker` directory
- **THEN** `husker serve` serves requests and creates no directory

### Requirement: Create access tokens

Husker SHALL provide `husker token` to sign an access token for the online server with the private key of the selected profile.
The online-server requirement `Access tokens are signed on the user's computer` defines the access token and how the online server checks it.

| Option | Meaning |
| --- | --- |
| `--name NAME` | The label of the access token, 1 to 64 characters. Required to sign an access token. |
| `--list LIST` | Limit the access token to this list of the profile. Without `--list`, the access token covers every list. |
| `--read-only` | Sign an access token without write permission. |
| `--days N` | The lifetime in days, from 1 through 400. The default is 365. |
| `--url URL` | The public address of the online server. Husker stores the address in `online.toml` and uses the stored address for later access tokens. |
| `--public-key` | Print the public key instead of signing an access token. `--public-key` conflicts with every other option in this table except `--url`. |

The private key and the public address of the online server are stored in the file `online.toml` in the profile directory.

| Key | Value |
| --- | --- |
| `key` | The Ed25519 private key as a PKCS#8 document in base64url without padding. |
| `url` | The public address of the online server: scheme, host, and an optional port. |

When `online.toml` does not exist, `husker token` creates `online.toml` with a new random private key.
Signing an access token needs an address from `--url` or from `online.toml`.
`--list LIST` selects the list whose name equals `LIST` after trimming and Unicode lowercase conversion. `husker token` stores the `uid` of that list in the access token.
When the list has no `uid` yet, the user must sync the profile once before `husker token` can sign an access token for that list.
The text output is the access token alone on stdout and one line on stderr with the token ID and the expiry date.
With `--json`, stdout is an object with the members `id`, `name`, `list`, `write`, `expires`, and `token`.
`--public-key` prints the public key in base64url without padding on stdout.

Each of these cases is an error, and `husker token` prints no access token.

| Error case |
| --- |
| `online.toml` cannot be read, is not valid TOML, has an unknown key, or holds a `key` value that is not a valid private key. |
| Neither `--url` nor `online.toml` gives an address. |
| The address has no `https` scheme, has no host, or has a path, a query, or a fragment. `http` is accepted only for the hosts `localhost`, `127.0.0.1`, and `[::1]`. |
| `--name` is missing, empty, or longer than 64 characters. |
| `--days` is outside 1 through 400. |
| The list that `--list` names does not exist or has no `uid`. |

An error names `online.toml` when that file causes the error. No error prints the private key.

#### Scenario: Sign a token for one list

- **WHEN** the profile holds a synced list Husker and the user runs `husker token --name codex --list Husker --url https://tasks.example.net`
- **THEN** stdout is one line that starts with `hsk1_`
- **AND** the payload of the access token holds the `uid` of Husker as `list`, `write` true, and `aud` `https://tasks.example.net`

#### Scenario: Print the public key for a deployment

- **WHEN** the profile has no `online.toml` and the user runs `husker token --public-key`
- **THEN** Husker creates `online.toml` and prints a public key of 43 characters
- **AND** a second run prints the same key

#### Scenario: Refuse a lifetime that is too long

- **WHEN** the user runs `husker token --name phone --days 500`
- **THEN** `husker token` fails and prints no access token

### Requirement: Pair a phone

Husker SHALL provide `husker pair` to sign an access token for a phone and hand it to the online server under a pairing code.
The phone-page requirement `Pairing codes hand an access token to a phone once` defines the pairing code.

| Option | Meaning |
| --- | --- |
| `--name NAME` | The label of the access token. The default is `phone`. |
| `--list LIST` | Limit the access token to this list of the profile. Without it the access token covers every list. |
| `--read-only` | Sign an access token without write access. |
| `--days N` | The lifetime in days, from 1 through 400. The default is 365. |
| `--url URL` | The public address of the online server. Husker stores it in `online.toml` after the online server answered with a pairing code. |

The command signs the access token as the requirement `Create access tokens` defines, sends it to `POST /api/pairings` of the online server, and receives a pairing code.
In an interactive terminal, the command prints a QR code of the address `URL/#code=CODE`, the pairing code in groups of four characters, how long the code works, and the ID and the expiry date of the access token.
The QR code is drawn black on white, whatever the colors of the terminal are, and the code with the two lines under it fits a terminal of 24 rows and 80 columns when the address with the code has at most 78 characters.
With `--json`, or when stdout is not a terminal, the command prints no QR code. With `--json` the output is an object with the members `code`, `link`, `expires_in`, `id`, and `expires`. Otherwise stdout is the link on one line, and the command writes the ID and the expiry date of the access token to stderr.
The command never prints the access token.
The command checks the certificate of the online server against the certificate store of the computer, as `husker sync` does, and follows no redirect.

Each of these cases is an error, and the command prints no code.

| Error case |
| --- |
| An error case of `husker token` for the same options. |
| The online server cannot be reached. |
| The online server refuses the access token, for example because the profile's key is not among the server's public keys. |

#### Scenario: Show a pairing code

- **WHEN** the online server runs at the address in `online.toml` and the user runs `husker pair` in a terminal
- **THEN** the terminal shows a QR code, a code such as `K7QF-2M9X-TB4H`, and that the code works for 5 minutes
- **AND** the terminal shows no access token

#### Scenario: Print a link for a script

- **WHEN** a script runs `husker pair` with redirected output
- **THEN** stdout is one line with an address that ends with `/#code=` and 12 characters
- **AND** stderr is one line that names the ID and the expiry date of the access token

#### Scenario: Mistype the address

- **WHEN** the user runs `husker pair --url URL` with an address where no online server answers
- **THEN** the command fails, and `online.toml` keeps the address it had

#### Scenario: Report a server that does not know the key

- **WHEN** the online server holds another public key and the user runs `husker pair`
- **THEN** the command fails and names the public key as the likely cause

