## Context

The task editor in src/ui.rs stores four text buffers by fixed index: Title, Description, Planned date, and List. Tab moves to the next index, and the editor draws the fields in index order.
`App::new_task` opens a new draft with the list of a list view, or the lowest-ID list in every other view.
Task IDs come from an `AUTOINCREMENT` column. A purged task's ID is never reused (read from source and the purge test in src/store.rs).

## Goals / Non-Goals

**Goals:**

- Reach List with one Tab from Title and Planned date with two.
- Start most new tasks in the list the user would pick.
- Keep the rule predictable: no model, no stored statistics, no configuration.

**Non-Goals:**

- No change to CLI `add` or MCP `add_task` defaults.
- No suggestion in list views. The list view already names the list.
- No auto-advance after choosing a list, and no change to Enter in any field.
- No visible marker that a list was suggested.
- No change to the date presets or to the field the editor focuses when it opens.

## Decisions

### Keep buffer indexes and add a display order

The four buffers keep their indexes behind named constants. A separate order array lists Title, List, Planned date, Description. Tab, Shift+Tab, and drawing follow that array.
Alternative rejected: renumbering the buffers to match the new order. It changes every index in the editor and its tests without changing behavior.

### Count the 10 newest tasks outside Trash

A pure function in src/planning.rs takes the loaded tasks and accessible lists and returns a list ID.
It takes tasks outside Trash whose list is accessible, keeps the 10 with the highest IDs, and counts them per list. The list with the highest count wins.
A tie goes to the tied list whose newest counted task has the highest ID. With no counted task, the function returns the lowest-ID list, which is the previous default.
Completed and cancelled tasks count. A task counts with its current list, so a task moved after creation votes for its destination.
Tasks in Trash do not count, because a trashed task is often a mistake.
The window is 10 so that a change in habits shows after a few tasks, while one odd task does not move the suggestion.
Alternative rejected: a local text classifier. The user asked for the simple statistical rule first.
Alternative rejected: a new store query. The terminal already loads every task on refresh, so the function reuses that data and stays testable without a database.

### Show Sometime in the empty date field

An empty Planned date already saves no date and lists the task under Sometime. The placeholder now shows the localized Sometime label, so the default is visible.
The field title keeps the accepted forms, so the format guidance remains on screen.

## Risks / Trade-offs

- A user who creates several tasks in one list from All tasks shifts the suggestion toward that list. This is the intended behavior, and Ctrl+Z in List restores the suggestion after a wrong pick.
- Users who learned the old Tab order press Tab once more or less for a few days. The field titles show the new order.
