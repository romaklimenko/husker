## Context

Read from source before this change: `suggested_list_id` in `src/planning.rs` counted the lists of the 10 counted tasks with the highest IDs.
The most common list won, and a tie went to the list with the newest task.
Read from source: the terminal interface passes every stored task and every accessible list, so filters do not change the result.

The user reports that agents create many tasks in the Labs list.
Labs therefore wins the count, even right after the user captured a task in another list.

## Goals / Non-goals

Goal: a new task from All tasks starts in the current list of the newest counted task.
Goal: keep the counted-task rules, the fallback, and the list-view behavior unchanged.

Out of scope:

1. Recording which client created a task. Tasks have no creator field, and adding one changes the database schema and the sync format.
2. Remembering the last list the user saved to in the terminal interface.
3. The CLI `add` command and the MCP `add_task` tool.
4. Lists chosen in the editor, undo, redo, and Save, which follow the requirement Choose a list for a draft by stable identity.

## Decisions

Use the counted task with the highest ID and return its current list.
Read from source: local task IDs grow with creation and are not reused, so the highest ID is the newest task on this computer.
A task moved after creation counts for its current list, as before.

Keep Trash excluded.
A task the user created and trashed was usually a mistake, so it should not steer the next task.

Keep done and cancelled tasks counted.
A task captured and closed right away still shows where the user was working.

Rejected: a smaller window, such as the 3 newest tasks.
An agent that creates 3 tasks in a row still wins, and the rule stays hard to predict.

Rejected: excluding a list by name or by a setting.
The user asked for the newest task's list. A list setting would add configuration for one case.

## Risks / Trade-offs

An agent task created after the user's last task sets the suggestion to the agent's list.
The user then presses Tab to focus List and picks another list, or opens the target list with `l` first.
Tasks record no creator, so this change cannot tell the two apart.

Read from source: a sync inserts each received task with a higher ID than every ID the local table has assigned.
A task that another computer created before the user's last local task can therefore set the suggestion after a sync.
The workaround is the same as for an agent task.

No migration is needed. Reinstall the prior executable to roll back.
