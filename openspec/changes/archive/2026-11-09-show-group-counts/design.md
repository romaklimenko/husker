## Context

The task view builds headings while iterating the visible tasks and inserts a heading whenever the group label changes. Held rows keep their previous open heading.

## Goals / Non-Goals

**Goals:**

- Show the number of rows under each heading using the same `(n)` form as the list picker.

**Non-Goals:**

- No totals in the header, no counts in Trash, and no change to CLI or MCP output.

## Decisions

### Count consecutive rows per label in a first pass

Compute each visible task's heading label once, then count the run of rows with the same label when a heading starts.
The count reflects the rows the user sees, so a held closed row counts under the open heading it keeps.
Alternative rejected: counting by planned group from the store, which would disagree with held rows and with History labels.

## Risks / Trade-offs

- [Heading text grows by a few cells] → Headings are not selectable and already fit the row width.
