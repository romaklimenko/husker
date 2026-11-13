## MODIFIED Requirements

### Requirement: Refresh dates and external changes

Husker SHALL check for external database commits at least once per second and provide an F5 refresh action in every screen.
The interface SHALL show a localized notice when another database connection changes data, unless a sync made the change.
A commit that increased the `commits` counter in the sync state is a sync commit. It refreshes the tasks without the notice.
The requirement `Sync in the background` defines the message a sync commit shows when it changed lists or tasks.
When the once-per-second check finds lists or tasks changed although the sync commits since the last check changed none, another connection also committed, and the notice appears.

Navigation results refresh automatically. Local date changes recalculate planned groups and default status visibility.
Refresh preserves selection by task or list ID where possible.
Refresh never replaces text being entered in an editor, filter, or list-name form.
An external update during editing shows a notice while preserving the draft and its originally read task version.
F5 acknowledges a notice that was already visible. A newly detected external update still produces a notice.
After leaving an input form, Husker refreshes stored tasks before drawing navigation results.
When the profile has sync settings, F5 also starts a sync.

#### Scenario: Observe a command-line change

- **WHEN** another Husker command adds a matching task while the interface is in navigation mode
- **THEN** the task appears within one refresh interval
- **AND** a notice identifies that another connection changed the database

#### Scenario: Refresh while editing

- **WHEN** another connection changes a task while the user edits its description and the user presses F5
- **THEN** the entered description and original version remain in the editor
- **AND** the interface shows the external-update notice

#### Scenario: Cross local midnight

- **WHEN** the local date advances to the next day
- **THEN** the next navigation refresh places yesterday's due open tasks in Earlier
- **AND** tasks completed yesterday leave the default results

#### Scenario: Notice a script that adds and syncs

- **WHEN** a script runs `husker add "Buy milk"` and `husker sync` within one second while the interface is in navigation mode
- **THEN** the task appears and the notice identifies that another connection changed the database

#### Scenario: Show a task from another device

- **WHEN** a sync adds a task from another device while the interface is in navigation mode
- **THEN** the task appears within one refresh interval with the message `Synced changes from another device.`
- **AND** no external-update notice appears

## ADDED Requirements

### Requirement: Sync in the background

Husker SHALL sync on a worker thread when the profile has sync settings, so that no key press waits for the network.
A profile has sync settings when its directory contains `sync.toml`, as the device-sync spec defines.
The interface opens with the local tasks before the first sync finishes.

| Trigger | Sync starts |
| --- | --- |
| Start | Immediately. |
| Local change | 60 seconds after the last commit to the database by any process. |
| Timer | 60 seconds after the previous sync ended. |
| Return from idle | On the first key, paste, or mouse event after 60 seconds without one. |
| F5 | Immediately. |
| Exit | Before the terminal is restored, only when the local database holds unsent changes. |

After a failed sync, the next attempt waits for the timer, F5, or a return from idle.
The exit sync shows `Syncing...`, or `Synkroniserer...` in Danish, and makes one attempt that ends within 10 seconds.
When the exit sync fails, Husker restores the terminal, writes a warning to stderr that the changes remain on this computer, and exits successfully.

The interface shows these localized messages. Each follows the requirement `Expire action messages after five seconds`.

| Event | English message | Shown as |
| --- | --- | --- |
| A sync changed local lists or tasks | `Synced changes from another device.` | Message |
| A sync created a conflict copy | `Sync kept both versions of a task. Search for #conflict.` | Message |
| A sync failed, once until a sync succeeds again | `Sync failed: REASON Your changes stay on this computer.` | Error |

REASON is a localized sentence for the failure kind.
The device-sync requirement `A failed sync keeps local data and hides the URL` lists the failure kinds.
A sync that changed nothing locally shows no message.
Sync never replaces text being entered and never changes the selected screen.

#### Scenario: Send a new task after a quiet minute

- **WHEN** the user creates a task and no process commits to the database for 60 seconds
- **THEN** Husker uploads the task without any key press

#### Scenario: Fetch when the user returns

- **WHEN** the user presses a key after five minutes without input
- **THEN** Husker starts a sync and handles the key without waiting for it

#### Scenario: Report a failure once

- **WHEN** the network is unavailable for ten minutes
- **THEN** the interface shows one sync failure message with the offline reason
- **AND** tasks stay editable

#### Scenario: Exit with unsent changes

- **WHEN** the user creates a task and quits ten seconds later
- **THEN** Husker shows `Syncing...`, uploads the task, and exits

#### Scenario: Exit without changes

- **WHEN** the user quits with no unsent changes
- **THEN** Husker exits without a network request
