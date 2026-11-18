## ADDED Requirements

### Requirement: Match day names in the task filter

The terminal task filter SHALL let an unquoted text term also match a task when a word of the task's day name starts with the term.

An unquoted text term is a text term that is not wholly quoted and does not start with `#`. Field terms such as `l:`, `due:`, and `s:` are not text terms.
The day name is the name of the day that the heading above the task's row shows, in the selected interface language. Each view takes it from a different date.

| View | Day names | Date the name comes from |
| --- | --- | --- |
| All tasks and list views | Earlier, Today, Tomorrow, and Sometime | The date group of the stored due date |
| History | Today and Yesterday | `status_changed_on`, the local date the task was closed |
| Trash | None | Trash has no headings |

In All tasks and list views, the date group comes from the stored due date and the current local date, as the Date groups requirement of the `planning-search` spec defines.

| Date group | `due:` value | English name | Danish name |
| --- | --- | --- | --- |
| Overdue | `overdue` | Earlier | Tidligere |
| Today | `today` | Today | I dag |
| Tomorrow | `tomorrow` | Tomorrow | I morgen |
| Soon | `soon` | None | None |
| Next | `next` | None | None |
| Sometime | `undated` | Sometime | Engang |

Soon and Next have no name to match, because their terminal headings show exact dates, such as `2026-11-20 Fri W47`.
A done or cancelled task in All tasks or a list view has the name of its date group. That is the heading its row shows while it is held, so a held row keeps matching.
In History, a task closed today has the name Today, or I dag in Danish. A task closed yesterday has the name Yesterday, or I går in Danish. A task closed on an earlier date or on an unknown date has no name.
Spaces separate the words of a name, so I dag has the words I and dag.
A term matches a word when the lowercase word starts with the lowercase term. `to` matches Today and Tomorrow, `tod` matches only Today, and `day` matches no name.
A task matches an unquoted text term when its title or description contains the term, as the `planning-search` spec defines, or when its day name matches the term.
Every term must match, as the Combined filters requirement of the `planning-search` spec defines.
Wholly quoted terms and terms that start with `#` match only titles and descriptions.
Names of the language that is not selected do not match. With Danish selected, `dag` matches I dag, and `tod` matches no day name.
No other heading text matches. That includes weekday names, dates, week numbers, Unknown date, and the status headings Done today, Cancelled today, Done, and Cancelled.
When the interface language changes, an applied filter uses the names of the new language at once and keeps the selected task when it is still visible.

#### Scenario: Narrow from Today and Tomorrow to Today

- **WHEN** today is `2026-11-18`, English is selected, All tasks holds `Call mom` due `2026-11-18`, `Pay rent` due `2026-11-19`, `Book tickets` due `2026-11-20`, and `Fix the toaster` without a due date, and the user presses `/` and types `to`
- **THEN** `Call mom`, `Pay rent`, and `Fix the toaster` are listed, and `Book tickets` is not
- **AND** typing `d` to make `tod` leaves only `Call mom`

#### Scenario: Match Earlier and Sometime

- **WHEN** today is `2026-11-18`, English is selected, and All tasks holds `Renew passport` due `2026-11-17` and `Paint fence` without a due date
- **THEN** the filter `ear` lists `Renew passport`, and the filter `some` lists `Paint fence`

#### Scenario: Match a quoted term against task text only

- **WHEN** All tasks holds the tasks of the scenario Narrow from Today and Tomorrow to Today, English is selected, and the user applies `"to"`
- **THEN** only `Fix the toaster` is listed

#### Scenario: Match Danish names

- **WHEN** All tasks holds the tasks of the scenario Narrow from Today and Tomorrow to Today, Danish is selected, and the user applies `dag`
- **THEN** only `Call mom` is listed
- **AND** the filter `tomorrow` lists no task

#### Scenario: Apply a language change to an applied filter

- **WHEN** All tasks holds the tasks of the scenario Narrow from Today and Tomorrow to Today and is filtered by `dag` with English selected, so no task is listed, and the user chooses Danish with F3
- **THEN** `Call mom` is listed

#### Scenario: Keep a completed task under a date word filter

- **WHEN** English is selected, All tasks is filtered by `tom`, and the user completes `Pay rent`, due `2026-11-19`, with Space on `2026-11-18`
- **THEN** `Pay rent` stays listed as a held row under Tomorrow

#### Scenario: Match the day a task was closed in History

- **WHEN** today is `2026-11-18`, English is selected, `Pay rent` due `2026-11-19` was completed on `2026-11-18`, `Call mom` due `2026-11-18` was completed on `2026-11-17`, and History is filtered by `tod`
- **THEN** only `Pay rent` is listed
- **AND** the filter `yes` lists only `Call mom`, and the filter `tom` lists no task

#### Scenario: Match only task text in Trash

- **WHEN** English is selected, Trash holds `Call mom` due today and `Fix the toaster` without a due date, and the user applies `to`
- **THEN** only `Fix the toaster` is listed

## MODIFIED Requirements

### Requirement: Filter list pickers by typing

Husker SHALL filter the entries of the list picker, the editor list picker, and the move-task picker by the text the user types while the picker is open.

A printable key adds its character to the picker filter. A printable key is a character key without Ctrl or Alt. Shift is allowed, and AltGr, which terminals report as Ctrl+Alt, also counts.
A printable key never runs an action in these pickers, even when the configuration binds it to one.
Backspace removes the last character of the picker filter. Backspace on an empty filter does nothing.
An entry matches when its visible name contains the filter text, ignoring case. The name excludes the count and the default marker. All tasks and Trash match by their localized names.
Matching entries keep their saved order. After each change that leaves filter text, the selection moves to the first match whose name starts with the filter text, or to the first match when none starts with it. When the filter becomes empty, the selected entry stays selected.
When no entry matches, the popup shows `No matching lists` in English or `Ingen lister passer` in Danish, and Open does nothing.
The popup's bottom border is empty while the filter is empty, and shows `Filter: ` followed by the filter text otherwise. The footer starts with `Type to filter` in English or `Skriv for at filtrere` in Danish, as the requirement Keep the display usable defines.
The popup keeps the height that the unfiltered entries need, so it does not jump while the user types.
Back with filter text clears the filter and keeps the selected entry selected. Back with an empty filter cancels the picker.
Every picker opens with an empty filter. Refresh, Help, and the language picker keep the filter text and the selected entry by identity.
Creating or renaming a list from the list picker clears the filter, so the saved list can be selected.
Mouse clicks and the wheel act on the displayed entries.

| Key in a list picker | Result |
| --- | --- |
| `a` to `z`, `A` to `Z`, digits, space, punctuation, and other characters | Add the character to the filter |
| Backspace | Remove the last filter character |
| Esc | Clear the filter, or cancel the picker when the filter is empty |
| Enter | Choose the selected displayed entry |
| Up, Down, Home, End, Left, Right, PageUp, PageDown | Move the selection among displayed entries |

#### Scenario: Pick a list by its first letters

- **WHEN** the picker shows All tasks, Inbox, Husker, Labs, and Trash and the user types `hu`
- **THEN** the picker shows only Husker, selected, and Enter opens `All tasks > Husker`

#### Scenario: Prefer a name that starts with the text

- **WHEN** the ordinary lists are `Shopping` and `Home` and the user types `ho` in the move-task picker
- **THEN** both lists match and Home is selected

#### Scenario: Type a letter that is also a shortcut

- **WHEN** the list picker is open with default bindings and the user types `j`, `k`, `n`, `r`, `d`, `l`, `q`, or `?`
- **THEN** the character is added to the filter and no selection move, list creation, rename, deletion, cancel, quit, or Help happens

#### Scenario: Clear the filter before closing

- **WHEN** the editor list picker has the filter `la` and the user presses Esc twice
- **THEN** the first Esc clears the filter and keeps the editor picker open, and the second Esc returns to the List selector with the draft unchanged

#### Scenario: Show that nothing matches

- **WHEN** the user types `zz` in the list picker and no entry name contains `zz`
- **THEN** the popup shows `No matching lists` and Enter keeps the picker open

#### Scenario: Keep the border empty until the user types

- **WHEN** English is selected and the user opens the list picker with `l`
- **THEN** the popup's bottom border shows no text and the footer starts with `Type to filter`
- **AND** typing `hu` shows `Filter: hu` in the bottom border
