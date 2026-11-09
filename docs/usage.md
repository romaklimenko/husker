# Use Husker

This guide is for people who want to capture and manage tasks from a terminal.

## Capture your first task

Run `husker` in an interactive terminal. Press `n`, type a task title, and
press Enter to save. For example, type `Buy milk`. The message `Created`
appears, and the task is listed under Sometime because it has no planned date.
Ctrl+S saves from any field. Ctrl+Enter also saves when your terminal sends
it as a distinct key.

Press Enter on a task to open its editor at Description. New tasks start at
Title. Use Tab and Shift+Tab to move between the fields:

| Field | What to enter |
| --- | --- |
| Title | A nonempty task title, such as `Buy milk`. Enter saves the task. |
| Description | Plain text. Press Enter here to add another line. |
| Planned date | `2026-11-09`, a compact date such as `261109`, or no date. Enter saves the task. |
| List | Press Enter to choose an existing list, such as `Inbox`. |

Description text wraps at the editor's width. Words move to the next visual
row when needed, and long words split across rows. Wrapping does not add
newlines to the saved text. Press Enter when you want a stored line break.
Up and Down move through the displayed rows. You can also click a wrapped
row to place the cursor there.

The heading shows an existing task's ID, such as `#42`. You can use that ID
with `husker show 42` or give it to an agent. A new task has no ID until you save it.

Press Ctrl+S or Ctrl+Enter to save every field together and return to the
task list. Enter does the same while Title or Planned date is focused.
Saving an existing task shows `Saved`.
Press Esc or Ctrl+G to leave the editor. If you changed any field, Husker asks
whether to discard the changes. Press `y` to discard them or `n` to keep
editing. No is selected initially, so Enter also keeps you in the editor.
You can also use an arrow key or Tab to select Yes, then press Enter to discard.
Esc or Ctrl+G at the prompt returns to editing as well.
An unchanged editor closes without a prompt.
Navigation shortcuts such as `q`, `d`, and `l` are ordinary text in Title
and Description. Planned date also accepts the `t`, `T`, `w`, and `x` presets
described below. The List field is a selector: Enter or `l` opens its picker.
The selector does not accept typing or paste.

Remove an existing planned date to move an open task to Sometime.
A done or cancelled task stays in its status section.
Write `#shopping` in the title or description if you want to find that text later.
Husker has no separate tags or readiness fields.

If a value is invalid, the editor stays open. Correct the value and press
Ctrl+S again. For example, replace `2026-02-30` with a date that exists.

If another Husker process changes the task while you edit, your save fails
and your text stays in the editor. Press F4 to copy any text you need to keep.
Return from the copy view with Esc. Press Esc again and then `y` to discard
the changed draft, then reopen the task to load its current values.

## Enter a date without separators

In the editor's Planned date field, type a date in any of these forms:

| Input | Result |
| --- | --- |
| `2026-11-09` | November 9, 2026. |
| `20261109` | November 9, 2026. |
| `261109` | November 9, 2026. The two-digit year means 2000 through 2099. |
| `1109` | November 9 in the current local year. |

The four-digit form always uses the current year when Husker reads the date.
For example, entering `0101` in December 2026 means January 1, 2026.
It does not jump ahead to January 2027. Use `270101` for that date.
Once the entry shows a full year, that year stays fixed. If an unfinished
`0101` entry stays compact across New Year, saving it uses the new year.

Press Tab or Shift+Tab, or click another field, to turn a valid entry into
`YYYY-MM-DD`. This changes the draft only. Ctrl+Z in Planned date restores the
original entry in one step. You can also press Enter or Ctrl+S directly to
save a compact date without leaving the field first.

Husker leaves the text alone while you type. An invalid entry such as `0230`
stays available for correction when you leave the field. Save reports the
invalid date and keeps the editor open.

An empty field shows a grey `YYYY-MM-DD` placeholder. Leave the field empty
to save no planned date. The placeholder is not part of your task text.

These compact forms work in the task editor. Use `YYYY-MM-DD` in commands,
MCP requests, and search filters, such as `due:2026-11-09`.

## Choose a task's list in the editor

Focus List with Tab and press Enter or `l` to open its picker. You can also
click the List field. Select a destination with Up or Down, then press Enter.
For example, choose Work to move the task there when you next save.
Press Esc or `l` in the picker to return without changing the draft's list.

The picker contains your ordinary lists in their saved order. The default
list has a marker. All tasks and Trash are view choices, so they do not appear
as task destinations. To create or rename a list, return to task navigation
and open its list picker with `l`.

Choosing a destination does not save the task. Your other fields keep their
unsaved text and undo history. With List focused, Ctrl+Z undoes the choice.
Ctrl+Y or Ctrl+Shift+Z restores it. Press Ctrl+S to save all task changes together.
Typing, paste, text selection, and Ctrl+C do nothing in the List selector.

Click a picker row to select it or double-click to choose it. Scroll inside
the popup to move through lists. Clicks and scrolling outside it do nothing.
F5 refreshes the list names without replacing your draft. Help and the language
picker return to the same editor picker when closed.

If another process renames the selected list, the selector shows its new name.
If that list is removed, choose a valid destination before saving.
Husker keeps the original list identity and does not substitute another list
that happens to have the same name. Undoing to a removed list also requires
a new valid choice before Save.

To change only the list of a saved task, press `m` on the task instead of
opening the editor. See [Move a task to another list](#move-a-task-to-another-list).

## Move a task to another list

Select a task in All tasks, an ordinary list, or History and press `m`.
A picker titled `Move the task to` opens over the task view. It lists your
ordinary lists in their saved order, marks the default list, and starts
with the task's current list selected. Select the destination with Up or
Down and press Enter. For example, to move `Buy milk` from Inbox to
Private, press `m`, select Private, and press Enter. The message shows
`Moved to Private`, and in All tasks the row starts with `[Private]`.

The task keeps its title, description, planned date, and status, and it
stays selected while it is still visible. If you move a task out of the
list you are viewing, it leaves that view. Choosing the task's current list
closes the picker without changing anything. Press Esc or `l` to cancel.
Press `u` afterwards to move the task back.

The shortcut does nothing in Trash. All tasks and Trash are not offered as
destinations. Help and the language picker return to the move picker when
you close them.

## Set a planned date with a shortcut

Select a task in All tasks, an ordinary list, or History and use one of these keys:

| Key | Planned date |
| --- | --- |
| `t` | Today. |
| `T` (Shift+T) | Tomorrow. |
| `w` | Next week on Monday. |
| `x` | No planned date. |

From a task row, the new planned date saves immediately. The message shows
the result, such as `Planned for 2026-11-10 Tue W46`. Clearing a date with
`x` shows `Planned date cleared (was 2026-11-10 Tue W46)`. Press `u` to put
the previous date back. A shortcut that changes nothing, such as `x` on an
undated task, writes nothing and shows no message.
Other task fields stay unchanged. A done task stays done and keeps its
completion date and time.
The same task stays selected if it still matches your filter. For example,
pressing `t` under `due:none` removes that task from the filtered results.
The shortcuts do nothing in Trash.

Next week means the next Monday after today. On Monday, November 9, `w`
sets November 16. On Sunday, November 8, it sets November 9.
Husker uses your computer's current local date, including after midnight.

In the editor, focus Planned date with Tab and use the same keys. A preset
replaces the whole field, even if you have selected only part of its text.
It does not save the task or move to another field. Press Ctrl+S to save.
Ctrl+Z restores the previous field text, cursor, and selection in one step.
You can still type a date such as `2026-11-09` manually.

The Planned date field shows the configured shortcuts while it is focused.
Press `?` to find all four actions under Task views in Help.
To rebind a preset, set an action such as `due_today = ["ctrl+t"]` under
`[keys]`. Use `due_tomorrow`, `due_next_week`, or `clear_due` for the others.
An empty array disables that preset, for example `clear_due = []`.

## Edit text with the keyboard

Use these shortcuts in Title, Description, Planned date, the filter input,
or a list-name form. List has the choice controls described above:

| Key | Result |
| --- | --- |
| Ctrl+Left or Ctrl+Right | Move by word. Punctuation forms a separate stop. |
| Ctrl+Backspace or Ctrl+Delete | Delete backward or forward to the next word boundary. |
| Home or End | Move to the start or end of a line separated by an explicit newline. |
| Ctrl+Home or Ctrl+End | Move to the start or end of the whole field. |
| Shift+Left, Shift+Right, Shift+Home, or Shift+End | Extend the selection. |
| Ctrl+Shift+Left or Ctrl+Shift+Right | Extend the selection by word. |
| Ctrl+Shift+Home or Ctrl+Shift+End | Select to the start or end of the field. |
| Ctrl+A | Select the whole current field. |
| Ctrl+C | Copy selected text to the system clipboard. |
| Ctrl+Z | Undo the last text edit in this field. |
| Ctrl+Y or Ctrl+Shift+Z | Redo an undone edit. |

Type or paste to replace selected text. Backspace or Delete removes the selection.
For example, focus Title, press Ctrl+A, and type `Read the report` to replace
the title. Description and other fields stay unchanged.
In Description, Shift+Up and Shift+Down select through the displayed wrapped rows.
Enter inserts a line break only in Description. In Title and Planned date,
Enter saves the task. Press `?` to see these keys under Text editing in Help.

Undo belongs to the current field. If you change Title and then Description,
Ctrl+Z in Description leaves the Title change in place. A paste is one undo
step. Typing new text after undo removes the previous redo choices.
Saving or discarding a form ends its undo history.
Press Ctrl+C to copy selected text without changing it or adding an undo step.
For example, select `æbler` in `Køb æbler` and press Ctrl+C to copy that word.
The selection stays highlighted. Without a selection, Ctrl+C leaves the
clipboard unchanged.
Use [F4 and your terminal's Copy command](#use-the-mouse-and-copy-text) to copy
plain task text or when native clipboard access fails.

## Organize tasks into lists

Press `l` to open the list picker over your current tasks. The picker shows
All tasks first, your ordinary lists next, and Trash last. It starts with
your current list selected. From All tasks, it starts with the list you
opened most recently in this Husker session, so pressing `l` and Enter
takes you back there. If you have not opened a list yet, or that list was
removed, it starts with All tasks selected.
Press Esc or `l` again to close the picker without changing the view.

Choose a list and press Enter. For example, choosing Private shows
`All tasks > Private`. New tasks created there belong to Private.
Press Esc to return to All tasks with its previous filter, selected task,
and scroll position. Choosing All tasks in the picker gives the same result.
For example, if All tasks was filtered by `milk`, that filter returns after
you leave Private. If Private itself has a filter applied, the first Esc
clears that filter and keeps you in Private. Press Esc again to leave.
Esc at All tasks without a filter leaves the view unchanged.

Choosing a different list or Trash starts that view without a filter.
Choosing the current list again preserves its filter, task selection,
and scroll position.

To create a list, press `n` or `N` in the picker, enter a name such as
`Private`, and press Enter. The footer shows `Enter a list name` while you
type. The picker returns with the new list selected immediately
before Trash. Press Enter again to open the list.
Esc in the list-name form returns to the picker without creating a list.

Select an ordinary list and press Alt+Up or Alt+Down, or `K` or `J`, to
change its position. Husker saves this order for the next session.
The default list, initially Inbox, stays first among ordinary lists and is
marked as the default. All tasks and Trash stay fixed.
New tasks in All tasks use the default list.

Press `r` or `R` on a list to rename it. Enter the new name and press Enter.
The tasks stay in the same list. Names are unique without regard to case,
so `Private` and `PRIVATE` cannot be separate lists.
`Trash` and `Papirkurv` are reserved names.
Renaming a list keeps its position and returns to the picker with that list
selected. Renaming the default list keeps it first among ordinary lists
and keeps it as the default destination for new tasks.

The number beside an ordinary list counts its open tasks outside Trash.
For example, `Private (4)` means that list has four open tasks.
Done and cancelled tasks do not contribute to that count.
All tasks has no count because it is a view option. If you name an ordinary
list All tasks, that list still appears separately with its count.
Help opened with `?` returns to the picker when you close it with Esc.

All tasks and all-list History show the list before each task title, such as
`[Work] Review`. The prefix remains when you filter All tasks with `l:Work`.
Opening Work itself removes the repeated prefix because the breadcrumb
already identifies the list. Renaming a list updates these labels on refresh.
The prefix is not part of the stored title or the text copied with F4.

## Remove a list without losing its tasks

Press `l`, select an ordinary list, and press Delete or `d`.
Husker shows the list name and the default list that will receive its tasks.
The confirmation counts open, done, cancelled, and trashed tasks separately.
For example, a list with three unfinished tasks warns you before moving them.
A picker count of zero means there are no open tasks outside Trash.
The list can still contain completed, cancelled, or trashed tasks.

No is selected first. Press `n`, Enter, or Esc to keep the list.
To remove the list, press `y`. You can also use an arrow key or Tab to
choose Yes and press Enter.
All of the list's tasks move to the default list, including tasks in Trash.
Their text, dates, and statuses stay unchanged. A trashed task stays in Trash
and will belong to the default list when you restore it.
The removed list's name and its task grouping are not retained.

The picker stays open with the default list selected after deletion.
If you were viewing the removed list, the tasks behind the picker return
to All tasks. The default list, All tasks, and Trash cannot be deleted.

If another process changes the list or its tasks while you confirm,
Husker updates the confirmation and selects No again. Review the new names
and counts before choosing Yes. An editor opened before the move keeps
your draft. Reopen that task to load its new list before editing again.
A stale editor cannot overwrite the transferred task.

## Plan tasks by date

Planned date is the day you intend to work on the task. For example, set a
report's Planned date to `2026-11-10` if you intend to work on it that Tuesday.

Set an open task's Planned date in the editor to place the task in a dated group.
Husker uses your computer's local calendar date to group open tasks:

| Group | Tasks shown |
| --- | --- |
| Earlier | Planned before today. Shown as Tidligere in Danish. |
| Today | Planned for today. |
| Tomorrow | Planned for tomorrow. |
| An exact date | Planned at least two days after today. Each date has its own heading. |
| Sometime | No planned date. Shown as Engang in Danish. |

If today is November 8, tasks planned for November 10 and November 16 appear
under separate headings for those dates. Today and Tomorrow also include
the actual date, such as `Tomorrow - 2026-11-09 Mon W46`.
Future dates appear in ascending order. Sometime is the
last open-task group. Earlier can contain several dates, so its heading
does not claim a single day.

Done today and Cancelled today follow the open tasks. Both headings show
today's actual date. For example, completing
an undated task moves it from Sometime to Done today without adding a planned date.
A task completed today appears there even if its planned date is before today.
If a status filter includes older tasks, Done and Cancelled appear afterward.
Within each finished section, newer status dates appear first. Tasks without
a recorded status date appear last. Within one date, the most recent recorded
completion or cancellation time appears first. Unknown times follow known
times on that date. Equal or unknown times use descending task ID.
Trash stays separate from these views.

Task rows use the full navigation width. The selected task has no side
description preview. Press Enter to open its Description field, or F4 to
read and copy its plain text.
Group headings are labels. They do not receive selection highlighting,
and keyboard navigation skips them.

Date labels include the weekday and ISO week number:
`2026-11-08 Sun W45`. Danish uses `2026-11-08 søn W45`.
At a year boundary, `2027-01-01 Fri 2026-W53` shows that January 1 belongs
to the final ISO week of 2026.

A task row repeats its planned date after the title only where the heading
does not already show that date. Rows under Earlier, in Done today,
Cancelled today, Done, and Cancelled, and in Trash end with the date label.
Rows under Today, Tomorrow, an exact date, or Sometime show only the title.
For example, an open task planned for November 10 appears as
`[ ] Write report` under `2026-11-10 Tue W46`. After you complete it, it
appears as `[x] Write report  2026-11-10 Tue W46` under Done today.
The stored date, filters, and the editor are not affected.

Use Alt+Up or Alt+Down, or `K` or `J` (Shift+K and Shift+J), to change the
order of open tasks sharing the same planned date. Undated open tasks can
move among other open tasks in Sometime. The message shows `Updated`, and
`u` undoes the move.
Done and cancelled tasks use their recorded status dates and times and cannot
be reordered manually.
Reordering never changes a planned date or moves a task between lists.
With a filter active, reordering uses neighboring tasks in the visible results.

## Find tasks

Press `/`, type a query, and press Enter to apply it. While you type, the
footer shows the hint `Combine text, l:list, due:today, and s:done. #words are text.`
Press Esc in the input to discard your query edits.

To clear an applied filter, press Esc in the task list. The view and the
selected task stay as they are, and the footer entry `Esc Clear filter`
disappears. Applying an empty query clears the filter too.
If you opened a particular list, clearing the filter keeps that list selected.

Combine terms to narrow the results. Every term must match:

| Query | Result |
| --- | --- |
| `milk` | Tasks whose title or description contains `milk`. |
| `l:private` | Tasks in the list named `Private`. |
| `#shopping` | Tasks whose title or description contains `#shopping`. |
| `l:private milk #shopping` | Tasks matching all three terms. |
| `due:today` | Tasks planned for today. |
| `due:2026-11-08` | Tasks planned for that exact date. |
| `due:none` or `due:undated` | Tasks with no planned date. |
| `s:open` | Open tasks only. |
| `s:done` | Completed tasks. |
| `s:cancelled` | Cancelled tasks. |
| `s:all` | Tasks of every status in the current view. |

The interface calls the field Planned date, or Planlagt dato in Danish.
Search keeps the `due:` name. For example, `due:overdue` finds tasks planned before today. Use any of these date-group tokens with `due:`: `overdue`, `today`,
`tomorrow`, `soon`, `next`, or `undated`.
`due:soon` still means two through seven days after today. `due:next` means
eight or more days after today. These filters keep their ranges even though
the terminal shows individual date headings.
For example, `due:none #shopping` finds undated tasks containing `#shopping`.
The Sometime label does not change the filter: use `due:none` or `due:undated`,
not `due:sometime`.

List names match whole names. Text matches part of a title or
description. These comparisons ignore case and preserve accents.
For example, `ÆBLE` matches `Køb æbler`, but does not match `Køb abler`.
`#` has no special tag meaning. A query such as `#shopping` also matches
`#shopping-list` because text searches match substrings.

Put values containing spaces in double quotes:

```text
l:"Home admin" "electricity bill"
```

Quote an entire term to search for literal filter syntax. For example,
`"due:today"` searches task text for those exact characters.
Inside quotes, use `\"` for a quote and `\\` for a backslash.

Use each of `l:`, `due:`, and `s:` at most once. Unknown fields, invalid dates,
and unfinished quotes produce an error. Husker keeps your previous results
until you correct and apply the new query.

## Find older completed tasks

Press `h` from All tasks or an ordinary list to open History.
For example, `h` in Work opens `All tasks > Work > History` and shows Work's
currently done and cancelled tasks from every date. Tasks do not expire.
History excludes open tasks and Trash.

The most recent recorded status dates appear first. Today and Yesterday
headings include the actual date, such as `Today - 2026-11-08 Sun W45`.
Older headings show the date, weekday, and week, such as `2026-11-06 Fri W45`.
Within one date, later completion or
cancellation times appear first. Tasks migrated without a recorded time
follow tasks with known times on the same date. Equal or unknown times use
descending task ID. Tasks without a recorded date appear under Unknown date.

Husker records the timestamp in UTC and keeps a separate local date for
headings and the daily task view. It does not invent times for older tasks.

History rows show the task without an appended planned date. For example,
a task planned for November 12 and completed on November 9 appears under
the November 9 completion heading. Press Enter to see November 12 in its
Planned date field. A missing completion date stays Unknown date even if
the task has a planned date.

Press `/` to search History. For example, `s:done milk` finds completed tasks
containing `milk`. `s:cancelled` shows cancelled tasks. `s:all` keeps both
closed statuses, and `s:open` matches nothing in History.
Due filters still check the task's planned date. `due:today` does not mean
completed today.

Press Enter to edit a historical task or F4 to read and copy its text.
Press `r` to reopen it. The task leaves History and becomes open in its
list, and the message shows `Reopened`.
You can also use the usual status and Trash shortcuts, press `m` to move
the task to another list, and press `u` to undo your last task action.
New-task creation and manual reordering are unavailable in History.

Press `h` to return to the view where you opened History.
Your previous filter, selected task, and scroll position return.
Esc does the same when History has no filter applied. With a filter
applied, the first Esc clears the filter and a second Esc returns.
Press `l` to choose All tasks or another task list and leave History.
Choosing the same list restores the view from before you opened History.
Choosing another list starts with an empty filter.

History shows each task's current status. Reopening clears its recorded
completion date and time. Husker does not keep a log of earlier status changes or
previous versions of task text. To find deleted tasks, open Trash from
the list picker instead.

## Complete, cancel, and restore tasks

Press Space to mark a task done. Press `c` to cancel a task. The message
shows `Marked done` or `Marked cancelled`, and the task is stored with its
new status at once. Its row stays where it was, under the same heading,
with a `[x]` or `[-]` marker and dimmed text, so you can see what you
changed. Selection moves to the next open task, or the previous open task
if none follows it.

The held row moves to Done today or Cancelled today as soon as the view
changes: when you choose another list, apply or clear a filter, open
History, open the editor, create a task, press F5, or the date changes.
Opening the list picker, the move picker, Help, the language picker, or the
F4 copy view does not move it. With `s:open` applied, the task leaves the
results immediately. Tomorrow, the default view hides the closed task.
Press `h` to find it in History.

If you pressed Space on the wrong task, press `u` to reopen it. See
[Undo the last task action](#undo-the-last-task-action).

Press Space on a task in Done today to reopen it. Press `c` on a task in
Cancelled today to reopen a cancelled task. The message shows `Reopened`.
Apply `s:done` or `s:cancelled` to find older closed tasks, or `s:all` to
review every status.

Press `d` or Delete to move a task to Trash. The message shows
`Moved to Trash`. Open `l` and select Trash, the last entry, to see deleted
tasks. Press `r` to restore the selected task. The message shows `Restored`.
The task returns to its original list with its original status.
A restored done task stays done. It appears in the default view only if
it was completed today. Restoring a task does not reset its completion date.

Trash counts every deleted task, regardless of status. Tasks in Trash are
excluded from ordinary task views even when the query is `s:all`.
Trash keeps the task's title, description, planned date, status date, and list membership.
Trash uses the heading `All tasks > Trash` and has no date-group headings.
Press Esc to return to your previous All tasks view.

## Undo the last task action

Press `u` in All tasks, an ordinary list, or History to reverse the last
task action of this Husker session. The message shows `Undone` and the task
stays selected. For example, press Space on the wrong task, then `u`: the
task is open again with the status date and time it had before.

Undo covers marking a task done or cancelled, reopening, moving to Trash,
restoring from Trash, a date shortcut that changed the date, a reorder with
Alt+Up, Alt+Down, `K`, or `J`, a move to another list with `m`, and a save
in the task editor. Undoing a save restores the title, description, planned
date, and list the task had before. Undoing a task you just created moves
that task to Trash. Creating or renaming lists, changing the language, and
filtering are not recorded, so `u` cannot reverse them. Deleting a list
forgets the entry. `u` does nothing while Trash is open.

Husker keeps one entry. Each new task action replaces it, and undoing uses
it up. There is no redo and no second level. For example, after you trash
two tasks, `u` restores only the second one.

If the task changed after your action, for example because an agent edited
it, Husker shows `Task 7 changed since the last action. Nothing was undone.`
and forgets the entry. The task keeps the later change. With nothing
recorded, `u` shows `Nothing to undo`.

## Use the mouse and copy text

Select text with the keyboard in Title, Description, Planned date, a filter input,
or a list-name form, then press Ctrl+C to copy that selection. A message confirms success.
Copying preserves your unsaved text, cursor, and selection.
Outside those inputs, Ctrl+C does not copy. In a task view, History, or the
list picker, Ctrl+C quits Husker.

If copying fails, the error leaves the selection intact. From a task editor,
press F4 and use your terminal's Copy command instead.
Text containing a null character cannot be copied through Ctrl+C.

Click a task or list to select it. Double-click the same row within 400 ms
to open it. The mouse wheel moves through the rows. In an editor, click
a text field to focus it and place the text cursor. Click List to open its picker.
When the picker is open, clicks and wheel movements outside the popup do
nothing. Scroll inside the picker to move through its entries.

Press F2 to turn mouse capture off when you want to select text with your
terminal. Press F2 again to restore Husker's mouse navigation.

Press F4 on a task to open its title and description as plain text.
F4 also works inside the task editor and shows your current unsaved text.
The copy view has no vertical borders or inserted line wrapping.
Drag to select text, then use your terminal's Copy command.
Use arrow keys to scroll vertically or horizontally when the text does not fit.
PageUp and PageDown scroll by one screen.
Press Esc to return to the previous screen without saving or discarding edits.

The copy view turns mouse capture off automatically and restores your
previous mouse setting when you leave it.

Ctrl+C uses the system clipboard available to the computer running Husker.
Native clipboard access is supported on Windows, macOS, and Linux when a
desktop clipboard is available. On Linux, keep Husker open until you paste.
Clipboard contents after quitting depend on the desktop's clipboard manager.
Husker does not forward clipboard contents through SSH to your local terminal.
For a remote session, use F4 and the local terminal's Copy command.

## Refresh changes from an agent

Press F5 to refresh database contents. Husker also checks for changes from
other processes once per second. Navigation updates automatically.
An on-screen notice tells you when another process changed the database.
Press F5 to acknowledge a notice that is already visible.
This notice has no time limit. F5 also moves a task you marked done or
cancelled from its held row to Done today or Cancelled today.

Other status messages disappear after five seconds. For example, `Saved`
clears after you save a task, even if you navigate to another list.
Errors also clear after five seconds. If you enter the invalid planned date
`2026-02-30`, the error disappears but the editor keeps your unsaved text.
Correct the date and press Ctrl+S to try again.

If you are editing a task, refresh preserves your unsaved text.
Your save still checks the task version. If the agent changed that task,
copy your draft with F4, cancel the editor, and reopen the task before saving again.

## Use the keyboard

The footer of every screen shows its most useful keys with short labels,
such as `Enter Edit`, `Space Done`, `t/T/w/x Date`, `u Undo`, and `m Move`.
The footer uses up to two lines and leaves out entries that do not fit the
terminal width, so widen the terminal to see more of them. Trash shows
`r Restore` and `F4 Copy` instead of the task edits. History adds
`r Reopen`. While a filter is applied, the Esc entry reads `Esc Clear filter`.
An entry shows the first key of an action, so a rebound action shows your key.

Press `?` to open Help with every shortcut from your own configuration.
Help is grouped by screen: Task views, List picker, Task editor, Text
editing, Filter and list name, Confirmations, and Everywhere. The section
for the screen you opened Help from comes first. An action with several
keys lists them separated by ` / `, such as `q / Ctrl+C`. Text editing
lists the built-in keys from [Edit text with the keyboard](#edit-text-with-the-keyboard).
Scroll Help with Up and Down, `k` and `j`, PageUp and PageDown, Home and
End, `g` and `G`, or the mouse wheel. Clicking in Help does nothing.
Press Esc or `?` to close Help.
Keys are printed the way they are labelled on the keyboard: Ctrl+S, Esc,
F4, Space, PageUp, Enter, Tab. Danish prints the arrow keys as Op, Ned,
Venstre, and Højre.

These are the default shortcuts:

| Key | Action |
| --- | --- |
| Up or `k` | Select the previous item, wrapping from first to last. |
| Down or `j` | Select the next item, wrapping from last to first. |
| Left, Home, or `g` | Select the first item. |
| Right, End, or `G` | Select the last item. |
| PageUp or PageDown | Move the selection by one screen without wrapping. |
| Enter | Choose a picker entry or edit a task. |
| `n` | Create a task outside History and Trash, or create a list from the picker. |
| `N` | Create a list from the picker. |
| `r` | Restore a task from Trash, reopen a task in History, or rename a list from the picker. |
| `R` | Rename an ordinary list from the picker. |
| `l` | Open or cancel the list picker. |
| `h` | Open History, or return to the view where you opened it. |
| `t`, `T`, `w`, `x` | Set today, tomorrow, next Monday, or no planned date outside Trash. |
| `m` | Move the selected task to another list outside Trash. |
| `u` | Undo the last task action. |
| `/` | Edit the task filter. |
| Esc | Clear an applied filter, return from a list or History to the previous view, or cancel the picker. |
| Space | Toggle done status. |
| `c` | Toggle cancelled status. |
| `d` or Delete | Move a task to Trash, or review deleting a list in the picker. |
| Alt+Up or `K` | Move an open task before its neighbor with the same planned date, or move an ordinary list up in the picker. |
| Alt+Down or `J` | Move an open task after that neighbor, or move an ordinary list down in the picker. |
| F2 | Toggle mouse capture. |
| F3 | Choose English or Dansk and save the interface language. |
| F4 | Open plain task text for copying. |
| F5 | Refresh database contents. |
| `?` | Open Help. Esc or `?` closes it. |
| `q` or Ctrl+C | Quit from a task view, History, or the list picker. |

Wrapping and first/last jumps work in task views, including History and
Trash, and in the list pickers and the language picker. PageUp and PageDown
work in the same places except the language picker. With a filter
applied, the keys move only through matching tasks. Group headings are
skipped, and the selected item scrolls into view. An empty result stays empty.
A result with one item keeps that item selected.

For example, press End to select the last task, then Down to return to the
first. In the language picker, Home selects English and End selects Dansk.
Press Enter to apply the selected language; moving selection alone does not save it.

Mouse-wheel movement stops at each end. Reordering with Alt+Up, Alt+Down,
`K`, or `J` also stops at the allowed boundary. In text fields, Left/Right
and Home/End move the cursor, and `g`/`G` enter text. In the F4 copy view,
Left/Right scroll horizontally and PageUp/PageDown scroll by one screen.

In the task editor, Ctrl+S or Ctrl+Enter saves from any field, and Enter
saves from Title or Planned date. Esc or Ctrl+G asks before discarding
changed fields. An unchanged editor closes immediately.
F4 opens the copy view without discarding the draft. In the filter and list-name
forms, Enter applies the input and Esc cancels it.
In the discard prompt and the list-deletion confirmation, `y` chooses Yes
and `n` chooses No without pressing Enter.
Backspace edits text in an input field. It does not delete a task.

## Set the language and shortcuts

Press F3 to open the language picker. Use Up or Down to select English or
Dansk, then press Enter. The interface changes after Husker saves the choice.
The next launch uses the same language. Press Esc to cancel without saving.
The language shortcut is listed in Help (`?`).

You can change language while editing a task. Your draft, cursor, and undo
history stay intact. Language selection also works from task views, the list
picker, filter and list-name forms, and Help. Close a confirmation or the
F4 copy view before opening the language picker.

Husker saves the choice to your profile's `config.toml`, or the file selected
with `--config`. Existing comments and custom shortcuts stay in the file.
A missing default configuration is created. If saving fails, the picker
shows an error and keeps the previous language. Esc returns to the unchanged
screen or draft.

Edit `config.toml` in your profile directory to change shortcuts manually.
The default profile is `~/.husker`. On Windows, that is normally
`C:\Users\<your-user-name>\.husker`.
A missing default configuration uses English and the built-in shortcuts.

To use Danish and change the quit shortcut, save this configuration:

```toml
language = "da"

[keys]
quit = ["ctrl+q"]
```

Restart Husker to load the file. Ctrl+Q now quits, and neither `q` nor
Ctrl+C quits. Use `language = "en"` for English. Your task text and list
names are never translated. Filter operators and CLI commands stay in English.

Each shortcut array replaces all default keys for that action. For example,
`trash = ["delete"]` keeps Delete and removes `d`. Use `trash = []` to disable
both shortcuts. Actions omitted from `[keys]` keep their defaults.
Footers and Help print the keys you configure in display form, so
`ctrl+s` appears as Ctrl+S and `pageup` as PageUp.

These actions are new in 0.12.0 or gained a default key in that version:

| Action | Default keys | Where it works |
| --- | --- | --- |
| `page_up`, `page_down` | PageUp, PageDown | Task views, History, the list pickers, Help, and the F4 copy view. |
| `move_task` | `m` | Task views and History, not Trash. |
| `undo` | `u` | Task views and History. |
| `new_list` | `n`, `N` | The list picker. |
| `rename_list` | `r`, `R` | The list picker. |
| `move_up`, `move_down` | Alt+Up and `K`, Alt+Down and `J` | Task views and the list picker. |
| `quit` | `q`, Ctrl+C | Task views, History, and the list picker. |

For example, `new_list = ["N"]` keeps only the capital letter for creating
a list, and `undo = ["ctrl+z"]` undoes a task action with Ctrl+Z in task
views. Ctrl+Z still undoes typing in text fields, because those are
different modes.

Esc is the default key of three actions, one per kind of screen:

| Action | Default keys | Screens |
| --- | --- | --- |
| `back` | Esc | Task views, History, the list and language pickers, the filter and list-name forms, and the list-deletion confirmation. |
| `discard` | Esc, Ctrl+G | The task editor and its discard prompt. |
| `close` | Esc, `?` | Help and the F4 copy view. |

Overriding one of them changes only that action. For example,
`discard = ["ctrl+g"]` removes Esc from the task editor only. Esc still
leaves a list, clears a filter, and closes Help.
An action override takes precedence over a built-in text-editing shortcut.
For example, `save = ["ctrl+a"]` makes Ctrl+A save instead of select all in
the task editor.
To copy with Ctrl+Shift+C, add `copy_selection = ["ctrl+shift+c"]` under
`[keys]`. Set `copy_selection = []` to disable Husker's clipboard shortcut.
F4 remains available for copying task text through your terminal.

Use `all_tasks` to change the root shortcut, for example
`all_tasks = ["ctrl+a"]`. The older name `planned` still works for an existing
override, such as `planned = ["p"]`. Use only one of those two names.
The All tasks action has no default shortcut. Use Esc to leave a scoped list,
or choose All tasks in the picker. Neither `a` nor `p` navigates by default.
Use `history`, `reopen`, `delete_list`, and `language` for those actions.
For example, `delete_list = []` disables removal shortcuts in the picker.
Under `[keys]`, `language = ["f6"]` changes the language-picker shortcut to F6.

Key strings accept characters such as `q` or `N`, named keys such as `enter`
or `f2`, and modifiers such as `ctrl+q` or `alt+up`.
Use `plus` for the `+` key. Run this command to see every action and default:

```text
husker config --example
```

Use `first` and `last` to customize boundary selection. For example,
`first = ["ctrl+home"]` replaces Home, `g`, and Left. Use `last = []` to
disable End, `G`, and Right for selecting the last item.

If a custom action already uses `g`, `G`, Home, End, Left, or Right in a selection screen,
the new defaults can cause a shortcut conflict. For example,
`history = ["g"]` conflicts with First. Keep that History shortcut by adding
`first = []` under `[keys]`, or rebind First to another key. Use `last = []`
for an existing action that conflicts with Last.
To keep a custom Left shortcut and retain the earlier First keys, set
`first = ["home", "g"]`. For a custom Right shortcut, use `last = ["end", "G"]`.

The same applies to the defaults added in 0.12.0. If a custom action already
uses `m`, `u`, `K`, `J`, Ctrl+C, PageUp, or PageDown in a task view, or `n`
or `r` in the list picker, Husker reports the conflict at startup. For
example, `history = ["m"]` conflicts with Move task. Keep that History
shortcut by adding `move_task = []` under `[keys]`, or rebind `move_task`
to another key. Use `page_up = []` or `page_down = []` for an action that
uses PageUp or PageDown, and `quit = ["q"]` for an action that uses Ctrl+C.

Husker rejects an unknown setting, action, key, or conflicting shortcut.
For example, `quit = ["n"]` conflicts with task creation.
Keys can be reused in separate modes, as Enter is for opening a task and
applying a filter. Avoid assigning printable letters to editor actions if
you need to type those letters in task text.

Use a different configuration file with `--config`:

```text
husker --profile ./work-tasks --config ./shared.toml
```

This command stores tasks in `./work-tasks/tasks.db` and reads the existing
`./shared.toml`. A missing explicitly selected file is an error.
Relative paths start from the directory where you ran Husker.

## Run commands from scripts

Use subcommands when you want to manage tasks without opening the terminal
interface. The UI label Planned date still uses `--due` in commands and
`due` in JSON. For example:

```text
husker list-add Private
husker add "Buy milk #shopping" --list Private --description "Two cartons"
husker tasks "l:private milk #shopping"
husker --json tasks
```

Use the task ID printed by `add` in later commands. The following examples
use ID `1`; replace that number with your task's ID:

```text
husker show 1
husker edit 1 --title "Buy oat milk" --due 2026-11-09
husker done 1
husker reopen 1
husker cancel 1
husker trash 1
husker tasks --trash
husker restore 1
```

`edit` changes only supplied fields. Use `--clear-due` to remove the due date
and place an open task in Sometime:

```text
husker edit 1 --clear-due
```

Command-line due dates require `YYYY-MM-DD`, such as `--due 2026-11-09`.
Compact editor entries such as `261109` are rejected by commands.

Plain CLI output still uses `undated` for a missing due date. JSON uses
`"due": null`. Sometime and Engang are terminal group labels.

Add `--json` to data commands for JSON output with stable English field
names. A task's `version` increases after a successful mutation.
Pass a version read earlier with `--if-version` to avoid overwriting a
task that changed after your script read it:

```text
husker --json show 1
husker edit 1 --title "Buy milk and bread" --if-version 3
```

Replace `3` with the version from `show`. A stale version fails without
changing the task. Errors go to stderr and return a nonzero exit code.

Run `husker lists` to see list IDs and counts in the order you chose in the
terminal. The default list stays first and Trash stays last.
The terminal picker's virtual All tasks entry is not included in command output.
Rename a list with `husker list-rename 2 Home`, replacing `2` with the list's ID.
The renamed list keeps its position.
Every command accepts `--profile DIRECTORY` to select its task collection.

## Update an existing profile

Back up your profile before updating to 0.12.0.
Follow [Back up your tasks](#back-up-your-tasks), then keep every old Husker
process closed, including MCP clients, until the upgrade is complete.

Install the current version from the repository:

```text
cargo install --path . --locked
```

Version 0.12.0 uses schema version 4, the same database format as 0.9.0
through 0.11.1. Updating from these versions requires no database migration.
CLI commands, JSON fields, MCP tools, and filter syntax are unchanged.

Version 0.12.0 adds default shortcuts: `n` and `r` in the list picker, `K`
and `J` for reordering, `m` for moving a task, `u` for undo, Ctrl+C for
quitting, and PageUp and PageDown for paging. If your `config.toml` already
uses one of these keys for another action on the same screen, Husker reports
the conflict at startup instead of opening. Rebind your action, or disable
the new default with an empty array such as `move_task = []` or
`page_up = []` under `[keys]`. See [Set the language and shortcuts](#set-the-language-and-shortcuts).
Footers and Help now print keys as Ctrl+S, Esc, or PageUp. A script that
matches the old `ctrl+s` text in the interface must expect `Ctrl+S`.

Planned date and Earlier are the current interface labels. Existing `--due`,
`due:` filters, JSON fields, and `due_*` shortcut settings keep their names.
Check any custom `g`, `G`, Home, End, Left, or Right bindings against the
First and Last defaults. Rebind them or set `first = []` or `last = []` under `[keys]`.

If you are updating from 0.8.0 or earlier, run `husker lists` once to upgrade
the database before reconnecting a read-only MCP client. For a custom profile,
run `husker --profile DIRECTORY lists`. A read-only connection cannot upgrade it.

Profiles from versions 0.4.0 through 0.8.0 keep every existing task field,
task version, and saved list position. The upgrade adds a nullable completion
timestamp. Older tasks receive no invented time. A new completion or
cancellation records its time automatically.

Keep older Husker executables closed after upgrading. Versions before 0.9.0
cannot open the upgraded profile. Task JSON now includes `status_changed_at`,
a UTC timestamp or null, alongside the existing local `status_changed_on` date.

If a custom action uses `t`, `T`, `w`, or `x` in an overlapping mode, rebind
or disable the corresponding preset. For example, `due_today = []` keeps
an existing `t` shortcut. All four preset actions are listed in Help.
If an input action already uses Ctrl+C, rebind or disable `copy_selection`
in `[keys]` to avoid a shortcut conflict. For example, set `copy_selection = []`
to keep your existing Ctrl+C action.
The steps below apply when upgrading older profiles or old shortcut overrides.

If your `config.toml` overrides the old `anytime` or `maybe` actions, remove
those lines. If you set `save = ["esc"]`, remove that override to use the new
save and cancel defaults. Esc now belongs to the discard action by default.
Old save and discard overrides can conflict until you update them together.

For a profile created before 0.4.0, the same upgrade also adds saved list positions.
The default list moves to the top. Other existing lists start in
case-insensitive alphabetical order. Use Alt+Up and Alt+Down, or `K` and
`J`, in the list picker to arrange them after the upgrade.
For a profile from version 0.3.0, every existing task field and task version stays unchanged.

For profiles from version 0.1 or 0.2, the upgrade also simplifies task fields.
It preserves task IDs, lists, due dates, and statuses.
Existing description text stays intact. Existing tags become `#words` in a
final description paragraph.
Tasks that gain this text get a new version, so an agent must read them again
before saving an edit prepared before the upgrade.
Readiness is removed, so open tasks without dates appear in Sometime.
Older completed tasks have no recorded completion date and stay hidden from
the default view. Open History with `h`, or use `s:done`, `s:cancelled`, or `s:all` to find them.

Update scripts that use `--tag`, `--clear-tags`, `--anytime`, or `--readiness`.
Those options now produce argument errors. Write `#words` in the title or
description instead. Replace `due:anytime` and `due:maybe` with `due:none`.
MCP clients must refresh their tool definitions after the upgrade.

Start Husker and reconnect MCP clients after the upgrade.
Use the updated executable for both terminal sessions and MCP connections.

## Back up your tasks

Close every Husker instance that uses the profile, including any running
commands. Disconnect any MCP clients that use the profile.
Copy the whole profile directory to your backup location.
For the default profile, copy `~/.husker`, including `tasks.db`, configuration,
and any database companion files that remain.

To restore a backup without replacing your current tasks, copy the backup
to a separate directory and open that directory as a profile:

```text
husker --profile ./restored-tasks
```

Keep the original profile until you have checked the restored tasks.
