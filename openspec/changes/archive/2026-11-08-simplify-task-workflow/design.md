## Context

The user now keeps development requests in the Husker list and asked to process its open tasks.
The existing implementation has separate readiness and tag fields, keyboard navigation, and one-second refresh while navigating.
The previous MCP change remains uncommitted and must be preserved.

## Goals / Non-Goals

Goals:

- Complete the eight captured project tasks and verify their behavior.
- Preserve existing task data through the field changes.
- Keep unsaved editor text safe when another process writes to SQLite.
- Retain scope enforcement and version checks for MCP clients.

Non-goals:

- Automatic installation or migration of the user's live profile.
- Clipboard integration requiring a desktop service or new platform dependency.
- Importing or deleting legacy draft files.
- Committing or pushing changes without a new user request.

## Decisions

### Simplify the task model and migrate once

Remove `ready` and `tags` from `Task` and `TaskInput`.
Add `status_changed_on: Option<NaiveDate>` to `Task`.
Keep readiness out of all user interfaces, command arguments, MCP schemas, and search groups.

Schema version 2 adds the status date and drops the obsolete columns in an immediate transaction.
Before dropping tags, append their literal `#word` forms to a final description paragraph.
Preserve the previous description bytes, including line breaks.
Invalid legacy tag data aborts the transaction rather than losing information.
Existing closed tasks receive no invented status date.
Increment each task's version when migration adds hashtags to its description.
This rejects clients that reconnect with a version read before that description changed.
Keep versions unchanged for tasks whose descriptions do not change.

`set_status_on` accepts an explicit local date for deterministic tests.
`set_status` uses the local current date.
Only a real transition into done or cancelled changes the status date.
Reopening clears the date. Editing, moving, trashing, and restoring preserve it.

### Share ordering and visibility rules across clients

Lists sort by normalized lowercase name and then ID.
Task creation without a list keeps the lowest accessible list ID as its default.
This prevents alphabetical sorting from redirecting new tasks to another list.

Task results sort by ascending due date, then undated tasks.
Position and ID break ties within an identical date or the undated group.
The default status selection includes open tasks and tasks closed on the local current date.
Explicit `s:` filters do not add other statuses. Trash keeps its existing all-status default.
Hashtag terms use ordinary case-insensitive substring matching in titles and descriptions.

### Use predictable editing and optional mouse capture

Default save keys become Ctrl+Enter and Ctrl+S. Escape and Ctrl+G discard editor changes.
Mouse capture starts enabled and F2 toggles it.
Single clicks select rows. Double clicks open tasks or lists. The wheel moves the selection.
Editor clicks select a field and cursor position.
Hit regions must follow rendered group headers, scroll offsets, and resize events.

F4 opens a full-width copy view for a selected task or the current editor buffer.
The copy view contains title and description without borders, prefixes, or vertical separators.
Mouse capture is disabled in that view so the terminal owns text selection.
Arrow keys scroll long content. Escape returns without changing the task or editor buffer.

### Detect external writes without replacing unsaved edits

Expose SQLite `PRAGMA data_version` through the store.
Poll for changes and calendar-day transitions every second.
Refresh navigation data and show a localized external-update notice when another connection writes.
Keep editor buffers intact. Existing version checks still reject stale saves.
F5 explicitly refreshes data and acknowledges the update notice.
Returning to the list chooser retains the current list ID or Trash selection.

## Risks / Trade-offs

| Risk | Response |
| --- | --- |
| Older processes use a migrated profile | Require closing old processes before the user's upgrade. Test with isolated profiles only. |
| Mouse capture interferes with copying | Provide F2 and a capture-free F4 copy view. |
| Ctrl+Enter is not distinct in a terminal | Provide Ctrl+S as a second default save key. |
| Today's completed tasks change existing queries | Apply one shared rule and document explicit `s:open` for open tasks only. |
| Schema migration loses tag information | Convert all tags to description text inside the same transaction before dropping columns. |

## Verification

Test migration rollback and field preservation with legacy fixture databases.
Test status dates across midnight, explicit status filters, ordering, and literal hashtag searches.
Test mouse hit regions, scrolling, editor cancellation and saving, external changes, and copy-view rendering.
Exercise CLI and MCP workflows in temporary profiles.
Run formatting, Clippy, all tests, a release build, OpenSpec validation, and independent review.
Inspect the updated terminal behavior without replacing the installed app.

## Evidence

- Read from source: version 1 stores readiness and JSON tags on each task.
- Read from source: current navigation refreshes once per second and preserves task identity.
- Read from source: all task writes already use version checks inside SQLite transactions.
- Observed through scoped MCP: eight open project requests were captured for this batch.
