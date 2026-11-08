# Connect an agent to Husker

This guide is for people who want an AI agent to read or manage their Husker tasks.
Husker uses MCP, the Model Context Protocol, to give the agent task tools.
The client starts Husker as a local process and communicates over standard input and output.

## Install or update Husker

This guide describes Husker 0.5.0 and its task tools.
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

## Use one installation across projects

Install Husker once and configure each project to pass its own `--list`.
For this example, create any lists you do not already have:

```text
husker list-add Private
husker list-add Work
```

| Project directory | Server command |
| --- | --- |
| `project-a` | `husker mcp --list Private` |
| `project-b` | `husker mcp --list Work` |

Both clients can run at the same time. Each starts a separate Husker process
from the same installed executable. Both use `~/.husker/tasks.db` unless you
choose a [different profile](#select-a-different-profile).
Each process enforces its own list restriction. The terminal can stay open too.

The examples below use `husker` from the client's `PATH`.
If the client cannot find it, use the installed executable's absolute path.
Project configuration selects where a connection is available. Husker's
`--list` selects which tasks that connection can access.

## Connect Claude Code for one project

Run this command from the project directory to keep the registration private
to you and available only in that project:

```text
claude mcp add --transport stdio --scope local husker-project -- husker mcp --list Private
```

Claude stores this local registration under the project's path in
`~/.claude.json`. To share the setup in the repository instead, use project scope:

```text
claude mcp add --transport stdio --scope project husker-project -- husker mcp --list Private
```

Project scope writes `.mcp.json` at the project root. You can also create or
merge this entry into that file:

```json
{
  "mcpServers": {
    "husker-project": {
      "type": "stdio",
      "command": "husker",
      "args": ["mcp", "--list", "Private"]
    }
  }
}
```

Use `Work` in the second project's `args`. Commit `.mcp.json` when you want
teammates to use the same setup. Each teammate needs Husker and that list in
their own profile. The file does not share your task database.

On Windows, the default Cargo installation also supports this command value:
`"command": "${USERPROFILE}/.cargo/bin/husker.exe"`.
Claude expands `${USERPROFILE}` from the environment.
See [Claude Code's MCP guide](https://code.claude.com/docs/en/mcp#environment-variable-expansion-in-mcpjson)
for variable expansion and [installation scopes](https://code.claude.com/docs/en/mcp#mcp-installation-scopes).

Run `claude mcp list` from the project directory, then start Claude and inspect
`/mcp`. Interactive sessions ask you to approve shared project servers.
Use either local or project scope for this entry, so a local registration
does not override a later edit to the shared file.

## Connect Codex for one project

Create or update `.codex/config.toml` in the project directory:

```toml
[mcp_servers.husker-project]
command = "husker"
args = ["mcp", "--list", "Private"]
```

Use `Work` in the second project's `args`. If Husker is not on the client's
`PATH`, replace `command` with its absolute executable path, such as
`"C:/path/to/husker.exe"` or `"/path/to/husker"`. Replace these placeholders
with your installation path.

Codex loads project configuration only for trusted projects. Start Codex in
the project directory, trust the project if appropriate, and check `/mcp`.
Use `.codex/config.toml` for this setup. The Claude `.mcp.json` example above
does not configure Codex. See [Codex configuration](https://learn.chatgpt.com/docs/config-file/config-basic)
and the [MCP settings guide](https://learn.chatgpt.com/docs/extend/mcp?surface=cli).

`codex mcp add` writes a user-level registration in `~/.codex/config.toml`.
Use the project file above when the connection should belong to one project.
The [Codex CLI reference](https://learn.chatgpt.com/docs/cli/reference#codex-mcp)
describes user-level registration commands.

## Remove other Husker connections

Check `/mcp` for every available Husker connection. Adding a project connection
does not remove user-level registrations. A second connection to another
list, or one without `--list`, still gives the agent that connection's access.

Remove an unwanted user-level registration with the matching client command:

```text
claude mcp remove --scope user husker-all
codex mcp remove husker-all
```

Replace `husker-all` with the registered name. To keep a global Codex server
but disable it in this project, add this override to `.codex/config.toml`:

```toml
[mcp_servers.husker-all]
enabled = false
```

The [Codex MCP settings guide](https://learn.chatgpt.com/docs/extend/mcp?surface=cli)
documents `enabled`. Restart the client after changing connections.
These settings restrict MCP access. They do not remove an agent's separate
shell or filesystem access to your profile.

## Allow reading without changes

Add `--read-only` to the Husker arguments in your project configuration:

```text
["mcp", "--list", "Private", "--read-only"]
```

The agent receives only `list_lists`, `list_tasks`, and `get_task`.
The database is opened without write access.
An attempt to call a mutation tool still fails even if the agent guesses its name.
The profile and database must already exist at the current schema version.
After upgrading an older profile, run `husker lists` once with the updated
executable before starting a read-only server.

If you previously registered a writable connection, [replace that registration](#change-a-saved-connection) and restart the client.
Leaving both connections available still gives the agent a way to write.

## Select a different profile

Put `--profile` before `mcp` in the server command to select another task collection.
Use an absolute profile path because the client's working directory can differ from your terminal's directory.
For example, use these arguments to select `Private` from `C:\Tasks\Husker`:

```text
["--profile", "C:/Tasks/Husker", "mcp", "--list", "Private"]
```

Replace `C:\Tasks\Husker` with the directory containing your `tasks.db`.
Check that profile's list names with `husker --profile "C:\Tasks\Husker" lists`.

To give a connection access to every list in the selected profile, omit `--list`.
For example, `husker mcp --read-only` can read every list in the default profile.

## Change a saved connection

Edit `args` in `.mcp.json` or `.codex/config.toml` to change a project connection.
For a Claude Code local registration, run this command from the project directory:

```text
claude mcp remove --scope local husker-project
```

Then repeat the local `mcp add` command with the new Husker arguments.
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
Results follow the [planning order](usage.md#plan-by-due-date), with open tasks
before done and cancelled tasks within each date group.

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
