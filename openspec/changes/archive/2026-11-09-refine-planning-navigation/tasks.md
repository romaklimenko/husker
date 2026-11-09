## 1. Specification

- [x] 1.1 Record the approved 0.11.0 contract in proposal, design, and capability deltas.
- [x] 1.2 Strictly validate refine-planning-navigation before implementation.

## 2. Implementation

- [x] 2.1 Update src/config.rs for configurable first/last actions, mode availability, defaults, overrides, and conflict tests.
- [x] 2.2 Update src/i18n.rs for English/Danish planning labels and action hints, preserving external identifiers.
- [x] 2.3 Update src/ui.rs for wrapping keyboard selection and first/last jumps in every specified mode, with boundary and scrolling tests.
- [x] 2.4 Update src/ui.rs to hide planned-date suffixes only in History and test known and unknown closure dates.
- [x] 2.5 Verify bounded mouse wheels, text editing, Help/Copy text scrolling, and manual reordering through regression tests.
- [x] 2.6 Update Cargo.toml/Cargo.lock to 0.11.0 and README.md/docs for the implemented labels and controls. Verify commands with isolated profiles.

## 3. Verification

- [x] 3.1 Run formatting, Clippy, all tests, a locked release build, and isolated terminal/CLI checks.
- [x] 3.2 Complete independent review and address findings.
- [x] 3.3 Archive the change and strictly validate all canonical capabilities.
