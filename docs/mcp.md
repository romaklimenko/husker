# Connect an agent to Husker

This guide is for people who want an AI agent to read or manage their Husker tasks.
Husker uses MCP, the Model Context Protocol, to give the agent task tools.
The client starts Husker as a local process and communicates over standard input and output.

## Install or update Husker

This guide describes Husker 0.4.0 and its task tools.
Close running Husker instances before updating the executable.
Disconnect MCP clients that already use Husker, then run this command from the source directory:

```text
cargo install --path . --locked
```

Run `husker --version` to check the installed version.
Run `husker mcp --help` to see the server options.
When upgrading an earlier version, follow [Update an existing profile](usage.md#update-an-existing-profile)
before reconnecting clients. A read-only server cannot migrate an older database.
You do not need to start `husker mcp` yourself before connecting a client.
The client starts and stops its own server process.

## Choose the list an agent can use

Run `husker lists` to see the lists in your default profile, `~/.husker`.
Choose an existing list name, such as `Private`.
If you need a new list, create it first:

```text
husker list-add Private
```

Pass `mcp --list Private` when registering the server with your client.
Husker then restricts every tool to that list.
The agent cannot read other lists, see their task counts, or change their tasks.
Trash searches and counts include only deleted tasks from `Private`.
An unknown list name prevents the server from starting.

Quote a name containing spaces, for example `mcp --list "Home admin"`.
Names ignore case, so `--list private` selects `Private`.

Husker keeps the selected list ID for the lifetime of the server.
If you rename `Private` to `Home`, the running server still uses that list.
Change the saved launch argument to `--list Home` before restarting the server.
Follow [Change a saved connection](#change-a-saved-connection) to replace the client registration.
Each new server process resolves the name again.

The restriction applies to this MCP connection.
An agent with separate shell or filesystem access can still open the same task database through those tools.
Another unrestricted Husker connection also gives access to the other lists.

## Connect Codex

Run this command in PowerShell to register access to `Private` in your default profile:

```powershell
codex mcp add husker-private -- "$env:USERPROFILE\.cargo\bin\husker.exe" mcp --list Private
codex mcp list
```

The executable path assumes Cargo's default installation directory.
If you installed Husker elsewhere, replace that path with the absolute path to `husker.exe`.
For Bash on macOS or Linux, use this command instead:

```bash
codex mcp add husker-private -- "$HOME/.cargo/bin/husker" mcp --list Private
codex mcp list
```

Start a new Codex session and use `/mcp` to inspect the connection.
Codex records everything after `--` as the server command and its arguments.
See the [Codex MCP guide](https://learn.chatgpt.com/docs/extend/mcp?surface=cli) for client settings.

## Connect Claude Code

Run this command in PowerShell to register the same list in Claude Code:

```powershell
claude mcp add --transport stdio --scope user husker-private -- "$env:USERPROFILE\.cargo\bin\husker.exe" mcp --list Private
claude mcp list
```

Replace the executable path if you installed Husker elsewhere.
For Bash on macOS or Linux:

```bash
claude mcp add --transport stdio --scope user husker-private -- "$HOME/.cargo/bin/husker" mcp --list Private
claude mcp list
```

`--scope user` makes the connection available to you across projects.
Use `--scope local` instead to keep the client registration local to the current project.
This client setting is separate from Husker's `--list` task restriction.
Start a new Claude Code session and use `/mcp` to inspect the connection.
See the [Claude Code MCP guide](https://code.claude.com/docs/en/mcp) for client settings.

## Allow reading without changes

Add `--read-only` to the Husker arguments when you register the connection.
For example, use this Codex command instead of the writable registration above:

```powershell
codex mcp add husker-private -- "$env:USERPROFILE\.cargo\bin\husker.exe" mcp --list Private --read-only
```

The agent receives only `list_lists`, `list_tasks`, and `get_task`.
The database is opened without write access.
An attempt to call a mutation tool still fails even if the agent guesses its name.
The profile and database must already exist at the current schema version.
After upgrading an older profile, run `husker lists` once with the updated
executable before starting a read-only server.

For Claude Code or another MCP client, append `--read-only` to the same Husker server arguments.
If you previously registered a writable connection, [replace that registration](#change-a-saved-connection) and restart the client.
Leaving both connections available still gives the agent a way to write.

## Select a different profile

Put `--profile` before `mcp` in the server command to select another task collection.
Use an absolute profile path because the client's working directory can differ from your terminal's directory.
For example, this PowerShell command selects `Private` from `C:\Tasks\Husker`:

```powershell
codex mcp add husker-private -- "$env:USERPROFILE\.cargo\bin\husker.exe" --profile "C:\Tasks\Husker" mcp --list Private
```

Replace `C:\Tasks\Husker` with the directory containing your `tasks.db`.
Check that profile's list names with `husker --profile "C:\Tasks\Husker" lists`.

To give a connection access to every list in the selected profile, omit `--list`.
For example, `husker mcp --read-only` can read every list in the default profile.

## Change a saved connection

Remove the old client registration before adding changed arguments under the same name.
For Codex, run:

```text
codex mcp remove husker-private
```

For the Claude Code user registration above, run:

```text
claude mcp remove --scope user husker-private
```

Run the client's `mcp add` command again with the new Husker arguments.
For example, change `--list Private` to `--list Home` after renaming the list.
Restart the client session so its server process uses the new arguments.
Removing a client registration does not delete your Husker tasks.

## Ask the agent to manage tasks

After connecting a writable `Private` server, try these requests:

- "Show my Husker tasks due today."
- "Add Buy milk #shopping to Husker."
- "Show completed Husker tasks."
- "Restore Buy milk from Husker's Trash."

An empty search returns open tasks and tasks completed or cancelled today, outside Trash.
Ask for open tasks only when that is what you need. The agent can use `s:open`.
The agent can use the [same search syntax as the terminal](usage.md#find-tasks), such as `due:today` or `s:done`.
`#shopping` searches for that literal text in titles and descriptions.
Tasks have no separate tags or readiness fields.
Searching another list with `l:Work` cannot broaden the connection's access.
Task searches return up to 100 results by default and allow pages of up to 200.

A writable connection can add and edit tasks, change their status, and move them to or from Trash.
A connection restricted with `--list` cannot create or rename lists.
An unrestricted writable connection can also create and rename lists.
List results follow the order saved in the terminal, with the default list first.
New lists append after existing lists. Renaming a list keeps its position.
Use Alt+Up or Alt+Down in the terminal's list chooser to change that order.

Task edits require the `version` returned when the agent read the task.
If you edit the task in the terminal first, the agent's stale edit fails.
The agent must read the current task before trying another edit.
Husker does not silently overwrite your newer save.
Task results include `status_changed_on`, the local date of completion or cancellation.
Reopening clears that date. Older tasks migrated without a known completion date return null.

While an agent works, the terminal refreshes automatically and shows a notice
when another process changes tasks. Press F5 to refresh immediately.
See [Refresh changes from an agent](usage.md#refresh-changes-from-an-agent) for editor conflicts.

## Check a failed connection

Run `husker mcp --help` to check the installed command.
If `mcp` is unrecognized, reinstall Husker from the updated source directory.
If the client cannot find Husker, check its saved executable path.

Run `husker lists` to check a rejected list name.
For a custom profile, include the same `--profile` path used by the client.
After renaming a list, update the client's saved `--list` argument before reconnecting.

Starting `husker mcp` directly in a terminal does not display the task interface or a ready message.
The process waits for MCP messages from a client.
Startup errors go to standard error so they do not mix with protocol messages.
