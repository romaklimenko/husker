# Use Husker online

This guide is for people who already sync Husker through Azure and want an
AI agent to reach their tasks without a Husker process on the agent's
computer. Set up sync first by following
[Sync tasks between computers](sync.md). The online server is available
from Husker 0.17.0.

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

Your computers work as before. Each keeps its own `tasks.db`, works
without a network, and syncs in the background. They need no new setting.
They never contact the online server. They receive its changes from the
sync file at their next sync.

Every request to the online server needs an access token. You create
access tokens on your own computer with `husker token`. Nothing in Azure
can create one.

The online server serves agents only. Husker has no page for a phone or a
browser.

## What you need

- Sync that already works. At least one computer must have run
  `husker sync`, so the sync file exists. The online server never creates
  the sync file.
- The Husker 0.17.0 source directory on a Windows computer. The deployment
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

The last two lines look like this:

```text
Husker 0.17.0 is online at https://<app>.azurewebsites.net. Writes allowed: False.
Next: husker token --url https://<app>.azurewebsites.net --name NAME
```

If the server does not answer in time, the script stops with this message.
Run the script again.

```text
The server did not answer with version 0.17.0 at https://<app>.azurewebsites.net/api/health.
```

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
| `-TimeZone` | `Europe/Copenhagen` | The time zone that decides which day is today. See [Choose the time zone for dates](#choose-the-time-zone-for-dates). |
| `-Revoked` | None | The token IDs of access tokens that the server refuses, separated by commas. See [Revoke one access token](#revoke-one-access-token). |
| `-CreatePolicy` | Off | Creates the stored access policy when it is missing. See [Cut the server off from the sync file](#cut-the-server-off-from-the-sync-file) before you use it after the first run. |
| `-HuskerProfile` | `~/.husker` | The profile directory whose key signs your access tokens. |
| `-SyncContainer` | `husker` | The container of the sync file. |
| `-SyncBlob` | `tasks.json` | The name of the sync file. |
| `-SyncPolicy` | `husker-online` | The name of the stored access policy for the server. |
| `-HostAccount` | Derived from `-App` | The name of the new storage account for the uploaded executable. |
| `-MaxInstances` | `1` | The most copies of the server that Azure may run at the same time. |
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

`husker token` signs every access token with a private key. The key is in
the file `online.toml` in your profile directory, next to `tasks.db`. For
the default profile, the file is `~/.husker/online.toml`. Husker creates
the file with a new key the first time you run `husker token` or the
deployment script.

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
  server refuses an access token that another profile's key signed. To
  sign on a second computer, copy `online.toml` to the profile directory
  of that computer.
- Don't edit the `key` line. If `online.toml` is damaged, `husker token`
  stops with an error that names the file. The error never shows the key.

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

## When your computers see a change

A change that an agent makes through the online server is in the sync file
as soon as the tool returns. Each computer receives it at its next sync:

| On the computer | When the change arrives |
| --- | --- |
| The terminal interface is open. | Within about a minute. Press F5 to sync at once. The terminal shows `Synced changes from another device.` |
| A writable local `husker mcp` server runs. | Within about a minute. |
| You only run commands such as `husker tasks`. | When you run `husker sync`. |

A change you make on a computer reaches the online server when that
computer has synced. The terminal interface sends a change within about a
minute. After a command such as `husker add`, run `husker sync`.

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

See [What happens when two computers change the same task](sync.md#what-happens-when-two-computers-change-the-same-task)
for how to find and clear the `#conflict` copies.

## Revoke one access token

Revoke an access token when the agent that used it should lose access, or
when the access token may have leaked. You need its token ID, the 12
characters that `husker token` printed when you created it.

Run the deployment script again with the token ID in `-Revoked`. Keep the
other parameters as they were, including `-AllowWrites`:

```powershell
./deploy/azure/deploy.ps1 -Subscription <subscription> -ResourceGroup husker -SyncAccount <account> -App <app> -AllowWrites -Revoked 0123456789ab
```

Separate several token IDs with commas, for example
`-Revoked 0123456789ab,ba9876543210`. The server then answers 401 to
those access tokens.

A token ID has 12 characters: digits and the lowercase letters a to f.
The script stops when a token ID has another form, and so does the
server. A misspelled token ID would otherwise revoke nothing.

Pass the full list of revoked token IDs on every later run of the script.
A run without `-Revoked` clears the list, and a revoked access token that
has not expired works again.

If you don't have the token ID, revoke every access token instead.

## Revoke every access token

Replace the key to make every access token useless at once. Do this when
`online.toml` may have leaked, or when you lost it.

1. Move `online.toml` out of your profile directory, or delete it.
2. Run the deployment script again with the same parameters. The script
   makes Husker create a new key and gives the new public key to the
   server.
3. Create new access tokens. The new `online.toml` does not know the
   address of your server yet, so give `--url` once:

   ```text
   husker token --name claude-code --url https://<app>.azurewebsites.net
   ```

4. Put the new access tokens where the old ones were, for example in
   `HUSKER_ONLINE_TOKEN`.

The server now refuses every access token that the old key signed.

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
have made, restore the version of the sync file from before the change.
Azure keeps every earlier version. Follow
[Recover an earlier version of the sync file](sync.md#recover-an-earlier-version-of-the-sync-file).

For a single task, it is quicker to correct the task in the terminal. A
task that an agent moved to Trash is still there. Restore it from Trash.
No tool of the online server deletes a task forever.

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
0.17.0, the answer holds `"version":"0.17.0"` and `"format":1`. `format`
is the format of the sync file that this server reads and writes.

## What the server costs

The server runs on the Flex Consumption plan of Azure Functions. You pay
for the requests it answers and for the time it works on them, and nothing
while it is idle. Azure gives a free monthly grant of 250,000 executions
and 100,000 GB-seconds on this plan. Each request from an agent is one
execution. An agent that makes a few hundred requests a day stays far
below the grant.

The storage account for the uploaded executable costs a few cents a month.
Each deployment adds one file of about 15 MB to its `packages` container.
Delete old files there when you like.

Azure stops the server when no request arrives for a while. The first
request after that takes a few seconds, because Azure starts the server
again.

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

Then check the access token and the address. This request asks the server
for nothing, so the status code alone tells you what is wrong. In
PowerShell:

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

A 401 after a local test often has one cause. `husker token` remembers the
last `--url`. If you created an access token for `http://127.0.0.1:8787`
with your real profile, later access tokens are for that address too. Pass
`--url https://<app>.azurewebsites.net` once to store the real address
again.

The server also answers 403 to a request that a web page in a browser
sends, and to a request that reaches it under another host name than
`<app>.azurewebsites.net`.

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
