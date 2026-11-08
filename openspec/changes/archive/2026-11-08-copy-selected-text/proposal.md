## Why

Copy selected input text to the system clipboard with Ctrl+C.
Keyboard selection already supports editing, but users currently need a separate copy view to copy the same text.

## What Changes

- Add a configurable `copy_selection` action with Ctrl+C as its default.
- Copy the active selection from task fields, filter input, and list-name forms.
- Preserve the draft, selection, cursor, and undo history on success or failure.
- Show localized success or error feedback and keep F4 available for terminal-native copying.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `terminal-interface`: Copy selected source text through the desktop clipboard.
- `profiles-localization`: Configure the new input-only shortcut.

## Impact

Release version 0.6.0 keeps schema 3 and all CLI/MCP contracts unchanged.
Clipboard access starts only when the user requests copying a nonempty selection.
Existing terminal-provided paste handling stays unchanged.
No installation, personal task migration, or client configuration change is required during development.
