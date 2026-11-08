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

Husker opens your planned tasks. Press `n` to create a task, enter a title,
and press Esc to save. Press `?` to see the shortcuts or `q` to quit.

To run from the source directory without installing the executable:

```text
cargo run --release --
```

Your tasks stay on your computer in `~/.husker/tasks.db`. A new profile starts
with an Inbox list. Use `--profile DIRECTORY` to keep another collection of
tasks:

```text
husker --profile ./work-tasks
```

## Use commands from scripts

Create and find tasks without opening the interface:

```text
husker add "Buy milk" --tag shopping
husker tasks "milk #shopping"
husker --json tasks
```

Commands print persistent task IDs. Use the ID printed by `add` with `show`,
`edit`, `done`, `cancel`, `reopen`, `trash`, or `restore`.
Run `husker --help` or `husker edit --help` for the available options.

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

## First-version scope

Husker supports local lists and tasks, plain text descriptions, tags, due
dates, recoverable Trash, configurable action keys, and JSON commands.
Concurrent task edits use version checks so an older edit cannot silently
overwrite a newer save.

Recurrence, reminders, subtasks, priorities, synchronization, list deletion,
and permanent task deletion are not included. MCP is a later addition.
Scripts can use the noninteractive commands now.

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
