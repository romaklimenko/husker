## Why

Husker now reads an Esc key that is directly followed by a plain arrow key as one key: Alt plus that arrow.
Option+Up and Option+Down then reorder tasks and lists in macOS Terminal.app when its Option key acts as Meta.

Option+Up and Option+Down did not reorder tasks on a Mac with Terminal.app 2.15 on macOS 27.2.1.
The profile had Use Option as Meta set to Both Option keys and no key mapping for the two shortcuts.
A terminal in that state sends Option+Up as the Esc byte followed by the bytes of the plain Up key: `ESC ESC [ A`.
crossterm 0.29.0 reads those four bytes as three keys: Esc, `[`, and `A`.
Husker then ran the action of Esc and ignored the rest. Four effects were measured on version 0.18.1:

1. In task navigation the selected task did not move.
2. In a filtered view the filter was cleared.
3. In the task editor with a changed field the `Discard unsaved changes?` prompt opened.
4. In the list picker the picker closed and no list moved.

The usage guide answered this with two key mappings for Terminal.app.
That left the wrong Esc action in place for everyone who has not added the mappings.

## What changes

1. Husker joins an Esc key and the two keys after it into one key when they are `[` and a capital `A`, `B`, `C`, or `D`. The joined key is Alt+Up, Alt+Down, Alt+Right, or Alt+Left, in that order.
   A capital `O` in the place of `[` joins in the same way. A terminal sends `O` while it is in application cursor mode.
2. Husker joins those three keys only when both keys after Esc are already waiting when it reads the Esc. They wait together when the terminal sends all bytes of the Alt+arrow key in one write.
3. Husker also joins an Esc and a plain arrow key that arrives within 10 milliseconds after it. The keys arrive this way when the terminal sends the Esc byte in a write of its own.
4. An Esc with no arrow key behind it acts after a wait of 10 milliseconds. In 0.18.1 it acted at once.
5. Any other keys after Esc stay separate keys in the order received.
6. A new integration test for Unix writes the bytes of each key to a pseudo-terminal and checks the keys that Husker reads.
7. The usage guide and the README say that Option+Up and Option+Down work in Terminal.app when Option acts as Meta. The two key mappings stay as the way for a profile where Option does not act as Meta.
8. The version becomes 0.18.2.

## Capabilities

### New capabilities

None.

### Modified capabilities

- `terminal-interface`: the new requirement Read an Esc-prefixed arrow key as Alt+arrow.

## Impact

1. `src/input.rs`: the new `Input` type that reads terminal events and joins the keys, with unit tests.
2. `src/lib.rs`: export the `input` module.
3. `src/ui.rs`: read events through `Input` in `run`.
4. `tests/terminal_keys.rs`: the integration test with a pseudo-terminal, for Unix only.
5. `Cargo.toml` and `Cargo.lock`: raise the version to 0.18.2 and add `rustix` as a development dependency for Unix.
6. `docs/usage.md` and `README.md`: describe Option+Up and Option+Down in Terminal.app when Option acts as Meta and when it does not.
7. `docs/development.md`: record the checks for version 0.18.2.

No storage migration, sync file format change, configuration change, CLI change, or MCP tool change is required.
Configured shortcuts need no change. The joined key resolves like any other Alt+arrow key.
A terminal that sends Alt+Up as `ESC [ 1 ; 3 A` works as before, and so do the two Terminal.app key mappings.
On Windows the console reports Alt+arrow as one key, so Alt+arrow does not change there. The wait after Esc applies on every platform.
Esc, `[` or a capital `O`, and a capital `A`, `B`, `C`, or `D` that arrive together are now one key in every screen.
A person can send those keys together only by typing ahead while Husker is busy: Esc, `[` or `O`, and the letter, or Esc and then an arrow key.
Version 0.18.1 read those keys as Esc and two characters.
An Esc and a plain arrow key that arrive within 10 milliseconds of each other are now one key too.
Two key presses in a row are further apart than that. Two keys pressed at the same moment can be that close.
GNU screen and tmux before 3.5 hold an Esc and deliver it together with the next key.
Inside them, an Esc and an arrow key pressed in a row within their escape timeout now count as Alt+arrow. The usage guide names the settings that shorten the timeout.
