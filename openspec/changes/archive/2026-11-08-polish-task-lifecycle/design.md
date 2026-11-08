## Context

Read from source: tasks retain their current status and one optional status_changed_on date.
Reopening clears that date. No task expires automatically and no event table exists.
Default queries show open tasks and records closed today. Explicit status filters retrieve older records.
Read from source: lists have stable IDs and tasks reference those IDs. The lowest-ID ordinary list is the default.

## Decisions

### Separate completion display from stored due dates

Keep Bucket as the due-date classification used by due filters.
Add PlannedGroup with Due(Bucket), DoneToday, CancelledToday, Done, and Cancelled variants.
Ordinary results place all open due groups first, followed by those four closed groups.
Open groups retain due-date, position, and ID ordering. Closed groups sort by status date descending, then ID descending; unknown dates follow known dates.
Expose planned_group so rendering and shared sorting use identical group boundaries.
Manual task reordering remains available in the terminal for open tasks only.
After closing an open task, retain the next available open neighbor, or the previous one if there is no next one.

### Expose current completed and cancelled records as History

Add visible_history using existing query terms and scope checks.
History always excludes open and trashed tasks, even with an explicit status filter.
Without a status filter it includes both closed statuses on every date.
Sort by status date descending and ID descending, with unknown dates last.
Render date headings with Today, Yesterday, calendar dates, and Unknown date labels.
Do not infer a timestamp or an earlier completion event from a current record.

History opens with an empty filter and stores the originating task view, filter, selected task ID/index, and scroll.
Esc or the History action restores that origin. All tasks returns the saved root state.
The picker exits History. Choosing its underlying task scope restores the origin; choosing another list starts an empty task view.
Task editing, copying, search, help, status changes, and Trash remain available. Reopen uses r in History.
New-task creation, manual reordering, and opening History from Trash or the picker are unavailable.
If an origin list disappears, return to All tasks with a valid selection.

### Remove only list metadata and retain tasks

Use ListDeletionPreview with source/destination names and IDs, non-Trash open/done/cancelled counts, a trashed count, and a private sorted task ID/version snapshot.
Read previews in a consistent transaction. Apply deletion in an immediate transaction.
Protect the default list and reject list mutation in read-only or list-scoped stores.
Recompute and compare the full preview before writing. Changed returns a fresh preview without mutation; Missing reports that the list is already gone.
For a matching preview, move every task to the default list, increment task versions, and delete the source list in the same transaction.
Preserve every other task field, including status, status date, Trash state, and position.
Removing a list permanently loses its name and membership grouping. The confirmation states the destination explicitly.

The confirmation defaults to No. Refreshing changed contents resets No and requires a fresh explicit Yes.
Successful deletion leaves the picker open with the destination selected.
If the removed list is the current scope, restore All tasks. Deleting another list preserves the current scope.
External deletion follows the same fallback while preserving open editor drafts and their originally read versions.
Existing MCP connections remain restricted to their original list ID after removal or same-name recreation.

### Rename root navigation without breaking configured shortcuts

Use All tasks and Alle opgaver for the root view, breadcrumbs, picker, and help.
Rename the action to all_tasks without a default shortcut. Back and the All tasks picker entry handle root navigation.
Accept planned as a legacy configuration alias and preserve explicitly configured shortcuts.
An explicit legacy override replaces the new default. Supplying both names is a configuration error.
Add history, reopen, and delete_list actions with mode-specific default h, r, and d/Delete bindings.
Keep History and deletion-confirmation modes distinct for shortcut conflict validation.
Mark the default list in the picker and expose the relevant new actions in hints.

### Save language choices before changing the interface

Add a language action with default F3 and a modal English/Dansk picker.
Open it from Tasks, History, Lists, Editor, Filter, ListName, or Help. Keep it unavailable from confirmations and CopyText.
Remember the underlying screen and preserve its selection, scroll, filter, and complete text-buffer state.
Use configured Up, Down, Apply, and Back controls. Cancel returns without changing configuration.

Retain the configuration path selected at startup. Save to that path, including an explicit --config file.
Read its current contents and update only the top-level language setting so comments and shortcut overrides survive.
Write a replacement file completely before atomically replacing the active file. Do not truncate the current file in place.
A missing implicit profile configuration can be created. A missing explicit configuration still fails at startup.
Only switch the running language after a successful save. Keep the picker open with an error after failure.
Keep running shortcut bindings unchanged until restart, even if another editor changed them before the language save.

Directly overwriting a generated default configuration is rejected because it would lose custom shortcuts and comments.
Switching the visible language before persistence is rejected because a failed save would disagree with the next launch.

## Non-goals

No immutable event log, list archive/restore system, new MCP tools, CLI delete commands, database migration, or installed-app replacement is included.

## Verification

Test grouping independently from due filters, midnight boundaries, history scope/filtering, unknown dates, and deterministic ordering.
Test atomic deletion, stale and missing previews, every preserved task field, moved-task versions, protected lists, rollback, and fixed-ID scopes.
Test History return paths, nested editors/help/copying, completion selection, delete confirmation, external deletion, configured shortcuts, and localized hints.
Test language saving with comments, custom keys, missing implicit configs, explicit paths, and write failures.
Test immediate switching, restart persistence, modal input isolation, and preserved drafts and navigation state.
Run formatting, Clippy, all tests, a release build, isolated CLI/terminal checks, and strict OpenSpec validation before archiving.
