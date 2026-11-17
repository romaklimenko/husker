## ADDED Requirements

### Requirement: Read an Esc-prefixed arrow key as Alt+arrow

Husker SHALL read an Esc key that is directly followed by a plain arrow key as one key: Alt plus that arrow.

Some terminals send Alt+arrow as the Esc byte followed by the bytes of the plain arrow key.
macOS Terminal.app sends Option+Up and Option+Down this way while the pressed Option key acts as Meta.
Its profile setting Use Option as Meta selects the Option keys that act as Meta: Both Option keys, Left Option only, or Right Option only.

| Bytes from the terminal | Key |
| --- | --- |
| `ESC ESC [ A` or `ESC ESC O A` | Alt+Up |
| `ESC ESC [ B` or `ESC ESC O B` | Alt+Down |
| `ESC ESC [ C` or `ESC ESC O C` | Alt+Right |
| `ESC ESC [ D` or `ESC ESC O D` | Alt+Left |

A terminal sends the bytes with `O` while it is in application cursor mode, and the bytes with `[` otherwise.
Husker never turns application cursor mode on, but another program can leave the terminal in it.

A terminal sends the bytes of an Esc-prefixed arrow key in one write or in two. Husker joins the Esc and the arrow in both cases.

| The terminal sends | Husker joins when |
| --- | --- |
| All bytes of the key in one write | The two keys after the Esc, `[` or `O` and the letter, are already waiting when Husker reads the Esc. |
| The first `ESC` byte in a write of its own, then the bytes of the plain arrow key | The plain arrow key arrives within 10 milliseconds after Husker reads the Esc. |

Husker cannot tell these bytes from an Esc and an arrow key that another program delivers together. It joins those too.
GNU screen holds an Esc for 300 milliseconds by default and delivers it together with an arrow key pressed in that time.

The joined key resolves through the configured shortcuts like an Alt+arrow key that a terminal sends in one sequence, such as `ESC [ 1 ; 3 A` for Alt+Up.
The rule applies in every screen. The Esc of a joined key runs no Back, Discard, or Close action.

Husker joins no keys in these cases.

| Input | Result |
| --- | --- |
| An Esc that no key follows within 10 milliseconds | Esc acts. Husker waits 10 milliseconds for an arrow key and then acts on the Esc. |
| An Esc, then a plain arrow key more than 10 milliseconds later | Esc acts, and then the arrow key acts as a plain arrow. |
| An Esc, then `[` or `O` and a letter that arrive after Husker has read the Esc | Each key keeps its own meaning. The three keys are the bytes of one plain arrow key, split over two reads. They are not an Esc-prefixed arrow. |
| `ESC ESC [ H`, `ESC ESC O H`, or any other key after Esc that is not a plain arrow | Esc and the keys after it stay separate keys in the order received. |
| `ESC ESC [ 1 ; 2 A`, or any arrow key with a modifier after Esc | Esc and the keys after it stay separate keys in the order received. |

#### Scenario: Reorder a task with an Esc-prefixed arrow

- **WHEN** two open undated tasks are listed, the second is selected, and the terminal sends `ESC ESC [ A`
- **THEN** the second task moves before the first, as Alt+Up would move it

#### Scenario: Reorder a task in application cursor mode

- **WHEN** two open undated tasks are listed, the first is selected, and the terminal sends `ESC ESC O B`
- **THEN** the first task moves after the second, as Alt+Down would move it

#### Scenario: Join an arrow that the terminal sends in a second write

- **WHEN** two open undated tasks are listed, the second is selected, and the terminal sends `ESC` and one millisecond later `ESC [ A`
- **THEN** the second task moves before the first, as Alt+Up would move it

#### Scenario: Keep Esc and a later arrow apart

- **WHEN** two open undated tasks are listed, the second is selected, and the user presses Esc and 200 milliseconds later Up
- **THEN** the first task becomes selected and no task moves

#### Scenario: Keep the filter while reordering

- **WHEN** a view is filtered by `milk`, two open undated tasks match, the first is selected, and the terminal sends `ESC ESC [ B`
- **THEN** the first task moves after the second and the filter stays active

#### Scenario: Keep the editor open

- **WHEN** the task editor has a changed Description and the terminal sends `ESC ESC [ A`
- **THEN** the editor stays open and shows no discard confirmation

#### Scenario: Move a list in the picker

- **WHEN** the picker shows the ordinary lists Inbox, Work, and Home with Home selected and the terminal sends `ESC ESC [ A`
- **THEN** Home moves before Work and the picker stays open

#### Scenario: Act on Esc alone

- **WHEN** a view is filtered by `milk` and the terminal sends one `ESC` byte
- **THEN** the filter clears after a wait of 10 milliseconds, without another key

#### Scenario: Leave other keys after Esc alone

- **WHEN** a view is filtered by `milk` and the terminal sends `ESC ESC [ H`
- **THEN** Husker handles Esc, `[`, and `H` as three keys in that order
- **AND** the filter clears, as for an Esc alone, and no task moves
