## Context

Key bindings live in src/config.rs as actions with per-mode availability. The terminal in src/ui.rs dispatches configured actions per screen and draws hints from binding names.
Help is one flat list. Footers show four or five actions. Closed tasks resort immediately because every refresh recomputes the visible results.

## Goals / Non-Goals

**Goals:**

- Make every screen show its most useful keys in readable form.
- Shorten common task operations: capture, move, undo, filter clearing.
- Keep configuration compatible: every new key is an action with the existing override and conflict rules.

**Non-Goals:**

- No multi-level undo or redo in navigation.
- No change to CLI commands, MCP tools, JSON fields, or the database schema.
- No change to text-editing keys inside fields.

## Decisions

### Format keys at the binding

Add a display name per binding, computed from the parsed modifiers and key code. The existing lookup returns display names joined by a slash. A new primary-key lookup returns the first binding for footers.
Danish localizes Space and the arrow names only. Other key names are the same on Danish keyboards.
Alternative rejected: formatting in each hint call site, which is where the inconsistency came from.

### Build Help from sections

Help is a list of sections, each a list of entries. An entry is a configured action or a built-in key with a fixed label.
The section for the screen that opened Help goes first. Help scrolls as a paragraph by offset instead of a selected row, so headings are never highlighted.
Mouse clicks in Help do nothing. The wheel scrolls.

### Pack footer hints by width

Each screen lists its hint entries in priority order. A greedy packer fills up to two lines at the current width and drops the rest.
The footer height is the number of packed lines plus the border and message rows. This keeps Danish labels and long custom bindings readable at 80 columns.

### Hold closed tasks by neighbor

Marking a task done or cancelled records a hold: the task ID, its new version, and the ID of the visible task above it.
Refresh computes the sorted results and then moves each held task back after its recorded neighbor. The row keeps its previous open heading, computed as if the task were still open, and is drawn dimmed.
Holds end on view, filter, editor, History, manual refresh, and date changes, and whenever the task version changes again, the task reopens, or it leaves the results.
Alternative rejected: a timer, because the user asked for the task to move when they come back to the view.

### Undo as one replayable entry

Each recorded task action stores the task ID, the version it produced, and the previous value: status with its closure date and timestamp, due date, Trash flag, list ID, or the reorder partner and its version.
Undo re-reads the task, refuses on a version mismatch, and writes the previous value through a guarded store method.
Two store methods are added. A list-only mutation mirrors the due-only mutation. A status-restore mutation writes a supplied status, date, and timestamp. Both validate access, scope, existence, and version inside an immediate transaction.
Alternative rejected: reusing the full task update for moves, because it rewrites every field and renormalizes text.

### Move through a dedicated picker screen

The move-task action opens a picker screen that reuses the editor list picker key mode and popup drawing, but draws the task view behind it.
Choosing writes through the list-only mutation with the originally read version.

### Enter in single-line fields

Enter is handled structurally in the editor: Title and Planned date invoke Save, Description inserts a newline, List opens its picker. Save keeps Ctrl+S and Ctrl+Enter as its configurable keys.

## Risks / Trade-offs

- [New default keys conflict with custom bindings] → Existing validation reports the conflict. The guide shows how to rebind or disable.
- [Held rows can confuse a reorder] → Reordering already requires open tasks, so a held closed row is skipped as before.
- [Undo after an external edit] → The version check refuses and reports. Nothing is written.
- [Two hint lines reduce the task area by one row] → The packer uses one line when everything fits.
