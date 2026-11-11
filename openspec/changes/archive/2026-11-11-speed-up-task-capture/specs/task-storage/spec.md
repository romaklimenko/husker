## MODIFIED Requirements

### Requirement: Lists have a persistent display order

Husker SHALL return the lowest-ID ordinary list first, followed by other lists in saved position order with list ID breaking ties.
The lowest-ID list is the default list and stays first even after a rename.
Virtual Trash appears after ordinary lists where a caller includes it.
New lists append after existing ordinary lists. Renaming a list preserves its position.
CLI `add` creates a task in the list given by `--list`, or in the default list without `--list`. MCP `add_task` creates a task in the list given by `list_id`, or without `list_id` in the restricted list of a restricted server and in the default list otherwise.
A new task created from All tasks in the terminal interface starts in the list defined by the `terminal-interface` requirement Suggest a list for a new task.
Saved positions are internal storage fields and do not change the `TaskList` JSON fields.

#### Scenario: Keep a renamed default list pinned

- **WHEN** list 1 Inbox is renamed to Personal
- **THEN** Personal remains first and remains the destination of `husker add` without `--list`

#### Scenario: Append a new list

- **WHEN** the displayed ordinary lists are Inbox, Work, and Home and a user creates Admin
- **THEN** the order becomes Inbox, Work, Home, Admin, followed by virtual Trash where present

#### Scenario: Preserve a renamed list's position

- **WHEN** the displayed ordinary lists are Inbox, Work, and Home and Work is renamed to Zebra
- **THEN** the order becomes Inbox, Zebra, Home
