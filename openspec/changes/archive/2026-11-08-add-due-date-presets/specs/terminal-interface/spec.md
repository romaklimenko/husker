## MODIFIED Requirements

### Requirement: Use keyboard actions

Husker SHALL provide the following default shortcuts in task navigation and the list picker and resolve configurable action shortcuts through the selected configuration.

| Key | Action |
| --- | --- |
| Up or `k` | Select the previous task or picker entry |
| Down or `j` | Select the next task or picker entry |
| `l` | Open or cancel the list picker |
| `h` | Open History from ordinary task navigation, or return from History |
| `t` | Set the selected task due today |
| `T` | Set the selected task due tomorrow |
| `w` | Set the selected task due next week on Monday |
| `x` | Clear the selected task's due date |
| `/` | Edit the task filter |
| Enter | Choose the picker entry or edit the selected task |
| Esc | Cancel the picker, or return from a scoped task view to All tasks |
| `n` | Create a task from a task view |
| `N` | Create a list from the picker |
| `R` | Rename the selected ordinary list from the picker |
| Space | Toggle the selected task between open and done |
| `c` | Toggle the selected task between open and cancelled |
| `d` or Delete | Move a task to Trash, or start deleting the selected list in the picker |
| `r` | Restore the selected task from Trash, or reopen it from History |
| Alt+Up | Move an open task before its eligible neighbor, or move the selected list up |
| Alt+Down | Move an open task after its eligible neighbor, or move the selected list down |
| F2 | Toggle mouse capture |
| F3 | Choose and save the interface language |
| F4 | Show the selected task's plain text for copying |
| F5 | Refresh database contents |
| `?` | Show help with the effective shortcuts |
| `q` | Quit |

Actions that require a selected task do nothing when no task is selected.
Creating, renaming, moving, or deleting a list requires the picker. All tasks and Trash cannot be renamed, moved, or deleted.
The default ordinary list cannot move or be deleted. Other list moves follow the pinned boundaries defined by task storage.
Task creation, editing, completion, cancellation, deletion, and task reordering are unavailable in Trash. Restore is available only in Trash.
Task actions do not affect background tasks while the picker is open.
Due-date presets are unavailable in Trash and use the navigation and editor contracts below.
The `profiles-localization` spec owns configuration syntax and binding validation. The optional `all_tasks` action and legacy `planned` alias follow that spec's compatibility rules.
The `all_tasks` action has no default shortcut. Back and the All tasks picker entry provide normal root navigation.
History is unavailable from Trash and the picker.
History supports opening, filtering, copying, due-date presets, status changes, moving tasks to Trash, reopening, help, refreshing, and navigation.
New-task creation and manual reordering are unavailable in History.

#### Scenario: Rebind an action

- **WHEN** configuration binds `quit` to `Ctrl+Q`
- **THEN** Ctrl+Q exits from task navigation or the list picker
- **AND** the help view displays Ctrl+Q as the effective quit shortcut

#### Scenario: Ignore an unavailable action

- **WHEN** a user presses Space while Trash is open
- **THEN** the deleted task keeps its stored status

#### Scenario: Use All tasks while the picker is open

- **WHEN** the picker is open over Work and the user chooses All tasks
- **THEN** the picker closes and All tasks returns with its saved task state

### Requirement: Edit task fields

Husker SHALL provide a task editor with these fields and save all valid changes together when the user invokes Save.
Default Save shortcuts are Ctrl+Enter and Ctrl+S. Esc and Ctrl+G invoke Discard with the confirmation rules defined below.

| Field | Input |
| --- | --- |
| Title | A nonempty task title |
| Description | Plain text with multiple lines |
| Due date | `YYYY-MM-DD`, or empty for no date |
| List | An existing list name |

Tab selects the next field. Shift+Tab selects the previous field. Enter inserts a newline only in Description.
Opening an existing task focuses Description. Creating a task focuses Title.
Ordinary text input does not invoke task-navigation writes.
The Due date field also accepts the preset actions defined below as draft edits. Other fields retain those characters as ordinary text.
Text editing follows the field navigation, selection, and history requirements below.
Field labels and hints use the selected language.
Validation follows the `task-storage` spec. Invalid input leaves the editor open with its entered text and an error message.
A failed write does not show a saved message.

#### Scenario: Save a description and due date

- **WHEN** a user opens a task, enters two description lines, sets its due date to `2026-11-09`, and presses Ctrl+S
- **THEN** both changes persist together
- **AND** Husker returns to navigation and shows the task in the applicable planned group

#### Scenario: Keep invalid input for correction

- **WHEN** a user enters `2026-02-30` as a due date and presses Ctrl+Enter
- **THEN** Husker displays an invalid-date message and keeps the editor open
- **AND** the stored task remains unchanged

#### Scenario: Discard confirmed edits

- **WHEN** a user changes a title, presses Esc or Ctrl+G, and confirms Yes in the discard prompt
- **THEN** Husker returns to navigation without changing the stored task

### Requirement: Review retained completed and cancelled tasks

Husker SHALL provide History for the current ordinary list or All tasks through the configured `history` action, which defaults to `h`.
History includes all currently done or cancelled non-Trash tasks in that scope, including today and unknown status dates.
History follows the date, timestamp, and ID ordering defined by planning-search.
Tasks completed later on the same local day appear first when their timestamps are known. Unknown completion times are not reconstructed.
History groups tasks under localized Today, Yesterday, calendar-date, or Unknown date headings based on `status_changed_on`.
The display date remains the local calendar date even though same-day ordering uses a UTC timestamp.
The headings identify each task's recorded status date. Stored due dates remain available in the editor.
History uses breadcrumbs such as `All tasks > History` and `All tasks > Work > History`.
History is a view of retained task records. It does not claim to preserve every status transition, edit, or previous task text.
Reopening clears the task's status date and timestamp according to task storage and removes the task from History after the write succeeds.
History starts with an empty filter and remembers the origin view, filter, parsed query, task ID, selection index, and scroll state.
Back or the `history` action restores that origin against fresh data, preserving task identity when present and clamping the prior index otherwise.
The `all_tasks` action returns to the saved root task state. Choosing a picker entry follows the navigation requirement.
History supports filtering, editing, copying, status changes, task Trash, reopening, and reading task descriptions.
Status changes refresh History after the write. Tasks that become open or move to Trash leave its results.
Its empty state explains that no completed or cancelled tasks match and identifies the configured return shortcut.

#### Scenario: Find an old completion

- **WHEN** Work contains a non-Trash task completed last month and the user presses `h` from Work
- **THEN** History shows that task with its recorded status date
- **AND** unrelated lists, open tasks, and Trash tasks are absent

#### Scenario: Keep legacy tasks with unknown dates

- **WHEN** a migrated completed task has no status-change date
- **THEN** History includes it after all tasks with recorded dates and labels the date as unknown

#### Scenario: Reopen from History

- **WHEN** the user selects a done or cancelled task in History and invokes `reopen`
- **THEN** the task becomes open, its status date and timestamp are cleared, and it leaves History
- **AND** Back restores the originating task view with current database contents

#### Scenario: Return to a filtered task view

- **WHEN** task 42 is selected in a scrolled Work view filtered by `milk`, the user opens History, and then presses Esc or `h`
- **THEN** Work returns with filter `milk`, task 42 selected if still visible, and the saved scroll position within current bounds

## ADDED Requirements

### Requirement: Apply due-date presets to the selected task

Husker SHALL apply a due-date preset immediately to the originally selected task in ordinary task navigation or History, outside Trash.
The planning-search spec defines the resolved date. The task-storage due-only mutation owns access checks, version validation, field preservation, and no-op behavior.
The action uses the selected task's ID and originally read version. No selected task means no action.
A stale task returns an error without overwriting newer data. A no-op makes no database write or version increment.
After success, the same task remains selected by ID if it still matches the current view and filter.
If it no longer matches, selection clamps the prior index to remaining results. An empty result has no selection.
Presets change no title, description, list, status, status-change date and timestamp, Trash state, or saved position.
Closed tasks remain in their existing status group or History unless a filter excludes their changed due date.
At midnight, the preset uses the new local date but retains the task identity and version from before result refresh.
Errors and action results use the selected language and existing message-lifetime rules.
The four actions appear in Help with configured shortcuts. They add no persistent navigation-header hint.

#### Scenario: Move an open task to Tomorrow

- **WHEN** an undated open task is selected on `2026-11-08` and the user presses `T` with default bindings
- **THEN** the stored due date becomes `2026-11-09` and the task moves to Tomorrow
- **AND** that same task remains selected

#### Scenario: Leave a due-date filter

- **WHEN** a selected task matches `due:none` and the user invokes Today
- **THEN** its date changes and it leaves the filtered results
- **AND** selection clamps the previous index or becomes empty when no results remain

#### Scenario: Change a date in History

- **WHEN** a done task in History receives Tomorrow
- **THEN** only its due date and version change
- **AND** its status date, timestamp, and History ordering remain unchanged when the active filter still matches

#### Scenario: Keep the original target across midnight

- **WHEN** a selected completed task would leave default results at midnight and the user invokes Today after midnight
- **THEN** the preset targets that task's original ID and version using the new day's date
- **AND** it does not change another task that occupies the same row after refresh

#### Scenario: Ignore a preset in Trash

- **WHEN** a user presses `t`, `T`, `w`, or `x` while Trash is open with default bindings
- **THEN** no task date, status, or version changes

### Requirement: Apply due-date presets as whole-field draft edits

Husker SHALL apply a preset to the entire Due date editor field when that field is focused, without saving the task or changing the focused field.
A dated result replaces the field with its `YYYY-MM-DD` value. Clear due date replaces it with an empty string.
A changed replacement is one undoable edit, regardless of the previous cursor position, selection, or invalid partial input.
The replacement places the cursor at the end and clears the text selection.
Undo restores the original text, cursor, and selection. Redo restores the replacement and its resulting cursor and selection.
An identical replacement changes no buffer state, undo history, task version, or draft dirtiness.
Other editor fields remain unchanged. Existing Save and dirty-discard behavior remains in force.
Today is resolved at the keypress using the current local day. A resolution error leaves the field unchanged.
Manual ISO-date typing, ordinary text editing, clipboard copying, language selection, and field navigation remain available.
The default letters `t`, `T`, `w`, and `x` remain ordinary text in Title, Description, List, filter inputs, and list-name inputs.
While Due date is focused, its localized configured preset hints replace the editor's existing editing-hint row without adding screen height.
Other fields keep their existing editing hints. Preset hints do not appear as new header rows on other screens.

#### Scenario: Replace an incomplete date and undo it

- **WHEN** Due date contains `2026-` with a partial selection and the user invokes Tomorrow on `2026-11-08`
- **THEN** the field becomes `2026-11-09`, the cursor moves to its end, and selection clears
- **AND** one Undo restores `2026-` with the original cursor and selection
- **AND** the stored task has not changed

#### Scenario: Clear a draft without saving

- **WHEN** Due date contains `2026-11-09` and the user invokes Clear due date
- **THEN** the field becomes empty and remains focused
- **AND** one Undo restores the prior field state while the stored due date stays unchanged until Save

#### Scenario: Keep other fields literal

- **WHEN** a user types `tTwx` into Description, Title, List, a filter, or a list-name form with default bindings
- **THEN** the field receives those characters as text and no preset is invoked

#### Scenario: Keep a repeated draft preset unchanged

- **WHEN** Due date already contains today's exact ISO date and the user invokes Today
- **THEN** its text, cursor, selection, undo history, and draft dirtiness remain unchanged

#### Scenario: Keep editor controls available

- **WHEN** Due date is focused after using a preset
- **THEN** Ctrl+S saves through the existing validation rules, Tab changes field, Ctrl+Z undoes, and Ctrl+C copies selected input with default bindings


### Requirement: Identify task lists in all-list views

Husker SHALL display the current list name in brackets immediately before the task title in All tasks and all-list History rows.
For example, an open task titled `Buy milk` in Work displays `[ ] [Work] Buy milk` before any existing date text.
An `l:` query filter does not remove the prefix while the containing view remains All tasks or all-list History.
Ordinary list-scoped views, list-scoped History, and Trash do not add this prefix.
The label uses the current list name after refresh, including after renaming or moving tasks between lists.
Embedded carriage returns and newlines in a list name render as spaces, using the same normalization as breadcrumbs.
The prefix occupies the existing task row and adds no extra rows.
This is display metadata only. Stored titles, editor fields, F4 copy text, CLI task titles, and MCP task titles remain unchanged.

#### Scenario: Distinguish matching titles across lists

- **WHEN** All tasks contains tasks titled `Review` in Work and Home
- **THEN** their rows display `[Work] Review` and `[Home] Review` after the status marker
- **AND** editing or copying either task still uses the stored title `Review`

#### Scenario: Keep labels after filtering all-list History

- **WHEN** all-list History applies `l:Work`
- **THEN** each matching task still shows its `[Work]` prefix
- **AND** opening Work's scoped History shows the same tasks without that prefix

#### Scenario: Refresh a renamed list label

- **WHEN** Work is renamed to Office while All tasks is open
- **THEN** the next refresh renders `[Office]` on its task rows without changing their stored titles
