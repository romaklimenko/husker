# Profiles and localization

## Purpose

Define how users select task profiles, configuration files, interface language, and action shortcuts.
The terminal interface owns keyboard actions. Task storage owns database contents.
## Requirements
### Requirement: Profile paths

Husker SHALL use `~/.husker` as the default profile directory and accept `--profile DIRECTORY` to select another directory.

The selected profile contains `tasks.db` and the default `config.toml`.
Husker creates a missing profile directory when opening its database.
Relative paths resolve against the current working directory.
Different profile directories keep separate task data.

#### Scenario: Select an independent profile

- **WHEN** Husker starts with `--profile ./work` from `C:/projects`
- **THEN** Husker uses `C:/projects/work/tasks.db` and does not read tasks from the default profile

### Requirement: Configuration paths

Husker SHALL read TOML configuration from the selected profile's `config.toml` unless `--config FILE` selects another file.

A missing default configuration uses built-in defaults.
A missing explicitly selected configuration returns an error.
An unreadable or invalid configuration returns an error before opening the terminal interface.
Unknown configuration keys return an error.
Selecting a configuration file does not change the selected profile.

#### Scenario: Use a separate configuration

- **WHEN** Husker starts with `--profile ./work --config ./shared.toml`
- **THEN** Husker reads `./shared.toml` and stores tasks in `./work/tasks.db`

#### Scenario: Reject a misspelled setting

- **WHEN** the configuration contains `langauge = "da"`
- **THEN** configuration loading returns an unknown-setting error

### Requirement: Interface language

Husker SHALL provide English and Danish terminal interface text through the `language` setting.

The accepted values are `en` and `da`.
The default is `en`.
The terminal language picker can change the setting immediately after saving it successfully.
Canceling the picker or failing to save leaves the active interface language unchanged.
Task content and list names remain unchanged.
Configuration keys, filter operators, CLI command names, and JSON fields remain in English.
CLI parser and storage errors may remain in English.

#### Scenario: Use Danish without translating task data

- **WHEN** the configuration contains `language = "da"` and a task is titled `Buy milk`
- **THEN** the terminal shows Danish interface labels and preserves the title `Buy milk`

#### Scenario: Save a language for the next launch

- **WHEN** a user selects Dansk in the terminal language picker and saving succeeds
- **THEN** the current interface switches to Danish and the next launch reads `language = "da"`
- **AND** task text, list names, filter terms, and command tokens remain unchanged

### Requirement: Calendar labels

Husker SHALL show dates with the calendar date, localized weekday abbreviation, and ISO week number.

The normal form is `2026-11-08 Sun W45` in English.
The Danish equivalent uses `søn` for Sunday.
When the ISO week-year differs from the calendar year, the week label includes the ISO week-year.
For example, `2027-01-01 Fri 2026-W53` identifies the week at the year boundary.

Known single-day task headings use this same format alongside their localized relative label.
Future exact-date headings use this format without Soon or Next labels. Aggregate or unknown headings have no fabricated date.

#### Scenario: Format a Danish Sunday

- **WHEN** the language is `da` and the date is `2026-11-08`
- **THEN** the date label is `2026-11-08 søn W45`

### Requirement: Configurable action keys

Husker SHALL accept a `[keys]` TOML table that maps action names to arrays of key strings.

An override replaces the default keys for that action.
An empty array disables the action's shortcuts.
Unspecified actions keep their defaults.
All action shortcuts shown by the interface use the resolved configuration.
Printable text and ordinary text editing remain available in text input fields.

For example, `quit = ["ctrl+q"]` replaces the default quit shortcut.
Key strings accept characters, named keys, and the `ctrl`, `alt`, and `shift` modifiers.
The shipped example configuration lists every action and its defaults.
The `copy_selection` action defaults to `ctrl+c` and is active only in task text fields, filter inputs, and list-name inputs.
It is unavailable in the task editor's List selector and editor list picker.
Copy-selection hints use the configured shortcut and selected language.
Conflicting input-mode action bindings, including another action bound to Ctrl+C while Copy selection keeps that default, follow the existing key-conflict validation requirement.

The `all_tasks` action has no default shortcuts and is the canonical action name for the root task view.
Normal root navigation uses Back or the All tasks picker entry. Users may assign an optional `all_tasks` shortcut.
The legacy `planned` configuration name remains an alias for `all_tasks`.
An explicit legacy override replaces the new default, including an empty array.
Configuration that names both `planned` and `all_tasks` is invalid, even when the two arrays are equal.
The generated example configuration and visible help use `all_tasks` and All tasks.
The `history` action defaults to `h` in ordinary task navigation and History.
The `reopen` action defaults to `r` in History. The existing `restore` action remains available for Trash.
The `delete_list` action defaults to `d` and Delete in the list picker.

The `language` action defaults to F3 in task navigation, History, both list pickers, task editing, filter editing, list-name editing, and Help.
It is unavailable in confirmation screens and Copy text.

The due preset actions have these defaults.

| Action | Default shortcut |
| --- | --- |
| `due_today` | `t` |
| `due_tomorrow` | `T` |
| `due_next_week` | `w` |
| `clear_due` | `x` |

They are available in ordinary task navigation and History outside Trash, and in the focused Due date editor field.
They are unavailable in other editor fields, filter and list-name inputs, list pickers, Help, confirmations, Copy text, and the language picker.
Each override replaces that action's defaults, and an empty array disables it.
Help and Due date hints use the configured bindings and selected language.

The List selector and editor list picker reuse the existing `open`, `lists`, `back`, navigation, Help, Language, Refresh, and Toggle mouse actions. No new action names or configuration keys are required.

#### Scenario: Replace a shortcut

- **WHEN** `[keys]` contains `quit = ["ctrl+q"]`
- **THEN** Ctrl+Q quits from navigation and the default quit key no longer triggers quit

#### Scenario: Rebind copying

- **WHEN** `[keys]` contains `copy_selection = ["ctrl+shift+c"]`
- **THEN** Ctrl+Shift+C copies selected input text and Ctrl+C no longer invokes that action
- **AND** the copy-selection hint shows Ctrl+Shift+C

#### Scenario: Disable clipboard copying

- **WHEN** `[keys]` contains `copy_selection = []`
- **THEN** no shortcut invokes Copy selection and F4 retains its existing copy-text behavior

#### Scenario: Reject an input-mode conflict

- **WHEN** Save is bound to Ctrl+C and Copy selection retains its default Ctrl+C binding
- **THEN** configuration loading reports a conflict before opening the terminal interface

#### Scenario: Preserve a legacy root shortcut

- **WHEN** `[keys]` contains `planned = ["ctrl+p"]`
- **THEN** Ctrl+P opens All tasks and the action has no other default shortcut
- **AND** visible help calls the action All tasks

#### Scenario: Preserve a disabled legacy action

- **WHEN** `[keys]` contains `planned = []`
- **THEN** no shortcut invokes All tasks

#### Scenario: Reject both root action names

- **WHEN** `[keys]` contains both `planned` and `all_tasks`
- **THEN** configuration loading reports the duplicate action names before opening the task database

#### Scenario: Rebind a due preset

- **WHEN** `[keys]` contains `due_today = ["ctrl+t"]`
- **THEN** Ctrl+T invokes Today in its permitted modes and `t` no longer invokes that preset
- **AND** Help and Due date hints show Ctrl+T

#### Scenario: Disable a due preset

- **WHEN** `[keys]` contains `clear_due = []`
- **THEN** no shortcut invokes Clear due date and manual field deletion remains available

### Requirement: Key conflict validation

Husker SHALL reject unknown action names, invalid key strings, and shortcuts assigned to multiple actions in the same mode.

| Mode | Purpose |
| --- | --- |
| Task navigation | Select tasks and invoke task actions. |
| List picker | Select a scope and create, rename, reorder, or delete ordinary lists in a modal popup. |
| History | Filter, edit, copy, change status, trash, or reopen completed and cancelled tasks. |
| List-deletion confirmation | Confirm moving a list's tasks to the default list before removing the list. |
| Language picker | Choose English or Danish and save the interface language. |
| Task editor | Enter Title or Description. |
| List field | Select a draft list and open its picker. |
| Editor list picker | Choose an ordinary list for an unsaved draft. |
| Due date field | Enter a supported editor date or use a preset while retaining all task-editor controls. |
| Filter editor | Enter a task filter. |
| List-name editor | Enter a new or replacement list name. |
| Discard confirmation | Choose whether to discard changed task fields. |
| Help | Read and scroll through shortcuts. |
| Copy text | Scroll plain task text for native terminal selection. |

The same key may trigger different actions in different modes.
The list picker retains its existing configurable bindings and conflict checks and adds `delete_list`.
Text input modes do not execute task-navigation writes.
Due date field mode includes every normal task-editor action and the due preset actions.
Preset actions in that mode edit only the draft field.
List field mode includes normal task-editor actions except Copy selection, and adds Open and Lists.
Editor list picker mode permits only Up, Down, Open, Lists, Back, Help, Language, Refresh, and Toggle mouse.
Neither List mode permits text input or task-navigation writes. Choice Undo and Redo remain built-in List field controls.
The navigation list picker retains its existing management actions.
Configured actions take precedence over built-in text-editing shortcuts in their active mode.
Discard confirmation uses fixed choice keys defined by the terminal-interface spec and retains configured Discard and Refresh actions.
List-deletion confirmation uses fixed choice keys and retains configured Back and Refresh actions.
History has a separate input mode so `reopen` and Trash's `restore` can both use `r`.
The language picker uses the configured Up, Down, Apply, and Back actions.
Removed action names `anytime` and `maybe` are unknown actions.
Existing configuration files must remove those overrides before loading.

#### Scenario: Reject a navigation conflict

- **WHEN** the same key is assigned to both task creation and quit
- **THEN** configuration loading returns an error that names the conflicting actions

#### Scenario: Allow Enter in separate modes

- **WHEN** Enter opens a task in navigation and applies a filter in the filter editor
- **THEN** configuration loading accepts both shortcuts

#### Scenario: Reject a removed action

- **WHEN** `[keys]` contains an `anytime` override
- **THEN** configuration loading reports an unknown action before opening the task database

#### Scenario: Preserve an explicit editing-mode action override

- **WHEN** the user assigns an otherwise unclaimed Ctrl+A shortcut to Save in the task editor
- **THEN** Ctrl+A invokes Save in that mode instead of selecting field text

#### Scenario: Use a configured picker shortcut

- **WHEN** `[keys]` assigns the `lists` action to Ctrl+L
- **THEN** Ctrl+L opens or cancels the picker and its hint shows Ctrl+L

#### Scenario: Reuse the removal key in separate modes

- **WHEN** `trash` and `delete_list` both use Delete
- **THEN** configuration loading accepts the bindings because task navigation and the picker use different modes

#### Scenario: Reject a History conflict

- **WHEN** `reopen` and `history` are both assigned `h`
- **THEN** configuration loading reports their conflict in History

#### Scenario: Leave root shortcuts unassigned by default

- **WHEN** configuration provides neither `all_tasks` nor `planned`
- **THEN** neither `a` nor `p` invokes root navigation
- **AND** Back and the All tasks picker entry retain their navigation behavior

#### Scenario: Reject a due-field action conflict

- **WHEN** `[keys]` assigns `due_today` to Ctrl+S while Save retains Ctrl+S
- **THEN** configuration loading reports the conflict in Due date field mode before opening the task database

#### Scenario: Keep a preset character literal in another field

- **WHEN** Title is focused and Today retains its `t` default
- **THEN** `t` is handled as text input rather than a configured preset action

#### Scenario: Rebind the editor picker

- **WHEN** `lists` is bound to Ctrl+L and the task editor's List selector is focused
- **THEN** Ctrl+L opens its picker, and Ctrl+L cancels that picker without changing the draft
- **AND** the visible hint shows the configured binding

#### Scenario: Reject an editor-picker conflict

- **WHEN** `lists` and `open` have the same key
- **THEN** configuration loading rejects their conflict in the List field and editor list picker modes

### Requirement: Persist an interface-language choice without replacing configuration

Husker SHALL save a language-picker choice to the active configuration path before switching the running interface language.
The path is the selected profile's `config.toml`, unless startup selected an explicit `--config FILE`.
A missing implicit configuration is created when saving. An explicitly selected missing configuration remains a startup error.
Saving reads the current file contents and changes only the top-level `language` value to `en` or `da`.
Existing shortcut overrides, comments, and other configuration values remain intact.
The running process keeps its already loaded shortcut bindings. A language change does not reload external shortcut edits.
The new contents replace the active file atomically only after they have been written successfully.
A save failure leaves the original configuration file and running language unchanged and reports an error.
Changing interface language does not write task data or translate user content.

#### Scenario: Keep comments and shortcuts

- **WHEN** the active configuration contains comments and `quit = ["ctrl+q"]` in `[keys]` and the user saves Dansk
- **THEN** the file contains `language = "da"` and retains those comments and the quit override
- **AND** the current quit shortcut remains Ctrl+Q

#### Scenario: Honor an explicit configuration path

- **WHEN** Husker starts with `--profile ./work --config ./shared.toml` and a language change succeeds
- **THEN** `./shared.toml` receives the new language and `./work/config.toml` is not created or changed

#### Scenario: Create the implicit configuration

- **WHEN** a profile has no `config.toml` and the user saves Dansk in the language picker
- **THEN** Husker creates that profile's `config.toml` with `language = "da"`

#### Scenario: Preserve changes made before saving

- **WHEN** another editor changes a comment or shortcut in the active config after Husker starts but before language saving reads the file
- **THEN** saving the language retains that comment or shortcut edit in the file
- **AND** the running process keeps its already loaded shortcuts until restart

#### Scenario: Keep the prior state after a save failure

- **WHEN** writing or replacing the active configuration fails
- **THEN** the previous file remains unchanged and the interface keeps its previous language
- **AND** the picker reports the failure without discarding the underlying screen or draft
