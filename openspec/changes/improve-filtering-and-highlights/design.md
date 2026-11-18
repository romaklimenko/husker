## Context

Read from source: `Config::action` in `src/config.rs` returns the first available action whose binding equals the key, and `Config::validate` rejects two actions that share a key in one `Mode`.
Read from source: `refresh` is available in every mode with the default F5. `restore` uses `r` in task navigation, `reopen` uses `r` in History, and `rename_list` uses `r` in the list picker.
Read from source: Trash is a view of task navigation, so `Mode::Tasks` covers both All tasks and Trash.
Read from source: the filter input in `src/ui.rs` parses and applies its text only on Enter.
Read from source: `draw_navigation` draws every heading yellow and every task row in the default color, except dark gray held rows.
Read from source: the three list pickers share `ListPicker` and `draw_list_popup`, and bind letters such as `j`, `k`, `g`, `l`, `n`, `r`, `d`, `q`, and `?`.

## Goals / Non-goals

Goal: `r` refreshes in All tasks and list views without taking `r` away from Restore, Reopen, or typed text.
Goal: the task filter and the list pickers react to every typed character.
Goal: Today stands out, and past and finished groups recede.

Out of scope:

1. The phone page. Its filter is a list menu, and its groups have their own styles.
2. Typing into the task editor's List selector. Enter or `l` still opens its picker, where typing works.
3. Fuzzy matching. Picker matching is a case-insensitive substring test.
4. History and Trash styles.
5. Configurable colors.

## Decisions

### A printable refresh key works only in task navigation

`Config` gets one predicate, `binding_active(action, binding, mode)`. Both `action` and `validate` use it.
A printable binding of `refresh` is active only in `Mode::Tasks`. A printable binding of any action is inactive in `Mode::Lists` and `Mode::EditorListPicker`.
So `r` refreshes in task navigation, while History, Trash, text inputs, and pickers never see it as a refresh.
Rejected: a separate action such as `refresh_tasks`. Two actions for one command confuse the configuration and Help.
Rejected: letting mode-specific actions shadow global ones. It would hide real conflicts, such as `quit = ["f5"]`.

### Trash gets its own mode

`Mode::Trash` permits every task-navigation action plus `restore` and `empty_trash`, which leave `Mode::Tasks`.
The precedent is History, which became its own mode so `reopen` and `restore` could share `r`.
Conflict checks for Trash keep their old meaning, because Trash permits the same actions as before.

### The filter applies valid text at once

`App::filter_origin` keeps the query, selection, and selected task from before the input opened.
Each edit that changes the text parses it. Valid text replaces `App::query`, clears held rows, and rebuilds the visible rows from the loaded tasks without a database read.
Text that does not parse leaves the rows alone and marks the input border red. Enter on such text shows the parse error, as before.
Esc restores the saved filter, query, and selected task.
Rejected: applying text that does not parse as plain text. `l:"Wo` would briefly match nothing and then jump back.

### Group styles come from the group key

`draw_navigation` maps the planned group key to a heading style and a row style. The keys are stable English values from `src/planning.rs`.
Today uses black on yellow with bold, because other headings are already yellow text and need a stronger signal.
Measured contrast is not available for every terminal theme. The design uses the 16 base colors, which every supported terminal maps to its theme.
Held rows keep their dark gray style, so a task the user just closed still looks closed.

### Pickers own a filter string

`ListPicker` gets a `filter` string and a list of displayed entries. The selection indexes the displayed entries.
The main picker's entries are All tasks, the ordinary lists, and Trash, as `RowIdentity` values. The editor and move-task pickers hold list IDs.
Matching lowercases the visible name and the filter text with Unicode rules, so `Æ` matches `æ`.
The popup height uses the unfiltered entry count, so it does not jump while the user types.
Management keys move to Ctrl keys because letters now type. Ctrl+N and Ctrl+R are free in every terminal Husker supports. Delete keeps working, and Ctrl+D covers Mac keyboards without a forward Delete key.
Rejected: a `/` key to start picker filtering. The user asked to type the first letters at once.
Rejected: letting unbound letters filter and bound letters act. A name such as `Husker` contains `k`, which would move the selection halfway through the word.

## Risks / Trade-offs

Users lose `n`, `r`, `d`, `K`, `J`, `q`, `?`, `j`, `k`, `g`, `G`, and `l` in the pickers. The footer shows the replacement keys.
In Terminal.app without Use Option as Meta and without the key mappings in the usage guide, no default key reorders lists any more, because `K` and `J` type in the picker. The usage guide describes the Option setting.
A configuration copied from an older example sets `new_list = ["n", "N"]`. That file still loads, and the footer shows `unbound` for New list until the user updates it.
A configuration that binds another task-navigation action to `r` fails to load with a conflict error that names `refresh`.
Rolling back means reinstalling 0.18.2. No data changes.
