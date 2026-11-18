## MODIFIED Requirements

### Requirement: Use keyboard actions

Husker SHALL provide the following default shortcuts in task navigation, History, and Trash and resolve configurable action shortcuts through the selected configuration.

| Key | Action |
| --- | --- |
| Up or `k` | Select the previous task |
| Down or `j` | Select the next task |
| Left, Home, or `g` | Select the first visible task |
| Right, End, or `G` | Select the last visible task |
| PageUp or PageDown | Move the selection one displayed page up or down |
| `l` | Open the list picker |
| `h` | Open History from ordinary task navigation, or return from History |
| `t` | Set the selected task due today |
| `T` | Set the selected task due tomorrow |
| `w` | Set the selected task due next week on Monday |
| `x` | Clear the selected task's due date |
| `m` | Move the selected task to another list |
| `u` | Undo the last recorded task action |
| `/` | Edit the task filter |
| Enter | Edit the selected task |
| Esc | Clear an active filter, or return from a scoped task view to All tasks |
| `n` | Create a task from All tasks or a list view |
| `r` | Refresh database contents in All tasks and list views, restore the selected task in Trash, or reopen it in History |
| Space | Toggle the selected task between open and done |
| `c` | Toggle the selected task between open and cancelled |
| `d` or Delete | Move a task to Trash. In Trash, start deleting the selected task forever |
| `D` | Start emptying Trash while Trash is open |
| Alt+Up or `K` | Move an open task before its eligible neighbor |
| Alt+Down or `J` | Move an open task after its eligible neighbor |
| F1 or `?` | Show help with the effective shortcuts |
| F2 | Toggle mouse capture |
| F3 | Choose and save the interface language |
| F4 | Show the selected task's plain text for copying |
| F5 | Refresh database contents on every screen |
| `q` or Ctrl+C | Quit |

The list picker types printable characters into its filter, as the requirement Filter list pickers by typing defines. It uses these default shortcuts.

| Key | Action in the list picker |
| --- | --- |
| Up or Down | Select the previous or next picker entry |
| Left or Home | Select the first picker entry |
| Right or End | Select the last picker entry |
| PageUp or PageDown | Move the selection one displayed page up or down |
| Enter | Choose the selected entry |
| Esc | Clear the picker filter, or cancel the picker when the filter is empty |
| Ctrl+N | Create a list |
| Ctrl+R | Rename the selected ordinary list |
| Ctrl+D or Delete | Start deleting the selected list, or start emptying Trash when the Trash entry is selected |
| Alt+Up or Alt+Down | Move the selected list up or down |
| F1 | Show help |
| F2, F3, or F5 | Toggle mouse capture, choose the language, or refresh |
| Ctrl+C | Quit |

Keyboard Up/Down wrap, First/Last jump, and PageUp/PageDown page according to the selection requirement below.
Ctrl+C quits only in task navigation, History, Trash, and the list picker. Text inputs keep Ctrl+C for Copy selection.
Actions that require a selected task do nothing when no task is selected.
Creating, renaming, moving, or deleting a list requires the picker. All tasks and Trash cannot be renamed, moved, or deleted.
The default ordinary list cannot move or be deleted. Other list moves follow the pinned boundaries defined by task storage.
Task creation, editing, completion, cancellation, moving to Trash, task reordering, moving to another list, and undo are unavailable in Trash.
Restore and Delete forever are available only in Trash. Empty Trash is available in Trash and from the Trash entry of the list picker.
The requirements Delete a Trash task forever and Empty Trash after confirmation define both confirmations.
Task actions do not affect background tasks while the picker is open.
Due-date presets are unavailable in Trash and use the navigation and editor contracts below.
The `profiles-localization` spec owns configuration syntax and binding validation. The optional `all_tasks` action and legacy `planned` alias follow that spec's compatibility rules.
The `all_tasks` action has no default shortcut. Back and the All tasks picker entry provide normal root navigation.
History is unavailable from Trash and the picker.
History supports opening, filtering, copying, due-date presets, status changes, moving tasks to Trash, moving tasks to another list, undo, reopening, help, refreshing, and navigation.
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

#### Scenario: Quit with Ctrl+C

- **WHEN** a user presses Ctrl+C in task navigation with default bindings
- **THEN** Husker quits and restores the terminal
- **AND** Ctrl+C in a Title field still copies the selection

#### Scenario: Reorder with Shift letters

- **WHEN** two open undated tasks are listed and the user presses `J` on the first
- **THEN** the tasks swap positions as Alt+Down would

#### Scenario: Refresh with r

- **WHEN** All tasks is open with default bindings and the user presses `r`
- **THEN** Husker refreshes database contents exactly as F5 does

#### Scenario: Keep r for Restore and Reopen

- **WHEN** Trash is open and the user presses `r` on a deleted task
- **THEN** the task leaves Trash and no refresh message appears
- **AND** `r` in History reopens the selected task

### Requirement: Filter tasks interactively

Husker SHALL apply the task filter to the current task view while the user types it.

The filter action opens a single-line input above the task rows.
Each edit that leaves valid filter text applies that text to the task rows at once and selects the first matching task.
While the text is not valid, the task rows keep the results of the last valid text and the input border is red.
Enter keeps valid filter text and returns to navigation. Enter on text that is not valid shows the filter error and keeps the input open.
Esc closes the input and restores the filter, results, held rows, selected task, and scroll position from before the input opened. When a refresh leaves a deleted list while the input is open, Esc restores the All tasks filter instead.
While the input is open, the footer shows the configured Apply, Back, and Copy selection keys and the filter syntax hint.
An empty filter removes text restrictions while preserving any list scope and the default status rules.
Back in navigation with a nonempty applied filter clears that filter, keeps the current view, and keeps the selected task when it is still visible. It does not leave the view.
A list-scoped view combines its list scope with the filter.
History combines its retained list scope and closed-task restriction with the filter.
An empty History filter includes completed and cancelled tasks from every date.
An explicit `s:done` or `s:cancelled` narrows History. `s:open` matches nothing there, and `s:all` does not add open tasks.
Due filters always test stored due dates, including in finished sections and History.
The `planning-search` spec defines accepted syntax.

#### Scenario: Combine list and text filters

- **WHEN** a user applies `l:private milk #shopping` in the All tasks
- **THEN** Husker shows only matching tasks according to the `planning-search` spec

#### Scenario: Narrow results while typing

- **WHEN** All tasks holds `Buy milk` and `Call mom`, and the user presses `/` and types `mi`
- **THEN** only `Buy milk` is listed before the user presses Enter

#### Scenario: Correct an invalid filter

- **WHEN** a user types `"milk`, with an unterminated quoted value, and presses Enter
- **THEN** Husker displays a filter error and keeps the entered filter available for editing
- **AND** all tasks remain listed, because the empty text was the last valid filter text

#### Scenario: Discard a typed filter

- **WHEN** Work is filtered by `milk`, the user presses `/`, replaces the text with `mom`, and presses Esc
- **THEN** Work is filtered by `milk` again with its earlier results and selected task

#### Scenario: Filter History without adding open tasks

- **WHEN** a user applies `s:all milk` in Work's History
- **THEN** results contain only Work's non-Trash done or cancelled tasks matching `milk`, regardless of completion date

#### Scenario: Clear a filter with Back

- **WHEN** Work is filtered by `milk` with task 4 selected and the user presses Esc
- **THEN** Work shows all its tasks with task 4 selected and the filter box is gone
- **AND** a second Esc returns to All tasks

### Requirement: Keep the display usable

Husker SHALL keep the selected row visible, handle terminal resizing, and show an explicit empty state when no lists or tasks match the current view.

Task navigation uses the available content width without a side description preview.
The interface shows its current view and active filter.
Action results and errors follow the requirement to expire action messages after five seconds.
Enter opens the task editor at Description. F4 opens plain task text for copying.
A small terminal receives a readable size message instead of causing a panic.

The footer lists hint entries in priority order and packs them into at most two lines at the current width, dropping entries that do not fit. Each entry shows the first configured key and a short label.
Task views list Open, Toggle done, Toggle cancelled, Trash, the four presets as one entry, Undo, Move task, Filter, Lists, New task, History, Back outside All tasks, Help, and Quit.
Trash lists Restore, Delete forever, Empty Trash, and Copy text instead of the task edits. History lists Reopen beside the task edits.
In the task-view section, Help also lists Delete forever with the `trash` keys and Empty Trash with the `empty_trash` keys.
In the list-picker section, Help also lists Empty Trash with the `delete_list` keys.
The list picker footer lists Type to filter, Open, Back, New list, Rename list, Delete list or Empty Trash, Reorder, Help, and Quit.
The editor list picker and move-task picker footers list Type to filter, Choose or Move, Back, and Help.
A footer entry in a list picker shows the first configured key that works there, so it never shows a printable character.
Help shows the available actions and effective shortcuts for the current configuration in sections: task views, list picker, task editor, text editing, filter and list-name forms, confirmations, and everywhere.
The section for the screen that opened Help appears first. Built-in text-editing keys appear with fixed labels.
Help shows every configured key of an action that works in the screens of its section. The list-picker section omits printable keys, and the everywhere section shows the `refresh` keys that work on every screen.
The task-view section lists Refresh with its keys that work in task navigation. Esc closes help without changing stored data.
The configured Up, Down, PageUp, PageDown, First, and Last actions and the mouse wheel scroll help. Mouse clicks in help do nothing.

#### Scenario: Use the full task area

- **WHEN** a user selects a task in a wide terminal
- **THEN** the task rows occupy the available navigation width and no description preview appears beside them

#### Scenario: Scroll through tasks

- **WHEN** a user moves selection below the last visible task
- **THEN** the task list scrolls so the selected task remains visible

#### Scenario: Show an empty filter result

- **WHEN** an applied filter matches no tasks
- **THEN** Husker shows an empty-result message and no task selection

#### Scenario: Show the most-used keys

- **WHEN** All tasks is open in an 80-column English terminal with default bindings
- **THEN** the footer shows `Enter`, `Space`, `c`, `d`, `t/T/w/x`, `u`, `/`, `l`, `n`, `m`, `h`, `?`, and `q` with their labels on two lines

#### Scenario: Show Trash keys

- **WHEN** Trash is open in an 80-column English terminal with default bindings
- **THEN** the footer shows `r Restore`, `d Delete forever`, `D Empty Trash`, and `F4 Copy`
- **AND** the footer does not show `Space Done` or `m Move`

#### Scenario: Open help from the picker

- **WHEN** a user presses F1 in the list picker
- **THEN** the list picker section appears first and the text-editing section lists Ctrl+A with Select the whole field

#### Scenario: Show picker keys that work while typing

- **WHEN** the list picker is open in an 80-column English terminal with default bindings
- **THEN** the footer shows `Ctrl+N New list`, `Ctrl+R Rename list`, `F1 Help`, and `Ctrl+C Quit`
- **AND** the footer shows no `n`, `r`, `?`, or `q` entry

### Requirement: Refresh dates and external changes

Husker SHALL check for external database commits at least once per second and provide a refresh action, F5 by default, in every screen.
In All tasks and list views, `r` also refreshes by default. The `profiles-localization` requirement Key conflict validation limits printable refresh keys to that task navigation.
In this requirement, F5 stands for any key of the refresh action.
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

#### Scenario: Name the refresh keys that work

- **WHEN** another connection changes a task while All tasks is open with default bindings
- **THEN** the notice names `r / F5` as the refresh keys
- **AND** the same notice in the task editor names only F5

### Requirement: Reorder lists from the chooser

Husker SHALL let users move a selected nondefault ordinary list with Alt+Up and Alt+Down in the modal picker.
The move uses the atomic operation defined by task storage and retains selection by list ID.
All tasks remains the first picker entry. The default list remains first among ordinary lists. Trash remains last.
Unavailable moves leave the order and selection unchanged.
While the picker filter has text, Alt+Up and Alt+Down move nothing and show the message `Clear the filter to reorder lists.`

#### Scenario: Keep the order while filtering

- **WHEN** the picker filter is `wo` with Work selected and the user presses Alt+Up
- **THEN** the list order is unchanged and the message `Clear the filter to reorder lists.` appears

#### Scenario: Keep a moved list selected

- **WHEN** the picker shows All tasks, Inbox, Work, Home, Trash and the user selects Home and presses Alt+Up
- **THEN** the picker shows All tasks, Inbox, Home, Work, Trash with Home selected
- **AND** reopening the app keeps the ordinary lists in that order

#### Scenario: Keep fixed rows in place

- **WHEN** the user attempts to move All tasks, the default Inbox list, or Trash
- **THEN** no fixed row moves and no task data changes

### Requirement: Navigate All tasks and lists

Husker SHALL open All tasks as the top-level task view and use a modal list picker to choose an ordinary list or Trash.
The global view heading is `All tasks`. A selected list uses a breadcrumb such as `All tasks > Work`; Trash uses `All tasks > Trash`.
The `lists` action opens the picker from any task view. Back with an empty picker filter cancels the picker. A `lists` key that is not a printable character also cancels it, because printable keys type into the picker filter.
Canceling the picker preserves the underlying view, filter, selected task, and scroll position.
The initial picker selection matches the current scope: the ordinary list by ID, or Trash.
From All tasks or all-list History, the picker selects the ordinary list most recently visited in the running process when it still exists, and All tasks otherwise.
Choosing the current scope closes the picker without resetting task state.
Choosing a different ordinary list or Trash starts that view with an empty filter and its first task selected, if present.
Back with an active filter clears the filter as defined by the filter requirement. Back without a filter from an ordinary list or Trash returns to All tasks. Back at All tasks without a filter leaves it unchanged.
The `all_tasks` action and choosing All tasks also return to All tasks with the saved parent state defined below.
All tasks groups, task ordering, and filter matching follow the `planning-search` spec. Trash shows deleted tasks without planned groups.
Editor save and dirty-discard confirmation keep their existing behavior and return to the task view from which the editor opened.

#### Scenario: Open a list and return to All tasks

- **WHEN** a user in All tasks presses `l`, selects `Work`, and presses Enter with default shortcuts
- **THEN** Husker closes the picker and shows `All tasks > Work` with tasks from Work
- **AND** pressing Esc returns to All tasks instead of opening the picker

#### Scenario: Cancel a list choice

- **WHEN** a user opens the picker from a filtered list view, moves its selection, and presses Esc
- **THEN** the picker closes and the original task filter, selection, and scroll position remain unchanged

#### Scenario: Keep state when choosing the current list

- **WHEN** a user opens the picker from Work and chooses Work again
- **THEN** the picker closes without resetting Work's filter, selected task, or scroll position

#### Scenario: Open Trash

- **WHEN** a user chooses the last picker entry, Trash
- **THEN** Husker shows `All tasks > Trash` with deleted tasks, no planning-group headings, and an empty filter
- **AND** Esc returns to All tasks

#### Scenario: Preselect the last visited list

- **WHEN** a user visits Work, presses Esc to return to All tasks, and presses `l`
- **THEN** the picker opens with Work selected
- **AND** after Work is deleted elsewhere the picker opens with All tasks selected

History is a separate task view defined below. Choosing its underlying task scope in the picker restores its saved origin state.
Choosing another ordinary list exits History and starts that list with an empty filter and its first task selected.
Choosing All tasks exits History and restores the saved root state.
If the current ordinary list is deleted locally or externally, Husker returns to All tasks using the saved root state.
An open task editor keeps its draft when its list is deleted.
A later stale save cannot overwrite the transferred task. List-name validation or task-version validation can reject that save.

#### Scenario: Leave History through its current list

- **WHEN** History was opened from Work and the user chooses Work in the picker
- **THEN** Husker leaves History and restores Work's original filter, task selection, and scroll position

#### Scenario: Recover when the current list is removed

- **WHEN** Work is the current list and another connection deletes Work
- **THEN** Husker returns to All tasks with the saved root state after refreshing
- **AND** any open task editor keeps its entered text and originally read version

### Requirement: Keep editor list picking modal

Husker SHALL display a modal editor list picker containing only ordinary lists in their persistent order with the default-list marker.
Virtual All tasks and Trash entries and all list-management actions are unavailable.
The current draft list is selected when present. Opening the picker does not change the draft if that ID is unavailable.
Configured Up and Down wrap selection. First and Last jump to the corresponding boundary. Open chooses the selected list and returns to the List selector.
Configured Back with an empty picker filter cancels and returns to the unchanged List selector, with Esc as the default. A `lists` key that is not a printable character also cancels.
Printable keys type into the picker filter as the requirement Filter list pickers by typing defines.
Single-click selects a visible row. Double-click on the same row within 400 milliseconds chooses it.
Wheel events move picker selection only inside the popup. Borders, outside clicks, and outside wheel events do nothing.
Background editor fields do not receive popup input.
Refresh preserves the picker selection by ID when present and otherwise clamps its previous index.
Refresh does not replace the draft's chosen ID or text fields.
Help and language-picker round trips preserve the editor picker selection, scroll, and underlying draft state.

#### Scenario: Cancel a destination choice

- **WHEN** a user opens the editor picker, selects Work, and presses Esc with default bindings
- **THEN** the picker closes with the draft's original list and all other field state unchanged

#### Scenario: Keep picker scope separate from navigation

- **WHEN** the editor picker is open
- **THEN** it shows ordinary lists and the default marker without All tasks, Trash, or management actions
- **AND** background fields receive no click or wheel action

#### Scenario: Refresh a removed picker row

- **WHEN** an externally deleted list was selected in the editor picker and the user refreshes
- **THEN** the picker clamps selection to an existing row without changing the draft's selected list ID

#### Scenario: Return from Help or language selection

- **WHEN** the user opens Help or the language picker from a scrolled editor list picker and returns
- **THEN** the editor picker retains its selected list ID and scroll within current bounds
- **AND** all underlying draft fields and their histories remain unchanged

### Requirement: Move the selected task to another list

Husker SHALL provide a configurable `move_task` action, bound to `m` by default, in ordinary task navigation and History outside Trash.
The action opens a modal picker over the current task view containing only ordinary lists in their persistent order with the default marker. The task's current list is selected.
The picker uses the editor list picker key mode. Up, Down, First, Last, PageUp, and PageDown move its selection. Open chooses. Back with an empty picker filter cancels without a write, and so does a `lists` key that is not a printable character.
Printable keys type into the picker filter as the requirement Filter list pickers by typing defines.
Help, Language, Refresh, and mouse behavior follow the editor list picker requirement. The underlying task view receives no popup input.
Choosing the task's current list closes the picker without a write.
Choosing another list changes only the task's list membership through the list-only mutation defined by task storage, using the originally read version.
After a successful move the message shows Moved to and the destination name. The same task remains selected if it is still visible. Otherwise selection clamps to the prior index.
The move records an undo entry with the previous list. A stale version returns an error without changing data.
No selected task, or a task in Trash, means no action.

#### Scenario: Move a task from All tasks

- **WHEN** task 4 in Inbox is selected in All tasks and the user presses `m`, selects Work, and presses Enter
- **THEN** task 4 belongs to Work with its other fields unchanged and its row shows `[Work]`
- **AND** the message shows Moved to Work and task 4 remains selected

#### Scenario: Cancel a move

- **WHEN** the move-task picker is open and the user presses Esc
- **THEN** the picker closes and the task's list and version remain unchanged

#### Scenario: Move a task out of its scoped view

- **WHEN** the user moves the only task in Work to Home while viewing Work
- **THEN** Work shows no tasks and the task appears in Home

## ADDED Requirements

### Requirement: Filter list pickers by typing

Husker SHALL filter the entries of the list picker, the editor list picker, and the move-task picker by the text the user types while the picker is open.

A printable key adds its character to the picker filter. A printable key is a character key without Ctrl or Alt. Shift is allowed, and AltGr, which terminals report as Ctrl+Alt, also counts.
A printable key never runs an action in these pickers, even when the configuration binds it to one.
Backspace removes the last character of the picker filter. Backspace on an empty filter does nothing.
An entry matches when its visible name contains the filter text, ignoring case. The name excludes the count and the default marker. All tasks and Trash match by their localized names.
Matching entries keep their saved order. After each change that leaves filter text, the selection moves to the first match whose name starts with the filter text, or to the first match when none starts with it. When the filter becomes empty, the selected entry stays selected.
When no entry matches, the popup shows `No matching lists` in English or `Ingen lister passer` in Danish, and Open does nothing.
The popup's bottom border shows `Type to filter` in English or `Skriv for at filtrere` in Danish while the filter is empty, and `Filter: ` followed by the text otherwise.
The popup keeps the height that the unfiltered entries need, so it does not jump while the user types.
Back with filter text clears the filter and keeps the selected entry selected. Back with an empty filter cancels the picker.
Every picker opens with an empty filter. Refresh, Help, and the language picker keep the filter text and the selected entry by identity.
Creating or renaming a list from the list picker clears the filter, so the saved list can be selected.
Mouse clicks and the wheel act on the displayed entries.

| Key in a list picker | Result |
| --- | --- |
| `a` to `z`, `A` to `Z`, digits, space, punctuation, and other characters | Add the character to the filter |
| Backspace | Remove the last filter character |
| Esc | Clear the filter, or cancel the picker when the filter is empty |
| Enter | Choose the selected displayed entry |
| Up, Down, Home, End, Left, Right, PageUp, PageDown | Move the selection among displayed entries |

#### Scenario: Pick a list by its first letters

- **WHEN** the picker shows All tasks, Inbox, Husker, Labs, and Trash and the user types `hu`
- **THEN** the picker shows only Husker, selected, and Enter opens `All tasks > Husker`

#### Scenario: Prefer a name that starts with the text

- **WHEN** the ordinary lists are `Shopping` and `Home` and the user types `ho` in the move-task picker
- **THEN** both lists match and Home is selected

#### Scenario: Type a letter that is also a shortcut

- **WHEN** the list picker is open with default bindings and the user types `j`, `k`, `n`, `r`, `d`, `l`, `q`, or `?`
- **THEN** the character is added to the filter and no selection move, list creation, rename, deletion, cancel, quit, or Help happens

#### Scenario: Clear the filter before closing

- **WHEN** the editor list picker has the filter `la` and the user presses Esc twice
- **THEN** the first Esc clears the filter and keeps the editor picker open, and the second Esc returns to the List selector with the draft unchanged

#### Scenario: Show that nothing matches

- **WHEN** the user types `zz` in the list picker and no entry name contains `zz`
- **THEN** the popup shows `No matching lists` and Enter keeps the picker open

### Requirement: Color planning groups by date and status

Husker SHALL style the planning-group headings and task rows of All tasks and list views by group.

| Group | Heading | Task rows |
| --- | --- | --- |
| Earlier | Dark gray | Dark gray |
| Today | Bold black text on a yellow background | Bold |
| Tomorrow, the dated groups, and Sometime | Yellow | Default color |
| Done today and Done | Green | Dark gray |
| Cancelled today and Cancelled | Dark gray | Dark gray |

The selected row keeps the selection style: cyan, bold, and the `> ` marker.
A just-closed task held in its row keeps the dark gray style under its open heading.
History and Trash keep their styles: yellow headings in History, no headings in Trash, and task rows in the default color.

#### Scenario: Highlight today's tasks

- **WHEN** All tasks shows one task planned today and one planned tomorrow
- **THEN** the Today heading is bold black on yellow and its task row is bold
- **AND** the Tomorrow heading is yellow and its task row uses the default color

#### Scenario: Dim past and finished tasks

- **WHEN** Work shows an open task planned yesterday and a task done today
- **THEN** the Earlier heading and its row are dark gray
- **AND** the Done today heading is green and its row is dark gray

#### Scenario: Keep the selection visible on a dimmed row

- **WHEN** the selected task is in Earlier
- **THEN** its row is cyan and bold with the `> ` marker

