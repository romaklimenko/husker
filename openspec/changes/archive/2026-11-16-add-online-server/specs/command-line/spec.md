## ADDED Requirements

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
