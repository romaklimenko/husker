## MODIFIED Requirements

### Requirement: Read tools

Husker SHALL expose these three tools in every MCP access mode.

| Tool | Result |
| --- | --- |
| `list_lists` | Accessible lists in their persistent display order, with open-task counts and a separate accessible Trash count. |
| `list_tasks` | A page of matching tasks in planning order. |
| `get_task` | All fields of one accessible task, including its version. |

`get_task` can read an accessible task in Trash. The task-storage spec owns task visibility, list order, and counts.
A list-scoped connection still returns only its selected list.

#### Scenario: Read scoped Trash counts

- **WHEN** a server is restricted to a list with one deleted task and another list has five deleted tasks
- **THEN** `list_lists` returns a Trash count of 1 and does not return the other list

#### Scenario: Read an order changed in the terminal

- **WHEN** the terminal saves list order Inbox, Home, Work and an unrestricted MCP client calls `list_lists`
- **THEN** the tool returns those lists in the same order
