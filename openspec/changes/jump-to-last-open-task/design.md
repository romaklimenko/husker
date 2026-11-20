## Context

Read from source: `App::handle_key` in `src/ui.rs` moves the task selection with `task_selection`. First and Down on the last row select the start task from `App::first_row_from_today`. Last selects the last row, and Up on the first row selects the last row.
Read from source: `planning::sort_tasks` puts open tasks first and closed tasks after them, in the groups Done today, Cancelled today, Done, and Cancelled. The default query of All tasks and list views shows Done today and Cancelled today.
Read from source: a held row is a done or cancelled task that keeps its row and its open-task heading until the view changes.
Reported by the user: as First goes to today's first task, the jump to the bottom should go to the last task that is not done. Up on the first task under Earlier should select that task too.

## Goals / Non-goals

Goal: in All tasks and list views, Right, End, `G`, and Up on the first task land on the last open task.
Goal: every task stays reachable with Up and Down.

Out of scope:

1. The selection when a view opens, when the filter changes, or after a refresh.
2. Down on the last task, First, PageUp, PageDown, and the mouse wheel.
3. History and Trash. History shows only closed tasks. Trash has no status groups.
4. The list pickers and the language picker.
5. The start task rule. A held row under Today can still be the start task.

## Decisions

### Find the end task by status

`App::last_open_row` returns the last visible task whose status is open in All tasks and list views, and the last row otherwise.
Open tasks always sort before closed tasks, so the end task is the last row before the status groups. Held rows sit among the open tasks and are skipped.
Rejected: the last row not under a status heading, which mirrors the start task. A held row shows an open-task heading, so a task the user just marked done could become the end task. The user asked for the last task that is not done.

### Skip done and cancelled tasks

A cancelled task is closed too, and Cancelled today follows Done today. Skipping only done tasks would leave the end task on a cancelled task at the bottom.

### Keep the last task when no task is open

With no open task, for example under the filter `s:done`, Last and the wrap of Up select the last task, as in 0.21.1.

### Change the action, not the keys

`task_selection` takes the end task next to the start task and replaces the Last and Up-wrap targets with it. The `last` action of a custom binding gets the same rule. The pickers keep `navigation_selection` unchanged.

## Risks / Trade-offs

A user who used Right to reach the done and cancelled tasks needs Down from the end task, or PageDown, instead.
When the end task is also the first task, Up keeps it selected. Down still reaches the closed tasks.
The start task follows headings and the end task follows status. They differ for a held row, which lasts until the view changes.
They also differ when every open task is under Earlier and Done today or Cancelled today has tasks. The start task is then the first closed task, below the end task. Up then cycles through Earlier and Down through the closed tasks, and the two keys together still reach every task.
Changing the start task for that case would change the rule of the jump-to-today change, so it stays out of scope.

Rolling back means reinstalling 0.21.1. No data changes.
