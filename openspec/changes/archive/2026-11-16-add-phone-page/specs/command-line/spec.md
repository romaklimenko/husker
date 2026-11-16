## ADDED Requirements

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
