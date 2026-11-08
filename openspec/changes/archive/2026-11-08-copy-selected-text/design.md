## Context

Read from source: TextBuffer exposes a selected source byte range and the terminal event loop already handles paste events.
The existing CopyText action opens an F4 plain-text view. It does not write the clipboard.

## Decisions

### Separate clipboard writing from editing

Introduce a TextClipboard interface with one set_text operation.
The application owns a native implementation with lazy initialization and a retained handle.
Tests inject a recording or failing implementation so automated tests do not overwrite the user's clipboard.
The native handle must remain alive while Husker runs because some desktop systems serve clipboard contents from the owning process.

### Copy the active selection through a configured action

Add CopySelection to task editor, filter, and list-name modes only.
Use the selected source string without visual wrapping or interface labels.
No selection means no clipboard call and no buffer change.
Copy does not create an undo entry, clear a selection, or make a clean draft dirty.
Report success only after the clipboard write succeeds. Preserve the draft and selection on failure.
Keep configured-action conflict validation and existing terminal paste behavior.

### Keep desktop and terminal copying available

Use the local desktop clipboard supported by the native backend.
Do not add OSC 52 output or claim synchronization with a remote SSH client's clipboard.
Keep F4 and terminal-native copying available when native clipboard access fails.

## Non-goals

Cut, direct clipboard reads, automatic copy-on-selection, mouse drag selection, and changes to Ctrl+V are outside this change.
No database schema, task model, CLI output, or MCP protocol changes are needed.

## Verification

Test exact Unicode and multiline source copying, all input modes, no selection, failures, shortcut configuration, and preserved editing state.
Run formatting, Clippy, full tests, release build, and isolated terminal checks.
Verify native clipboard round-trip on the available desktop without exposing prior clipboard contents.
