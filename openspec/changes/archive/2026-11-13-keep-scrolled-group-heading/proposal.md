## Why

Keep the first visible task group's heading on screen while scrolling.
Long task lists can hide the heading, leaving tasks without their planned-date context.

## What changes

1. Keep the first visible group's existing heading above its tasks in ordinary views and History.
2. Keep selection, mouse targets, and page movement aligned with the displayed rows.
3. Preserve English and Danish labels, dates, and group counts.

## Capabilities

### New capabilities

None.

### Modified capabilities

- `terminal-interface`: Keep group headings visible when task lists scroll.

## Impact

1. Update navigation rendering and regression tests in `src/ui.rs`.
2. Update README and usage guidance.

No storage migration, configuration change, new dependency, CLI change, or MCP change is required.
