## Context

Read from source: `App::handle_key` in `src/ui.rs` moves the task selection with `task_selection`. It selects the start task for First and for Down on the last row, and the end task for Last and for Up on the first row. Other moves go through `navigation_selection`.
Read from source: the three list pickers and the language picker move with `navigation_selection`, which wraps Up on the first item to the last item and Down on the last item to the first item.
Read from source: `page_selection` and the mouse wheel already stop at the ends.
Reported by the user: with six tasks under Earlier, they walk up from today's first task to the first task under Earlier. One Up too many jumps to the end of the view. Left already selects today's first task and Right the last open task, so Up and Down should stop at the ends.

## Goals / Non-goals

Goal: Up and Down never jump to the other end of a selection list.
Goal: First and Last keep their targets, so both ends stay one key press away.

Out of scope:

1. The start task and end task rules of First and Last.
2. PageUp, PageDown, the mouse wheel, and Help and Copy text scrolling.
3. Tab and Shift+Tab in the task editor, which still wrap between fields. They move focus, not a list selection.
4. The selection when a view opens, when the filter changes, or after a refresh.
5. The phone page. It has no keyboard selection.

## Decisions

### Stop at the ends in every selection list

`navigation_selection` clamps Up at the first item and Down at the last item. Task views, History, Trash, the list pickers, and the language picker share this function, so one rule covers all of them.
Rejected: stopping only in All tasks and list views. The pickers would then wrap while task views stop, and the same keys would behave differently one screen apart.

### Keep start and end tasks for First and Last only

`task_selection` keeps the start task for First and the end task for Last. It no longer uses them for Up and Down.
Up from the start task still reaches the tasks under Earlier. Down from the end task still reaches the done and cancelled tasks.

### Keep the language picker as two choices

With the stop rule, Up selects English and Down selects Dansk. First and Last already did this. Rejected: keeping a toggle for the language picker alone, because it would be the only list that wraps.

## Risks / Trade-offs

A user used to the wrap now presses Up or Down at an end and nothing happens. The selection stays visible, so nothing is lost. Left and Right reach the other end in one key press.
Rolling back means reinstalling 0.22.0. No data changes.
