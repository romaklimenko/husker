## 1. Implementation

- [x] 1.1 Add a lazy clipboard backend and injectable writer interface with retained ownership.
- [x] 1.2 Add configurable Ctrl+C input copying, localized feedback, and visible hints.
- [x] 1.3 Test exact selection copying, no-op and failure behavior, preserved history, modes, and shortcut conflicts.

## 2. Verification

- [x] 2.1 Update README and guides and verify examples.
- [x] 2.2 Run formatting, Clippy, all tests, release build, and clipboard/terminal checks.
- [x] 2.3 Complete independent review and resolve findings.
- [x] 2.4 Validate and archive the change, then validate canonical specs.

## 3. Spec conformance audit

- [x] 3.1 Audit all six canonical capability specs and archived task lists for unfinished requirements.
- [x] 3.2 Fix alternate Shift+Tab events in discard confirmation and verify both event forms preserve the draft.
