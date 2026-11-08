## Context

The user requested five changes through the Husker task inbox.
Version 0.3 shows a description preview in wide terminals and uses unwrapped text for every editor field.
Planning headings share a Ratatui list item with the group's first task, so the selected style affects both lines.
List ordering currently follows lowercase names.

## Goals / Non-Goals

Goals:

- Give task navigation the full available width.
- Wrap Description naturally while preserving the original text.
- Keep heading styling and mouse behavior independent from task selection.
- Persist manual list order while pinning the original default list and Trash.
- Clear old action results without hiding external-update notices.

Non-goals:

- Editing descriptions in a second sidebar interface.
- Soft wrapping the F4 copy view or adding clipboard integration.
- Exposing a new MCP mutation tool for list ordering.
- Automatically installing the new binary or migrating the personal profile.

## Decisions

### Use a full-width task list

Remove the responsive two-column split and description preview.
Enter remains the path to editing a selected task. F4 remains the plain-text copy view.
Planning headings keep their own style and never receive a selection marker.
Keyboard navigation and mouse targets refer only to task rows.

### Keep wrapping separate from stored text

Build a visual layout for the Description field from its current text and display width.
Prefer word boundaries and split long words when they cannot fit on a row.
Keep explicit newlines and all original text unchanged in the editor buffer.
Use display-column widths for Unicode text and map visual positions back to valid UTF-8 boundaries.

Drawing, cursor placement, vertical movement, mouse clicks, and scrolling must use the same layout.
Recompute the layout after text changes and terminal resizing.
Keep the cursor visible by scrolling visual rows, without horizontal scrolling in Description.
Other fields keep their existing single-line behavior.
The F4 copy view keeps its existing unwrapped representation.

### Store list order without changing public list fields

Add an internal `lists.position` integer in schema version 3.
Keep `TaskList` and MCP list result fields unchanged.
Pin the lowest-ID list as the original default list even after a rename.
Trash remains a virtual last entry and cannot move.

Migration assigns the default list position 0 and assigns the remaining lists positions in their current lowercase-name order.
New lists append after existing ordinary lists. Rename preserves position.
Task IDs, fields, and versions are unchanged by the version 2 to 3 migration.

`Store::move_list(id, down)` checks write and list-scope permissions before moving a list.
The method finds the current adjacent nondefault list inside an immediate transaction and swaps their positions.
Moving the default list or moving beyond the first or last movable list is a no-op.
An unknown list ID is an error.
The interface refreshes by list ID after a move so selection follows the same list.

### Expire transient status messages after five seconds

Record a monotonic timestamp whenever an action result or error replaces the current message.
Clear that message after five seconds, including when the app receives no input.
Navigation does not restart its lifetime. A new message starts its own five-second lifetime.
Keep external-update state separate from transient messages and preserve its existing explicit refresh acknowledgment.
Keep errors ahead of the external notice until the error expires, then reveal the notice again.
Test time boundaries with supplied timestamps rather than sleeping in tests.

## Risks / Trade-offs

| Risk | Response |
| --- | --- |
| Wrapping puts the cursor or click target on a different character | Use one layout and test Unicode, spaces, long words, resizing, and soft boundaries. |
| Headings change selection offsets | Verify rendered task rows and mouse targets after scrolling. |
| Concurrent list edits reorder stale neighbors | Find neighbors inside the write transaction. |
| A profile is opened by an older binary | Keep old processes closed during the user's upgrade. Test only isolated profiles during development. |

## Verification

Test list migration, boundaries, rename, persistence, scope restrictions, and concurrent list moves.
Test full-width rendering and the exact styles of selected rows and their headings.
Test description wrapping, cursor movement, mouse hit mapping, and unchanged saved text.
Run the full suite, formatting, Clippy, release build, and OpenSpec validation.
Exercise the changed controls in a temporary terminal profile and complete independent review.

## Evidence

- Read from source: wide navigation currently uses a 62/38 column split.
- Read from source: one selected list item contains both a heading and the first task.
- Read from source: Description currently shares single-line horizontal cursor scrolling with other fields.
- Confirmed by the user: new lists append before Trash and Alt+Up/Alt+Down reorder ordinary lists.
