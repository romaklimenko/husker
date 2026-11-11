# Husker

Husker is a local todo app for people who manage tasks from a terminal.
Use the keyboard to capture tasks, organize lists, and choose a planned date.
English and Danish interface text are included.

## Build and start Husker

Install Rust 1.88 or newer, then run these commands from this directory:

```text
cargo install --path . --locked
husker
```

Husker opens All tasks. Press `n`, type a title, and press Enter to save.
The new task starts in the list that holds the most of your 10 newest tasks
outside Trash, or in Inbox in a new profile. Before you save, press Tab
once to reach List, or twice to reach Planned date. For example, press `n`,
type `Pay rent`, press Tab twice, then press `t` and Enter to plan it for today.
The footer shows the most useful keys for the current screen. Press `?` to
see every shortcut grouped by screen, or `q` to quit.
With a task selected, press Space to mark it done, `t` for today, `T` for
tomorrow, `w` for next Monday, or `x` to clear its planned date. These
changes save immediately. Press `u` to undo the last one.
The same date shortcuts in the editor's Planned date field change the draft until you save.
In 2026, you can also type `20261109`, `261109`, or `1109` for November 9.
The four-digit form uses the current local year. Press `m` on a task to
move it to another list.

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

Press F3, choose English or Dansk, and press Enter to change the interface
language. Husker saves the choice for the next launch. Esc cancels the picker.
An open task draft stays intact. The language shortcut is listed in Help (`?`).

Create or edit `config.toml` in your profile directory to override shortcuts:

```toml
language = "da"

[keys]
quit = ["ctrl+q"]
```

The language picker keeps existing shortcuts and comments in that file.
Run `husker config --example` to print all default settings.
Use `--config FILE` to read an existing configuration file from another path.

Read [Use Husker](docs/usage.md) for the editor, date groups, search syntax,
History, Trash and permanent deletion, moving tasks, undo, list removal,
configuration, and backups.

Click a task to select it or double-click to edit it. Press F4 to open plain
task text for copying. Press F5 to refresh after another process changes tasks.
Action messages and errors clear after five seconds. External-update notices
stay until you acknowledge them with F5.
Task rows use the full width. Press Enter to read or edit the description.
Long descriptions wrap in the editor without adding line breaks to your text.
Use Ctrl+Left and Ctrl+Right to move by word, Shift with movement keys to
select text, Ctrl+C to copy the selection, and Ctrl+Z to undo typing.
The editor heading shows an existing task's ID.

Use Up/Down or `k`/`j` to move through tasks and picker entries. Selection
wraps at either end. Left, Home, or `g` selects the first item. Right, End,
or `G` selects the last. PageUp and PageDown move by one screen.
These keys also work in History, Trash, and the list pickers. Left and Right
keep their cursor and scrolling controls in text fields and the F4 copy view.
Ctrl+C quits from a task view, History, or the list picker.

Press `l` to open the list picker over your tasks. Choose a list to show a
view such as `All tasks > Work`. If that view has a filter applied, Esc
clears the filter first. Otherwise Esc returns to All tasks with the filter,
selected task, and scroll position you had there. Esc inside the picker cancels it.
All tasks is the first picker entry and Trash is last.
All tasks and all-list History prefix task titles with their list, such as
`[Work] Review`. A view scoped to Work does not repeat that label.
Press `n` in the picker to create a list and `r` to rename the selected one.
Use Alt+Up and Alt+Down, or `K` and `J`, to arrange ordinary lists.
The default list stays first among them and is marked as the default. Husker remembers their order.
Press Delete on a list to review its removal, then `y` to confirm.
After confirmation, every task moves to the default list and only the list is removed.

Press `d` on a task to move it to Trash. Choose Trash in the list picker
and press `r` to restore the selected task. To remove tasks for good, press
`d` in Trash to delete the selected task forever, or `D` to empty Trash.
Delete on the Trash entry in the list picker also empties Trash. Husker
asks you to confirm each of these, and a task deleted forever cannot be
restored or undone.

## Current scope

Husker supports local lists and tasks, plain text descriptions, planned dates,
Trash with restore and permanent deletion, one-step undo of task actions, mouse navigation,
configurable action keys, JSON commands, and local MCP tools.
Write `#words` in titles or descriptions when useful.
Search treats those words as ordinary text.
Concurrent task edits use version checks so an older edit cannot silently
overwrite a newer save.
Open tasks planned before today appear under Earlier, or Tidligere in Danish.
Future open tasks appear under their exact planned dates. Every heading shows
the number of tasks in its group. Today and Tomorrow headings also show the
date, weekday, and ISO week. Open tasks without dates appear under
Sometime, or Engang in Danish. Tasks completed or cancelled today appear in
Done today or Cancelled today after the open tasks. A task you mark done
with Space keeps its row, dimmed, until you change the view.
Press `h` to see older completed and cancelled tasks in History. Press `r`
there to reopen a task. Same-day completions use their recorded completion
times, newest first. History headings show completion or cancellation dates.
Open the editor to see a historical task's Planned date. History retains
current task records, not every past edit or status change.

Recurrence, reminders, subtasks, priorities, synchronization, and an event
log are not included. Permanent deletion is available only in the terminal
interface. No command or MCP tool deletes a task forever.

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
