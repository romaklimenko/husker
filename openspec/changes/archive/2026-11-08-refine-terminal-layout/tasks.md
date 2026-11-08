## 1. Storage

- [x] 1.1 Add schema 3 list positions and atomic upgrades from versions 1 and 2; verify task preservation, initial order, and read-only refusal.
- [x] 1.2 Implement append, rename preservation, and atomic current-neighbor list moves; test pinned defaults, boundaries, persistence, and access restrictions.

## 2. Terminal

- [x] 2.1 Remove the description sidebar and render selected task rows separately from group headings; verify full width, styles, and mouse targets.
- [x] 2.2 Add natural Description wrapping with shared drawing, cursor, movement, and click mapping; test Unicode, long words, whitespace, newlines, and resize.
- [x] 2.3 Enable existing move shortcuts in the list chooser and preserve selection by ID; update localized help and tests.
- [x] 2.4 Expire transient action results and errors after five seconds without input; preserve external-update notices and test timing, replacement, and navigation.

## 3. Verification and completion

- [x] 3.1 Update affected CLI/MCP integration checks, README, and guides for saved list ordering and schema 3.
- [x] 3.2 Run formatting, Clippy, the full suite, release build, and terminal checks against isolated profiles.
- [x] 3.3 Complete independent reviews and address findings with affected verification.
- [x] 3.4 Validate and archive the completed change, then validate canonical specs.
- [x] 3.5 Recheck the Husker inbox and mark only verified tasks done using current versions.
