# Build and check Husker

This page is for contributors who need to build Husker and verify a change.

## Run the checks

Run these commands from the repository directory:

```text
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release --locked
openspec validate --all --strict
```

The test suite uses temporary profiles. It does not read or change your
`~/.husker` profile. Keep a separate profile for interactive checks too:

```text
cargo run -- --profile ./target/manual-profile
```

Run the installed copy while developing so the repository's executable stays
available for rebuilding:

```powershell
& "$env:USERPROFILE\.cargo\bin\husker.exe"
```

On Windows, `cargo run` and direct launches from `target/` lock that build
executable until the process exits. A rebuild then fails with
`Access is denied (os error 5)`.
The installed copy does not lock the repository's build executable.
Close installed Husker processes and disconnect MCP clients before replacing
the installed copy with `cargo install --path . --locked`.

Use only temporary profiles for migration tests. Starting version 0.5.0
against a schema version 1 or 2 profile performs a migration on writable access.
Versions 0.4.0 and 0.5.0 both use schema version 3.
Follow the [upgrade instructions](usage.md#update-an-existing-profile) for a real profile.

## Checks for version 0.5.0

Version 0.5.0 passed these checks on November 8, 2026:

| Check | Result |
| --- | --- |
| Unit tests | 105 passed, including Unicode word movement, selection, replacement, undo/redo, discard confirmation, task IDs, planning order, and reorder boundaries. |
| CLI integration tests | 8 passed against isolated profiles. |
| MCP integration tests | 7 passed through child processes, including scoped pagination with open tasks first. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.5.0. |
| OpenSpec | All six capability specs passed strict validation after archiving the editor controls change. |
| Guide examples | Verified task commands, open-first ordering, both no-date filters, the stable `undated` CLI token, versioned edits, Trash, list ordering, and default keys. |

Interactive checks ran in an 80 by 24 pseudo-terminal with a scratch profile
created by version 0.4.0. Sometime showed an open task before done and cancelled
tasks. Opening the task displayed `Edit task #2` in the heading.

Injected control-key sequences moved by word and selected `beta` in Description.
Typing replaced the selection. Ctrl+Z and Ctrl+Y undid and restored the edit.
Esc opened a confirmation with No selected. Enter returned to the unchanged
draft. Ctrl+S saved the expected description and incremented the task version.
A CLI read confirmed the saved text, including its newline and Danish characters.

Ctrl+A selected the whole Description field for replacement. Esc, Right, and
Enter explicitly discarded that later change. A CLI read confirmed the stored
text and version stayed unchanged. Reopening the task and pressing Esc without
editing closed the editor immediately. Quit restored the terminal settings.

Unit tests verified that one paste event is one undo step. The pseudo-terminal
injection did not establish that paste behavior, so the interactive checks
cover typed edits only.
The checks ran on Windows with Rust 1.91.1 and isolated profiles.
Physical keyboard delivery of modified shortcuts, Linux, and macOS were not
tested in this session.

## Checks for version 0.4.0

Version 0.4.0 passed these checks on November 8, 2026:

| Check | Result |
| --- | --- |
| Unit tests | 87 passed, including atomic migrations, saved list order, wrapped text and cursor movement, Unicode, mouse positions, heading styles, and message expiration. |
| CLI integration tests | 7 passed against isolated profiles. |
| MCP integration tests | 6 passed through child processes. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.4.0. |
| OpenSpec | All six capability specs passed strict validation after archiving the terminal layout change. |
| Guide examples | Verified help, default shortcuts, task commands, filters, status changes, Trash, versioned edits, list ordering, and the default list after a rename. |

Interactive checks ran in an 80 by 24 pseudo-terminal with a scratch profile
created by version 0.3.0. Migration preserved a task's full description,
due date, and version. Its description wrapped into three displayed rows,
followed by a paragraph separated by an existing newline.

An injected mouse click placed the cursor on a wrapped row. Ctrl+S saved the
inserted text at that position. A CLI read confirmed the exact description,
its original newline, and the new task version.
Clicking the Tomorrow heading left the selected task unchanged.
Unit tests checked full-width task rows and heading styles at 140 columns.

The migrated lists appeared as Inbox, Alpha, Beta, Zulu, and Trash.
Alt+Down on Alpha moved it below Beta and kept Alpha selected.
Attempts to move Inbox or Trash did nothing. Restarting Husker preserved
the order Inbox, Beta, Alpha, Zulu, and Trash.
Quit restored the terminal's normal screen and input settings.

F5 displayed a refresh result. Navigating to the list chooser did not extend
the message's lifetime. The footer cleared during six seconds of idle time.
A task added by a separate CLI process triggered an external-update notice.
The notice remained through more than twelve seconds of idle time.
F5 acknowledged that notice, and the replacement refresh result then expired.

The checks ran on Windows with Rust 1.91.1 and isolated profiles.
Mouse checks used injected terminal events, not physical mouse gestures.
Linux and macOS were not tested in this session.

## Checks for version 0.3.0

Version 0.3.0 passed these checks on November 8, 2026:

| Check | Result |
| --- | --- |
| Unit tests | 68 passed, including schema migration, stale edits, local status dates, filters, sorting, mouse hit regions, copy text, and refresh notices. |
| CLI integration tests | 7 passed against isolated profiles. |
| MCP integration tests | 6 passed through child processes. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.3.0. |
| OpenSpec | All six capability specs passed strict validation after archiving the workflow change. |
| Guide examples | Verified help, default shortcuts, task commands, literal hashtag searches, status visibility, and list ordering against the debug executable. |

Interactive checks ran in an 80 by 24 pseudo-terminal with a scratch profile
created by version 0.2.0. Migration preserved the description and appended
`#shopping`. The changed task received a new version.

Ctrl+S saved an added description line. Esc discarded a later unsaved change.
CLI reads confirmed both results. F4 displayed plain task text and disabled
mouse capture. A task completed by another connection remained visible with
an `[x]` marker and an external-update notice. F5 acknowledged the notice.

Injected terminal mouse events selected the drawn task and opened its editor
on a double-click. Lists appeared as Alpha, Inbox, Work, and Trash.
Returning from Work with Esc kept Work selected. Quit restored the main screen
and disabled raw input and mouse capture.

The checks ran on Windows with Rust 1.91.1. Tests used isolated profiles.
The installed app and personal profiles were not upgraded during development.
Physical mouse selection into the system clipboard was not tested.
Linux and macOS were not tested in this session.

## Checks for version 0.2.0

The MCP version passed these checks on November 8, 2026:

| Check | Result |
| --- | --- |
| Unit tests | 52 passed, including list restrictions, read-only storage, and list identity after a rename. |
| CLI integration tests | 6 passed against isolated profiles. |
| MCP integration tests | 6 passed through child processes. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.2.0 and includes the MCP options. |
| OpenSpec | All six capability specs passed strict validation after archiving the MCP change. |
| Setup guide | Verified MCP help and list commands against the debug executable. Verified Codex and Claude Code registration syntax with their local help and official guides. |

The MCP tests use the official Rust SDK as a client.
They cover both the `initialize` lifecycle and the newer `discover` lifecycle.
The tests check restricted list reads and writes, read-only access, paginated searches,
and task creation through completion, Trash, and restoration.
They also check stale versions and tasks moved to another list by a separate connection.
Protocol checks verify JSON-RPC output and shutdown after standard input closes.

The MCP checks ran on Windows with Rust 1.91.1.
Codex and Claude Code registration commands were checked without changing client settings.
Linux, macOS, and an interactive MCP session inside either client have not been tested.

## Checks for version 0.1.0

The first working version passed these checks on November 8, 2026:

| Check | Result |
| --- | --- |
| Unit tests | 45 passed. Covers storage, planning, search, configuration, localization, and terminal actions. |
| CLI integration tests | 6 passed. Runs commands against isolated profiles. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Release build | `cargo build --release --locked` passed. |
| Local installation | `cargo install --path . --locked --offline --root .scratch/install` passed. |
| Interactive terminal | English launch and quit passed. Danish task creation, multiline text, due date, description focus, discard, and help scrolling passed at 80 by 24 cells. |
| Persistence | The CLI returned the exact Danish title, two description lines, and date saved through the terminal. |
| OpenSpec | All five capability specs passed strict validation. |

The checks ran on Windows build 26300 with Rust 1.91.1. Linux and macOS
were not tested in this session. AltGr handling has a regression test based
on Crossterm's Windows key events. A physical Danish keyboard was not tested.

## Performance sample from version 0.1.0

The release executable searched 10,000 tasks and printed all matches as
JSON in a median 73.5 ms across five warm runs. This is a local measurement,
not a latency guarantee.

The fixture used one list and 10,000 open tasks outside Trash. Every task
had the `shopping` tag and the description `Fixture description with Danish: Køb æbler`.
Titles were `Task 1` through `Task 10000`. Two thirds of tasks had dates
from five days before today through 24 days after today. The rest were undated.

The measured command was:

```text
husker --profile FIXTURE_DIRECTORY --json tasks "#shopping æbler"
```

Timing included process startup, database reads, Unicode search, sorting,
and JSON output captured in memory. Terminal drawing and fixture creation
were outside the measurement.

| Run | Elapsed time |
| --- | --- |
| First run | 71.5 ms |
| Median of five later runs | 73.5 ms |
| Fastest later run | 62.5 ms |
| Slowest later run | 75.3 ms |

The machine reported `Intel64 Family 6 Model 189 Stepping 1, GenuineIntel`
and `Microsoft Windows NT 10.0.26300.0`. The benchmark used a local SQLite
database and a release build with thin link-time optimization.
