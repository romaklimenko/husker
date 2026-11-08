## MODIFIED Requirements

### Requirement: Key conflict validation

Husker SHALL reject unknown action names, invalid key strings, and shortcuts assigned to multiple actions in the same mode.

| Mode | Purpose |
| --- | --- |
| Task navigation | Select tasks and invoke task actions. |
| List navigation | Select, create, and rename lists. |
| Task editor | Enter task fields. |
| Filter editor | Enter a task filter. |
| List-name editor | Enter a new or replacement list name. |
| Discard confirmation | Choose whether to discard changed task fields. |
| Help | Read and scroll through shortcuts. |
| Copy text | Scroll plain task text for native terminal selection. |

The same key may trigger different actions in different modes.
Text input modes do not execute navigation actions.
Configured actions take precedence over built-in text-editing shortcuts in their active mode.
Discard confirmation uses fixed choice keys defined by the terminal-interface spec and retains configured Discard and Refresh actions.
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
