## MODIFIED Requirements

### Requirement: Suggest a list for a new task

Husker SHALL select a suggested list for a new task created from All tasks. The suggested list is the current list of the newest counted task, which is the counted task with the highest task ID.

A counted task is outside Trash and belongs to an accessible ordinary list. Open, done, and cancelled tasks count.
A task counts with its current list, even when it moved after creation.
How many counted tasks each list holds does not affect the suggestion. A filter applied to All tasks does not change which tasks are counted.
A task that another device created before a local task can get a higher ID when a sync inserts it, as the `device-sync` requirement Local IDs are never reused for another record defines. The suggestion still uses the highest task ID.
When no task is counted, the suggested list is the lowest-ID ordinary list.
The suggestion only sets the draft's initial List value. Choosing another list, undo, redo, and Save follow the requirement Choose a list for a draft by stable identity.
List views, the CLI `add` command, and the MCP `add_task` tool do not use the suggestion.

| Case | Suggested list |
| --- | --- |
| No task outside Trash | The lowest-ID ordinary list |
| The highest-ID task is in Trash | The list of the highest-ID counted task |
| The highest-ID counted task moved after creation | The list it is in now |
| Another list holds more counted tasks | The list of the highest-ID counted task |

#### Scenario: Suggest the list of the newest task

- **WHEN** 9 of the 10 newest tasks are in Labs, the newest task is in Private, and the user presses `n` in All tasks
- **THEN** the editor opens with Private in List and empty Title, Description, and Planned date

#### Scenario: Follow a newer task in another list

- **WHEN** the newest task is in Private, an agent then adds a task to Labs with the MCP `add_task` tool, and the user presses `n` in All tasks
- **THEN** the editor opens with Labs in List

#### Scenario: Ignore tasks in Trash

- **WHEN** task 13 is the highest-ID task and is in Errands and in Trash, task 12 is in Private and outside Trash, and the user presses `n` in All tasks
- **THEN** the editor opens with Private in List

#### Scenario: Count a closed task

- **WHEN** the newest task is a done task in Work and the user presses `n` in All tasks
- **THEN** the editor opens with Work in List

#### Scenario: Count a moved task by its current list

- **WHEN** the newest task was created in Inbox and later moved to Work, and the user presses `n` in All tasks
- **THEN** the editor opens with Work in List

#### Scenario: Fall back to the default list

- **WHEN** every task is in Trash and the user presses `n` in All tasks
- **THEN** the editor opens with the lowest-ID ordinary list in List

#### Scenario: Keep the list of a list view

- **WHEN** the newest task is in Work and the user presses `n` while viewing Private
- **THEN** the editor opens with Private in List
