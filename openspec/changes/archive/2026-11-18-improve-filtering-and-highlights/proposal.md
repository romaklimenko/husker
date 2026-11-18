## Why

Four requests from the Husker task inbox make the terminal interface faster to read and to drive from the keyboard.

1. `r` refreshes in All tasks and list views, so a refresh no longer needs F5. F5 needs the fn key on many laptops.
2. The task filter narrows the task rows while the user types it. Today nothing changes until Enter.
3. Today's tasks stand out, and past and finished tasks are dimmed, so the groups read at a glance.
4. The three list pickers filter their entries as the user types the first letters of a list name.

## What changes

1. The `refresh` action defaults to `r` and F5. The `r` key refreshes only in task navigation outside Trash and History. F5 keeps working on every screen.
2. Trash gets its own key mode, Trash navigation, so `r` keeps restoring there. History keeps `r` for Reopen.
3. The filter input applies each valid text at once. Text that does not parse keeps the last valid results and turns the input border red. Esc restores the filter from before the input opened.
4. In All tasks and list views, the Today heading is bold black on yellow and its rows are bold. Earlier, Done, and Cancelled rows are dark gray. Done headings are green.
5. In the list picker, the editor list picker, and the move-task picker, printable keys type into a picker filter. Entries whose name contains the text stay, and the first name that starts with it is selected.
6. Because letters now type in the pickers, list management in the list picker moves to Ctrl keys. New list is Ctrl+N, Rename list is Ctrl+R, and Delete list is Ctrl+D or Delete. Help gets F1 as a second key, because `?` types in the pickers. Reordering uses Alt+Up and Alt+Down.
7. Esc in a picker first clears its filter and then closes the picker. `l` no longer closes a picker.

## Capabilities

### New capabilities

None.

### Modified capabilities

- `terminal-interface`: keyboard defaults, filter behavior, refresh keys, footer and Help keys, picker cancel keys, list reordering while filtering, and two new requirements, Filter list pickers by typing and Color planning groups by date and status.
- `profiles-localization`: new defaults for `refresh`, `new_list`, `rename_list`, `delete_list`, and `help`, the Trash navigation mode, and the rules for printable keys in the pickers and for the printable `refresh` key.

## Impact

| File | Change |
| --- | --- |
| `src/config.rs` | `Mode::Trash`, new defaults, printable-key rules in `action` and `validate`, and key display for one mode |
| `src/ui.rs` | Live filter, picker filter state and drawing, group styles, footer and Help keys, and the refresh notice |
| `src/i18n.rs` | Strings for the picker filter, its empty result, and the reorder message, in English and Danish |
| `README.md`, `docs/usage.md`, `docs/sync.md` | The new keys and behavior |
| `docs/development.md` | The checks for version 0.19.0 |
| `Cargo.toml`, `Cargo.lock` | Version 0.19.0 |

Compatibility effects:

1. A configuration file that sets `new_list`, `rename_list`, or `delete_list` to printable keys only, such as the old example's `["n", "N"]`, still loads. Those keys then type into the picker filter, and the footer shows `unbound` for that action. Users replace them with the new defaults or remove the lines.
2. A configuration that binds another task-navigation action to `r` now conflicts with `refresh` and fails to load. Users set `refresh = ["f5"]` or move the other action.
3. A conflict with `empty_trash` or `restore` now names Trash mode in its error, for example `... 'history' and 'empty_trash' in Trash mode.`
4. No database migration, sync format change, CLI change, MCP change, or phone page change is required.
