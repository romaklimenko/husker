## Context

Reported on November 17, 2026: Option+Up and Option+Down do not reorder tasks in Terminal.app 2.15 (488.9) on macOS 27.2.1.

Read from the Terminal.app preferences of that Mac: the profile `Basic` stores `useOptionAsMetaKey = 1`.
Its `keyMapBoundKeys` hold no entry for Option+Up (`~F700`) or Option+Down (`~F701`).
Read from the Terminal.app settings window, `TTAppPreferences.nib`: the menu Use Option as Meta stores None as 0, Both Option keys as 1, Left Option only as 2, and Right Option only as 3.
Read from the default key mappings of Terminal.app, `keyMappings.plist`: Option+Left sends `ESC b` and Option+Right sends `ESC f`. Option+Up and Option+Down have no default mapping.
The same file maps Control+Option+Forward Delete to `ESC ESC [ 3 ; 5 ~`, which is the Esc byte before the sequence of Control+Forward Delete.

Assumed: with Option as Meta, Terminal.app sends Option+Up as `ESC ESC [ A` and Option+Down as `ESC ESC [ B`.
This is the Meta convention of terminals: the Esc byte before the bytes of the key.
Assumed: with Use Option as Meta set to None and no key mapping, Terminal.app sends the plain `ESC [ A`.
Not known: whether Terminal.app sends the four bytes in one write, or the Esc byte in a write of its own.
The session that made this change could not press a key in Terminal.app.
For task 3.4 the user pressed Option+Up and Option+Down in Husker 0.18.2 on that Mac, and the selected task moved.
That supports the first assumption. The bytes themselves were not captured, and the second assumption is untested.

Read from source, crossterm 0.29.0, `src/event/sys/unix/parse.rs`: `parse_event` returns an Esc key for the two bytes `ESC ESC`.
It then reads `[` as a character key without modifiers and `A` as a character key with Shift.
It returns an Esc key at once for one `ESC` byte with nothing behind it in the same read.
It reads `ESC [ 1 ; 3 A` as Up with Alt.
It reads both `ESC [ A` and `ESC O A` as a plain Up. A terminal sends the second form in application cursor mode.
Read from source, `src/event/source/unix/mio.rs`: the reader parses every byte of one read before it returns the first key.
A poll with a zero timeout then returns the keys that are already parsed.
Measured with `cargo search crossterm`: 0.29.0 is the newest release.

The bytes `ESC ESC [ A` therefore reach Husker in one of two forms.

| The terminal sends | crossterm reads |
| --- | --- |
| All four bytes in one write | Esc, `[`, and `A`, ready together |
| `ESC` in one write and `ESC [ A` in the next | Esc, and then a plain Up |

Measured by the review with two writes to a pseudo-terminal: without a pause between the writes, 9 of 600 pairs arrived in the second form. With 0.2 milliseconds between them, 300 of 300 did. One write arrived in the first form 2500 of 2500 times.

Measured on version 0.18.1 in a pseudo-terminal with a temporary profile:

1. `ESC [ 1 ; 3 A` on the second of three undated tasks moves it before the first.
2. `ESC ESC [ A` on the same task changes no order.
3. `ESC ESC [ B` in a view filtered by `milk` clears the filter and changes no order.
4. `ESC ESC [ A` in the task editor after typing in Description opens `Discard unsaved changes?`.
5. `ESC ESC [ A` in the list picker closes the picker and moves no list.

## Goals / Non-goals

Goal: Option+Up and Option+Down reorder tasks and lists in Terminal.app when Option acts as Meta, without a key mapping.
Goal: the result does not depend on whether the terminal sends the key in one write or in two.
Goal: the Esc byte of such a key never runs Back, Discard, or Close.
Goal: an Esc pressed alone still feels immediate. It acts after a wait of 10 milliseconds.

Out of scope:

1. Terminal.app with Use Option as Meta set to None and no key mapping. The terminal then sends the plain arrow key, and Husker cannot see Option. The key mappings in the usage guide and `K` and `J` cover that case.
2. Other keys after Esc, such as Home, End, PageUp, Delete, function keys, and arrows with a modifier parameter such as `ESC ESC [ 1 ; 2 A`.
3. Option+Left and Option+Right in Terminal.app. The default mappings send `ESC b` and `ESC f`, which crossterm reads as Alt+B and Alt+F.
4. Changing Terminal.app settings from Husker.
5. Showing Option in place of Alt in help and hints on a Mac.
6. A terminal or a network that delays the arrow by more than 10 milliseconds after the Esc byte.

## Decisions

Join the keys in Husker, after crossterm has read them.
A new type `Input` in `src/input.rs` wraps the event source. `run` in `src/ui.rs` polls and reads through it.
`Input` acts when it reads a pressed Esc without modifiers. It then joins either of the two forms into Alt+Up, Alt+Down, Alt+Right, or Alt+Left.
Anything else leaves all events as they are, in order.

Join the first form only from events that are ready with a zero timeout.
The form is `[` without modifiers and a capital `A`, `B`, `C`, or `D` with Shift. Those are the exact keys that crossterm makes from the bytes.
crossterm parsed them in the same read as the Esc, so they are ready when the Esc is read.
`Input` does not wait for the `[` or for the letter. Keys that come later are most likely the rest of a plain arrow key whose bytes arrived in two reads.
A wait would turn that plain Up into Alt+Up every time. Without the wait it still happens when the rest arrives within microseconds.
Measured by the review: 2 of 200 plain Up keys written as `ESC` and `[ A` without a pause became Alt+Up. No terminal is known to split a plain arrow key that way.

Accept a capital `O` with Shift in the place of `[`.
`ESC O A` is the plain Up key of a terminal in application cursor mode, and crossterm reads it as Up.
Husker never turns that mode on, but another program can leave the terminal in it.
Plain arrows work in that state, so Option+arrow has to work in it too.

Join the second form when a pressed arrow key without modifiers follows the Esc within 10 milliseconds.
`Input` polls for up to 10 milliseconds after an Esc that has nothing ready behind it. `ARROW_WAIT` holds the value.
A terminal that sends the Esc byte in a write of its own sends the arrow within microseconds. A person needs far longer to press a second key.
10 milliseconds is also the default escape time of tmux since version 3.5, read from the tmux `CHANGES` file.
The cost is that an Esc pressed alone acts about 10 milliseconds later than in 0.18.1. Measured by the review: 10.1 to 12.3 milliseconds, and 19 under load.
Measured on 0.18.2 with the Esc byte and the arrow in two writes: with 0, 0.2, 1, and 5 milliseconds between the writes, 20 of 20 presses moved the task. With 30 and 100 milliseconds, 0 of 20 did.

Put the Esc back when the look-ahead fails.
`Input::read` returns the error and keeps the Esc as the next event, in front of the events it has read ahead. No key is lost if a caller reads again.

Join all four arrows.
The default shortcuts use only Up and Down. The same rule serves a configured `alt+left` or `alt+right` in a terminal that sends `ESC ESC [ D`, such as iTerm2 with Option set to Esc+.

Apply the rule on every platform.
On Windows the console reports Alt+arrow as one key, so no terminal sends the separate keys there. One code path keeps the unit tests the same on every platform.

Read every terminal event through one `Input`.
`Input` holds the events it has read ahead. An event read from crossterm directly would overtake them. `AGENTS.md` states the rule.

Test with a real pseudo-terminal.
`tests/terminal_keys.rs` puts a pseudo-terminal in the place of standard input, writes the bytes of each key, and compares what `Input` reads.
The join depends on how crossterm splits `ESC ESC [ A`. Unit tests with scripted events cannot notice when a new crossterm splits the bytes another way. This test can.
The test holds both forms. For the second form it writes the Esc byte, waits until crossterm has read it, and then writes the arrow.
The arrow is then already there when `Input` looks, so the test needs no real time and cannot fail on a slow computer.
It also holds the case `ESC ESC [ H`, which records the split itself.
The test body runs in a thread, and the test fails after 30 seconds. A read without a ready key would otherwise block the test run forever.
`rustix` opens the pseudo-terminal. It is already in `Cargo.lock` through crossterm, and it is a development dependency for Unix only.
Unit tests in `src/input.rs` cover the rule with an `EventSource` that hands out scripted events in groups. Each group arrives after a delay that passes only while a caller polls, so the wait is tested without real time.
The tests state the wait in literal milliseconds. They fail when `ARROW_WAIT` changes.

Rejected: keep the two key mappings in the usage guide as the only answer.
Option+Up then still runs the Esc action for everyone who has Option as Meta and no mapping. That clears a filter or opens the discard prompt.

Rejected: join only what is ready with a zero timeout.
This was the first version of this change. It misses the key whenever crossterm reads the Esc byte in a read of its own, as the review measured.

Rejected: wait longer than 10 milliseconds, or wait for any key and not only an arrow.
A longer wait makes Esc feel slow and joins an Esc and an arrow that a person pressed in a row.

Rejected: ask macOS whether Option is down when a plain arrow arrives.
That would also cover Use Option as Meta set to None. It needs code for one platform, and it reads the keyboard of the wrong computer over SSH.

Rejected: turn on the kitty keyboard protocol. Assumed: Terminal.app does not support it.

Rejected: parse the terminal bytes in Husker in place of crossterm.
Two readers on one terminal would compete, and Husker would repeat the parser for every other key.

## Risks / Trade-offs

Esc, `[`, and a capital `A`, `B`, `C`, or `D` that a person types ahead of a busy Husker now make one Alt+arrow key.
Those are the same bytes as the terminal's Alt+arrow, so no program can tell the two apart without timing.
Version 0.18.1 read them as Esc and two characters.

The same bytes arrive when a person presses Esc and then an arrow key while Husker is busy. They also become Alt+arrow.
crossterm 0.29.0 already reads that as Esc, `[`, and a letter, so the arrow was lost before as well.

An Esc and an arrow key that a person presses within 10 milliseconds of each other now make one Alt+arrow key.
Two separate key presses are further apart. Pressing both keys at the same moment can come that close.

GNU screen and tmux before 3.5 hold an Esc and pass it on together with the next key. By default screen holds it for 300 milliseconds and tmux for 500.
Inside them, an Esc and an arrow key that a person presses in a row within that time arrive like Option+arrow, and Husker cannot tell the two apart.
Measured in screen 4.00.03 and tmux 3.1c on 0.18.2: Esc and Up 50 milliseconds apart moved the task in 5 of 5 runs each.
With `maptimeout 10` in screen and `set -s escape-time 10` in tmux, 0 of 5 runs did, and `ESC ESC [ A` still moved the task in 5 of 5. The usage guide names both settings.
On 0.18.1, the keys that screen delivers this way were Esc and two characters.

A task reordered by mistake is undone with `u`. A list moved by mistake has no undo. The opposite shortcut moves it back.

An Esc pressed alone acts about 10 milliseconds later than in 0.18.1.

The bytes of Terminal.app were not captured. The user's key press shows only that 0.18.2 reorders with them.
A terminal that sends other bytes for Option+Up gets no join, and Option+Up then behaves as in 0.18.1.

If a later crossterm reads `ESC ESC [ A` as Alt+Up by itself, the join never runs and can be removed.

No migration is needed. Reinstall the prior executable to roll back.
