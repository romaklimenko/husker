## Context

Read from source: TextBuffer has a text string and byte cursor but no selection or history.
Description drawing already maps source bytes to wrapped visual rows.
All task result surfaces use planning::sort_tasks.
Four open Husker inbox tasks request ordering, task IDs, a renamed heading, and discard confirmation.

## Decisions

### Use a shared text buffer for editing

Extract TextBuffer into its own internal module so input behavior can be tested independently from drawing.
Keep a byte cursor and optional selection anchor at grapheme boundaries.
Reuse the wrapped layout for vertical movement and selection rendering in Description.
Other inputs retain their current unwrapped layout.
Treat Unicode letters, numbers, and underscores as word runs, whitespace as another class, and punctuation or symbols as another class.
Ctrl+Left skips preceding whitespace then the preceding run. Ctrl+Right consumes the current run and following whitespace.
Deletion uses those same word boundaries.
Keep up to 100 undo snapshots per buffer. A snapshot includes text, cursor, and selection.
Each insertion or paste event is one undo step. Movement alone creates no history.
New edits after undo clear redo history. Selection replacement is one edit.

### Confirm only changed task drafts

Capture the initial field strings when the task editor opens, including the supplied list for a new task.
Compare current field strings with that baseline when Esc or Ctrl+G requests dismissal.
Show a discard confirmation only if those strings differ.
Default to keeping the draft. Esc from the prompt keeps the draft too.
Undo back to the original strings makes dismissal immediate again.
Preserve the existing task version so confirmation never bypasses stale-save checks.

### Keep grouping ahead of status

The user confirmed open-first ordering within each planning group.
Sort by bucket, open versus closed, due date, saved position, then ID.
Done and cancelled share the closed category. Groups retain their existing order.
Reordering can only swap tasks with the same exact due date and the same open/closed category.
Check that category inside the storage write transaction using current task values.

### Keep stable query values

Change the translated group heading to Sometime or Engang.
Keep the internal Undated bucket and the existing due:none and due:undated filters.
Show the existing task's ID in the editor header. A new draft has no ID until saved.

## Non-goals

This batch implements the proposed first set of keyboard editing controls.
Mouse drag selection, double-click word selection, system clipboard integration, and automatic copy-on-selection remain outside this batch.
F4 still provides terminal-native copying. No database migration or new MCP tools are needed.

## Verification

Test Unicode navigation, selection replacement, wrapped selection rendering, undo/redo, and discarded versus retained drafts.
Test ordering across dates, statuses, and planning groups through shared logic, CLI, and MCP.
Run formatting, Clippy, the full test suite, release build, isolated terminal checks, and independent review.
