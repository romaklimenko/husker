## 1. Specification

- [x] 1.1 Complete and strictly validate all six capability deltas, including task retention and navigation compatibility.

## 2. Implementation

- [x] 2.1 Add atomic list-deletion previews and outcomes with retention, version, race, protection, and scope tests.
- [x] 2.2 Separate due buckets from completion groups and add filtered, dated History results with regression tests.
- [x] 2.3 Add All tasks compatibility, History, reopening, deletion-confirmation actions, and localized labels.
- [x] 2.4 Implement History navigation, completion focus, list deletion, external-deletion fallback, and default-list/hint improvements.
- [x] 2.5 Update docs and verify examples for version 0.8.0, including retention versus an activity log.
- [x] 2.6 Add atomic active-config language persistence in config/main with tests for explicit paths, missing implicit configs, preserved comments/overrides, external edits, and failure preservation.
- [x] 2.7 Add the localized F3 picker and configurable controls in ui/config/i18n with tests for modal isolation, cancellation, immediate switching, and complete draft/navigation preservation.

## 3. Verification

- [x] 3.1 Run formatting, Clippy, all tests, release build, and isolated CLI and terminal checks, including language persistence across restart.
- [x] 3.2 Complete independent review and resolve findings.
- [x] 3.3 Archive the change and validate all canonical specs.
