# Husker

Husker is a local todo app for people who manage tasks from a terminal.
Use the keyboard to capture tasks, organize lists, and plan by due date.
English and Danish interface text are included.

## Build and start Husker

Install Rust 1.88 or newer, then run these commands from this directory:

```text
cargo install --path . --locked
husker
```

Husker opens Planned. Press `n` to create a task, enter a title,
and press Ctrl+S to save. Esc asks before discarding changed task fields.
Press `?` to see the shortcuts or `q` to quit.

Run the installed `husker` command while someone edits or builds the repository.
On Windows, you can select that copy explicitly:

```powershell
& "$env:USERPROFILE\.cargo\bin\husker.exe"
```

The installed copy leaves the repository's build executable free to replace.
Close Husker and disconnect its MCP clients before running `cargo install` again.
For an existing profile, follow [Update an existing profile](docs/usage.md#update-an-existing-profile).

Your tasks stay on your computer in `~/.husker/tasks.db`. A new profile starts
with an Inbox list. Use `--profile DIRECTORY` to keep another collection of
tasks:

```text
husker --profile ./work-tasks
```

## Use commands from scripts

Create and find tasks without opening the interface:

```text
husker add "Buy milk #shopping"
husker tasks "milk #shopping"
husker --json tasks
```

Commands print persistent task IDs. Use the ID printed by `add` with `show`,
`edit`, `done`, `cancel`, `reopen`, `trash`, or `restore`.
Run `husker --help` or `husker edit --help` for the available options.

## Connect an agent

Register `husker mcp --list Private` as a local stdio MCP server in your agent's
client settings. Replace `Private` with an existing list name.
Every tool is restricted to that list, including searches and Trash.
Add `--read-only` if the agent should only read tasks.

Use one Husker installation for several projects. Each project can start its
own MCP process with a different `--list` while using the same task database.
Configure Claude Code through a local registration or a shared `.mcp.json`.
Configure Codex through the project's `.codex/config.toml`.
Read [Connect an agent to Husker](docs/mcp.md#use-one-installation-across-projects)
for project setup, custom profiles, and access settings.

## Configure the interface

Create `config.toml` in your profile directory. Set the language and override
only the shortcuts you want to change:

```toml
language = "da"

[keys]
quit = ["ctrl+q"]
```

Run `husker config --example` to print all default settings.
Use `--config FILE` to read an existing configuration file from another path.

Read [Use Husker](docs/usage.md) for the editor, date groups, search syntax,
Trash, configuration, and backups.

Click a task to select it or double-click to edit it. Press F4 to open plain
task text for copying. Press F5 to refresh after another process changes tasks.
Action messages and errors clear after five seconds. External-update notices
stay until you acknowledge them with F5.
Task rows use the full width. Press Enter to read or edit the description.
Long descriptions wrap in the editor without adding line breaks to your text.
Use Ctrl+Left and Ctrl+Right to move by word, Shift with movement keys to
select text, Ctrl+C to copy the selection, and Ctrl+Z to undo.
The editor heading shows an existing task's ID.

Press `l` to open the list picker over your tasks. Choose a list to show a
view such as `Planned > Work`. Esc returns to Planned with your previous
filter, selected task, and scroll position. Esc inside the picker cancels it.
All tasks is the first picker entry and Trash is last.
Use Alt+Up and Alt+Down in the picker to arrange ordinary lists.
The default list stays first among them. Husker remembers their order.

## Current scope

Husker supports local lists and tasks, plain text descriptions, due dates,
recoverable Trash, mouse navigation, configurable action keys, JSON commands,
and local MCP tools. Write `#words` in titles or descriptions when useful.
Search treats those words as ordinary text.
Concurrent task edits use version checks so an older edit cannot silently
overwrite a newer save.
Tasks completed or cancelled today stay visible until the next local day.
Open tasks appear first within each date group. Tasks without due dates appear
under Sometime, or Engang in Danish.

Recurrence, reminders, subtasks, priorities, synchronization, list deletion,
and permanent task deletion are not included.

## Work on the project

Read [AGENTS.md](AGENTS.md) for repository instructions.
Substantial features use [OpenSpec](https://github.com/Fission-AI/OpenSpec).
The behavior specs live under `openspec/`.
Read [Build and check Husker](docs/development.md) for the verified workflows
and the first performance measurement.

Run these checks after changing the application:

```text
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
openspec validate --all --strict
```
