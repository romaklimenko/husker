# Use Husker

This guide is for people who want to capture and manage tasks from a terminal.

## Capture your first task

Run `husker` in an interactive terminal. Press `n`, type a task title, and
press Enter to save. For example, type `Buy milk`. The message `Created`
appears, and the task is listed under Sometime because it has no planned date.
The task belongs to the list shown in the editor's List field. In a new
profile, that's Inbox. See [Start a new task in the right list](#start-a-new-task-in-the-right-list).
Ctrl+S saves from any field. Ctrl+Enter also saves when your terminal sends
it as a distinct key.

To plan a task while you capture it, press Tab twice after the title. For
example, press `n`, type `Pay rent`, press Tab, press Tab again, press `t`,
and press Enter. The first Tab focuses List and the second focuses Planned
date. `t` fills in today's date, and Enter creates `Pay rent` planned for today.

The editor shows its fields from top to bottom in this order:

| Field | What to enter |
| --- | --- |
| Title | A nonempty task title, such as `Buy milk`. Enter saves the task. |
| List | Press Enter or `l` to choose an existing list, such as `Inbox`. |
| Planned date | `2026-11-09`, a compact date such as `261109`, or nothing for no date. Enter saves the task. |
| Description | Plain text. Press Enter here to add another line. |

Press Tab to move to the next field down and Shift+Tab to move up. Tab on
Description goes back to Title, and Shift+Tab on Title goes to Description.
A new task opens at Title. Press Enter on an existing task to open its
editor at Description.

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

## Start a new task in the right list

If you know where a task belongs, open that list first. Press `l`, choose
the list, and press `n`. For example, in `All tasks > Private`, a new task
always starts in Private.

All tasks shows every list, so when you press `n` there, Husker suggests a
list. The suggestion is the list of the newest task outside Trash. For
example, if 9 of your 10 newest tasks are in Work and the newest one is
`Call the dentist` in Private, the new task starts in Private. It doesn't
matter that Work holds more of them. The List field shows the suggested
list as soon as the editor opens.

Husker finds the newest task this way:

- The newest task is the one with the highest ID. Each task you create
  gets a higher ID than every task before it. The editor heading shows a
  task's ID, such as `#42`.
- Done and cancelled tasks count. Tasks in Trash don't. If you create a
  task by mistake and move it to Trash, the suggestion goes back to the
  list of the newest task that's still outside Trash.
- A task counts for the list it's in now. If you created your newest task
  in Inbox and later moved it to Work, the new task starts in Work.
- Editing or moving an older task doesn't make it the newest.
- If every task is in Trash, or you have no tasks yet, the new task starts
  in the default list. In a new profile, that's Inbox.

Tasks created with `husker add` or by an agent count too, because Husker
doesn't record who created a task. If an agent adds a task after your last
one, the suggestion follows the agent. For example, you create
`Call the dentist` in Private, and then an agent adds a task to Labs. When
you press `n` in All tasks, the List field shows Labs.

To use another list, press Tab from Title to focus List, press Enter or
`l`, and choose the list. Ctrl+Z with List focused undoes your choice and
brings back the suggested list. See [Choose a task's list in the editor](#choose-a-tasks-list-in-the-editor).
To avoid the suggestion, open the list with `l` before you press `n`.

If you set up [sync between computers](sync.md), tasks that sync brings in
from another computer count too. Each task that arrives gets a higher ID
than every task already on this computer, so it becomes the newest task,
even if it was created before your last task. See
[Task IDs on different computers](sync.md#task-ids-on-different-computers).

A filter doesn't change the suggestion. For example, if you filter All
tasks with `l:Work` and press `n`, the new task still starts in the list
of the newest task, which can be Private. Open Work with `l` when you want
new tasks to start there.

The suggestion is only for the terminal interface. `husker add` without
`--list` puts the task in the default list. An agent connected through MCP
adds tasks to the list its server is restricted to with `--list`. Without
that restriction, the task goes to the list the agent names, or to the
default list.

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

An empty Planned date field shows a grey `Sometime` placeholder, or `Engang`
in Danish. Leave the field empty to save no planned date. An open task then
appears under Sometime. The placeholder disappears when you type, and it is
not part of your task text. The field title lists the forms Husker accepts:
`YYYY-MM-DD / YYYYMMDD / YYMMDD / MMDD`.

These compact forms work in the task editor. Use `YYYY-MM-DD` in commands,
MCP requests, and search filters, such as `due:2026-11-09`.

## Choose a task's list in the editor

Focus List and press Enter or `l` to open its picker. From Title, press Tab
once to focus List. An existing task's editor opens at Description, so press
Shift+Tab twice there. You can also click the List field.
Select a destination with Up or Down, or type part of its name, then press
Enter. For example, type `wo` and press Enter to choose Work. The task
moves there when you next save.
Press Esc in the picker to return without changing the draft's list. If you
typed text, the first Esc clears it and a second Esc returns. In the
picker, `l` and other letters type into its filter. See
[Find a list by typing its name](#find-a-list-by-typing-its-name).

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
Down, or type part of its name, and press Enter. For example, to move
`Buy milk` from Inbox to Private, press `m`, type `pr`, and press Enter.
The message shows `Moved to Private`, and in All tasks the row starts with
`[Private]`. See [Find a list by typing its name](#find-a-list-by-typing-its-name).

The task keeps its title, description, planned date, and status, and it
stays selected while it is still visible. If you move a task out of the
list you are viewing, it leaves that view. Choosing the task's current list
closes the picker without changing anything. Press Esc to cancel. If you
typed text, the first Esc clears it and a second Esc cancels.
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

In the editor, focus Planned date and use the same keys. From Title, press
Tab twice. An existing task's editor opens at Description, so press
Shift+Tab once there. A preset replaces the whole field, even if you have
selected only part of its text.
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
Press Esc to close the picker without changing the view. If you typed text
in the picker, the first Esc clears it and a second Esc closes the picker.
To find a list quickly, type part of its name. See
[Find a list by typing its name](#find-a-list-by-typing-its-name).

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

To create a list, press Ctrl+N in the picker, enter a name such as
`Private`, and press Enter. The footer shows `Enter a list name` while you
type. The picker returns with the new list selected immediately
before Trash. Press Enter again to open the list.
Esc in the list-name form returns to the picker without creating a list.
If you had typed text in the picker, Husker clears it when you save the
name, so the new list is visible and selected.

Select an ordinary list and press Alt+Up or Alt+Down to change its
position. Husker saves this order for the next session.
The default list, initially Inbox, stays first among ordinary lists and is
marked as the default. All tasks and Trash stay fixed.
Lists don't move while you have typed text in the picker. Alt+Up then shows
`Clear the filter to reorder lists.` Press Esc once to clear the text, then
move the list.
On a Mac, Alt is the Option key. Terminal.app needs a setting before
Option+Up and Option+Down reach Husker. See
[Use Option+arrows in macOS Terminal](#use-optionarrows-in-macos-terminal).
`K` and `J` reorder tasks without that setting, but they type text in the
picker, so they don't reorder lists.
A new task you create in All tasks starts in a suggested list, which can
differ from the default list. See [Start a new task in the right list](#start-a-new-task-in-the-right-list).

Press Ctrl+R on a list to rename it. Enter the new name and press Enter.
The tasks stay in the same list. Names are unique without regard to case,
so `Private` and `PRIVATE` cannot be separate lists.
`Trash` and `Papirkurv` are reserved names.
Renaming a list keeps its position and returns to the picker with that list
selected. Renaming the default list keeps it first among ordinary lists
and keeps it as the default. For example, `husker add` without `--list`
still puts tasks there.

The number beside an ordinary list counts its open tasks outside Trash.
For example, `Private (4)` means that list has four open tasks.
Done and cancelled tasks do not contribute to that count.
All tasks has no count because it is a view option. If you name an ordinary
list All tasks, that list still appears separately with its count.
Press F1 in the picker to open Help. Help returns to the picker when you
close it with Esc. `?` types into the picker's filter, so it doesn't open
Help there.

All tasks and all-list History show the list before each task title, such as
`[Work] Review`. The prefix remains when you filter All tasks with `l:Work`.
Opening Work itself removes the repeated prefix because the breadcrumb
already identifies the list. Renaming a list updates these labels on refresh.
The prefix is not part of the stored title or the text copied with F4.

These keys work in the list picker:

| Key | Action |
| --- | --- |
| Letters, digits, Space, and other characters | Type into the picker's filter. |
| Backspace | Remove the last character of the filter. |
| Up or Down | Select the previous or next entry. |
| Left or Home | Select the first entry. |
| Right or End | Select the last entry. |
| PageUp or PageDown | Move the selection by one screen. |
| Enter | Open the selected list, All tasks, or Trash. |
| Esc | Clear the filter, or close the picker when the filter is empty. |
| Ctrl+N | Create a list. |
| Ctrl+R | Rename the selected ordinary list. |
| Ctrl+D or Delete | Review removing the selected list, or emptying Trash on the Trash entry. |
| Alt+Up or Alt+Down | Move the selected ordinary list up or down. |
| F1 | Open Help. |
| F2, F3, or F5 | Toggle mouse capture, choose the language, or refresh. |
| Ctrl+C | Quit Husker. |

## Find a list by typing its name

Type part of a list's name in any of the three list pickers: the list
picker you open with `l`, the List field's picker in the task editor, and
the picker that `m` opens to move a task. The picker shows only the lists
whose name contains the text you typed. Case doesn't matter. The lists
keep their usual order.

For example, your lists are Inbox, Husker, and Labs. Press `l`, type `hu`,
and press Enter. The picker shows only Husker, with Husker selected, and
Enter opens `All tasks > Husker`.

After each character, Husker selects the first list whose name starts with
the text. If no name starts with it, Husker selects the first list that
contains it. For example, with the lists Shopping and Home, typing `ho`
keeps both, because both names contain `ho`, and selects Home, because
Home starts with `ho`.

In the list picker you open with `l`, All tasks and Trash match by the
names you see in the picker. Type `tra` to find Trash, or `all` to find All
tasks. In Danish, they match as Alle opgaver and Papirkurv. The count and
the default marker beside a list are not part of its name, so typing `4`
doesn't find `Private (4)`.

The picker's bottom border shows `Type to filter`, or
`Skriv for at filtrere` in Danish, until you type. Then it shows the text,
such as `Filter: hu`. If no list matches, the picker shows
`No matching lists`, or `Ingen lister passer` in Danish, and Enter does
nothing.

Press Backspace to remove the last character. Press Esc to clear all the
text and keep the picker open with the same list selected. Press Esc again
to close the picker. When Backspace removes the last character, the picker
shows every list again, and the list that was selected stays selected.
Every picker opens with empty text.

Letters always type in a list picker, even letters that are shortcuts
elsewhere. For example, `j`, `k`, `l`, `n`, `r`, `d`, `q`, and `?` add
their character to the text. Use the arrow keys, Home, End, PageUp, and
PageDown to move the selection. In the list picker you open with `l`,
manage lists with Ctrl+N, Ctrl+R, Ctrl+D or Delete, and Alt+Up or
Alt+Down. Clicks and the mouse wheel act on the lists the picker shows.

## Remove a list without losing its tasks

Press `l`, select an ordinary list, and press Delete or Ctrl+D.
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
to All tasks. The default list and All tasks cannot be deleted. Delete or
Ctrl+D on the Trash entry empties Trash instead of removing it. See
[Empty Trash](#empty-trash).

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
Every heading ends with the number of tasks under it, such as
`Tomorrow - 2026-11-09 Mon W46 (3)` or `Sometime (5)`. With a filter
applied, the count covers the matching tasks only. A task you just marked
done still counts under its old heading while its row is held.
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

In All tasks and list views, Today stands out, and Earlier and the finished
groups are dimmed. Each group uses these colors:

| Group | Heading | Task rows |
| --- | --- | --- |
| Earlier | Dark gray | Dark gray |
| Today | Bold black text on a yellow background | Bold |
| Tomorrow, each exact date, and Sometime | Yellow | Your terminal's normal text color |
| Done today and Done | Green | Dark gray |
| Cancelled today and Cancelled | Dark gray | Dark gray |

The selected task is cyan and bold with a `>` marker in every group, so you
can still see it in a dimmed group. A task you just marked done or
cancelled stays dark gray under its open heading until its row moves.
History keeps yellow headings and rows in the normal text color. Trash has
no headings, and its rows use the normal text color.

Task rows use the full navigation width. The selected task has no side
description preview. Press Enter to open its Description field, or F4 to
read and copy its plain text.
Group headings are labels. They do not receive selection highlighting,
and keyboard navigation skips them.
Use Down or PageDown to move through a long list. The first visible group's
heading stays at the top of the task area. For example, Tomorrow keeps its
date and full group count above the remaining visible tasks, even after
earlier tasks have scrolled out of view. If only one row is available,
Husker shows the selected task instead.

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
On a Mac, Alt is the Option key. In Terminal.app, Option+Up and Option+Down
move a task after you [let an Option key act as Meta or add two key mappings](#use-optionarrows-in-macos-terminal).
`K` and `J` work without any Terminal setting.

In GNU screen, and in tmux before version 3.5, Esc followed shortly by Up
or Down can move a task. These programs hold an Esc, for 300 milliseconds
in screen and 500 in tmux, and then send it together with the arrow key.
Husker cannot tell that from Alt+Up or Alt+Down. Press `u` to undo the
move. To prevent it, add `maptimeout 10` to `~/.screenrc` or
`set -s escape-time 10` to `~/.tmux.conf`, and start a new screen or tmux
session.

## Find tasks

Press `/` and type a query. The task rows narrow with each character you
type, and the first matching task is selected. For example, in All tasks
with `Buy milk` and `Call mom`, press `/` and type `mi`. Only `Buy milk` is
listed. Press Enter to keep the filter and return to the task rows.
While you type, the footer shows the hint
`Combine text, l:list, due:today, and s:done. #words are text.`

Press Esc in the input to close it without keeping your changes. Husker
brings back the filter, the results, and the selected task you had before
you pressed `/`. For example, Work is filtered by `milk`. You press `/`,
replace the text with `mom`, and press Esc. Work is filtered by `milk`
again, with the same task selected.

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

Use each of `l:`, `due:`, and `s:` at most once. Unknown fields, invalid
dates, and unfinished quotes are errors. While you type, a query is often
unfinished for a moment, such as `"milk` before you type the closing quote,
or `due:` before you type the value. Until the text is valid again, the
input border is red and the task rows keep the results of the last valid
text. If you press Enter on such text, Husker shows the error, such as
`Error: Unclosed quoted filter value.`, and keeps the input open so you can
correct it.

## Find older completed tasks

Press `h` from All tasks or an ordinary list to open History.
For example, `h` in Work opens `All tasks > Work > History` and shows Work's
currently done and cancelled tasks from every date. Tasks do not expire.
History excludes open tasks and Trash.

The most recent recorded status dates appear first. Today and Yesterday
headings include the actual date, such as `Today - 2026-11-08 Sun W45 (2)`.
Older headings show the date, weekday, and week, such as `2026-11-06 Fri W45 (1)`.
The number in parentheses is the count of tasks under that heading.
As you scroll, the first visible group's heading stays above its tasks.
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
previous versions of task text. To find tasks you moved to Trash, open Trash
from the list picker instead.

## Complete, cancel, and restore tasks

Press Space to mark a task done. Press `c` to cancel a task. The message
shows `Marked done` or `Marked cancelled`, and the task is stored with its
new status at once. Its row stays where it was, under the same heading,
with a `[x]` or `[-]` marker and dimmed text, so you can see what you
changed. Selection moves to the next open task, or the previous open task
if none follows it.

The held row moves to Done today or Cancelled today as soon as the view
changes: when you choose another list, change or clear the filter, open
History, open the editor, create a task, refresh with `r` or F5, or the
date changes.
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
`Moved to Trash`. Open `l` and select Trash, the last entry, to see the
tasks in Trash. Press `r` to restore the selected task. The message shows `Restored`.
The task returns to its original list with its original status.
A restored done task stays done. It appears in the default view only if
it was completed today. Restoring a task does not reset its completion date.

The Trash count includes every task in Trash, whatever its status. Tasks in Trash are
excluded from ordinary task views even when the query is `s:all`.
Trash keeps the task's title, description, planned date, status date, and list membership.
Trash uses the heading `All tasks > Trash` and has no date-group headings.
Press Esc to return to your previous All tasks view.

A task stays in Trash until you restore it or delete it forever.
See [Delete a task forever](#delete-a-task-forever) and [Empty Trash](#empty-trash).

## Delete a task forever

Delete a task forever to remove it from your task database for good. Only
a task in Trash can be deleted this way, so press `d` on the task first to
move it there.

1. Press `l`, select Trash, and press Enter.
2. Select the task and press `d` or Delete.
3. Read the confirmation and press `y`.

The confirmation is titled `Delete this task forever?`. It shows the task's
title and ID, such as `Buy milk #7`, and the warning `This cannot be undone.`
After you press `y`, Husker removes the task from your task database and
shows `Deleted forever`. Trash stays open, and the selection moves to a
nearby task.

No is selected when the confirmation opens. Press `n` or Esc to keep the
task. Enter does the same while No is selected. Husker returns to Trash
with the same filter and selected task. You can also use an arrow key, Tab,
or Shift+Tab to select Yes and then press Enter to delete the task.

A task deleted forever can't be restored, and `u` can't bring it back.
Deleting a task forever clears the undo entry, so `u` afterward shows
`Nothing to undo`. Husker never reuses a task ID. If task 7 is deleted
forever, the next task you create still gets a higher ID, so an old
reference to `#7` never points at a different task.

If you set up [sync between computers](sync.md), deleting a task forever
also deletes it on your other computers at their next sync. The same
applies to emptying Trash. A task that another computer edited before it
received the deletion is kept. See
[What happens when two computers change the same task](sync.md#what-happens-when-two-computers-change-the-same-task).

If you might need the task again, leave it in Trash or
[back up your tasks](#back-up-your-tasks) first. Permanent deletion is
available only in the terminal interface. No `husker` command or MCP tool
deletes a task forever, although agents can move tasks to Trash.

## Empty Trash

Empty Trash to delete every task in it forever at once.

1. Press `l`, select Trash, and press Enter.
2. Press `D` (Shift+D).
3. Read the confirmation and press `y`.

The confirmation is titled `Empty Trash?` and shows how many tasks Trash
holds, such as `In Trash: 3`. It also shows
`Every task in Trash is deleted, including tasks the filter hides.` and the
warning `This cannot be undone.` After you press `y`, the message shows
`Trash emptied`.

The count covers all of Trash, including rows a filter hides. For example,
if Trash holds five tasks and the filter `milk` shows two of them, the
confirmation shows `In Trash: 5`, and `y` deletes all five. To delete only
some tasks, use `d` on each one instead.

You can also empty Trash from the list picker without opening it. Press
`l`, select the Trash entry, and press Ctrl+D or Delete. To find Trash
quickly, type `tra` in the picker. While Trash is selected, the picker
hint reads `Ctrl+D Empty Trash`. The same confirmation opens. After you
confirm, the picker stays open with Trash selected.

The confirmation keys are the same as for deleting one task. No is selected
first. Press `y` to empty Trash. Press `n` or Esc to keep every task, or
press Enter while No is selected. Husker then returns to Trash or the list
picker, wherever you started.

If Trash is already empty, `D` in Trash and Ctrl+D or Delete on the Trash entry show
`Trash is empty`, and no confirmation opens. `D` does nothing outside Trash,
for example in All tasks or History.

Emptying Trash deletes tasks of every status from every list. Your lists
and the tasks outside Trash stay unchanged. The Trash entry stays in the
picker, because Trash is not a list. Like deleting one task, emptying Trash
clears the undo entry and can't be undone.

## Confirm again after another program changes Trash

An agent, a script, or another Husker window can change Trash while a
confirmation is open. Husker checks again before it deletes anything and
deletes only what the confirmation shows. It also checks when you press F5
and during its automatic check for changes once per second.

| What changed | What Husker does |
| --- | --- |
| The task you're deleting was edited and is still in Trash. | Shows the confirmation again with the current title, selects No, and adds `The task changed. Review it and confirm again.` |
| The task you're deleting was restored or no longer exists. | Closes the confirmation, returns to Trash, and shows `This task is no longer in Trash.` |
| Tasks were moved to Trash, restored, deleted, or edited, and Trash still holds tasks. | Shows the Empty Trash confirmation again with the new count, selects No, and adds `Trash changed. Review the count and confirm again.` |
| Trash is now empty. | Closes the confirmation, returns to Trash or the list picker, and shows `Trash is empty`. |

Review the updated confirmation and press `y` again to continue. The Empty
Trash notice can appear even when the count stays the same. For example, if
an agent restores one task and moves another task to Trash, Trash still
holds three tasks, but they're not the three tasks you reviewed.

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
filtering are not recorded, so `u` cannot reverse them. Deleting a list,
deleting a task forever, and emptying Trash forget the entry. For example,
mark a task done with Space, open Trash, and delete another task forever.
Back in All tasks, `u` shows `Nothing to undo`, and the first task stays done.
`u` does nothing while Trash is open.

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

In All tasks or a list view, press `r` or F5 to refresh database contents.
On other screens, press F5. F5 refreshes on every screen, but `r` doesn't:

- In Trash, `r` restores the selected task.
- In History, `r` reopens the selected task.
- In the task editor, the filter input, a list-name form, and the list
  pickers, `r` types the letter.

Husker also checks for changes from other processes once per second.
Navigation updates automatically. An on-screen notice tells you when
another process changed the database. The notice reads
`Database updated elsewhere. Unsaved text is kept.` and names the refresh
keys that work on the current screen. In All tasks it ends with
`r / F5 Refresh data`. In the task editor it ends with `F5 Refresh data`.
Press one of those keys to acknowledge a notice that is already visible.
This notice has no time limit. A refresh also moves a task you marked done
or cancelled from its held row to Done today or Cancelled today.

If you set up [sync between computers](sync.md), a refresh with `r` or F5
also starts a sync at once and does not wait for it to finish. Changes
that a sync brings in from another computer do not show the notice. They
show `Synced changes from another device.` for five seconds, and you don't
need to acknowledge them. See
[Read the sync messages in the terminal](sync.md#read-the-sync-messages-in-the-terminal).

If your `config.toml` binds `refresh` to other keys, Husker uses them
instead. A letter or other printable key in `refresh` works only in All
tasks and list views. See [Set the language and shortcuts](#set-the-language-and-shortcuts).

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
`r Restore`, `d Delete forever`, `D Empty Trash`, and `F4 Copy` instead of
the task edits. History adds `r Reopen`.
While a filter is applied, the Esc entry reads `Esc Clear filter`.
An entry shows the first key of an action that works on that screen, so a
rebound action shows your key.
In Danish, Delete forever reads Slet permanent and Empty Trash reads
Tøm papirkurven.

The list picker's footer starts with `Type to filter` and shows only keys
that work while you type, such as `Ctrl+N New list`, `Ctrl+R Rename list`,
`Alt+Up/Alt+Down Reorder`, `F1 Help`, and `Ctrl+C Quit`. The delete entry
reads `Ctrl+D Empty Trash` while the Trash entry is selected. The picker
for the editor's List field shows `Type to filter`, `Enter Choose`,
`Esc Back`, and `F1 Help`. The picker for moving a task shows `Enter Move`
instead of `Enter Choose`.

Press `?` or F1 to open Help with every shortcut from your own
configuration. In the list pickers, press F1, because `?` types into the
picker's filter there.
Help is grouped by screen: Task views, List picker, Task editor, Text
editing, Filter and list name, Confirmations, and Everywhere. The section
for the screen you opened Help from comes first. An action with several
keys lists them separated by ` / `, such as `q / Ctrl+C`. Text editing
lists the built-in keys from [Edit text with the keyboard](#edit-text-with-the-keyboard).
The Task views section lists Delete forever in Trash and Empty Trash, and
lists Refresh as `r / F5`.
The List picker section starts with `Letters and digits` and Backspace,
which type and remove filter text, and lists Empty Trash from the Trash
entry. It leaves out letter keys of other actions, because letters type
there. For example, it lists Quit as `Ctrl+C` only.
The Everywhere section lists Refresh as `F5`, because F5 is the refresh key
that works on every screen.
Scroll Help with Up and Down, `k` and `j`, PageUp and PageDown, Home and
End, `g` and `G`, or the mouse wheel. Clicking in Help does nothing.
Press Esc or `?` to close Help.
Keys are printed the way they are labelled on the keyboard: Ctrl+S, Esc,
F4, Space, PageUp, Enter, Tab. Danish prints the arrow keys as Op, Ned,
Venstre, and Højre.

These are the default shortcuts in All tasks, list views, History, and
Trash:

| Key | Action |
| --- | --- |
| Up or `k` | Select the previous item, wrapping from first to last. |
| Down or `j` | Select the next item, wrapping from last to first. |
| Left, Home, or `g` | Select the first item. |
| Right, End, or `G` | Select the last item. |
| PageUp or PageDown | Move the selection by one screen without wrapping. |
| Enter | Edit a task. |
| `n` | Create a task in All tasks or a list view. |
| `r` | Refresh database contents in All tasks and list views. Restore a task in Trash, or reopen a task in History. |
| `l` | Open the list picker. |
| `h` | Open History, or return to the view where you opened it. |
| `t`, `T`, `w`, `x` | Set today, tomorrow, next Monday, or no planned date outside Trash. |
| `m` | Move the selected task to another list outside Trash. |
| `u` | Undo the last task action. |
| `/` | Edit the task filter. |
| Esc | Clear an applied filter, or return from a list or History to the previous view. |
| Space | Toggle done status. |
| `c` | Toggle cancelled status. |
| `d` or Delete | Move a task to Trash, or review deleting a task forever while Trash is open. |
| `D` (Shift+D) | Review emptying Trash while Trash is open. |
| Alt+Up or `K` | Move an open task before its neighbor with the same planned date. |
| Alt+Down or `J` | Move an open task after that neighbor. |
| `?` or F1 | Open Help. Esc or `?` closes it. |
| F2 | Toggle mouse capture. |
| F3 | Choose English or Dansk and save the interface language. |
| F4 | Open plain task text for copying. |
| F5 | Refresh database contents on every screen. Also start a sync if [sync](sync.md) is set up. |
| `q` or Ctrl+C | Quit. |

The list picker has its own keys, because letters type into its filter
there. See the table in [Organize tasks into lists](#organize-tasks-into-lists).
The pickers for the editor's List field and for moving a task use the
arrow keys, Home, End, PageUp, PageDown, Enter, Esc, F1, and Backspace in
the same way.

Wrapping and first/last jumps work in task views, including History and
Trash, and in the list pickers and the language picker. PageUp and PageDown
work in the same places except the language picker. In the list pickers,
use Up, Down, Home, End, Left, and Right, because `k`, `j`, `g`, and `G`
type text there. With a filter
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
In the discard prompt and in the confirmations for deleting a list,
deleting a task forever, and emptying Trash, `y` chooses Yes and `n`
chooses No without pressing Enter.
Backspace edits text in an input field. It does not delete a task.

### Use Option+arrows in macOS Terminal

On a Mac, Alt is the Option key. To reorder tasks and lists with Option+Up
and Option+Down in Terminal.app, let an Option key act as Meta or add two
key mappings. Without one of these, Terminal sends a plain Up or Down, and
Husker moves the selection instead of the task.
In All tasks and list views, `K` and `J` (Shift+K and Shift+J) reorder
tasks without any Terminal setting. In the list picker, `K` and `J` type
into the picker's filter, so Option+Up and Option+Down are the only
default keys that reorder lists.

Meta is a modifier key for programs that run in Terminal, such as Husker.
An Option key that acts as Meta no longer types characters in Terminal.
On a Danish keyboard, Option types characters such as `@`, `|`, `{`, `[`,
and `\`.

| Way | Setup in Terminal | Husker version | Typing characters with Option |
| --- | --- | --- | --- |
| Let an Option key act as Meta | One setting | 0.18.2 or later | An Option key that acts as Meta types no characters. |
| Add two key mappings | Two key mappings | Every version | Both Option keys type characters. |

If an Option key already acts as Meta in your profile, Husker 0.18.2 and
later need no setup. Otherwise, let an Option key act as Meta if one Option
key is enough for typing. Add the two key mappings if you need both Option
keys for typing, or if your Husker is older than 0.18.2. Both ways change
one Terminal profile. They apply to every program in a window that uses
the profile.

To let an Option key act as Meta:

1. Run `husker --version`. The version must be 0.18.2 or later. With an
   older version, [update Husker](#update-an-existing-profile) or add the
   two key mappings instead.
2. In Terminal.app, open Settings, choose Profiles, select the profile
   you use for Husker, then choose Keyboard.
3. Set Use Option as Meta to Left Option only, Right Option only, or Both
   Option keys. In Danish, the menu is named Brug Alternativtast som meta.

| Choice | In Danish | Option keys that reorder | Option keys that type characters |
| --- | --- | --- | --- |
| Left Option only | Kun venstre Alternativtast | Left | Right |
| Right Option only | Kun højre Alternativtast | Right | Left |
| Both Option keys | Begge Alternativtaster | Left and right | Neither |
| None | Ingen | Neither, unless you add the two key mappings | Left and right |

For example, choose Right Option only. Option+Up with the right Option key
then moves a task. On a Danish keyboard, the left Option key still types `@`.

An older macOS version shows a checkbox named Use Option as Meta key
instead of the menu. Select the checkbox. Both Option keys then act as
Meta. If you type characters with Option, add the two key mappings instead.

To try the shortcuts, start Husker and select an open task with a neighbor
on the same planned date. Option+Up moves the task before that neighbor.
Option+Down moves it after the neighbor. The message shows `Updated`, and
the task stays selected.

To keep both Option keys for typing, add two key mappings instead:

1. In Terminal.app, open Settings, choose Profiles, select the profile
   you use for Husker, then choose Keyboard.
2. Click `+` to add a key mapping. If Option+Up already appears, edit it.
   Set Key to Cursor Up, Modifier to Option, and Action to Send Text.
3. In the text field, press Esc once, then type `[1;3A`.
   The field should show `\033[1;3A`. Click OK.
4. Add or edit Option+Down in the same way. Choose Cursor Down and send
   Esc followed by `[1;3B`, shown as `\033[1;3B`.
5. Try Option+Up and Option+Down in Husker as described above.

Keep Use Option as Meta set to None (Ingen in Danish) so that both Option
keys type characters. If Terminal shows a checkbox, leave it unchecked.
The two mappings affect only Option+Up and Option+Down.
In Danish, Key, Modifier, and Action are named Tast, Kombitast, and
Handling. Their choices are Markør op or Markør ned, Alternativ, and Send tekst.
Apple describes the controls in [Create custom function keys](https://support.apple.com/en-ca/guide/terminal/trml108/mac).

If a task does not move, check what Terminal sends. Quit Husker and run
`cat -v` at the shell prompt. Press Option+Up, then Option+Down, with the
Option key you use in Husker. Press Ctrl+C to leave `cat`.

| `cat -v` shows | Meaning | In Husker |
| --- | --- | --- |
| `^[^[[A` and `^[^[[B` | The Option key you pressed acts as Meta. | The shortcuts reorder in 0.18.2 and later. Earlier versions read them as Esc. |
| `^[[1;3A` and `^[[1;3B` | The two key mappings are in use. | The shortcuts reorder in every version. |
| `^[[A` and `^[[B` | Terminal sent a plain Up and Down without Option. | The selection moves and nothing is reordered. |

If you see `^[^[[A` and `^[^[[B` with a Husker older than 0.18.2, update
Husker or add the two key mappings.
If you see `^[[A` and `^[[B`, check that the window uses the profile you
changed. Then check that you pressed an Option key that acts as Meta, or
that both key mappings exist. If the settings have not taken effect, finish
your work in existing Terminal windows and restart Terminal when convenient.

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
both shortcuts. The `trash` keys also delete a task forever while Trash is
open, so this override changes both uses. Actions omitted from `[keys]`
keep their defaults.
Footers and Help print the keys you configure in display form, so
`ctrl+s` appears as Ctrl+S and `pageup` as PageUp.

These actions are new since 0.12.0 or have a new default key. The table
shows the defaults in 0.19.0:

| Action | Default keys | Where it works |
| --- | --- | --- |
| `page_up`, `page_down` | PageUp, PageDown | Task views, History, the list pickers, Help, and the F4 copy view. |
| `move_task` | `m` | Task views and History, not Trash. |
| `undo` | `u` | Task views and History. |
| `new_list` | Ctrl+N | The list picker. |
| `rename_list` | Ctrl+R | The list picker. |
| `delete_list` | Ctrl+D, Delete | The list picker. |
| `move_up`, `move_down` | Alt+Up and `K`, Alt+Down and `J` | Task views. Only Alt+Up and Alt+Down work in the list picker. |
| `help` | `?`, F1 | Task views, History, and the list pickers. Only F1 works in the list pickers. |
| `refresh` | `r`, F5 | F5 works on every screen. `r` works only in All tasks and list views. |
| `quit` | `q`, Ctrl+C | Task views and History. Only Ctrl+C works in the list picker. |

For example, `undo = ["ctrl+z"]` undoes a task action with Ctrl+Z in task
views. Ctrl+Z still undoes typing in text fields, because those are
different modes. `new_list = ["insert"]` creates a list with Insert in the
list picker instead of Ctrl+N.

Two rules decide where a letter or other printable key works. A printable
key is a character you can type, with or without Shift, such as `n`, `N`,
`?`, or `4`. Keys with only Ctrl or only Alt, such as `ctrl+n` or
`alt+up`, and named keys such as F5 or Delete, are not printable. A key
with both Ctrl and Alt is printable, because terminals report AltGr that
way. For example, `@` typed with AltGr on a Danish keyboard is printable.
So a binding such as `quit = ["ctrl+alt+q"]` doesn't work in the list
pickers, and `refresh = ["ctrl+alt+r"]` works only in All tasks and list
views.

- In the three list pickers, printable keys always type into the picker's
  filter. They never run an action there, even if `config.toml` binds one.
  For example, `new_list = ["N"]` loads, but `N` in the list picker types
  `N`, and no key creates a list. The footer and Help then show `unbound`
  for New list, or `ingen genvej` in Danish.
- A printable `refresh` key works only in All tasks and list views. For
  example, `r` refreshes there, restores in Trash, reopens in History, and
  types `r` in text fields and pickers.

Version 0.13.0 adds the `empty_trash` action with the default key `D`.
It belongs to task views and works only while Trash is open. For example,
`empty_trash = ["ctrl+d"]` moves Empty Trash to Ctrl+D. Deleting one task
forever has no action of its own. It uses the `trash` keys while Trash is
open. Emptying Trash from the picker uses the `delete_list` keys on the
Trash entry.

Esc is the default key of three actions, one per kind of screen:

| Action | Default keys | Screens |
| --- | --- | --- |
| `back` | Esc | Task views, History, the list and language pickers, the filter and list-name forms, and the deletion confirmations for a list, a task in Trash, and Empty Trash. |
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
For example, `delete_list = []` disables list removal and Empty Trash in
the picker. `D` in Trash still empties Trash.
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
uses `m`, `u`, `K`, `J`, Ctrl+C, PageUp, or PageDown in a task view,
Husker reports the conflict at startup. For
example, `history = ["m"]` conflicts with Move task. Keep that History
shortcut by adding `move_task = []` under `[keys]`, or rebind `move_task`
to another key. Use `page_up = []` or `page_down = []` for an action that
uses PageUp or PageDown, and `quit = ["q"]` for an action that uses Ctrl+C.

The `D` default added in 0.13.0 works the same way. If a custom action
already uses `D` in a task view, Husker reports the conflict at startup
instead of opening. For example, `history = ["D"]` stops Husker with
`Key 'D' conflicts between 'history' and 'empty_trash' in Trash mode.`
Keep that History shortcut by moving Empty Trash to another key, such as
`empty_trash = ["ctrl+d"]`, or by adding `empty_trash = []` under `[keys]`.
With `empty_trash = []`, you can still empty Trash with Ctrl+D or Delete on
the Trash entry in the list picker.

The `r` default for `refresh` added in 0.19.0 works the same way in All
tasks and list views. If a custom action already uses `r` there, Husker
reports the conflict at startup. For example, a configuration with
`restore = ["R"]` and `new_task = ["r"]` stops Husker with
`Key 'r' conflicts between 'new_task' and 'refresh' in Tasks mode.`
Keep your `r` shortcut by adding `refresh = ["f5"]` under `[keys]`, or move
the other action to another key.

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

`husker trash` moves a task to Trash, where you can still restore it.
No command deletes a task forever. To do that, use Trash in the terminal
interface. See [Delete a task forever](#delete-a-task-forever).

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

If you set up [sync between computers](sync.md), run `husker sync` to sync
the profile once. The commands above change only the local database and do
not sync, so run `husker sync` after them when the change should reach your
other computers right away:

```text
husker add "Buy milk"
husker sync
```

The second command prints what the sync received and sent:

```text
Synced. Received 0, sent 1.
```

Both numbers count lists and tasks. `Received` is how many changed on this
computer. `sent` is how many changed in the sync file. If two computers
changed the title or description of the same task, the line ends with the
number of tasks Husker kept in two versions, such as
`Synced. Received 3, sent 3. Kept both versions of 1.` See
[What happens when two computers change the same task](sync.md#what-happens-when-two-computers-change-the-same-task).

`husker --json sync` prints the same three numbers as JSON:

```json
{
  "received": 0,
  "sent": 1,
  "conflicts": 0
}
```

If the sync fails, `husker sync` prints the reason to stderr and returns a
nonzero exit code. Your changes stay in the local database, and the next
sync sends them. Without `sync.toml` in the profile directory, the command
fails with `Sync is not set up.` and names the file to create.

## Update an existing profile

Back up your profile before updating to 0.15.0.
Follow [Back up your tasks](#back-up-your-tasks), then keep every old Husker
process closed, including MCP clients, until the upgrade is complete.

Install the current version from the repository:

```text
cargo install --path . --locked
```

Version 0.19.0 changes these keys and screens:

- `r` refreshes in All tasks and list views, like F5. In Trash, `r` still
  restores a task, and in History it still reopens one.
- The task filter narrows the rows while you type. Esc in the filter input
  brings back the filter you had before you pressed `/`. See
  [Find tasks](#find-tasks).
- Typing in the list picker, the List field's picker, and the move picker
  filters the lists. See [Find a list by typing its name](#find-a-list-by-typing-its-name).
- In the list picker, create a list with Ctrl+N instead of `n`, rename it
  with Ctrl+R instead of `r`, and remove it with Ctrl+D or Delete instead
  of `d`. Reorder lists with Alt+Up and Alt+Down, because `K` and `J` type
  text. Open Help with F1 and quit with Ctrl+C, because `?` and `q` type
  text. `l` no longer closes a picker. Press Esc instead.
- F1 opens Help on every screen where `?` does.
- In All tasks and list views, Today stands out in bold black on yellow,
  and Earlier, Done, and Cancelled are dimmed. See
  [Plan tasks by date](#plan-tasks-by-date).

Check your `config.toml` for two cases:

- If it sets `new_list`, `rename_list`, or `delete_list` to letters only,
  such as `new_list = ["n", "N"]` copied from an older example, Husker
  starts, but those letters type into the picker's filter. The footer
  then shows `unbound` for New list. Remove the line to use Ctrl+N, or set
  `new_list = ["ctrl+n"]`. Do the same with `rename_list = ["ctrl+r"]` and
  `delete_list = ["ctrl+d", "delete"]`.
- If another action in All tasks or list views uses `r`, Husker stops at
  startup with an error such as
  `Key 'r' conflicts between 'new_task' and 'refresh' in Tasks mode.`
  Add `refresh = ["f5"]` under `[keys]` to keep your `r` shortcut, or move
  the other action to another key.

Run `husker config --example` to see every default of your version.
Version 0.19.0 doesn't change the database.

Version 0.18.2 changes Option+Up and Option+Down in macOS Terminal.app.
While an Option key acts as Meta, the two shortcuts now reorder tasks and
lists. Before 0.18.2, Husker read them as Esc. For example, Option+Up
cleared an applied filter. Key mappings that you added for the two
shortcuts keep working. See [Use Option+arrows in macOS Terminal](#use-optionarrows-in-macos-terminal).
Version 0.18.2 doesn't change the database.

Version 0.16.0 changes the list a new task starts in when you press `n` in
All tasks. Husker now suggests the list of the newest task outside Trash.
Before 0.16.0, it suggested the list that held the most of your 10 newest
tasks. See [Start a new task in the right list](#start-a-new-task-in-the-right-list).
Version 0.16.0 doesn't change the database, so updating from 0.15.0 or
0.15.1 needs no migration.

Version 0.15.0 changes the database format. The first time you start
`husker` or run a command such as `husker lists`, Husker migrates the
profile's database to schema version 5. The migration keeps every list,
every task field, and every task version.

A read-only MCP server cannot migrate the database. Run `husker lists` once
before you reconnect a read-only MCP client. For a custom profile, run
`husker --profile DIRECTORY lists`.

Husker 0.14.0 and older cannot open a migrated profile. Update every
Husker executable that uses the profile at the same time. That includes the
executable you start in the terminal and the executable each MCP client
starts. For example, if Claude Code starts `husker` from another path than
your terminal does, update that copy too. To go back to 0.14.0, restore the
backup you made before the update.

Existing commands, JSON fields, MCP tools, filter syntax, and configuration
settings work as before. Version 0.15.0 adds sync between computers and the
`husker sync` command. Sync is off until you create `sync.toml`, and
without that file Husker makes no network request. See
[Sync tasks between computers](sync.md).

Versions 0.9.0 through 0.14.0 use schema version 4. Updating between those
versions requires no database migration.

Version 0.14.0 changes how you create tasks in the terminal:

- The editor fields are now Title, List, Planned date, and Description,
  from top to bottom. Tab and Shift+Tab follow that order. From Title,
  press Tab once for List and twice for Planned date. If you press Tab a
  fixed number of times out of habit, or a terminal macro does, update
  that count. See [Capture your first task](#capture-your-first-task).
- A new task you create in All tasks starts in a suggested list instead of
  always the default list. Version 0.16.0 changes how Husker picks that
  list. See [Start a new task in the right list](#start-a-new-task-in-the-right-list).
- An empty Planned date shows `Sometime`, or `Engang` in Danish, instead
  of `YYYY-MM-DD`. Leaving it empty still saves no planned date.

Version 0.13.0 lets you delete tasks forever in Trash. In Trash, `d` or
Delete now asks to delete the selected task forever, and `D` asks to empty
Trash. Delete on the Trash entry in the list picker also asks to empty
Trash. See [Delete a task forever](#delete-a-task-forever) and
[Empty Trash](#empty-trash).
If your `config.toml` already uses `D` for another action in task views,
Husker reports a conflict at startup instead of opening. Add
`empty_trash = []` under `[keys]`, or bind `empty_trash` to another key.

Version 0.12.1 adds a task count to every group heading in the terminal.

Version 0.12.0 adds default shortcuts: `n` and `r` in the list picker, `K`
and `J` for reordering, `m` for moving a task, `u` for undo, Ctrl+C for
quitting, and PageUp and PageDown for paging. Version 0.19.0 replaces `n`
and `r` in the list picker with Ctrl+N and Ctrl+R. If your `config.toml` already
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

Profiles from versions 0.4.0 through 0.8.0 keep every existing task field,
task version, and saved list position. The upgrade adds a nullable completion
timestamp. Older tasks receive no invented time. A new completion or
cancellation records its time automatically.

Keep older Husker executables closed after upgrading. Versions before 0.15.0
cannot open the upgraded profile. Since 0.9.0, task JSON includes `status_changed_at`,
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
case-insensitive alphabetical order. Use Alt+Up and Alt+Down in the list
picker to arrange them after the upgrade.
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

A backup is the way to get back a task you deleted forever or removed
by emptying Trash. Open a backup made before the deletion as a separate
profile, as shown above, and copy the task's text from there.

[Sync between computers](sync.md) is not a backup of deleted tasks, because
a task you delete forever is also deleted on your other computers. Blob
versioning in Azure does keep earlier uploads of the sync file. See
[Recover an earlier version of the sync file](sync.md#recover-an-earlier-version-of-the-sync-file).

If sync is set up, the profile directory you copy contains `sync.toml` with
the URL of the sync file. Store that backup as carefully as a password.
