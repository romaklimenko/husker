## Why

A new task from All tasks starts in the current list of the newest task outside Trash.
Today the suggestion is the most common list among the 10 newest tasks.
Agents create many tasks in one list, such as Labs, so that list wins the count almost every time.
Labs wins even right after the user captures a task in another list.
The newest task's list is easy to predict, because it is the list of the last captured task.

## What changes

1. The suggested list becomes the current list of the counted task with the highest ID.
2. The definition of a counted task does not change: outside Trash, in an accessible ordinary list, with any status.
3. The tie rule and the window of 10 tasks are removed.
4. The fallback to the lowest-ID ordinary list stays for a profile without counted tasks.
5. List views, the CLI `add` command, and the MCP `add_task` tool stay unchanged.

## Capabilities

### New capabilities

None.

### Modified capabilities

- `terminal-interface`: The requirement Suggest a list for a new task uses the newest counted task instead of a count.

## Impact

1. `src/planning.rs`: replace the count in `suggested_list_id` and its unit tests.
2. `src/ui.rs`: update the editor test that opens a new task from All tasks.
3. `README.md` and `docs/usage.md`: describe the new rule.
4. `docs/development.md`: record the checks for version 0.16.0.
5. `Cargo.toml` and `Cargo.lock`: raise the version to 0.16.0.

No storage migration, sync format change, configuration change, CLI change, or MCP change is required.
A task an agent creates after the user's last task still sets the suggestion, because tasks record no creator.
A task that a sync inserts gets a higher ID than every local task, so it also sets the suggestion.
