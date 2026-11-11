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

Use only temporary profiles for migration tests. Versions 0.9.0 through
0.14.0 use schema version 4. Updating between them requires no migration.
Starting the current version against a schema version 1, 2, or 3 profile
migrates it to version 4 on writable
access. Versions 0.4.0 through 0.8.0 use schema version 3 and cannot open an
upgraded profile. Read-only connections require the writable upgrade first.
Follow the [upgrade instructions](usage.md#update-an-existing-profile) for a real profile.

## Delete tasks permanently in the store

Three `Store` operations in `src/store.rs` remove task rows. Every other
task write keeps the row. That includes `set_deleted`, which moves a task
to or from Trash.

| Operation | What it does |
| --- | --- |
| `purge_task(id, version)` | Deletes one task in Trash if its stored version still equals `version`. Returns `TaskPurgeOutcome::Deleted`, `Changed(task)` with the current task, or `NotInTrash` when the task is outside Trash or no longer exists. |
| `preview_empty_trash()` | Returns a `TrashPreview` with the number of tasks in Trash and a snapshot of each task's ID and version. |
| `empty_trash(&preview)` | Deletes every task in Trash if Trash still matches the preview. Returns `EmptyTrashOutcome::Emptied(count)`, or `Changed(preview)` with a fresh preview. |

`purge_task` and `empty_trash` check and delete inside one immediate SQLite
write transaction. Another writer therefore cannot change the task or Trash
between the check and the delete. A `Changed` or `NotInTrash` result writes
nothing. Show the fresh task or preview to the user and ask for
confirmation again before you retry.

All three operations reject read-only and list-scoped stores before they
read Trash. A list-scoped store fails with
`A list-scoped store cannot delete tasks permanently.` A read-only store
fails with `This store is read-only.` Only the terminal interface calls
these operations. No CLI command or MCP tool calls them.

Task IDs come from an `AUTOINCREMENT` column, so a deleted ID is never
reused. Permanent deletion needs no schema change. The schema stays at
version 4.

## Checks for version 0.14.0

The task-capture update completed these checks on November 11, 2026:

| Check | Result |
| --- | --- |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Unit tests | 234 passed. New tests cover the suggested list: the most common of the 10 newest tasks, ties, the window after Trash filtering, closed and moved tasks, inaccessible lists, and the default-list fallback. Others cover Tab and Shift+Tab order with wrapping, the field positions and mouse hits, the two-Tab capture with `t`, and the `Sometime` and `Engang` placeholders. |
| Review | An independent review found no code defects. It found two rules that no test checked, a task-storage spec sentence that the change contradicted, and three inaccurate sentences in the spec delta, the guide, and the README. All were fixed. Removing either rule from the code now fails a test. |
| CLI integration tests | 9 passed against isolated profiles. |
| MCP integration tests | 8 passed through child processes. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.14.0. |
| Manual CLI check | In a temporary profile, `husker add` without `--list` still created the task in Inbox, list 1. |
| Live terminal | The release executable ran in an 80 by 24 Windows pseudo-terminal against a temporary profile. With 3 of the 6 newest tasks in Work, `n` opened the editor with Work in List and `Sometime` in Planned date. Tab focused List, a second Tab focused Planned date and showed the preset hint, and `t` and Enter created the task under Today in Work. Shift+Tab from Title focused Description. The Danish run showed `Liste`, `Planlagt dato`, and `Engang`. `q` exited. |
| OpenSpec | The change validated strictly, and all six capability specs passed strict validation after archiving. |

Mouse clicks were covered by automated tests only. Linux and macOS were not tested in this session.

## Checks for version 0.13.0

The permanent-deletion update completed these checks on November 9, 2026:

| Check | Result |
| --- | --- |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Unit tests | 225 passed. They cover deleting one Trash task, emptying Trash, stale and restored tasks, a changed Trash with an equal count, ID reuse, read-only and list-scoped stores, both confirmations with No as the default, refresh while a confirmation is open, the picker's Trash entry, undo after deletion, long titles, footer hints, Help, and the `D` key conflict. |
| CLI integration tests | 9 passed against isolated profiles. |
| MCP integration tests | 8 passed through child processes. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.13.0. |
| Manual CLI check | A temporary profile showed `Trash (1)` after `trash`, `config --example` listed `empty_trash = ["D"]`, and `new_task = ["D"]` failed with the `D` conflict until `empty_trash = []` was added. |
| Rendered screens | The Trash footer, both confirmations in English and Danish, the emptied Trash, and the picker on Trash were rendered at 80 by 24 cells and inspected. |
| OpenSpec | The change validated strictly, and all six capability specs passed strict validation after archiving. |

No live pseudo-terminal session was driven. Linux and macOS were not tested in this session.

## Checks for version 0.12.1

The heading-count update completed these checks on November 9, 2026:

| Check | Result |
| --- | --- |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Unit tests | 214 passed, including a rendered check that every heading in task views and History ends with its row count in English and Danish, that a held row counts under its old heading, that a filter changes the counts, and that Trash shows no headings. |
| CLI integration tests | 9 passed against isolated profiles. |
| MCP integration tests | 8 passed through child processes. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.12.1. |
| OpenSpec | The change validated strictly, and all six capability specs passed strict validation after archiving. |

No live pseudo-terminal session was driven. Linux and macOS were not tested in this session.

## Checks for version 0.12.0

Version 0.12.0 completed these checks on November 9, 2026:

| Check | Result |
| --- | --- |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Unit tests | 213 passed, including key display names, the new actions and their conflicts, list-only and status-restore mutations, held rows and their ordering, undo of every recorded action, the move picker, Enter saving, filter clearing, paging, confirmation letters, footer packing, and grouped Help. |
| Review | An independent review reported eight findings. Hold ordering for chained rows, reorders next to held rows, hold release at a date change, paging by task rows, undo of editor saves, and a Help row that depends on the Enter binding were fixed and covered by tests. |
| CLI integration tests | 9 passed against isolated profiles. |
| MCP integration tests | 8 passed through child processes. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.12.0. |
| OpenSpec | The change validated strictly before and after implementation. All six capability specs passed strict validation after archiving. |
| CLI examples | The release executable created lists and tasks, completed a task, printed JSON, and printed the example configuration against an isolated profile. A configuration binding `history` to `m` was rejected with the conflict message, and adding `move_task = []` made it load. |

The terminal screens were rendered through the test backend at 80 by 24
cells with sample tasks in every date group, in English and Danish. The
inspected screens were All tasks with a task held after Space, the filter
input with its syntax hint, the move picker, Help from navigation and from
the picker, the cleared-date and Undone messages, the new-task editor, the
discard prompt, the list picker, the list-deletion confirmation, a list
view, History, and Trash. Footers fit in two lines at 80 columns in both
languages for All tasks. The Danish list view drops the last footer entry at
exactly 80 columns, as the packing rule allows.

No live pseudo-terminal session was driven in this session, so physical
key delivery of Ctrl+C, PageUp, PageDown, and Shift letters was not tested
on Windows. Mouse gestures were covered by automated tests only. Linux and
macOS were not tested in this session.

## Checks for version 0.11.1

The Left/Right alias update passed all 213 tests on November 9, 2026:
196 unit tests, 9 CLI tests, and 8 MCP tests. Formatting, all-target Clippy
with warnings denied, and all six strict OpenSpec validations passed.
The debug executable reported 0.11.1. Isolated CLI checks confirmed the
new defaults, Left/Right conflicts, and the documented override remedies.
All 15 local documentation links and 5 JSON/TOML examples passed validation.

An 80 by 24 Windows pseudo-terminal check confirmed that Right selected the
last task and Left selected the first. In Description, Left and Right still
moved the text cursor. Typing `ABC`, Left, `X`, Right, and `Y` produced
`ABXCY`. Ctrl+S saved that text, and a separate CLI read confirmed version 2.
Quitting restored the terminal. These checks used isolated profiles.
Linux and macOS were not tested in this session.

## Checks for version 0.11.0

Version 0.11.0 completed these checks on November 9, 2026:

| Check | Result |
| --- | --- |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.11.0. |
| OpenSpec | All six capability specs passed strict validation after archiving the planning and navigation change. No active changes remain. |
| Unit tests | 196 passed, including keyboard wrapping, boundary jumps, History date display, configuration, and bounded movement regressions. |
| CLI integration tests | 9 passed against isolated profiles. |
| MCP integration tests | 8 passed through child processes. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Guide examples | Documented task/list commands, stale-version guards, strict external date input, and MCP help passed against an isolated profile. |
| Shortcut compatibility | Verified First/Last defaults, conflicts with custom `g`, `G`, Home, and End bindings, and the documented rebinding or disabled-action remedies. Existing `h`, `l`, and date-preset defaults stayed unchanged. |
| Date compatibility | JSON retained `due`. `due:overdue` matched both open and completed tasks with past planned dates. `due:earlier`, `due:tidligere`, and `planned:today` were rejected without task changes. |
| Documentation | All 15 local links and anchors resolved. All 5 JSON and TOML examples parsed. |

An 80 by 24 Windows pseudo-terminal check displayed Earlier and Planned
date in English and Tidligere and Planlagt dato in Danish. Task selection
wrapped with `k` and `j`. Home, End, and `G` selected boundary tasks.
History showed the completion-day heading without the future planned-date
suffix. Opening that task's editor still showed `2026-11-13`, although the
task was completed on `2026-11-09`.

In the navigation picker, `G` selected Trash and `j` wrapped to All tasks.
The editor picker wrapped through ordinary lists with `j` and `k`.
The language picker wrapped with `k` and `j`, and `G` followed by Enter
saved Danish. Quitting restored the terminal. A separate CLI read confirmed
the original task JSON and version 2 were unchanged, and configuration
contained `language = "da"`.

These checks used an isolated Windows profile. Mouse interaction was not
checked manually. Linux and macOS were not tested in this session.

## Checks for version 0.10.0

Version 0.10.0 completed these checks on November 8, 2026:

| Check | Result |
| --- | --- |
| Unit tests | 188 passed, including date parsing, dated headings, list-choice history, modal state, and mouse hit regions. |
| CLI integration tests | 9 passed against isolated profiles. |
| MCP integration tests | 8 passed through child processes. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.10.0. |
| Guide examples | Task commands, version checks, Trash/restore, list rename identity, picker bindings, and MCP help passed against an isolated profile. |
| Strict external dates | CLI add/edit and query input rejected `20261109`, `261109`, and `1109` without changing or creating tasks. The existing `due:soon` and `due:next` boundaries passed. |
| Profile compatibility | Opening a version 0.9.0 scratch profile with 0.10.0 preserved every task JSON field. Both versions use schema 4. |
| OpenSpec | All six capability specs passed strict validation after archiving the date and list editing change. No active changes remain. |
| Documentation | All 15 local links and anchors resolved. All 5 JSON and TOML examples parsed. |

An 80 by 24 Windows pseudo-terminal displayed actual dates on Today,
Tomorrow, and Done today headings. Future tasks appeared under separate
November 10 and November 20 headings without Soon or Next labels.
The empty Due date placeholder and the full list of accepted date formats
fit in the editor.

Entering `1112` and pressing Tab displayed `2026-11-12`. Enter on List
opened a popup with ordinary lists and the default marker. The popup
excluded All tasks and Trash. Choosing Home changed only the draft.
A separate CLI read confirmed that the stored task still belonged to Work,
had no due date, and retained version 1 before Save.

The terminal checks also verified English and Danish date headings, the
placeholder and format hint, and normalization from both four-digit and
six-digit entries. Ctrl+Z undid a List choice. A temporary configuration
bound Save to F6 for a persistence check. The following CLI read confirmed
the selected list and due date were saved together.

A separate Danish editor check entered raw `20261114` and pressed the
default Ctrl+S shortcut without leaving Due date first. The editor returned
to navigation. A CLI read confirmed `2026-11-14`, the selected Home list,
and the unchanged title, description, and status. All three compact input
lengths were exercised manually: four and six digits normalized on Tab,
and eight digits saved directly. Danish History showed I dag with its actual date.

All terminal processes exited cleanly. These checks used Windows and
isolated profiles. Linux and macOS were not tested in this session.

## Checks for version 0.9.0

Version 0.9.0 passed these checks on November 8, 2026:

| Check | Result |
| --- | --- |
| Unit tests | 170 passed, including calendar presets, due-only writes, whole-field undo, timestamp transitions and sorting, schema migration, and task-list labels. |
| CLI integration tests | 9 passed against isolated profiles. |
| MCP integration tests | 8 passed through child processes. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.9.0. |
| Guide examples | Verified preset defaults and overrides, conflicting Due date bindings, timestamp-aware ordering, repeated status, due edits, Trash, reopening, and MCP help. |
| Migration | An isolated profile created by version 0.8.0 upgraded from schema 3 to schema 4 with its existing fields, task versions, and list order unchanged. |
| OpenSpec | All six capability specs passed strict validation after archiving the presets change. No active changes remain. |
| Documentation | All 15 local links and anchors resolved. All 7 JSON and TOML examples parsed. |

The migration check created open, done, and trashed tasks with the old
executable. Read-only version 0.9.0 refused the old schema without changing
it. A writable `lists` command completed the upgrade. Every migrated task
received a null timestamp, including tasks with known completion dates.
The old executable then refused to open the upgraded profile.

Separate CLI completions recorded a lower-ID task after a higher-ID task
on the same local day. The newer completion appeared first, followed by
the older recorded time and the migrated task with an unknown time.
Repeating done preserved the original timestamp. Due-date edits and
Trash/restore also preserved it. Changing done to cancelled recorded a new
timestamp, and reopening cleared both status-change fields.

An 80 by 24 Windows pseudo-terminal check exercised `t`, `T`, `w`, and `x`
against an isolated profile. A preset that repeated the stored date kept
the task version unchanged. Editor presets supported one-step undo, and
Ctrl+S persisted the resulting date. English Due date hints were visually
inspected.

Alt+Down on a closed task left its position, version, and completion timestamp
unchanged. All tasks and all-list History displayed list prefixes, while the
Work view omitted them. Same-day closed tasks appeared newest first despite
their reversed task IDs.

The terminal check switched the interface to Danish, used `w` in the editor,
and saved. A release CLI read confirmed the due date was November 9.
Danish preset hints were checked by unit tests. They were not visually
inspected in this terminal session.

These checks used Windows and isolated profiles. Linux and macOS were not
tested in this session.

## Checks for version 0.8.0

Version 0.8.0 passed these checks on November 8, 2026:

| Check | Result |
| --- | --- |
| Unit tests | 149 passed, including finished-task groups, History, list-deletion previews and retention, shortcut compatibility, and language persistence. |
| CLI integration tests | 8 passed against isolated profiles. |
| MCP integration tests | 8 passed through child processes. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.8.0. |
| OpenSpec | All six capability specs passed strict validation after archiving the lifecycle change. No active changes remain. |
| Guide examples | Verified default and overridden shortcuts, duplicate root-action rejection, old and unknown-date completions, shared ordering, due filters, clearing dates, reopening, Trash, list JSON, and MCP help. |
| Documentation | All 15 local links and anchors resolved. All 7 JSON and TOML examples parsed. |

The CLI guide checks used a new scratch profile. Two fixture completion dates
were set to yesterday and unknown to verify retained records without waiting
for another day. Explicit status filters returned both records. Default
results excluded them. Clearing a completed task's due date preserved its
status and status-change date.

Interactive checks ran in an 80 by 24 Windows pseudo-terminal with isolated
profiles. The initial English view displayed All tasks, Today, Done today,
and Cancelled today. F3 opened the language picker. Choosing Dansk changed
labels and date text immediately and created `config.toml` with `language = "da"`.
Quitting and reopening Husker retained Danish. History opened with `h` and
displayed localized labels.

The Danish list-deletion dialog showed one unfinished task, one cancelled
task, and one task in Trash, with No selected initially. Confirming deletion
moved all three tasks to Inbox. CLI reads confirmed their statuses, dates,
and Trash flags were preserved and their versions increased.

A separate run used an explicit `--config shared.toml` file.
F3 switched from English to Danish while a task editor contained the unsaved
title `Preserved draft æøå`. The editor returned with that exact draft.
The shared file changed only its language value and retained both test
comments and the Ctrl+Q override. The profile's default configuration stayed
unchanged. Ctrl+S saved the exact Unicode title, and Ctrl+Q quit.

Linux and macOS were not tested in this session.

## Checks for version 0.7.0

Version 0.7.0 passed these checks on November 8, 2026:

| Check | Result |
| --- | --- |
| Unit tests | 120 passed, including list picker navigation, saved Planned state, list forms, help return, and modal mouse handling. |
| CLI integration tests | 8 passed against isolated profiles. |
| MCP integration tests | 7 passed through child processes. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.7.0. |
| OpenSpec | All six capability specs passed strict validation after archiving the navigation change. No active changes remain. |
| Guide examples | Verified default and rebound picker shortcuts, ordinary list ordering and JSON fields, task commands, and MCP help. The virtual All tasks entry did not appear in CLI list output. |
| Documentation | All 15 local links and anchors resolved. All 7 JSON and TOML examples parsed. |

Interactive checks ran in an 80 by 24 pseudo-terminal with isolated profiles.
The English check opened Work and Trash through the picker and showed their
`Planned > Work` and `Planned > Trash` headings. Returning to a filtered
Planned view restored the same task and scroll position. Trash kept its
ungrouped task rows.

Help and a canceled list-name form returned to the scrolled picker.
Creating and renaming a list kept the saved list selected in the picker.
A separate CLI read confirmed the renamed list existed and the canceled
draft list did not. Discarding changed task text returned to Work.
A CLI read confirmed that task's description and version stayed unchanged.

The Danish check displayed `Planlagt`, `Vælg en liste`, and `Alle opgaver`.
Choosing Work displayed `Planlagt > Work`, and Esc returned to Planlagt.
The footer used Danish text. Both terminal checks exited cleanly.

The checks ran on Windows. Mouse behavior was checked by automated tests.
Physical mouse gestures were not tested in this session. Linux and macOS
were not tested in this session.

## Checks for version 0.6.0

Version 0.6.0 passed these checks on November 8, 2026:

| Check | Result |
| --- | --- |
| Unit tests | 112 passed, including exact selected-text copying, all input modes, clipboard failures, NUL rejection, preserved editing state, and shortcut configuration. |
| CLI integration tests | 8 passed against isolated profiles. |
| MCP integration tests | 7 passed through child processes. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.6.0. |
| OpenSpec | All six capability specs passed strict validation after archiving the clipboard change. No active changes remain. |
| Guide examples | Verified default, rebound, and disabled copy shortcuts, conflicting Ctrl+C bindings, task commands, no-date filters, and MCP help. |
| Native clipboard | A Windows clipboard read matched the selected multiline text exactly, including LF newlines, Danish letters, and a joined emoji. |

The native clipboard check ran against a scratch profile in the terminal.
Ctrl+A selected Description and Ctrl+C displayed `Copied selection` while
keeping the editor open. `Get-Clipboard -Raw` returned the expected text,
including `Køb æbler` and `👩‍💻`. Esc then closed the unchanged editor without
a discard prompt. The test harness restored the previous clipboard text
after the check.

Automated UI tests use recording and failing clipboard implementations.
They do not read or overwrite the system clipboard. The discard-confirmation
tests also cover both terminal event encodings of Shift+Tab.

The checks ran on Windows with Rust 1.91.1 and isolated profiles.
Linux and macOS native clipboard behavior was not tested in this session.
No SSH clipboard forwarding was tested or added.

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
