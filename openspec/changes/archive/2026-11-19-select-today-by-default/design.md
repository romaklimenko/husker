## Context

Read from source: `App::refresh` in `src/ui.rs` remembers the selected task before it rebuilds the rows and selects it again afterwards. Without a remembered task it kept the row index, which `restore_view_state` and the filter code set to 0 or to a saved index.
Read from source: startup, `set_view`, and `open_history` clear the rows before `refresh`, so no task is remembered there.
Read from source: `apply_typed_filter` and the Apply branch of the filter input set the selection to 0. `clear_filter` selects the earlier task again and keeps the row index when the task is gone.
Read from source: version 0.21.0 added `App::first_row_from_today`, the row that First selects.
Reported by the user: "when Husker starts - select first task from today (actually in all cases when not sure - select the first today's task)".

## Goals / Non-goals

Goal: every selection that Husker makes without a known task lands on the start item.
Goal: a task that the user selected, or that a view saved, stays selected while it is visible.

Out of scope:

1. A refresh while a task is selected. When that task leaves the results, the selection keeps its row position. The user is working at that spot, so a jump to today would move them away. A change of the interface language follows the same rule.
2. Esc in the filter input, which restores the state from before the input opened. Only when no task was selected then does it select the start item.
3. The list pickers and the language picker.
4. Selection after task actions such as marking a task done or moving it to Trash. Those keep their existing rules.

## Decisions

### Select the start item in `refresh` when no task is remembered

After `refresh` rebuilds the rows, it selects the start item when no task was selected before. One change therefore covers startup, opening a view, and an empty view that receives tasks.
When the current list is deleted, `refresh` restores All tasks from its saved state. It then selects the saved task when visible, and the start item otherwise.
Rejected: a flag that each caller sets before `refresh`. Callers would have to remember it, and a missed caller would keep the old row.

`App::reselect` holds that rule: it selects the task again, keeps the row position of a task that left the rows, and selects the start item without a task. `refresh` and `set_language` use it.

### Filters select the start item explicitly

`apply_typed_filter` and Enter call `select_start` after the rows are rebuilt. `clear_filter` calls `select_task_or_start`, which keeps a visible task and selects the start item otherwise.
Enter needs its own call, because `refresh` would otherwise keep the task that was selected when the input opened.
Enter and `clear_filter` skip their own selection when the `refresh` before it left a deleted list. That `refresh` has already selected the saved task of All tasks or its start item. A review found that Enter overwrote the saved task in that case.
`discard_typed_filter` selects the start item when no task was selected when the input opened. Without that, a task the user reached by typing would stay selected after Esc.

### Replace a gone saved task with the start item

A view that returns to its saved state used to select the saved row index when the saved task was gone. That row is an arbitrary task, so it now selects the start item.
The saved row index in `PlannedBookmark` is no longer read, so the field is removed.

## Risks / Trade-offs

A user who expected the first row at startup, or after a filter, now starts on today's first task. Up reaches the tasks under Earlier.
`first_row_from_today` groups the visible rows once per default selection. That is the same work as drawing the task list once.

Rolling back means reinstalling 0.21.0. No data changes.
