## MODIFIED Requirements

### Requirement: Filter tasks interactively

Husker SHALL apply the task filter to the current task view while the user types it.

The filter action opens a single-line input above the task rows.
Each edit that leaves valid filter text applies that text to the task rows at once and selects the start item of the matching tasks.
The requirement Select the start item when no task is known defines the start item.
While the text is not valid, the task rows keep the results of the last valid text and the input border is red.
Enter keeps valid filter text, returns to navigation, and selects the start item of the matching tasks. Enter on text that is not valid shows the filter error and keeps the input open.
Esc closes the input and restores the filter, results, held rows, selected task, and scroll position from before the input opened. When a refresh leaves a deleted list while the input is open, Esc restores the All tasks filter instead.
While the input is open, the footer shows the configured Apply, Back, and Copy selection keys and the filter syntax hint.
An empty filter removes text restrictions while preserving any list scope and the default status rules.
Back in navigation with a nonempty applied filter clears that filter, keeps the current view, and keeps the selected task when it is still visible. Otherwise it selects the start item. It does not leave the view.
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

### Requirement: Navigate All tasks and lists

Husker SHALL open All tasks as the top-level task view and use a modal list picker to choose an ordinary list or Trash.
The global view heading is `All tasks`. A selected list uses a breadcrumb such as `All tasks > Work`; Trash uses `All tasks > Trash`.
The `lists` action opens the picker from any task view. Back with an empty picker filter cancels the picker. A `lists` key that is not a printable character also cancels it, because printable keys type into the picker filter.
Canceling the picker preserves the underlying view, filter, selected task, and scroll position.
The initial picker selection matches the current scope: the ordinary list by ID, or Trash.
From All tasks or all-list History, the picker selects the ordinary list most recently visited in the running process when it still exists, and All tasks otherwise.
Choosing the current scope closes the picker without resetting task state.
Choosing a different ordinary list or Trash starts that view with an empty filter and its start item selected, if present.
The requirement Select the start item when no task is known defines the start item.
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
Choosing another ordinary list exits History and starts that list with an empty filter and its start item selected.
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

### Requirement: Wrap keyboard selection and jump to boundaries

Husker SHALL wrap configured Up and Down selection and support configured First and Last actions in the following selection modes.

| Mode | Selectable items |
| --- | --- |
| Ordinary task view | Tasks in the current visible query results. |
| History | Closed tasks in the current visible History results. |
| Trash | Tasks in the current visible Trash results. |
| Navigation list picker | All tasks, ordinary lists, and Trash in their existing order. |
| Editor list picker | Ordinary destination lists in their existing order. |
| Move-task picker | Ordinary destination lists in their existing order. |
| Language picker | English, then Dansk. |

Up on the first item selects the last item. Down on the last item selects the start item.
First selects the start item and Last selects the last item without changing result order or applying a picker choice.
The ordinary task view is All tasks or a list view. Its start item is the first task whose row is not under the Earlier heading.
When every visible task in the ordinary task view is under Earlier, its start item is the first task.
In every other mode, the start item is the first item. History and Trash have no Earlier heading.
A row is under Earlier when the nearest heading above it is Earlier, or Tidligere in Danish. A held row counts under the heading it shows. The `planning-search` spec owns the date groups.
When the last item is also the start item, Down on it keeps it selected.
Up and Down reach every item. When tasks under Earlier precede the start item, Up from the start item selects the last task under Earlier.
Default keys are Up or `k` and Down or `j`.
First defaults to Home, `g`, and Left. Last defaults to End, `G`, and Right.
Configured PageUp and PageDown move the selection by the number of selectable rows last displayed in that selection area, not counting heading rows, clamped at the first or last item without wrapping. They are unavailable in the language picker.
In Help, Up, Down, PageUp, PageDown, First, and Last scroll the text by one row, one viewport, or to an end. In Copy text, PageUp and PageDown scroll by one viewport.
An empty result has no selection and each movement action is a no-op. One item stays selected after every movement action.
Group headings remain unselectable and do not count as items.
Selection uses the filtered visible results. After wrapping, jumping, or paging, the selected target is visible within the existing scrollable layout.
Selection does not save tasks, choose a list, change the interface language, or alter task and list positions.
Refresh retains its existing identity-preservation and missing-selection clamping rules. It does not wrap a removed selection.
The requirement Select the start item when no task is known defines when Husker selects the start item without a movement key.
Mouse wheels remain bounded. Text cursor movement, Help and Copy text scrolling, and reordering retain their bounded behavior.
First and Last have no selection action in text fields, the closed List selector, Copy text, or confirmation screens.
Left and Right retain text-cursor movement in text inputs and horizontal scrolling in Copy text.

#### Scenario: Wrap a filtered task list without selecting headings

- **WHEN** a query shows task 3 under Today and task 8 under Tomorrow and task 3 is selected
- **THEN** Up selects task 8 and makes its row visible
- **AND** Down returns to task 3 without selecting either heading

#### Scenario: Jump through a long task view

- **WHEN** a task view with no task under Earlier has more rows than fit on screen and the user invokes Last then First
- **THEN** the last visible-result task and then the first visible-result task are selected and scrolled into view

#### Scenario: Skip Earlier with First and with Down from the last task

- **WHEN** All tasks shows `Old report` under Earlier, `Pay rent` under Today, and `Call the bank` under Tomorrow, and `Old report` is selected
- **THEN** Left selects `Pay rent`
- **AND** Right selects `Call the bank`, and Down then selects `Pay rent`
- **AND** Up then selects `Old report`

#### Scenario: Start after Earlier when nothing is planned today

- **WHEN** a list view shows two tasks under Earlier and one task under Sometime
- **THEN** Home selects the task under Sometime

#### Scenario: Use the first task when every task is under Earlier

- **WHEN** the filter `due:overdue` shows three tasks, all under Earlier, and the third is selected
- **THEN** Down selects the first task, and `g` keeps it selected

#### Scenario: Count a held row under its heading

- **WHEN** All tasks shows one task under Earlier and one under Today, and the user marks the Earlier task done
- **THEN** the done task stays held under Earlier
- **AND** Left selects the task under Today

#### Scenario: Keep the last task when it is the start item

- **WHEN** All tasks shows two tasks under Earlier and one task under Today, and the task under Today is selected
- **THEN** Down keeps the task under Today selected

#### Scenario: Wrap History and Trash

- **WHEN** the last visible task is selected in History or Trash and the user invokes Down
- **THEN** the first visible task becomes selected without changing any task data

#### Scenario: Keep empty and single-item results stable

- **WHEN** a selection view has zero or one item and receives Up, Down, First, Last, PageUp, or PageDown
- **THEN** zero items remain unselected and one item remains selected without an error

#### Scenario: Navigate both list pickers

- **WHEN** a navigation list picker or editor list picker receives Up on its first item
- **THEN** its last item is selected without choosing it
- **AND** First returns to the first item and Last selects the last item

#### Scenario: Navigate language choices without applying them

- **WHEN** English is selected in the language picker and the user invokes Up
- **THEN** Dansk becomes selected without saving or changing the active language
- **AND** First selects English and Last selects Dansk

#### Scenario: Keep wheel and non-selection controls bounded

- **WHEN** the user wheels past the last task or picker item
- **THEN** selection remains at that last item
- **AND** text editing, Help/Copy text scrolling, and manual reordering do not wrap at their boundaries

#### Scenario: Page through a long task view

- **WHEN** a task view shows 10 rows, 30 tasks match, and the first task is selected
- **THEN** PageDown selects a task about ten rows further and scrolls it into view
- **AND** repeated PageDown stops at the last task and PageUp returns toward the first


### Requirement: Restore All tasks after visiting a scoped view

Husker SHALL retain the All tasks filter, parsed query, selected task ID, and task scroll state when leaving All tasks for an ordinary list or Trash.
Switching between ordinary lists or Trash preserves that original All tasks state.
Back from a scoped view, the `all_tasks` action, and choosing All tasks restore that state against fresh database contents.
If the saved selected task is still visible, it becomes selected again by ID.
If the saved task is no longer visible, Husker selects the start item, as the requirement Select the start item when no task is known defines.
An empty result has no selected task. Rendering clamps the restored scroll position to the available rows and keeps the selected task visible.
Invoking `all_tasks` while already at All tasks preserves the current filter, selection, and scroll position.
The saved All tasks state lasts only for the running process.

#### Scenario: Restore a filtered and scrolled All tasks view

- **WHEN** task 42 is selected in a scrolled All tasks view filtered by `milk`, the user visits Work, and then presses Esc
- **THEN** All tasks returns with filter `milk`, task 42 selected if still visible, and its saved scroll position within current row bounds

#### Scenario: Keep the original parent across list switches

- **WHEN** the user leaves All tasks for Work, switches from Work to Home through the picker, and presses Esc
- **THEN** Husker restores the original All tasks state from before Work was opened

#### Scenario: Recover after the selected task leaves the results

- **WHEN** the saved All tasks task is removed from its results while a scoped view is open
- **THEN** returning to All tasks selects its start item, or no task when the results are empty

### Requirement: Review retained completed and cancelled tasks

Husker SHALL provide History for the current ordinary list or All tasks through the configured `history` action, which defaults to `h`.
History includes all currently done or cancelled non-Trash tasks in that scope, including today and unknown status dates.
History follows the date, timestamp, and ID ordering defined by planning-search.
Tasks completed later on the same local day appear first when their timestamps are known. Unknown completion times are not reconstructed.
History groups tasks under localized Today, Yesterday, calendar-date, or Unknown date headings based on `status_changed_on`.
The display date remains the local calendar date even though same-day ordering uses a UTC timestamp.
The headings identify each task's recorded status date.
History rows do not append the stored planned date as metadata. Task titles, including dates typed into titles, remain unchanged.
The planned date remains available in the editor and retains its row suffix in other task views where the row-suffix requirement shows it.
Unknown closure dates remain Unknown date even when a task has a known planned date.
History uses breadcrumbs such as `All tasks > History` and `All tasks > Work > History`.
History is a view of retained task records. It does not claim to preserve every status transition, edit, or previous task text.
Reopening clears the task's status date and timestamp according to task storage and removes the task from History after the write succeeds.
History starts with an empty filter and remembers the origin view, filter, parsed query, task ID, and scroll state.
The `history` action restores that origin against fresh data, preserving task identity when present and selecting the start item otherwise.
Back with an active History filter clears the filter first. Back without a filter restores the origin in the same way as the `history` action.
The `all_tasks` action returns to the saved root task state. Choosing a picker entry follows the navigation requirement.
History supports filtering, editing, copying, status changes, task Trash, moving tasks to another list, undo, reopening, and reading task descriptions.
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

#### Scenario: Distinguish the completion day from the planned day

- **WHEN** a task planned for November 12 was completed on November 9 and is shown in History
- **THEN** its heading uses the November 9 status date and its row has no November 12 planned-date suffix
- **AND** opening the editor still shows November 12 as Planned date

#### Scenario: Keep an unknown closure date unknown

- **WHEN** a completed task has a planned date but no recorded status date
- **THEN** History places it under Unknown date without appending the planned-date suffix or inventing a closure day

#### Scenario: Clear a History filter with Back

- **WHEN** History is filtered by `milk` and the user presses Esc
- **THEN** History shows all its tasks and remains open
- **AND** a second Esc restores the origin view


## ADDED Requirements

### Requirement: Select the start item when no task is known

Husker SHALL select the start item of a task view whenever it shows rows without a known task to select.

The requirement Wrap keyboard selection and jump to boundaries defines the start item. In All tasks and list views, it is the first task whose row is not under the Earlier heading, or the first task when every visible task is under Earlier. In History and Trash, it is the first task.
A known task is a task that was selected or saved for the view and is still among its visible results.
A filter edit and Enter in the filter input have no known task. They select the start item of the matching tasks even when the selected task still matches.
A refresh, Esc in the filter input, and a language change keep the row position when the selected task leaves the results. They do not select the start item in that case.
A view has saved state in two cases: All tasks after the user leaves it for a list view or Trash, and the view from which History was opened. The requirements Restore All tasks after visiting a scoped view and Review retained completed and cancelled tasks define that state.
The following table lists each situation and the task that Husker selects.

| Situation | Selected task |
| --- | --- |
| Husker starts | The start item of All tasks |
| A list view, Trash, or History opens without saved state | The start item |
| A view returns to its saved state, and the saved task is visible | The saved task |
| A view returns to its saved state, and the saved task is no longer visible | The start item |
| The current list is deleted, and All tasks returns to its saved state | The saved task when visible, otherwise the start item |
| A refresh finds rows in a view that had no selected task | The start item |
| A refresh while a task is selected | The selected task, or the same row position when the task left the results |
| An edit in the filter input leaves valid filter text | The start item of the matching tasks |
| Enter applies valid filter text | The start item of the matching tasks |
| Esc closes the filter input | The task selected before the input opened, or that row position when the task is gone. The start item when no task was selected. |
| The interface language changes | The selected task, or the same row position when the task left the results. The start item when no task was selected. |
| Back clears an applied filter | The selected task when still visible, otherwise the start item |
| Choosing the current scope in the list picker | The selected task stays selected |

An empty view has no selection. Selecting the start item changes no stored data and does not end a held row.

#### Scenario: Start at today's first task

- **WHEN** Husker starts and All tasks shows `Old report` under Earlier, `Pay rent` under Today, and `Someday` under Sometime
- **THEN** `Pay rent` is selected

#### Scenario: Select the start item while typing a filter

- **WHEN** All tasks shows `Work earlier` under Earlier and `Work soon` under a later date, and the user presses `/` and types `work`
- **THEN** `Work soon` is selected after each letter
- **AND** `Work soon` stays selected after Enter

#### Scenario: Replace a saved task that is gone

- **WHEN** All tasks shows `Old report` under Earlier, `Pay rent` under Today, `Call the bank` under Tomorrow, and `Someday` under Sometime. The user leaves All tasks for Work with `Call the bank` selected, `Call the bank` moves to Trash, and the user presses Esc in Work
- **THEN** All tasks returns with `Pay rent`, its start item, selected

#### Scenario: Return to a saved task that is still visible

- **WHEN** the user leaves All tasks for Work with `Someday` selected and presses Esc in Work
- **THEN** All tasks returns with `Someday` selected

#### Scenario: Clear a filter whose selected task is not in the default results

- **WHEN** All tasks holds `Old report` under Earlier and `Pay rent` under Today, the user filters it by `s:done`, and selects `Filed taxes`, which was completed last week
- **THEN** Esc clears the filter and selects `Pay rent`, because the default results do not show `Filed taxes`

#### Scenario: Fill an empty view

- **WHEN** the Later list is empty and open, and a refresh brings `Later old` under Earlier and `Later today` under Today
- **THEN** `Later today` is selected
- **AND** after the user selects `Later old`, a later refresh keeps `Later old` selected
