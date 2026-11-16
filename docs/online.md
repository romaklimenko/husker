# Use Husker online

This guide is for people who already sync Husker through Azure and want
one of two things: an AI agent that reaches their tasks without a Husker
process on the agent's computer, or their tasks on a phone. Set up sync
first by following [Sync tasks between computers](sync.md). The online
server is available from Husker 0.17.0. Its page for a phone is available
from Husker 0.18.0.

## What the online server does

The online server is Husker running as a web server in your own Azure
subscription. It reads and writes the sync file, the file in Azure Blob
Storage that your computers already sync with. It keeps no database of its
own.

An agent connects to the online server over HTTPS and gets the same task
tools as from a [local `husker mcp` server](mcp.md). It can find, add, and
edit tasks, change their status, and move them to or from Trash.

Two things change when an agent uses the online server:

- The agent starts no Husker process on your computer. A local
  `husker mcp` server keeps `husker.exe` running for as long as the agent
  is connected. On Windows, a running program locks its file, so
  `cargo install --path . --locked` fails until you disconnect the agent.
  With the online server, you can install a new build while the agent
  stays connected.
- The agent's computer holds no copy of your tasks. An agent that runs
  `husker mcp` on another computer syncs every list to that computer. An
  agent on the online server reads only what its access token allows.

The online server also serves a page for your phone, the phone page. The
phone page is at the address of the server,
`https://<app>.azurewebsites.net/`. It opens on All tasks with the
headings you know from the terminal. On the phone page you add tasks, mark
them done, plan them, edit them, and move them to another list or to
Trash. Pair the phone once with `husker pair` on your computer. See
[Pair a phone](#pair-a-phone).

Your computers work as before. Each keeps its own `tasks.db`, works
without a network, and syncs in the background. They need no new setting.
They never contact the online server. They receive its changes from the
sync file at their next sync.

Every request for your tasks needs an access token. You create access
tokens on your own computer: `husker token` creates one for an agent, and
`husker pair` creates one for a phone. Nothing in Azure can create one.
Anyone who knows the address can open the phone page. Without a pairing
code, the phone page shows the pairing screen and none of your tasks.

## What you need

- Sync that already works. At least one computer must have run
  `husker sync`, so the sync file exists. The online server never creates
  the sync file.
- The Husker 0.18.0 source directory on a Windows computer. The deployment
  script builds Husker from source and is written for PowerShell on
  Windows.
- The [Azure CLI](https://learn.microsoft.com/cli/azure/install-azure-cli),
  signed in with `az login`. Your account must be allowed to create
  resources in the resource group that holds the sync storage account, and
  to read the keys of that account. The Owner and Contributor roles allow
  both.
- The tools for a Linux build. The server runs on Linux in Azure, so the
  script builds a Linux executable on your computer. Add the Linux target
  to Rust and install `cargo-zigbuild`:

  ```text
  rustup target add x86_64-unknown-linux-musl
  cargo install cargo-zigbuild
  ```

  Install [Zig](https://ziglang.org/download/) too. `zig version` prints
  a version number when Zig is ready.

## Deploy the server to Azure

Open PowerShell in the Husker source directory and run the deployment
script:

```powershell
./deploy/azure/deploy.ps1 -Subscription <subscription> -ResourceGroup husker -SyncAccount <account> -App <app> -CreatePolicy
```

| Parameter | Value |
| --- | --- |
| `-Subscription` | The name or ID of the Azure subscription that holds the sync file. `az account show --query id -o tsv` prints the ID of the subscription you are signed in to. |
| `-ResourceGroup` | The resource group that holds the sync storage account. The sync guide creates it as `husker`. The server goes into the same resource group. |
| `-SyncAccount` | The storage account that holds the sync file. It is the `<account>` in the URL in `sync.toml`. |
| `-App` | A new name for the server, made of lowercase letters, digits, and hyphens. The address of the server becomes `https://<app>.azurewebsites.net`, so the name must be unique in all of Azure. |
| `-CreatePolicy` | Lets the script create the stored access policy in step 3. Pass it on the first run. Later runs don't need it. |

The script runs these steps. Each step is safe to repeat, so you can run
the script again after a failure.

1. It builds Husker for your computer and for Linux. The first run
   compiles Husker twice.
2. It reads the public key of your Husker profile. If the profile has no
   key yet, Husker creates one. See
   [Keep the key file safe](#keep-the-key-file-safe).
3. It checks the stored access policy `husker-online` on the container of
   the sync file. A stored access policy is a set of permissions that you
   can revoke later. This policy lets the server read and write the sync
   file. With `-CreatePolicy` the script creates the policy when it is
   missing. Without it, the script stops when the policy is missing.
4. It creates the server and the Azure resources the server needs.
5. It uploads the Linux executable and starts the server.
6. It waits until the server answers.

The last three lines look like this:

```text
Husker 0.18.0 is online at https://<app>.azurewebsites.net. Writes allowed: False.
Next, for an agent: husker token --url https://<app>.azurewebsites.net --name NAME
Next, for a phone: husker pair --url https://<app>.azurewebsites.net
```

[Create an access token](#create-an-access-token) explains the first
command, and [Pair a phone](#pair-a-phone) explains the second.

If the server does not answer in time, the script stops with a message
like this one. Run the script again.

```text
The server did not answer with version 0.18.0 at https://<app>.azurewebsites.net/api/health. The last answer was: version 0.17.0
```

The text after `The last answer was:` is what the script got last. In
this example, the previous version still answered. The text can also be
an error, such as a refusal from the server that names a host name.

The script prints no secret. Azure creates the key that lets the server
reach the sync file and puts it straight into the server's settings.

The script creates these resources in the resource group:

| Resource | Name | What it is for |
| --- | --- | --- |
| Function app | `<app>` | Runs the server. |
| Plan | `<app>-plan` | The Flex Consumption plan of the function app. See [What the server costs](#what-the-server-costs). |
| Storage account | `<app>` without hyphens, followed by `host` | Holds the uploaded executable. It is separate from the sync storage account, so Azure's function host never holds a key of the account with your tasks. |
| Stored access policy | `husker-online`, on the `husker` container of the sync storage account | The server's permission to read and write the sync file. |

A storage account name can hold 24 characters at most, so the script cuts
a longer name. If the name is taken, pass another one with `-HostAccount`.

## Check the read-only server, then allow changes

The first deployment is read-only. Every access token can read tasks, and
no access token can change them. Use this state to check that the server
reads the right sync file:

1. [Create an access token](#create-an-access-token) and
   [connect Claude Code](#connect-claude-code) or
   [Codex](#connect-codex).
2. Ask the agent to list your tasks. Run `husker sync` and then
   `husker tasks` on a computer, and compare the two lists.
3. Run the script again with `-AllowWrites` added:

   ```powershell
   ./deploy/azure/deploy.ps1 -Subscription <subscription> -ResourceGroup husker -SyncAccount <account> -App <app> -AllowWrites
   ```

The last line of the output then ends with `Writes allowed: True.`
Restart the agent's session, so it asks the server for its tools again.
On a paired phone, tap `↻`. The phone page then shows the controls that
change tasks.

If you use the phone page and no agent, do steps 1 and 2 with the phone.
[Pair a phone](#pair-a-phone), run `husker sync` on a computer, and
compare All tasks on the phone page with All tasks in the terminal. The
phone page shows `Read only` until you have run step 3.

Every run of the script sets all server settings again from its
parameters. A run without `-AllowWrites` makes the server read-only again.
Pass `-AllowWrites` on every later run. The same holds for the optional
parameters below: a run without `-Revoked` lets revoked access tokens work
again.

## Set other deployment options

Add these parameters to the script when the defaults don't fit:

| Parameter | Default | Meaning |
| --- | --- | --- |
| `-AllowWrites` | Off | Lets access tokens with write access change tasks. |
| `-TimeZone` | `Europe/Copenhagen` | The time zone that decides which day is today for agents. The phone page uses the phone's own date. See [Choose the time zone for dates](#choose-the-time-zone-for-dates). |
| `-Revoked` | None | The token IDs of access tokens that the server refuses, separated by commas. See [Revoke one access token](#revoke-one-access-token). |
| `-CreatePolicy` | Off | Creates the stored access policy when it is missing. See [Cut the server off from the sync file](#cut-the-server-off-from-the-sync-file) before you use it after the first run. |
| `-HuskerProfile` | `~/.husker` | The profile directory whose key signs your access tokens. |
| `-SyncContainer` | `husker` | The container of the sync file. |
| `-SyncBlob` | `tasks.json` | The name of the sync file. |
| `-SyncPolicy` | `husker-online` | The name of the stored access policy for the server. |
| `-HostAccount` | Derived from `-App` | The name of the new storage account for the uploaded executable. |
| `-MaxInstances` | `1` | The most copies of the server that Azure may run at the same time. Keep `1` if you pair phones, because a pairing code is known to one copy only. See [Pair a phone](#pair-a-phone). |
| `-SkipBuild` | Off | Uses the executables from an earlier run and skips the build. |

For example, this run allows changes and uses the time zone of New York:

```powershell
./deploy/azure/deploy.ps1 -Subscription <subscription> -ResourceGroup husker -SyncAccount <account> -App <app> -AllowWrites -TimeZone America/New_York
```

## Create an access token

Run `husker token` on the computer that ran the deployment script. Give
the access token a name that tells you who uses it, and give the address
of your server:

```text
husker token --name claude-code --url https://<app>.azurewebsites.net
```

Husker prints the access token on one line. The access token starts with
`hsk1_`. A second line shows the token ID and the day the access token
stops working:

```text
Token 0123456789ab expires on 2027-11-15.
```

The second line goes to standard error. A script that captures the output
of `husker token` therefore gets only the access token.

Write down the name and the token ID. You need the token ID to
[revoke the access token](#revoke-one-access-token) later. The token ID is
not a secret.

Treat the access token like a password. Anyone who has it can read your
tasks until it expires, and can change them unless you created it with
`--read-only`.

A phone needs no `husker token`. `husker pair` creates the phone's access
token and hands it to the phone without showing it. See
[Pair a phone](#pair-a-phone).

| Option | Meaning |
| --- | --- |
| `--name NAME` | Required. A label of 1 to 64 characters, such as `claude-code` or `codex-laptop`. |
| `--url URL` | The address of your online server. Husker stores the address in `online.toml`, so you can leave `--url` out the next time. |
| `--list LIST` | The access token reaches only this list. Without `--list`, it reaches every list. `LIST` is the name of a list in your profile. The name ignores case. |
| `--read-only` | The access token can read tasks and cannot change them. |
| `--days N` | The number of days the access token works, from 1 through 400. The default is 365. |
| `--public-key` | Prints the public key and creates no access token. The deployment script uses it. You can combine it only with `--url`. |

For example, this command creates an access token that can only read the
list `Work`, for 30 days. It uses the address that Husker stored:

```text
husker token --name reviewer --list Work --read-only --days 30
```

A script can ask for JSON. Put `--json` before `token`:

```text
husker --json token --name codex-laptop --days 90
```

```json
{
  "expires": "2027-02-13",
  "id": "0123456789ab",
  "list": null,
  "name": "codex-laptop",
  "token": "hsk1_...",
  "write": true
}
```

`list` is `null` for an access token that reaches every list. For an
access token for one list, `list` is the 32-character identifier that the
list has in the sync file.

`husker token` fails with a message when something is missing. Without a
stored address and without `--url`, it prints this:

```text
Error: Give the address of the online server once with --url URL. Husker stores it in online.toml.
```

## Keep the key file safe

`husker token` and `husker pair` sign every access token with a private
key. The key is in the file `online.toml` in your profile directory, next
to `tasks.db`. For the default profile, the file is
`~/.husker/online.toml`. Husker creates the file with a new key the first
time you run `husker token`, `husker pair`, or the deployment script.

```toml
key = "<private key>"
url = "https://<app>.azurewebsites.net"
```

The online server holds only the public key. It can check an access token
with that key, and it cannot create one.

- Keep `online.toml` private. Anyone who has the private key can create
  access tokens for your server, for every list and with write access.
  Keep the file out of repositories, screenshots, and chat messages.
- Back up `online.toml`. A copy of your profile directory contains it, so
  store that copy as carefully as a password. If you lose the file, your
  existing access tokens keep working, but you cannot create new ones with
  the old key. You then start with a new key. See
  [Revoke every access token](#revoke-every-access-token).
- Sign access tokens with the profile that the deployment script used. The
  server refuses an access token that another profile's key signed, and
  `husker pair` then stops with an error that names the public key. To
  sign on a second computer, copy `online.toml` to the profile directory
  of that computer.
- Don't edit the `key` line. If `online.toml` is damaged, `husker token`
  and `husker pair` stop with an error that names the file. The error
  never shows the key.

## Connect Claude Code

Claude Code reads the address of your server and the access token from two
environment variables, `HUSKER_ONLINE_URL` and `HUSKER_ONLINE_TOKEN`. The
project file then holds no secret.

Set both variables for your user account. In PowerShell on Windows:

```powershell
$address = 'https://<app>.azurewebsites.net'
[Environment]::SetEnvironmentVariable('HUSKER_ONLINE_URL', $address, 'User')
[Environment]::SetEnvironmentVariable('HUSKER_ONLINE_TOKEN', (husker token --name claude-code --url $address), 'User')
```

In a POSIX shell such as Bash, add both variables to `~/.profile`. For
zsh, use `~/.zshrc`:

```sh
address="https://<app>.azurewebsites.net"
echo "export HUSKER_ONLINE_URL=\"$address\"" >> ~/.profile
echo "export HUSKER_ONLINE_TOKEN=\"$(husker token --name claude-code --url "$address")\"" >> ~/.profile
```

The last line of each block signs a new access token and stores it without
showing it. Only the line with the token ID appears on the screen. If you
keep your shell files in a repository, write the two `export` lines to a
file that is not in the repository.

Then create `.mcp.json` in the project directory, or add this entry to the
file:

```json
{
  "mcpServers": {
    "husker-project": {
      "type": "http",
      "url": "${HUSKER_ONLINE_URL}/mcp/list/Work",
      "headers": {
        "Authorization": "Bearer ${HUSKER_ONLINE_TOKEN}"
      }
    }
  }
}
```

Replace `Work` with the name of the list this project may use. Write a
space in a list name as `%20`, for example `/mcp/list/Home%20admin`. To
let the project reach every list, end the address at `/mcp`.

Open a new terminal, so it has the two variables, and start Claude Code
there. Run `/mcp` in Claude Code to check the connection. Claude Code asks
you to approve a server from `.mcp.json` the first time. See
[Claude Code's MCP guide](https://code.claude.com/docs/en/mcp#environment-variable-expansion-in-mcpjson)
for the `${NAME}` syntax.

The access token in this example reaches every list. The path
`/mcp/list/Work` limits this one connection to `Work`. An agent that can
read the variable can use the access token without the path. See
[Limit an agent to one list](#limit-an-agent-to-one-list) for an access
token that cannot leave one list.

## Connect Codex

Codex reads the access token from an environment variable that you name
in its settings. Set `HUSKER_ONLINE_TOKEN` as in
[Connect Claude Code](#connect-claude-code). Then add this entry to
`~/.codex/config.toml`:

```toml
[mcp_servers.husker-project]
url = "https://<app>.azurewebsites.net/mcp/list/Work"
bearer_token_env_var = "HUSKER_ONLINE_TOKEN"
```

Replace `<app>` with the name of your server and `Work` with the name of
the list. To reach every list, end the address at `/mcp`.

This command adds the same server:

```text
codex mcp add husker-project --url https://<app>.azurewebsites.net/mcp/list/Work --bearer-token-env-var HUSKER_ONLINE_TOKEN
```

Start Codex from a new terminal and check `/mcp`. See the
[Codex MCP settings guide](https://learn.chatgpt.com/docs/extend/mcp?surface=cli)
for the other settings of an entry.

## Limit an agent to one list

You can limit an agent in two places. The access token sets the limit that
holds. The address can narrow it further.

- The access token. `husker token --name vm-agent --list Work` creates an
  access token that reaches only `Work`. No address can widen it.
- The address. `/mcp/list/Work` at the end of the address limits one
  connection to `Work`. It works with an access token for every list, so
  one access token can serve several projects, each with its own list.

| Access token | Address ends with | What the agent reaches |
| --- | --- | --- |
| For every list | `/mcp` | Every list. |
| For every list | `/mcp/list/Work` | Only `Work`. |
| For every list | `/mcp/list/NAME`, and no list has that name | Nothing. The server answers 404. |
| For `Work` | `/mcp` | Only `Work`. |
| For `Work` | `/mcp/list/Work` | Only `Work`. |
| For `Work` | `/mcp/list/Private`, or a name that no list has | Nothing. The server answers 403. |

An access token for one list gets the same answer for another list and
for a name that no list has. The answer therefore does not show which
lists you have.

Use an access token for one list when the limit matters, for example for
an agent on a computer you don't control. Use the address when the limit
is a convenience, for example to give each of your own projects its list.

An agent that is limited to one list cannot read other lists, see their
task counts, or move a task to another list. It cannot create or rename
lists. Searches and Trash cover only that list.

The two limits follow a renamed list differently:

- An access token for one list stays with the list. If you rename `Work`
  to `Office`, the access token reaches `Office`.
- The address follows the name. After the rename, `/mcp/list/Work` no
  longer finds the list. Change the address to `/mcp/list/Office`. To make
  an address stay with a list, write the list's 32-character ID in place
  of the name. The agent's list results show that ID.

The agent gets these tools:

| Connection | Tools |
| --- | --- |
| Every list, with write access | All ten tools. |
| One list, with write access | Eight tools. `create_list` and `rename_list` are missing. |
| Read-only | Three tools: `list_lists`, `list_tasks`, and `get_task`. |

See [Ask the agent to manage tasks](mcp.md#ask-the-agent-to-manage-tasks)
for what the tools do. They work the same online, except for the
differences in the next three sections.

## Use the IDs and versions from the online server

Online, the ID of a task or a list is text of 32 characters, such as
`0123456789abcdef0123456789abcdef`. It is the identifier that the task or
list has in the sync file, so it is the same for every agent. It is not
the number that your computer shows. Task 7 in `husker tasks` has another
ID online.

Don't give an agent on the online server a number from your terminal. Ask
for the task by its title, and let the agent search for it.

A task's `version` is text of 16 characters online. The agent sends the
version it last read with every change. The version becomes another one
when anything changes the task, including a change on one of your
computers that has synced. The agent's change then fails with a message
that ends with `Reload it before saving.` The agent reads the task again
and repeats the change. Husker never overwrites the newer state.

When an agent adds a task without naming a list, the task goes to the
agent's one list. On a connection that reaches every list, it goes to
your default list.

The online server accepts a title of up to 1,000 characters, a
description of up to 50,000 characters, and a list name of up to 200
characters. Longer text fails with a message that names the limit. The
terminal has no such limits. A task that is longer on a computer stays
as it is, and an agent can still change its other fields.

## Choose the time zone for dates

The server has its own idea of which day is today. It takes the day from
one time zone that you choose at deployment with `-TimeZone`. The default
is `Europe/Copenhagen`. Use an IANA time zone name, such as
`America/New_York`.

The day decides three things for an agent:

- Which tasks a search for `due:today` or `due:overdue` finds, and how
  the results are grouped by date.
- Which closed tasks count as done today in a search without filters.
- The completion date of a task that the agent marks done or cancelled.

For example, an agent marks a task done at 00:30 on November 16 in
Copenhagen. With `-TimeZone Europe/Copenhagen`, the completion date is
November 16. With `-TimeZone UTC`, it is November 15, because the time in
UTC is 23:30 on that day. History on your computer then shows the task
under November 15.

To change the time zone, run the deployment script again with the new
`-TimeZone`.

The phone page does not use this time zone. The phone page sends the
phone's own date with every request, so Today on the phone page is today
on the phone. See [Use the phone page](#use-the-phone-page).

## Pair a phone

Pair a phone to open your tasks on the phone page. The phone page is the
page that the online server serves for a phone, at
`https://<app>.azurewebsites.net/`. Pairing gives the browser on the phone
an access token of its own. You pair each phone once. The phone stays
paired until its access token expires, after 365 days by default.

Pairing needs Husker 0.18.0 or newer in two places:

- On the server. If the server runs Husker 0.17.0,
  [update the server](#update-the-server) first.
- On the computer where you run `husker pair`. `husker --version` prints
  the installed version.

Run `husker pair` in a terminal on the computer that ran the deployment
script:

```text
husker pair
```

Husker uses the address of your server that `husker token --url` stored
in `online.toml`. If Husker has no stored address yet, give the address
once: `husker pair --url https://<app>.azurewebsites.net`. Husker stores
the address when the server has answered with a pairing code.

`husker pair` waits up to 40 seconds for the server to answer. A server
that Azure stopped has to start first. Then the terminal shows a QR code
and two lines:

```text
Scan the code with the phone, or open https://<app>.azurewebsites.net and type K7QF-2M9X-TB4H.
The code works once, for 5 minutes. Token 0123456789ab expires on 2027-11-15.
```

`K7QF-2M9X-TB4H` is the pairing code. A pairing code is 12 characters that
let one browser fetch the phone's access token from the server. The QR
code holds a link to the phone page with the same pairing code in it.

Pair the phone within 5 minutes:

1. Scan the QR code with the phone's camera and open the link.
2. Wait until the phone page shows All tasks. The phone is now paired.
   While the server checks the pairing code, the phone page shows
   `Loading tasks...`, or `Henter opgaver...` in Danish.
3. Write down the token ID from the second line, here `0123456789ab`,
   and the phone it belongs to. You need the token ID if you lose the
   phone. See
   [Sign out or revoke a lost phone](#sign-out-or-revoke-a-lost-phone).
   The token ID is not a secret. The menu of the phone page shows the
   token ID too, but you cannot open the menu of a lost phone.

If the phone cannot scan the QR code, type the pairing code:

1. Open `https://<app>.azurewebsites.net/` in the phone's browser. The
   phone page shows `Pair this phone`, or `Tilknyt denne telefon` in
   Danish.
2. Type the pairing code into the field that shows `XXXX-XXXX-XXXX`.
   The field's name for a screen reader is `Pairing code`, or
   `Tilknytningskode` in Danish.
3. Tap `Pair`, or `Tilknyt` in Danish. The phone page shows
   `Loading tasks...` while the server checks the pairing code, and
   then All tasks. One tap is enough. While a pairing code is being
   checked, a second tap on `Pair` does nothing.

You can type lowercase letters and leave out the hyphens. A pairing code
never contains the letters I, L, O, or U. The phone page reads a typed O
as the digit 0, and a typed I or L as the digit 1. The field also takes
the whole link from `husker pair`, for example when you paste it.

The QR code is black on white on every terminal theme. The QR code and
the two lines fit a terminal window of 24 rows and 80 columns when the
name of your app has at most 33 characters. The camera needs the whole QR
code. If the QR code does not fit in the terminal window, make the window
larger and run `husker pair` again, or type the pairing code.

Treat the QR code and the pairing code like a password for those 5
minutes. The first browser that sends the pairing code gets access to
your tasks, so don't run `husker pair` while you share your screen. The
access token itself never appears on a screen. The browser on the phone
stores the access token in a cookie, so the phone stays paired when you
close the browser. The phone page removes the pairing code from the
address as soon as it has read the code, so a bookmark of the phone page
holds no pairing code.

Pairing covers the browser that opened the link. Another browser on the
same phone needs a pairing code of its own. When the access token has
expired, the phone page shows `Pair this phone` again, and you pair the
phone again.

Before the phone page uses the pairing code of a link, it asks the server
whether this browser is paired already. If the browser is paired, the
phone page asks you first:
`This phone is paired already. Pair it again with this code?` Confirm to
replace the access token in that browser. Decline to keep the pairing as
it is. The phone page then does not use the pairing code.

If the phone does not reach the server when you open the link, the phone
page does not use the pairing code either. The phone page waits up to 30
seconds for the server. What it shows then depends on the phone:

- On a phone that is not paired, the phone page shows `Pair this phone`
  with the message `No connection to the server.`, or
  `Ingen forbindelse til serveren.` in Danish. The pairing code is
  already in the field. Tap `Pair` to try again.
- On a phone that is paired and shows its tasks, the phone page keeps
  showing the tasks that it saved on the phone, with
  `No connection to the server. Showing saved tasks.` above them. Scan
  the QR code again when the phone has a connection.

If the phone does not reach the server after you typed a pairing code
and tapped `Pair`, the phone page returns to `Pair this phone` and shows
`No connection to the server.` The pairing code that you typed is still
in the field. Tap `Pair` to try again.

A pairing code that the phone page did not use still works until its 5
minutes are over.

The menu of the phone page shows which access token a browser holds. Tap
`☰`. The first line reads
`Paired as phone until 2027-11-15 (token 0123456789ab)`. `phone` is the
name from `--name`. `2027-11-15` is the day the access token expires.
`0123456789ab` is the token ID, the value that `-Revoked` takes when you
[revoke the access token](#revoke-one-access-token).

| Option | Meaning |
| --- | --- |
| `--name NAME` | A label of 1 to 64 characters for the phone's access token, such as `tablet`. The default is `phone`. The menu of the phone page shows the label. |
| `--list LIST` | The phone reaches only this list. Without `--list`, it reaches every list. See [Limit a phone to one list or to reading](#limit-a-phone-to-one-list-or-to-reading). |
| `--read-only` | The phone shows tasks and cannot change them. |
| `--days N` | The number of days the phone stays paired, from 1 through 400. The default is 365. |
| `--url URL` | The address of your online server. Husker stores the address in `online.toml` when the server has answered with a pairing code, so you can leave `--url` out the next time. A mistyped address is not stored, and the address that Husker stored before stays. `husker token` uses the same stored address. |

If the phone page answers a pairing code with
`The code is wrong, used, or expired. Run husker pair again.`, run
`husker pair` again and use the new pairing code. The two common causes
are a pairing code older than 5 minutes and a server that started again
after `husker pair`. The server keeps pairing codes only in its memory.
Azure stops the server when no request arrives for a while, and a server
that starts again knows no earlier pairing code. See
[Check a failed connection](#check-a-failed-connection) for the other
causes.

A run of `husker pair` whose pairing code was never used leaves nothing
behind. Its access token reached no phone, and the server forgets the
access token together with the pairing code. You need no token ID from
that run.

`husker pair` draws the QR code only in a terminal. When its output goes
to a file or to another program, `husker pair` prints one line, the link
that the QR code holds:

```text
https://<app>.azurewebsites.net/#code=K7QF2M9XTB4H
```

A browser that opens the link is paired, once and within 5 minutes. The
token ID goes to standard error, as it does for `husker token`:

```text
Token 0123456789ab expires on 2027-11-15.
```

A script that captures the output of `husker pair` therefore gets only
the link. A script that needs the link and the token ID together asks
for JSON. Put `--json` before `pair`:

```text
husker --json pair --name tablet
```

```json
{
  "code": "K7QF2M9XTB4H",
  "expires": "2027-11-15",
  "expires_in": 300,
  "id": "0123456789ab",
  "link": "https://<app>.azurewebsites.net/#code=K7QF2M9XTB4H"
}
```

`expires_in` is the number of seconds that the pairing code works. `id`
is the token ID, and `expires` is the day the phone's access token stops
working. None of the three forms of output holds the access token.

## Use the phone page

Open `https://<app>.azurewebsites.net/` in the browser of a paired phone.
The phone page opens on All tasks. It shows your tasks under the headings
that All tasks has in the terminal: Earlier, Today, Tomorrow, each later
date, and Sometime, followed by Done today and Cancelled today. Every
heading ends with the number of tasks under it, such as
`Today - 2026-11-15 Sun W46 (2)`.

A row shows the title of a task. Below the title, in smaller text, the
row shows the task's list while the list filter is on `All tasks`. Rows
under Earlier, Done today, and Cancelled today also show the planned
date, such as `Work · 2026-11-10 Tue W46`.

The top of the phone page holds these controls:

| Control | What it does | In Danish |
| --- | --- | --- |
| The list filter, which starts as `All tasks` | Shows the tasks of one list. Each list shows its number of open tasks, such as `Work (3)`. Choose `All tasks` to show every list again. | `Alle opgaver` |
| `↻` | Loads the current tasks from the server. The sign turns gray and rotates while the phone page loads them. On a phone that is set to reduce motion, the sign only turns gray. | |
| `☰` | Opens the menu. Its first line names the pairing, such as `Paired as phone until 2027-11-15 (token 0123456789ab)`. Below it are `All tasks`, `History`, `Trash`, `English`, `Dansk`, `Sign out`, and `Close`. | `Tilknyttet som phone indtil 2027-11-15 (token 0123456789ab)`, `Alle opgaver`, `Historik`, `Papirkurv`, `English`, `Dansk`, `Log ud`, `Luk` |
| The `New task` field, the list next to it, and `+` | Adds a task. See [Add a task and mark tasks done on the phone page](#add-a-task-and-mark-tasks-done-on-the-phone-page). | `Ny opgave` |

Below the controls, each row has a checkbox on the left. Tap the checkbox
to mark the task done. Tap the row to read or change the task. See
[Change a task on the phone page](#change-a-task-on-the-phone-page).

The controls without a label of their own have a name for a screen
reader. `↻` is `Refresh data`, `☰` is `Menu`, and `+` is `Add`. The list
filter is `Show the tasks of a list`. The list next to the `New task`
field is `List for the new task`. A checkbox is `Mark done`, or
`Reopen task` on a done or cancelled task. In Danish, the names are
`Opdater data`, `Menu`, `Tilføj`, `Vis opgaverne på en liste`,
`Liste til den nye opgave`, `Markér som udført`, and `Genåbn opgave`.

The phone page loads the current tasks when you open it, when you return
to it from another app or browser tab, when the phone reports a
connection again, and when you tap `↻`. It does not update by itself
while it stays open on the screen. To see a change from a computer or an
agent, tap `↻`.

The phone page saves the tasks of All tasks on the phone each time it
loads them. When you open the phone page again, it shows these saved
tasks at once and replaces them when the server answers. You therefore
see your tasks while Azure starts a server that it stopped. The first
time after pairing, the phone has no saved tasks yet, and the phone page
shows `Loading tasks...`, or `Henter opgaver...` in Danish, until the
server answers.

If the phone cannot reach the server, the phone page shows
`No connection to the server. Showing saved tasks.` above the saved
tasks. In Danish, the message is
`Ingen forbindelse til serveren. Viser gemte opgaver.` The phone page
waits up to 30 seconds for an answer of the server. On a connection that
hangs, the message can therefore take 30 seconds to appear. You can read
the saved tasks. A change to a task needs a connection. A new task that
you add meanwhile stays on the phone, and the phone page sends it when
the server answers again. See
[Add a task and mark tasks done on the phone page](#add-a-task-and-mark-tasks-done-on-the-phone-page).

Today on the phone page is the phone's own date. The headings, the
`Today`, `Tomorrow`, and `Next Monday` buttons, and the completion date
of a task that you mark done all use that date. For example, if you mark
a task done on the phone at 00:30 on November 16, its completion date is
November 16. The `-TimeZone` deployment option sets the day for agents
only.

The phone page is in Danish when the phone's browser is set to Danish,
and in English otherwise. To choose the language yourself, tap `☰`, then
`English` or `Dansk`. The phone remembers the choice. The choice does not
change the language of the terminal. The colors follow the light or dark
appearance of the phone.

## Add a task and mark tasks done on the phone page

To add a task on the phone page, type its title into the `New task` field
at the top of All tasks and tap `+`. The Enter key of the phone's
keyboard adds the task too. In Danish, the field reads `Ny opgave`.

The list next to the field decides where the task goes. It starts on the
list that you chose in the list filter. With `All tasks` in the list
filter, it starts on the list of your newest task. A list that you choose
there yourself stays chosen until you change the list filter or choose
`All tasks`, `History`, or `Trash` in the menu. For example, choose
`Private` once, then add `Buy milk` and `Buy stamps`. Both tasks go to
`Private`.

The field empties as soon as you send a task, so you can type the next
task at once. The new task shows at once in a dimmed row above the
headings. When the server has answered, the task takes its place under
Sometime, because a new task has no planned date. To plan it, tap its row
and choose a date. See
[Change a task on the phone page](#change-a-task-on-the-phone-page).

If the server cannot save the new task now, the row of the new task
stays and reads `Not sent. Tap to send again.`, or
`Ikke sendt. Tryk for at sende igen.` in Danish. The phone keeps the
task. You can close the phone page or reload it, and the row is there
when you open the phone page again. The row stays in these cases:

- The server does not answer. The phone page tries a second time by
  itself before the row reads `Not sent. Tap to send again.` On a
  connection that hangs, the row can therefore stay dimmed for up to a
  minute.
- The server answers with an error about the sync file, such as
  `No connection to the sync storage.` The message also shows at the
  bottom of the phone page for 6 seconds.
- Something in front of the server answers with an error, such as 502
  or 503. The message at the bottom is then
  `The server answered with an error (503). Try again in a moment.`

To send the task, tap its row. The phone page also sends the task by
itself as soon as the server answers again, for example when you tap `↻`
or when the server saves another change. A task is never added twice. If
an earlier try did reach the server, the row above the headings leaves
the screen, and the task shows in the place that it has on the server.

If a row still reads `Not sent. Tap to send again.`, wait with
`Sign out` until the task is sent. `Sign out` removes the task from the
phone. So does a pairing that has ended, for example because the access
token expired or you revoked it. In both cases the phone page shows
`Pair this phone`, and you add the task again after pairing.

The server can also refuse the task itself. The row then leaves the
screen, the reason shows at the bottom of the phone page, and the title
returns to the `New task` field. Remove the cause and send the task
again. For example, when the date of the phone is wrong, the reason is
`The date on this device is not today's date. Check its date and time.`
If you already typed the next title, the field keeps that title, and the
message starts with the title of the refused task.

To mark a task done, tap the checkbox at the left of its row. The
checkbox gets a check mark, and the title is crossed out. The task stays
in its row, as it does in the terminal after Space. It moves to Done
today when the phone page loads the tasks again, for example when you
tap `↻`.

To reopen a done or cancelled task, tap its checkbox again. The checkbox
of a cancelled task shows a cross. In All tasks, the reopened task stays
in its row until the phone page loads the tasks again. In History, the
reopened task leaves History at once. See
[Open History and Trash on the phone page](#open-history-and-trash-on-the-phone-page).

A row is dimmed while the phone page sends its change to the server. A
dimmed row does not react to a tap until the server has answered. If the
change fails, the row returns to what it showed before, and a message
appears at the bottom of the phone page for 6 seconds. If the phone has
no connection, or the server gives no answer within 30 seconds, the
message is `No answer from the server. The change may not be saved.` The
phone page then loads the tasks again. When that works, the row shows
what the server has.

## Change a task on the phone page

Tap the row of a task on the phone page to open a panel with the task.
The panel shows the `Title` and `Description` fields, the `Planned date`
buttons with a date field below them, the task's list, and a row of
buttons. The phone's keyboard stays closed until you tap a field.

The buttons at the lower edge of the panel, from `Save` to `Close`, stay
in place while you scroll the fields above them. `Save` therefore stays
in reach on a small screen. The phone page is also built to lift the
panel above the phone's keyboard. That part has not been tried on a real
phone yet. If the keyboard covers the buttons on your phone, close the
keyboard to reach `Save`.

The panel has two kinds of controls:

- Three fields: `Title`, `Description`, and the date field. Change a
  field and tap `Save`. The date field shows no label. Its name for a
  screen reader is `Planned date`, or `Planlagt dato` in Danish.
- The buttons and the list. Each of them acts at once, without `Save`.

| To do this | Do this in the panel | In Danish |
| --- | --- | --- |
| Change the title or the description | Edit the `Title` or `Description` field and tap `Save`. The Enter key in the `Title` field saves too. In `Description`, the Enter key starts a new line. | `Titel`, `Beskrivelse`, `Gem` |
| Plan the task for today, tomorrow, or next Monday | Under `Planned date`, tap `Today`, `Tomorrow`, or `Next Monday`. | `Planlagt dato`, `I dag`, `I morgen`, `Næste mandag` |
| Plan the task for another day | Choose the day in the date field and tap `Save`. | `Gem` |
| Remove the planned date | Tap `Clear date`. | `Fjern dato` |
| Move the task to another list | Choose the list under `Move to another list`. | `Flyt til en anden liste` |
| Mark the task done | Tap `Mark done`. | `Markér som udført` |
| Reopen a done or cancelled task | Tap `Reopen task`. | `Genåbn opgave` |
| Cancel the task | Tap `Cancel task`. The panel of a cancelled task does not show this button. | `Annullér opgave` |
| Move the task to Trash | Tap `Move to Trash`. | `Flyt til papirkurv` |
| Close the panel | Tap `Close`. After you change a field, the button reads `Discard changes`, and a tap drops what you changed in the fields. | `Luk`, `Kassér ændringer` |

A button or the list sends the fields that you changed together with its
own action. For example, type a new title and tap `Today`. The task gets
the new title and is planned for today.

With no changed field, the panel closes at once and the action follows.
With a changed field, the panel stays open until the server has accepted
the change, so a refusal appears next to your text.

While the panel waits for the server, its fields take no text, and its
buttons are dimmed and do not react. The row of the task is dimmed too.
The button that closes the panel is the exception. It reads `Close`
while the panel waits, because your changes are on their way, and it
still works. When the server has accepted the change, the panel closes.
When the request fails, the panel shows the reason, and the fields and
buttons react again. The close button reads `Discard changes` again
while a field differs from the task. If the server has not answered
after 30 seconds, the panel shows
`No answer from the server. The change may not be saved.`

Closing the panel while it waits does not take the request back. The
phone page has sent your change already, and the server saves the change
if the request arrives. The row of the task stays dimmed until the
server has answered. If you open another task meanwhile, its panel waits
too. If the request fails after you closed the panel, the failure
appears at the bottom of the phone page and starts with the title of the
task, such as
`Call Ann: No answer from the server. The change may not be saved.`

A date that you tap wins over a date in the date field. For example,
choose December 24 in the date field and then tap `Tomorrow`. The task is
planned for tomorrow.

`Next Monday` is the first Monday after today. On Sunday, November 15,
2026, it is November 16. On a Monday, it is the Monday one week later.

In All tasks, a task that you mark done, cancel, or reopen in the panel
stays in its row until the phone page loads the tasks again, as it does
after a tap on its checkbox. In History, a task that you reopen leaves
History at once. That holds for `Reopen task` in the panel and for a tap
on the checkbox of its row.

After `Move to Trash`, the phone page shows `Moved to Trash` with an
`Undo` button at the bottom for 6 seconds. Tap `Undo` to put the task
back. After the 6 seconds, restore the task from Trash. See
[Open History and Trash on the phone page](#open-history-and-trash-on-the-phone-page).
In Danish, the message is `Flyttet til papirkurven` and the button is
`Fortryd`.

In Trash, and on a phone that can only read, the panel shows the title
and the description, and its only button is `Close`.

If a change in the panel fails, see
[Repeat a failed change on the phone page](#repeat-a-failed-change-on-the-phone-page).

## Repeat a failed change on the phone page

A change on the phone page can fail, for example because the phone has
no connection, or because a computer changed the task meanwhile. Where
the phone page shows the reason depends on how you made the change:

- You tapped a checkbox, or a button in the panel of a task without
  changing a field. The panel is closed. The row of the task returns to
  what it showed before, and the reason appears at the bottom of the
  phone page for 6 seconds. Repeat the tap if the row does not show
  your change.
- You changed a field in the panel of a task. The panel is still open
  and keeps your text. The reason shows above the buttons of the panel.

The panel is the view of one task that opens when you tap its row. See
[Change a task on the phone page](#change-a-task-on-the-phone-page).

In an open panel, the reason is one of these:

- `No answer from the server. The change may not be saved.` The phone
  has no connection, or the server did not answer within 30 seconds.
- `This task changed elsewhere. Save again to use your changes, or discard them.`
  A computer, an agent, or another phone changed the task after the
  phone page loaded it.
- A message from the server about another problem, such as
  `No connection to the sync storage.` See
  [Check a failed connection](#check-a-failed-connection).

After each of these messages, you have two choices:

- Tap `Save`. The phone page sends the fields that you changed again.
  If the failed request came from a button or from the list, `Save`
  sends that action again too. For example, you typed a new title and
  tapped `Today`, and the panel showed
  `No answer from the server. The change may not be saved.` Tap `Save`
  when the phone has a connection again. The task gets your title and
  is planned for today. You don't need to tap `Today` again.
- Tap `Discard changes`. The panel closes, and the phone page sends
  nothing more. The action of the button that you had tapped is dropped
  together with your text.

You can also tap another button, or choose another list, in place of
`Save`. That action replaces the action that failed. For example, after
a failed `Today`, tap `Tomorrow`. The task gets your text and is planned
for tomorrow.

You can take back a failed move or a failed date button without losing
your text:

- After a failed move to another list, the list under
  `Move to another list` keeps showing the list that you chose, and
  `Save` moves the task there. To leave the task in its list, choose
  the task's own list again. `Save` then sends your text and moves
  nothing. For example, the task is in `Work`, and the failed move went
  to `Private`. Choose `Work` under `Move to another list` and tap
  `Save`. The task stays in `Work` and gets your text.
- After a failed `Today`, `Tomorrow`, `Next Monday`, or `Clear date`,
  `Save` repeats that button. To plan the task for another day, choose
  the day in the date field. `Save` then plans the task for the day in
  the date field. For example, after a failed `Today`, choose December
  24 in the date field and tap `Save`. The task is planned for
  December 24.

The server refuses a new title or description when a computer, an agent,
or another phone changed the task after the phone page loaded it. The
server applies nothing of the refused request, and the panel shows this
message:

```text
This task changed elsewhere. Save again to use your changes, or discard them.
```

In Danish, the message is
`Opgaven er ændret et andet sted. Gem igen for at bruge dine ændringer, eller kassér dem.`

The phone page then reads the task again. The fields that you changed
keep your text. The other fields follow the task as it is now. So do
`Cancel task` and the status button, which reads `Mark done` or
`Reopen task`. For example, if a computer marked the task done
meanwhile, the status button reads `Reopen task`. `Save` sends only the
fields that you changed, so the change from elsewhere stays in the other
fields.

The server checks only a change that holds a new title or description.
A planned date, a list, a status, and a move to Trash without new text
always apply to the newest state of the task. For example, `Mark done`
works after a computer changed the task's title, and the task keeps the
title from the computer.

After `No answer from the server. The change may not be saved.`, the
server can have saved your change all the same. The phone page therefore
reads the task again. If the server did save your change, the panel
shows the saved task. The close button then reads `Close`, because no
field differs from the task any more. Tap `Save`, and the panel closes
without sending your text again.

If the close button still reads `Discard changes`, the phone page did
not find your change on the server, or it could not read the task again.
Tap `Save` to send the change again. If the first try did reach the
server and held a new title or description, `Save` then shows the
message about a task that changed elsewhere, because the task changed
through your own first try. Tap `Save` once more, and the panel closes.

A failed change can also end the panel. If the task is no longer among
the tasks on screen when the phone page has read the tasks again, the
panel shows
`This task is no longer in this view. Your changes may not be saved.`
That happens, for example, when a computer moved the task to Trash. If
the task no longer exists, the panel shows
`This task was deleted elsewhere.` After either message, the `Title` and
`Description` fields keep your text in sight and take no more text, and
the only button left is `Discard changes`. See
[Check a failed connection](#check-a-failed-connection) for what to do
next.

## Open History and Trash on the phone page

Tap `☰` on the phone page, then `History` or `Trash`. To return, tap `☰`,
then `All tasks`. In Danish, the three entries are `Historik`,
`Papirkurv`, and `Alle opgaver`.

History shows your done and cancelled tasks under the day you closed
them, newest first, as History in the terminal does. Each heading counts
all tasks of its day, also the tasks that are not loaded yet. Tap the row
of a task to change it.

To reopen a task in History, tap the checkbox at the left of its row.
The row is dimmed while the phone page sends the change. When the server
has answered, the task leaves History, because History holds only done
and cancelled tasks. You don't need to tap `↻`. The task is open again in
All tasks. `Reopen task` in the panel of the task does the same.

Trash shows the tasks in Trash. A row in Trash also shows the planned
date of its task, if the task has one. Tap `Restore`, or `Gendan` in
Danish, next to a task to put the task back in its list. Tap the row to
read the task.

History and Trash load 100 tasks at a time. Tap `Show more`, or
`Vis flere` in Danish, at the bottom to load the next 100. The rows that
`Show more` loaded stay when the phone page loads the tasks again, up to
200 rows. The list filter at the top works here too. Its first entry
reads `History` or `Trash`, and a list shows its tasks among the tasks
loaded so far. The number next to a list is still its number of open
tasks. It does not count the rows in History or Trash.

The `New task` field appears only in All tasks. The phone page cannot
delete a task forever or empty Trash. Do that in the terminal. See
[Delete a task forever](usage.md#delete-a-task-forever) and
[Empty Trash](usage.md#empty-trash).

## Add the phone page to the home screen

Add the phone page to the home screen of the phone to open it like an
app. The phone page brings its own icon and the name Husker.

- On an iPhone, open the phone page in Safari, tap the Share button, and
  choose `Add to Home Screen`.
- On an Android phone, open the phone page in Chrome, open the menu, and
  choose `Add to Home screen` or `Install app`, whichever the menu shows.

If the phone page that opens from the home screen shows
`Pair this phone`, pair it too. Run `husker pair` again on your computer
and type the new pairing code into the phone page.

Write down the token ID of that run as well. The phone then holds two
access tokens, one for the browser and one for the home screen. Each of
the two shows its own token ID in the first line of its menu. If you lose
the phone, revoke both token IDs.

## Limit a phone to one list or to reading

Add `--list` or `--read-only` to `husker pair` to limit what a phone can
do. The limit is part of the phone's access token, so nothing on the
phone can widen it.

- `husker pair --list Work` pairs a phone that reaches only the list
  `Work`. All tasks, History, and Trash on that phone hold only tasks of
  `Work`. The list filter, the list for a new task, and
  `Move to another list` offer only `Work`, so the phone cannot move a
  task to another list. `LIST` is the name of a list in your profile.
  The name ignores case.
- `husker pair --read-only` pairs a phone that shows tasks and cannot
  change them. The phone page shows `Read only`, or `Kun læsning` in
  Danish. The `New task` field is missing, the checkboxes do not react,
  and the panel of a task shows its title and description with `Close`
  as the only button. While the phone does not reach the server, the
  phone page shows the message about the connection in place of
  `Read only`.

You can combine the options. For example, this command pairs a tablet
that can only read the list `Work`, for 30 days:

```text
husker pair --name tablet --list Work --read-only --days 30
```

A phone that is limited to one list stays with the list when you rename
the list. If you rename `Work` to `Office`, the phone reaches `Office`.

A server that you deployed without `-AllowWrites` makes every phone
read-only, whatever its access token allows. Deploy with `-AllowWrites`
to let a phone with write access change tasks. See
[Check the read-only server, then allow changes](#check-the-read-only-server-then-allow-changes).

To change the limit of a paired phone, run `husker pair` again with the
options you want and scan the new QR code with the same phone. The phone
page asks `This phone is paired already. Pair it again with this code?`
Confirm, and the new access token replaces the old one in that browser.
To type the pairing code instead, tap `☰` and `Sign out` first, because
only the pairing screen has the field for a pairing code.

## What the phone page cannot do

The phone page cannot do the things in this table. The second column
says what to do instead:

| The phone page cannot | What to do instead |
| --- | --- |
| Change a task without a connection. It shows `No answer from the server. The change may not be saved.` and does not keep the change to send it later. A new task is the exception: the phone keeps it and sends it when the server answers again. | Repeat the change when the phone has a connection. In a panel that is still open, tap `Save`. See [Repeat a failed change on the phone page](#repeat-a-failed-change-on-the-phone-page). |
| Delete a task forever, or empty Trash. | Use Trash in the terminal. See [Delete a task forever](usage.md#delete-a-task-forever) and [Empty Trash](usage.md#empty-trash). |
| Create, rename, reorder, or remove a list. | Use the list picker in the terminal. See [Organize tasks into lists](usage.md#organize-tasks-into-lists). |
| Change the order of tasks under a heading. | Use Alt+Up and Alt+Down in the terminal. See [Plan tasks by date](usage.md#plan-tasks-by-date). |
| Search, or filter by text, date, or status. | Filter by list on the phone page, or press `/` in the terminal. See [Find tasks](usage.md#find-tasks). |
| Undo an action other than a move to Trash. | Reverse the action yourself. For example, tap the checkbox again to reopen a task. |
| Set a planned date while you add a task. | Add the task, then tap its row and choose a date. |

## Sign out or revoke a lost phone

To sign a phone out, tap `☰` on the phone page, then `Sign out`, or
`Log ud` in Danish. The tasks leave the screen and the phone at once, and
the phone page shows `Pair this phone`. The browser forgets the access
token as soon as the server has answered.

After signing out, the phone keeps none of your tasks, and the phone
page shows none. The saved tasks, the rows, and the text in the fields
are gone. Signing out also removes a new task that was not sent yet, and
a title that you typed into the `New task` field. If a row in All tasks
reads `Not sent. Tap to send again.`, send that task before you sign
out. The phone keeps your language choice.

The next pairing opens on All tasks, also when you signed out from
History or Trash. If you type a pairing code right after signing out,
the phone page sends the pairing code only when the server has confirmed
the sign-out. Until then, the phone page keeps showing `Pair this phone`,
and a second tap on `Pair` does nothing.

Without a connection, the phone page shows this message under
`Pair this phone`:

```text
No connection to the server. The sign-out finishes when this phone reaches the server again.
```

The tasks are gone from the screen and the phone all the same. The
browser still holds the access token until the sign-out has finished.
The phone page finishes the sign-out the next time it starts, before it
shows anything. It also tries when you tap `Pair` with a pairing code,
and it sends the pairing code only after the sign-out has finished. If
the phone still does not reach the server, the message stays, and the
pairing code stays in the field.

Before you give the phone to someone else, open the phone page once with
a connection. The sign-out has finished when `Pair this phone` shows and
the message has not come back after 30 seconds.

`Sign out` does not revoke the access token. It makes one browser forget
the access token. If you lose a phone, you cannot tap `Sign out` on it.
Revoke its access token from your computer:

1. Find the token ID that `husker pair` printed when you paired the
   phone, such as `0123456789ab`. A phone that you also paired from the
   home screen has a second token ID.
2. Run the deployment script again with the token IDs in `-Revoked`.
   Keep the other parameters as they were, including `-AllowWrites`:

   ```powershell
   ./deploy/azure/deploy.ps1 -Subscription <subscription> -ResourceGroup husker -SyncAccount <account> -App <app> -AllowWrites -Revoked 0123456789ab
   ```

3. Pass the same token IDs in `-Revoked` on every later run of the
   script, until the day the access token expires. A run without them
   lets the lost phone in again.

[Revoke one access token](#revoke-one-access-token) explains the
details. If you did not write down the token ID of a lost phone,
[revoke every access token](#revoke-every-access-token) and pair your
other phones again.

After the revocation, the server refuses the lost phone. The next time
the phone page on that phone reaches the server, it removes the saved
tasks and shows `Pair this phone`. Until then, a person who can unlock
the phone can still read the saved tasks of All tasks, for example in
airplane mode. That person gets no newer tasks and cannot change any
task.

To revoke the access token of a phone that you still hold, read the token
ID from the phone page. Tap `☰`. The first line ends with the token ID,
such as `(token 0123456789ab)`. Then follow steps 2 and 3 above.

## When your computers see a change

A change that an agent makes through the online server is in the sync file
as soon as the tool returns. A change on the phone page is in the sync
file as soon as the server has answered the phone. Until then, the row
of the task is dimmed, and so are the buttons of its open panel. Each
computer receives the change at its next sync:

| On the computer | When the change arrives |
| --- | --- |
| The terminal interface is open. | Within about a minute. Press F5 to sync at once. The terminal shows `Synced changes from another device.` |
| A writable local `husker mcp` server runs. | Within about a minute. |
| You only run commands such as `husker tasks`. | When you run `husker sync`. |

A change you make on a computer reaches the online server when that
computer has synced. The terminal interface sends a change within about a
minute. After a command such as `husker add`, run `husker sync`. The
phone page shows the change the next time it loads the tasks, for
example when you tap `↻`.

Sometimes an agent and a computer change the same task before the computer
has synced. The agent's change is already in the sync file, so the rules
for two computers apply with the agent as the one that synced first:

- Changes to different fields combine. If the agent marks `Buy milk` done
  and you change its planned date, the task is done and has your date.
- For the same field, the agent's value stays. If the agent plans a task
  for Monday and you plan it for Friday, the task is planned for Monday.
- For a title or a description, Husker keeps both texts. The task keeps
  the agent's text. Your text becomes a new task whose title ends with
  `#conflict`, and `husker sync` ends its line with
  `Kept both versions of 1.`

A change on the phone page counts like an agent's change in all three
cases. For example, you plan a task for Monday on the phone page and for
Friday on a laptop that has not synced yet. After the laptop syncs, the
task is planned for Monday.

See [What happens when two computers change the same task](sync.md#what-happens-when-two-computers-change-the-same-task)
for how to find and clear the `#conflict` copies.

## Revoke one access token

Revoke an access token when the agent or the phone that used it should
lose access, or when the access token may have leaked. You need its token
ID, the 12 characters that `husker token` or `husker pair` printed when
you created it. A paired phone shows its token ID too. Tap `☰` on the
phone page and read the end of the first line, such as
`(token 0123456789ab)`.

Run the deployment script again with the token ID in `-Revoked`. Keep the
other parameters as they were, including `-AllowWrites`:

```powershell
./deploy/azure/deploy.ps1 -Subscription <subscription> -ResourceGroup husker -SyncAccount <account> -App <app> -AllowWrites -Revoked 0123456789ab
```

Separate several token IDs with commas, for example
`-Revoked 0123456789ab,ba9876543210`. The server then answers 401 to
those access tokens. A phone that used one of them shows
`Pair this phone` the next time its phone page reaches the server. See
[Sign out or revoke a lost phone](#sign-out-or-revoke-a-lost-phone).

A token ID has 12 characters: digits and the lowercase letters a to f.
The script stops when a token ID has another form, and so does the
server. A misspelled token ID would otherwise revoke nothing.

Pass the full list of revoked token IDs on every later run of the script.
A run without `-Revoked` clears the list, and a revoked access token that
has not expired works again.

If you don't have the token ID, revoke every access token instead.

## Revoke every access token

Replace the key to make every access token useless at once. Do this when
`online.toml` may have leaked, or when you lost it. Every agent loses its
access, and every phone has to be paired again.

1. Move `online.toml` out of your profile directory, or delete it.
2. Run the deployment script again with the same parameters. The script
   makes Husker create a new key and gives the new public key to the
   server.
3. Create new access tokens for your agents. The new `online.toml` does
   not know the address of your server yet, so give `--url` once:

   ```text
   husker token --name claude-code --url https://<app>.azurewebsites.net
   ```

4. Put the new access tokens where the old ones were, for example in
   `HUSKER_ONLINE_TOKEN`.
5. Pair each phone again with `husker pair`. See
   [Pair a phone](#pair-a-phone).

The server now refuses every access token that the old key signed. A
phone that was paired with the old key shows `Pair this phone`.

## Cut the server off from the sync file

Delete the stored access policy `husker-online` to stop the server from
reading and writing the sync file at once. Do this when you think the
server itself is compromised:

```text
az storage container policy delete --container-name husker --name husker-online --account-name <account> --auth-mode key
```

Every request for tasks then fails with `The sync storage refused access.`
Your computers are not affected. They use the policy `husker-sync` from
the sync guide.

To give the server access again, run the deployment script with a new
policy name and `-CreatePolicy`, for example
`-SyncPolicy husker-online-2 -CreatePolicy`. Pass the new name on every
later run.

Don't create `husker-online` again. The key in the server's settings
names the policy. When a policy with a deleted name exists again, Azure
accepts every old key of that name again, including one that leaked.
The script therefore stops when the policy is missing, unless you pass
`-CreatePolicy`.

To remove the server for good, delete the function app, its plan, and its
storage account in the Azure portal, and delete the policy as above.

## Undo a bad change

The online server changes the sync file directly, and your computers take
the change at their next sync. To undo a change that an agent should not
have made, or a change you made by mistake on the phone page, restore the
version of the sync file from before the change. Azure keeps every
earlier version. Follow
[Recover an earlier version of the sync file](sync.md#recover-an-earlier-version-of-the-sync-file).

For a single task, it is quicker to correct the task in the terminal or
on the phone page. A task that an agent or the phone page moved to Trash
is still there. Restore it from Trash. No tool of the online server
deletes a task forever, and the phone page cannot do it either.

## Update the server

Run the deployment script again from the new Husker source directory, with
the same parameters as before. The script builds the new version, uploads
it, and waits until the server reports the new version.

Usually you can update your computers and the server in any order. One
case needs an order: a release that changes the format of the sync file.
Deploy the server first, then update Husker on your computers. If a
computer writes the new format first, the older server refuses every
request with `The sync file needs a newer Husker.` It changes nothing
until you deploy the new version. The release notes say when a release
changes the format.

To see which version runs, open
`https://<app>.azurewebsites.net/api/health` in a browser. For Husker
0.18.0, the answer holds `"version":"0.18.0"` and `"format":1`. `format`
is the format of the sync file that this server reads and writes. Husker
0.18.0 keeps format 1, so you can update the server from 0.17.0 before or
after your computers.

A phone keeps a copy of the files of the phone page. After you update the
server, the phone page opens once with the old files and fetches the new
files in the background. It then starts again by itself with the new
files, usually within a few seconds. It waits while the `New task`
field or the field for a pairing code holds text, while a panel or the
menu is open, and while `Undo` shows at the bottom. It also waits while
a change, a new task, or a pairing code is on its way to the server.

## What the server costs

The server runs on the Flex Consumption plan of Azure Functions. You pay
for the requests it answers and for the time it works on them, and nothing
while it is idle. Azure gives a free monthly grant of 250,000 executions
and 100,000 GB-seconds on this plan. Each request from an agent or from
the phone page is one execution. The phone page sends a few requests when
it opens and one request for each change. An agent that makes a few
hundred requests a day stays far below the grant. So does a phone that
opens the phone page a hundred times a day.

The storage account for the uploaded executable costs a few cents a month.
Each deployment adds one file of about 15 MB to its `packages` container.
Delete old files there when you like.

Azure stops the server when no request arrives for a while. The first
request after that takes a few seconds, because Azure starts the server
again. The phone page shows your saved tasks while it waits. See
[Use the phone page](#use-the-phone-page).

<!-- PLACEHOLDER: add the measured cold start time after the first deployment. -->

See [Azure Functions pricing](https://azure.microsoft.com/pricing/details/functions/)
for the current prices.

## Try the online server on your own computer

You can run the online server on your computer against a sync file that
also lives on your computer. Nothing leaves the computer, and nothing
touches Azure or your real tasks. Use a separate profile directory, such
as `./target/online-test`, so your real `online.toml` keeps the address of
your real server.

1. In the Husker source directory, start the stand-in for the storage:

   ```text
   cargo run --example fake_blob
   ```

   It prints the URL of a local sync file and keeps running:

   ```text
   Sync file URL: http://127.0.0.1:<port>/husker/tasks.json?sig=S3CRET
   The file exists after the first `husker sync`. Press Ctrl+C to stop.
   ```

2. In a second terminal, also in the Husker source directory, create the
   directory `./target/online-test` and a file `sync.toml` in it with the
   printed URL:

   ```toml
   url = "http://127.0.0.1:<port>/husker/tasks.json?sig=S3CRET"
   ```

3. Add a task and sync, so the sync file exists:

   ```text
   husker --profile ./target/online-test add "Buy milk"
   husker --profile ./target/online-test sync
   ```

   The second command prints `Synced. Received 0, sent 2.`

4. Print the public key of the test profile:

   ```text
   husker --profile ./target/online-test token --public-key
   ```

5. In a third terminal, give the server its settings and start it. In
   PowerShell:

   ```powershell
   $env:HUSKER_SYNC_URL = "http://127.0.0.1:<port>/husker/tasks.json?sig=S3CRET"
   $env:HUSKER_ONLINE_KEYS = "<public key>"
   $env:HUSKER_ONLINE_ORIGIN = "http://127.0.0.1:8787"
   husker serve
   ```

   In a POSIX shell:

   ```sh
   export HUSKER_SYNC_URL="http://127.0.0.1:<port>/husker/tasks.json?sig=S3CRET"
   export HUSKER_ONLINE_KEYS="<public key>"
   export HUSKER_ONLINE_ORIGIN="http://127.0.0.1:8787"
   husker serve
   ```

   The server prints `Husker online server listening on 127.0.0.1:8787.`
   and keeps running.

6. In the terminal where you will start the agent, create an access token
   for the local address and set the two variables for that terminal
   only. In PowerShell:

   ```powershell
   $env:HUSKER_ONLINE_URL = "http://127.0.0.1:8787"
   $env:HUSKER_ONLINE_TOKEN = husker --profile ./target/online-test token --name local --url http://127.0.0.1:8787
   ```

   In a POSIX shell:

   ```sh
   export HUSKER_ONLINE_URL="http://127.0.0.1:8787"
   export HUSKER_ONLINE_TOKEN="$(husker --profile ./target/online-test token --name local --url http://127.0.0.1:8787)"
   ```

7. Start the agent from that terminal, with the `.mcp.json` entry from
   [Connect Claude Code](#connect-claude-code). End the address in the
   entry at `/mcp`, or name a list of the test profile, such as
   `/mcp/list/Inbox`.

8. After the agent adds a task, sync the test profile:

   ```text
   husker --profile ./target/online-test sync
   ```

   It prints `Synced. Received 1, sent 0.` The test profile now holds the
   agent's task. List the tasks to see it:

   ```text
   husker --profile ./target/online-test tasks
   ```

The local server also serves the phone page, at `http://127.0.0.1:8787/`.
To try the phone page in a browser on the same computer, do steps 1 to 5
and then these steps:

1. Create a pairing code with the test profile:

   ```text
   husker --profile ./target/online-test pair --url http://127.0.0.1:8787
   ```

   The terminal shows a QR code and a pairing code such as
   `K7QF-2M9X-TB4H`.

2. Open `http://127.0.0.1:8787/` in a browser on the same computer, type
   the pairing code, and select `Pair`. The phone page shows All tasks
   with `Buy milk` under Sometime.

3. Add a task on the phone page. Then sync the test profile and list its
   tasks:

   ```text
   husker --profile ./target/online-test sync
   husker --profile ./target/online-test tasks
   ```

   The second command lists the task from the phone page.

Write the address exactly as it is in `HUSKER_ONLINE_ORIGIN`, both after
`--url` and in the browser. `http://localhost:8787` does not work,
because the server accepts only the address it was started with. A phone
cannot open this phone page, because `husker serve` listens only on
`127.0.0.1`.

`husker serve` reads these environment variables when it starts. The
deployment script sets the same variables in Azure.

| Variable | Required | Value |
| --- | --- | --- |
| `HUSKER_SYNC_URL` | Yes | The URL of the sync file, with its SAS token, as in `sync.toml`. |
| `HUSKER_ONLINE_KEYS` | Yes | The public key from `husker token --public-key`. Separate several keys with commas. |
| `HUSKER_ONLINE_ORIGIN` | Yes | The address that clients use for the server, without a path. It must start with `https://`. `http://` is accepted only for `localhost`, `127.0.0.1`, and `[::1]`. |
| `HUSKER_ONLINE_TZ` | No | The IANA time zone for dates. The default is `UTC`. |
| `HUSKER_ONLINE_REVOKED` | No | Token IDs that the server refuses, separated by commas. |
| `HUSKER_ONLINE_READ_ONLY` | No | `true` makes every access token read-only. The default is `false`. |

`husker serve` listens only on `127.0.0.1`, so other computers cannot
reach it. It uses port 8787. `husker serve --port 9000` uses port 9000.
The server reads no profile and no database. If a required variable is
missing, it stops with an error that names the variable, such as
`Error: HUSKER_ONLINE_KEYS is not set.`

## Check a failed connection

First check that the server runs. Open
`https://<app>.azurewebsites.net/api/health` in a browser. A running
server shows its name, version, and sync file format. This address needs
no access token.

For a phone, continue with the last two tables of this section. One
covers `husker pair`, and the other covers the messages of the phone
page.

For an agent, check the access token and the address next. This request
asks the server for nothing, so the status code alone tells you what is
wrong. In PowerShell:

```powershell
curl.exe -i -H "Authorization: Bearer $env:HUSKER_ONLINE_TOKEN" "$env:HUSKER_ONLINE_URL/mcp/list/Work"
```

In a POSIX shell:

```sh
curl -i -H "Authorization: Bearer $HUSKER_ONLINE_TOKEN" "$HUSKER_ONLINE_URL/mcp/list/Work"
```

The first line of the answer holds the status code:

| Status | Cause | What to do |
| --- | --- | --- |
| 405 | Nothing is wrong. The server accepts the access token and the list. It answers 405 because this test did not send a tool request. | Look at the agent's settings: the name of the variable, the address, and a restart after you changed a variable. |
| 401 | The server refuses the access token. The variable is empty, or the access token expired, was revoked, was created for another address, or was signed by another profile's key. | Create a new access token with `husker token --name NAME --url https://<app>.azurewebsites.net` on the computer that ran the deployment script. Store it and restart the agent. |
| 403 | The access token is for one list, and the address names another list or a name that no list has. | Use the list of the access token in the address, or end the address at `/mcp`. |
| 404 | The access token is for every list, and no list has the name in the address. Or the address has a path other than `/mcp` and `/mcp/list/NAME`. | Check the spelling of the list name and the path. After a list rename, use the new name. |

A 401 after a local test often has one cause. `husker token` remembers
the last `--url`, and so does `husker pair` once the server has answered
with a pairing code. If you used `--url http://127.0.0.1:8787` with your
real profile, later access tokens are for that address too, and
`husker pair` contacts that address. Pass
`--url https://<app>.azurewebsites.net` once to store the real address
again.

The server also answers 403 to a request that a web page of another
website sends, and to a request that reaches it under another host name
than `<app>.azurewebsites.net`. The second answer names the host name
that arrived, for example
`This address does not accept requests for the host 'tasks.example.net'.`
The phone page comes from the server itself, so the server accepts its
requests.

When the connection works and a tool fails, the agent gets one of these
sentences:

| Sentence | Cause | What to do |
| --- | --- | --- |
| `The sync file does not exist. Sync a computer first.` | No computer has synced yet, or the sync file was deleted. The server never creates the file. | Run `husker sync` on a computer. |
| `The sync storage refused access.` | The stored access policy `husker-online` is missing or expired. | Run the deployment script with a new `-SyncPolicy` name and `-CreatePolicy`. See [Cut the server off from the sync file](#cut-the-server-off-from-the-sync-file). |
| `The change may not have been saved. Read the task again before you repeat it.` | The server sent a change, got no answer, and could not read the sync file afterwards. | Let the agent read the task again. Repeat the change only if it is not there. |
| `The sync file is too large for the online server.` | The change would make the sync file larger than 16 MiB. | Empty Trash and delete old tasks forever in the terminal. |
| `No connection to the sync storage.` | The server could not reach the storage within 10 seconds. | Try again. |
| `The sync storage gave an unexpected answer.` | The storage answered with another error, for example because the container does not exist. | Check `-SyncContainer` and `-SyncBlob` in the deployment. |
| `The sync file needs a newer Husker.` | A computer with a newer Husker wrote the sync file in a newer format. | Deploy the server from the newer source. See [Update the server](#update-the-server). |
| `The sync file is not valid.` | The file at the URL is not a Husker sync file. | Restore an earlier version. See [Recover an earlier version of the sync file](sync.md#recover-an-earlier-version-of-the-sync-file). |
| `The sync file kept changing.` | Your computers replaced the sync file five times while the server tried to save one change. | Try again. |
| `Unknown or unavailable tool 'add_task'.`, or the agent says it has no tool to change tasks | The connection is read-only. The access token is read-only, or the server was deployed without `-AllowWrites`. | Create an access token without `--read-only`, or deploy with `-AllowWrites`. Then restart the agent's session. |
| A sentence that ends with `Reload it before saving.` | The task changed after the agent read it. | Nothing. The agent reads the task again and repeats the change. |

When the address names a list, the server reads the sync file before it
answers. A storage problem then shows as status 502 or 503, with the same
sentence in the answer.

When `husker pair` fails, it prints one of these errors and no pairing
code:

| Error | Cause | What to do |
| --- | --- | --- |
| `Error: Give the address of the online server once with --url URL. Husker stores it in online.toml.` | Husker has no stored address for this profile. | Run `husker pair --url https://<app>.azurewebsites.net`. |
| `Error: Cannot reach the online server at https://<app>.azurewebsites.net:` followed by a reason | The computer has no connection, the address is wrong, or the server did not answer within 40 seconds. A reason that names a certificate means that your computer does not trust the server's certificate. `husker pair` uses the certificate store of the computer, as `husker sync` does. | Read the address in the error. Open `https://<app>.azurewebsites.net/api/health` in a browser, then run `husker pair` again. An address that you passed with `--url` is not stored after this error, so pass the right address with `--url`. The address that Husker stored before stays, and `husker pair` without `--url` uses that address again. |
| `Error: The online server at https://<app>.azurewebsites.net refused the access token. The server may hold another public key than this profile's. Deploy the server from this computer, or copy online.toml from the computer that deployed it.` | The key in the `online.toml` of this profile is not the key that the server knows. | Run `husker pair` on the computer and with the profile that ran the deployment script, or copy `online.toml` from there. See [Keep the key file safe](#keep-the-key-file-safe). |
| `Error: The online server at https://<app>.azurewebsites.net answered with status 404.` | The server runs Husker 0.17.0, which has no phone page. | [Update the server](#update-the-server). |
| The same error with a redirect status, such as 301, 302, 307, or 308 | The address that you gave answers with a redirect to another address. `husker pair` does not follow a redirect, because its request holds the phone's access token. The other address gets no request. | Pass the address of the server itself with `--url`, such as `https://<app>.azurewebsites.net`. |

When the phone page has a problem, it shows one of these messages:

| Message | In Danish | Cause | What to do |
| --- | --- | --- | --- |
| `The code is wrong, used, or expired. Run husker pair again.` | `Koden er forkert, brugt eller udløbet. Kør husker pair igen.` | The pairing code is older than 5 minutes, was used before, or has a typing mistake. Or the server no longer knows the pairing code: the server started again after `husker pair`, or you deployed with `-MaxInstances` above `1` and another copy of the server got the request. | Run `husker pair` again and use the new pairing code at once. A phone that is paired already stays paired: it shows its tasks and this message at the bottom. If another person can have used the pairing code, [revoke](#revoke-one-access-token) the token ID that `husker pair` printed. |
| `This phone is paired already. Pair it again with this code?` | `Telefonen er allerede tilknyttet. Vil du tilknytte den igen med denne kode?` | You opened a pairing link in a browser that is paired. | Confirm to replace the access token in that browser. Decline to keep the pairing. |
| `Pair this phone` on a phone that was paired, sometimes with `This phone is no longer paired.` | `Tilknyt denne telefon`, `Telefonen er ikke længere tilknyttet.` | The server refuses the phone's access token, or the browser no longer has it. The access token expired, you revoked it or replaced the key, you signed out, the browser's data was cleared, or you opened the phone page in another browser or from the home screen. | Pair the phone again with `husker pair`. A change that you made right before the message was not saved. A new task that was not sent yet is removed from the phone too. Repeat both after pairing. |
| `No connection to the server.`, alone or followed by `Showing saved tasks.` | `Ingen forbindelse til serveren.`, `Viser gemte opgaver.` | The phone did not reach the server, or the server did not answer within 30 seconds. | Check the phone's connection and tap `↻`. If other websites work, open `https://<app>.azurewebsites.net/api/health` on the phone to check the server. Under `Pair this phone`, the message means that the phone page could not check the pairing code. The pairing code is still in the field, whether it came from a link or you typed it. Tap `Pair` to try again. |
| `No connection to the server. The sign-out finishes when this phone reaches the server again.` | `Ingen forbindelse til serveren. Du bliver logget helt ud, når telefonen får forbindelse til serveren igen.` | You tapped `Sign out` while the phone did not reach the server. The tasks are gone from the screen and the phone. The browser still holds the access token. | Nothing. The phone page finishes the sign-out the next time it starts, or when you tap `Pair` with a pairing code. See [Sign out or revoke a lost phone](#sign-out-or-revoke-a-lost-phone). |
| `No answer from the server. The change may not be saved.`, alone or after the title of a task | `Intet svar fra serveren. Ændringen er måske ikke gemt.` | The phone sent a change to a task and got no answer within 30 seconds. The server can have saved the change or not. The phone page does not keep the change to send it later. | The phone page loads the tasks again. Look at the task, and repeat the change if it is missing. In a panel that is still open, tap `Save`. If the close button reads `Close`, the server did save your change, and `Save` closes the panel without sending your text again. If the close button reads `Discard changes`, `Save` sends your text again, together with the action of the button that you had tapped. See [Repeat a failed change on the phone page](#repeat-a-failed-change-on-the-phone-page). |
| `Not sent. Tap to send again.` in the row of a new task | `Ikke sendt. Tryk for at sende igen.` | The server did not answer when the phone page sent the new task. Or the answer was an error about the sync file, or an error from something in front of the server. The phone keeps the task, also while the phone page is closed. | Tap the row, or wait. The phone page sends the task as soon as the server answers again, for example after a tap on `↻`. Don't sign out before that, because `Sign out` removes the task from the phone. |
| `This task changed elsewhere. Save again to use your changes, or discard them.` | `Opgaven er ændret et andet sted. Gem igen for at bruge dine ændringer, eller kassér dem.` | You changed the title or the description in the panel of a task after a computer, an agent, or another phone changed the task. The message can also follow `No answer from the server. The change may not be saved.` when the server did save your first try and the phone page could not read the task again. | Tap `Save` to send your changes again, or `Discard changes` to drop them. See [Repeat a failed change on the phone page](#repeat-a-failed-change-on-the-phone-page). |
| `This task is no longer in this view. Your changes may not be saved.` | `Opgaven er ikke længere i denne visning. Dine ændringer er måske ikke gemt.` | A change in the panel of a task failed, and the phone page read the tasks again. The task is no longer among the tasks on screen, for example because a computer moved it to Trash. | Tap `Discard changes`, the only button left in the panel. Until then, the panel keeps your text in sight and takes no more text. To make the change after all, find the task where it is now. For a task in Trash, tap `☰`, then `Trash`, and tap `Restore` next to the task. If you had tapped `Move to Trash` in the panel, look at the task in Trash before you repeat anything. When only the answer of the server was lost, the task has your changes already. |
| `This task was deleted elsewhere.` | `Opgaven blev slettet et andet sted.` | The task no longer exists, for example because you deleted it forever in the terminal. On a phone that is limited to one list, the task can also be in another list now. | Tap `↻`. In a panel, the message ends the panel: your text stays in sight, and the only button left is `Discard changes`. Tap it first. |
| `Read only` | `Kun læsning` | The phone was paired with `--read-only`, or the server was deployed without `-AllowWrites`. | Pair the phone again without `--read-only`, or deploy with `-AllowWrites` and tap `↻`. |
| `The date on this device is not today's date. Check its date and time.` | `Datoen på denne enhed er ikke dags dato. Kontrollér dato og klokkeslæt.` | The date of the phone is more than one day away from the date of the server. | Correct the date and time of the phone. |
| A sentence from the table for agents above, such as `The sync storage refused access.` | The Danish form of that sentence | The cause in the table for agents. | Follow the table for agents. Where it says to read the task again, tap `↻`. |
| `The server answered with an error (503). Try again in a moment.` | `Serveren svarede med en fejl (503). Prøv igen om lidt.` | The answer did not come from Husker. It came from something in front of the server and holds only a status code, here 503. | Tap `↻` after a moment. If the message stays, check the server with `/api/health`. |
