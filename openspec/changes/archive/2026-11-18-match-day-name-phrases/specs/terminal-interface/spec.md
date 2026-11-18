## MODIFIED Requirements

### Requirement: Match day names in the task filter

The terminal task filter SHALL let an unquoted text term also match a task when the task's day name, read from the start of one of its words, starts with the term.

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
Two or more adjacent unquoted text terms form one phrase term when their text, joined by single spaces, is the start of a day name that the current view shows, in the selected language. Case is ignored.
Terms are adjacent when no other term stands between them in the filter text. A field term between two words keeps them apart, so `i l:home mo` stays the terms `i` and `mo`.
All tasks and list views check Earlier, Today, Tomorrow, and Sometime. History checks Today and Yesterday. Trash checks no names, so no phrase term forms there.
With Danish selected, `i g` forms a phrase term in History for I går, and stays two terms in All tasks, which has no I går heading. `i mo` stays two terms in History, which has no I morgen heading.
The filter reads the text terms from the left. At each term, the longest run of two or more terms that starts a day name forms one phrase term, and reading continues after that run.
A term that starts no such run stays a separate term. A wholly quoted term or a term that starts with `#` never joins a phrase term, so `"i" mo` stays two terms.
With Danish selected, `i mo` is one phrase term because I morgen starts with it, and `i d` is one phrase term because I dag starts with it. A single `i` stays one term and matches both names.
English day names have one word each, so English filters never form a phrase term. Terms that do not start a day name together, such as `buy milk` or `mo i`, stay separate terms.
A phrase term matches a day name when the lowercase name, read from the start of one of its words, starts with the lowercase phrase. It matches a title or description that contains the phrase, with one space between its words.
A one-word term matches a word when the lowercase word starts with the lowercase term. `to` matches Today and Tomorrow, `tod` matches only Today, and `day` matches no name.
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

#### Scenario: Match a Danish name typed as two words

- **WHEN** today is `2026-11-18`, Danish is selected, All tasks holds `Pay rent` due `2026-11-19` and `Configure model source` due `2026-11-18`, and the user presses `/` and types `i mo`
- **THEN** only `Pay rent` is listed, because `i mo` starts I morgen and no title contains `i mo`
- **AND** the filter `i` lists both tasks

#### Scenario: Keep other words separate

- **WHEN** Danish is selected, All tasks holds `Buy oat milk` without a due date, and the user applies `buy milk`
- **THEN** `Buy oat milk` is listed, because `buy` and `milk` do not start a day name together and each matches the title
