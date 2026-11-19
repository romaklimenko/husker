## Context

Read from source: `App::handle_key` in `src/ui.rs` moves the task selection with `navigation_selection`. First selects row 0, and Down on the last row selects row 0.
Read from source: `planning::sort_tasks` puts open tasks planned before today first, under the Earlier heading. `planning::group_rows` gives each displayed group a stable key, and Earlier has the key `overdue`.
Read from source: a held row keeps its position and the heading it had while open. `group_rows` takes a held-row test for that.
Reported by the user: Left should select the first task of today, not a task before today. Down at the bottom should do the same. Up and Down should still reach yesterday's tasks.

## Goals / Non-goals

Goal: in All tasks and list views, Left, Home, `g`, and Down on the last task land on the first task that is not under Earlier.
Goal: every task stays reachable with Up and Down.

Out of scope:

1. The selection when a view opens, when the filter changes, or after a refresh. It stays on the first task or the remembered task.
2. Up on the first task, Last, PageUp, PageDown, and the mouse wheel.
3. History and Trash. History lists the newest day first and has no Earlier heading. Trash has no headings.
4. The list pickers and the language picker.

## Decisions

### Find the start task from the displayed groups

`App::first_row_from_today` calls `group_rows` with the same arguments as the task list drawing and returns the first row of the first group whose key is not `overdue`.
The rule therefore follows the headings on screen, including held rows and filtered results.
Rejected: testing each task's date bucket again in `ui.rs`. A held row shows its open heading although its status is closed, so a second test would have to repeat that rule and could drift from the drawing.

### Skip only Earlier

The start task is the first task after Earlier, even when no task is planned for today. With nothing under Today, it is the first task under Tomorrow, a date, Sometime, or a status group.
Rejected: selecting only a task under Today. With no task planned for today, First would need a second fallback, and the user's words were "not before today".

### Keep the first task when every task is under Earlier

With no task after Earlier, First and the wrap of Down select the first task, as in 0.20.1.

### Change the action, not the keys

`task_selection` wraps `navigation_selection` and replaces its First and Down-wrap targets with the start task. The `first` action of a custom binding gets the same rule. The pickers keep `navigation_selection` unchanged.

## Risks / Trade-offs

A user who used Left to reach the tasks under Earlier needs Up from the start task, or PageUp, instead.
When the start task is also the last task, Down keeps it selected. Up still reaches the tasks under Earlier.

Rolling back means reinstalling 0.20.1. No data changes.
