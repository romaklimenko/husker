## 1. Storage and task rules

- [x] 1.1 Simplify the model and atomically migrate schema 1 to 2 while preserving tags as description text; verify data preservation and rollback.
- [x] 1.2 Record local status dates and preserve them across other mutations; test real transitions, reopen, stale writes, and old unknown dates.
- [x] 1.3 Sort lists alphabetically, preserve the default creation list, and sort tasks by due date; verify scope and ordering.
- [x] 1.4 Replace tag/readiness search with literal text and one undated group; test today's closed tasks, explicit statuses, and midnight.

## 2. Terminal interaction

- [x] 2.1 Add mouse selection, opening, scrolling, and editor focus with correct rendered hit regions and capture cleanup.
- [x] 2.2 Add the borderless copy view and capture toggle; verify multiline text, scrolling, and editor preservation.
- [x] 2.3 Change save and cancel defaults, remove obsolete fields/actions, and update English and Danish text.
- [x] 2.4 Add manual refresh and external-change notices without replacing unsaved edits; retain list selection on return.

## 3. Automation and guidance

- [x] 3.1 Update CLI and MCP fields, arguments, search guidance, and integration tests for the simplified model.
- [x] 3.2 Update AGENTS.md to use the scoped Husker inbox and preserve legacy draft files.
- [x] 3.3 Update README and guides with exact controls, migration steps, query behavior, and verified examples.

## 4. Verification and completion

- [x] 4.1 Run formatting, Clippy, the full suite, release build, and terminal checks with isolated profiles.
- [x] 4.2 Complete independent reviews and address findings with affected verification.
- [x] 4.3 Validate and archive the completed change and validate canonical specs.
- [x] 4.4 Recheck the Husker inbox and mark only verified, unchanged tasks done through scoped MCP.
