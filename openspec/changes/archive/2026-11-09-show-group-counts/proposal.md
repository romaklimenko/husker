## Why

A task view can hold many rows per date group, and the user asked to see how many tasks each group contains without counting rows.

## What Changes

1. Every group heading in a task view and in History ends with the number of task rows under it in parentheses, such as `Today - 2026-11-09 Mon W46 (3)`.
2. A held closed row counts under the heading it keeps.
3. Trash keeps no headings and no counts.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `terminal-interface`: Group headings include a row count.

## Impact

Release 0.12.1 keeps database schema 4 and every CLI command, JSON field, MCP tool, and filter.
Heading text changes. A script that matches heading text in the terminal must expect the count suffix.
