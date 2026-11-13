## 1. Rendering and regression coverage

- [x] 1.1 Update `src/ui.rs` to retain the first visible group heading and share the viewport mapping with mouse targets and page counts.
- [x] 1.2 Add rendering regressions for End/Home, scrolling, resizing, English and Danish, History, Trash, and small task areas.
- [x] 1.3 Update README and usage guidance and review the capability spec with the docs writer.

## 2. Verification and delivery

- [x] 2.1 Run formatting, Clippy, Rust tests, and strict OpenSpec validation.
- [x] 2.2 Exercise the CLI and terminal UI against a temporary profile and obtain an independent review.
- [x] 2.3 Build the release executable and verify version 0.15.1 before installation.

## Verification evidence

The End/Home regression failed before the fix and passed afterward.
Formatting, Clippy, strict OpenSpec validation, and all 312 Rust tests passed.
The first full run hit an existing random-UID-dependent sync count assertion.
The complete rerun passed. Sync code and its tests remain unchanged.

A macOS pseudo-terminal ran the release executable with a temporary 29-task profile.
English and Danish checks covered startup, End/Home, wrapping, paging, resizing, and exit.
The Tomorrow group had four tasks and retained its label, date, and count.
CLI reads confirmed navigation did not change tasks.
An independent source review found no actionable issues.
Windows and Linux terminal interaction were not verified in this change.
