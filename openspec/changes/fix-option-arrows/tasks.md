## 1. Read Esc-prefixed arrow keys

- [x] 1.1 Add `src/input.rs` with `EventSource`, `TerminalEvents`, `Input`, and `ARROW_WAIT`. For a pressed Esc, `Input::read` returns Alt+Up, Alt+Down, Alt+Right, or Alt+Left in two cases. In the first, `[` or a capital `O` and then a capital `A`, `B`, `C`, or `D` are ready with a zero timeout. In the second, a pressed arrow key without modifiers arrives within `ARROW_WAIT`, which is 10 milliseconds. Export the module from `src/lib.rs`.
- [x] 1.2 In `run` in `src/ui.rs`, poll and read terminal events through `Input`.
- [x] 1.3 Add unit tests in `src/input.rs` with scripted events that arrive after scripted delays: each arrow joins after `[` and after `O`, each arrow joins when it follows the Esc at once, after 5 milliseconds, and after 10, an arrow after 11 milliseconds stays apart, an Esc alone waits exactly 10 milliseconds, a plain arrow key in two reads does not join, a letter that comes later than its `[` does not join, 21 other keys after Esc stay in order, only a pressed Esc without modifiers joins, a held key joins once per repeat, an Esc read ahead joins, events read ahead count as ready, a failed look-ahead loses no key, and other events pass through.
- [x] 1.4 Add `tests/terminal_keys.rs` for Unix. It puts a pseudo-terminal in the place of standard input, writes the bytes of each key in its table in one write or in two, and compares the keys that `Input` reads. It fails after 30 seconds instead of blocking. Add `rustix` with `fs`, `pty`, and `stdio` as a development dependency for Unix in `Cargo.toml`.
- [x] 1.5 Confirm that the joining unit tests and `tests/terminal_keys.rs` fail when `Input::read` skips either join, and that `tests/terminal_keys.rs` fails after 30 seconds when the look-ahead reads without polling.
- [x] 1.6 Add the rule to `AGENTS.md` that terminal events are read through `Input`.

## 2. Version and documentation

- [x] 2.1 Raise the version to 0.18.2 in `Cargo.toml` and `Cargo.lock`.
- [x] 2.2 In `docs/usage.md`, describe Option+Up and Option+Down in Terminal.app when an Option key acts as Meta and when none does, and keep the two key mappings for a profile where Option does not act as Meta.
- [x] 2.3 In `README.md`, update the sentence about Option+Up and Option+Down in Terminal.app.
- [x] 2.4 Record the checks for version 0.18.2 in `docs/development.md`.
- [x] 2.5 In `docs/usage.md`, name the settings of GNU screen and tmux that keep an Esc and a later arrow key apart, after confirming both in screen 4.00.03 and tmux 3.1c.

## 3. Verification

- [x] 3.1 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `openspec validate fix-option-arrows --strict`.
- [x] 3.2 Run version 0.18.1 and version 0.18.2 in a pseudo-terminal with a temporary profile. Send `ESC ESC [ A` and `ESC ESC [ B` in task navigation, in a filtered view, in the task editor, and in the list picker. Confirm that 0.18.2 moves the task or list and runs no Esc action. Send the Esc byte and the arrow in two writes with 0 to 100 milliseconds between them and confirm that 0.18.2 joins them up to 5 milliseconds and keeps them apart from 30.
- [x] 3.3 Obtain an independent review, fix its findings, and rerun the affected checks.
- [x] 3.4 In Terminal.app on macOS with Use Option as Meta set to Both Option keys and no key mapping, press Option+Up and Option+Down in Husker 0.18.2 and confirm that the selected task moves. This needs a person at the keyboard.
