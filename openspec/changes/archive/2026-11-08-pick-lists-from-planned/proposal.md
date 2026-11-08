## Why

Make Planned the top navigation level and choose a list from a popup.
A list shows the same planned task groups restricted to that list.
Esc should return from a list to Planned, with the user's previous position restored.

## What Changes

- Open a modal list picker with the existing `lists` action, bound to `l`.
- Show All tasks first, ordinary lists in their saved order, and Trash last.
- Show scoped task headings as `Planned > Work` or `Planned > Trash`.
- Restore the Planned filter, task selection, and scroll when returning from a child view.
- Keep list creation, renaming, reordering, help, refresh, and mouse support in the picker.
- Preserve editor save and discard behavior, including confirmation of unsaved edits.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `terminal-interface`: Replace the separate list screen with a modal picker and define parent navigation.
- `profiles-localization`: Describe list mode as the picker while retaining its existing bindings and validation.

## Impact

Release version 0.7.0 keeps schema 3 and the CLI and MCP contracts unchanged.
All tasks is a virtual picker entry and adds no stored list or reserved list name.
Existing action names and configured shortcuts remain valid.
No installed executable or personal profile changes are required during development.
