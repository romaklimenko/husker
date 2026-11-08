## MODIFIED Requirements

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
The `copy_selection` action defaults to `ctrl+c` and is active only in task, filter, and list-name input modes.
Copy-selection hints use the configured shortcut and selected language.
Conflicting input-mode action bindings, including another action bound to Ctrl+C while Copy selection keeps that default, follow the existing key-conflict validation requirement.

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
