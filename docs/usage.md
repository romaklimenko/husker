# Use Husker

This guide is for people who want to capture and manage tasks from a terminal.

## Capture your first task

Run `husker` in an interactive terminal. Press `n`, enter a task title, and
press Ctrl+S to save. For example, enter `Buy milk`.
The task appears in Sometime because it has no due date.
Ctrl+Enter also saves when your terminal sends it as a distinct key.

Press Enter on a task to open its editor at Description. New tasks start at
Title. Use Tab and Shift+Tab to move between the fields:

| Field | What to enter |
| --- | --- |
| Title | A nonempty task title, such as `Buy milk`. |
| Description | Plain text. Press Enter here to add another line. |
| Due date | A date such as `2026-11-09`, or leave the field empty. |
| List | An existing list name, such as `Inbox`. |

Description text wraps at the editor's width. Words move to the next visual
row when needed, and long words split across rows. Wrapping does not add
newlines to the saved text. Press Enter when you want a stored line break.
Up and Down move through the displayed rows. You can also click a wrapped
row to place the cursor there.

The heading shows an existing task's ID, such as `#42`. You can use that ID
with `husker show 42` or give it to an agent. A new task has no ID until you save it.

Press Ctrl+S or Ctrl+Enter to save every field together and return to the task list.
Press Esc or Ctrl+G to leave the editor. If you changed any field, Husker asks
whether to discard the changes. No is selected initially, so Enter keeps you
in the editor. Use an arrow key or Tab to select Yes, then press Enter to discard.
Esc or Ctrl+G at the prompt also returns to editing.
An unchanged editor closes without a prompt.
Navigation shortcuts such as `q`, `d`, and `l` are ordinary text while you edit.
Due date also accepts the `t`, `T`, `w`, and `x` presets described below.
Those letters stay ordinary text in every other input field.

Remove an existing due date to move an open task to Sometime.
A done or cancelled task stays in its status section.
Write `#shopping` in the title or description if you want to find that text later.
Husker has no separate tags or readiness fields.

If a value is invalid, the editor stays open. Correct the value and press
Ctrl+S again. For example, replace `2026-02-30` with a date that exists.

If another Husker process changes the task while you edit, your save fails
and your text stays in the editor. Press F4 to copy any text you need to keep.
Return from the copy view with Esc. Press Esc again and confirm Yes to discard
the changed draft, then reopen the task to load its current values.

## Set a due date with a shortcut

Select a task in All tasks, an ordinary list, or History and use one of these keys:

| Key | Due date |
| --- | --- |
| `t` | Today. |
| `T` (Shift+T) | Tomorrow. |
| `w` | Next week on Monday. |
| `x` | No due date. |

From a task row, the new due date saves immediately. Other task fields stay
unchanged. A done task stays done and keeps its completion date and time.
The same task stays selected if it still matches your filter. For example,
pressing `t` under `due:none` removes that task from the filtered results.
The shortcuts do nothing in Trash.

Next week means the next Monday after today. On Monday, November 9, `w`
sets November 16. On Sunday, November 8, it sets November 9.
Husker uses your computer's current local date, including after midnight.

In the editor, focus Due date with Tab and use the same keys. A preset
replaces the whole field, even if you have selected only part of its text.
It does not save the task or move to another field. Press Ctrl+S to save.
Ctrl+Z restores the previous field text, cursor, and selection in one step.
You can still type a date such as `2026-11-09` manually.

The Due date field shows the configured shortcuts while it is focused.
Press `?` from navigation to find all four actions in Help.
To rebind a preset, set an action such as `due_today = ["ctrl+t"]` under
`[keys]`. Use `due_tomorrow`, `due_next_week`, or `clear_due` for the others.
An empty array disables that preset, for example `clear_due = []`.

## Edit text with the keyboard

Use these shortcuts in task fields, the filter input, or a list-name form:

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
your current list selected, or All tasks when you are in All tasks.
Press Esc or `l` again to close the picker without changing the view.

Choose a list and press Enter. For example, choosing Private shows
`All tasks > Private`. New tasks created there belong to Private.
Press Esc to return to All tasks with its previous filter, selected task,
and scroll position. Choosing All tasks in the picker gives the same result.
For example, if All tasks was filtered by `milk`, that filter returns after
you leave Private. Esc at All tasks leaves the view unchanged.

Choosing a different list or Trash starts that view without a filter.
Choosing the current list again preserves its filter, task selection,
and scroll position.

To create a list, press `N` in the picker, enter a name such as `Private`,
and press Enter. The picker returns with the new list selected immediately
before Trash. Press Enter again to open the list.
Esc in the list-name form returns to the picker without creating a list.

Select an ordinary list and press Alt+Up or Alt+Down to change its position.
Husker saves this order for the next session. The default list, initially
Inbox, stays first among ordinary lists and is marked as the default.
All tasks and Trash stay fixed.
New tasks in All tasks use the default list.

Press `R` on a list to rename it. Enter the new name and press Enter.
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

No is selected first. Press Enter or Esc to keep the list.
To remove the list, use an arrow key or Tab to choose Yes and press Enter.
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

## Plan by due date

Set an open task's Due date in the editor to place the task in a dated group.
Husker uses your computer's local calendar date to group open tasks:

| Group | Tasks shown |
| --- | --- |
| Overdue | Due before today. |
| Today | Due today. |
| Tomorrow | Due tomorrow. |
| Soon | Due from two days through seven days after today, inclusive. |
| Next | Due eight or more days after today. |
| Sometime | No due date. Shown as Engang in Danish. |

If today is November 8, Soon covers November 10 through November 15.
Next starts on November 16. Open tasks appear in those groups first.
Within a dated group, earlier due dates appear first. Sometime is the last
open-task group.

Done today and Cancelled today follow the open tasks. For example, completing
an undated task moves it from Sometime to Done today without adding a due date.
A task completed today appears there even if its stored due date is overdue.
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

Use Alt+Up or Alt+Down to change the order of open tasks sharing the same
due date. Undated open tasks can move among other open tasks in Sometime.
Done and cancelled tasks use their recorded status dates and times and cannot
be reordered manually.
Reordering never changes a due date or moves a task between lists.
With a filter active, reordering uses neighboring tasks in the visible results.

## Find tasks

Press `/`, type a query, and press Enter to apply it. Press Esc to discard
your query edits. Apply an empty query to clear the filter.
If you opened a particular list, clearing the filter keeps that list selected.

Combine terms to narrow the results. Every term must match:

| Query | Result |
| --- | --- |
| `milk` | Tasks whose title or description contains `milk`. |
| `l:private` | Tasks in the list named `Private`. |
| `#shopping` | Tasks whose title or description contains `#shopping`. |
| `l:private milk #shopping` | Tasks matching all three terms. |
| `due:today` | Tasks due today. |
| `due:2026-11-08` | Tasks due on that exact date. |
| `due:none` or `due:undated` | Tasks with no due date. |
| `s:open` | Open tasks only. |
| `s:done` | Completed tasks. |
| `s:cancelled` | Cancelled tasks. |
| `s:all` | Tasks of every status in the current view. |

Use any of these date-group tokens with `due:`: `overdue`, `today`,
`tomorrow`, `soon`, `next`, or `undated`.
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

The most recent recorded status dates appear first. Headings show Today,
Yesterday, or the calendar date. Within one date, later completion or
cancellation times appear first. Tasks migrated without a recorded time
follow tasks with known times on the same date. Equal or unknown times use
descending task ID. Tasks without a recorded date appear under Unknown date.

Husker records the timestamp in UTC and keeps a separate local date for
headings and the daily task view. It does not invent times for older tasks.

Press `/` to search History. For example, `s:done milk` finds completed tasks
containing `milk`. `s:cancelled` shows cancelled tasks. `s:all` keeps both
closed statuses, and `s:open` matches nothing in History.
Due filters still check the task's due date. `due:today` does not mean
completed today.

Press Enter to edit a historical task or F4 to read and copy its text.
Press `r` to reopen it. The task leaves History and becomes open in its list.
You can also use the usual status and Trash shortcuts.
New-task creation and manual reordering are unavailable in History.

Press Esc or `h` to return to the view where you opened History.
Your previous filter, selected task, and scroll position return.
Press `l` to choose All tasks or another task list and leave History.
Choosing the same list restores the view from before you opened History.
Choosing another list starts with an empty filter.

History shows each task's current status. Reopening clears its recorded
completion date and time. Husker does not keep a log of earlier status changes or
previous versions of task text. To find deleted tasks, open Trash from
the list picker instead.

## Complete, cancel, and restore tasks

Press Space to mark a task done. Press `c` to cancel a task.
The task moves to Done today or Cancelled today for the rest of the local day.
Selection moves to the next open task, or the previous open task if none
follows it. Tomorrow, the default view hides the closed task.
Press `h` to find it in History. Use `s:open` to hide completed and cancelled
tasks immediately.

Apply `s:done` to see completed tasks, then press Space on a task to reopen
it. Apply `s:cancelled` and press `c` to reopen a cancelled task.
You can also use `s:all` to review every status.

Press `d` or Delete to move a task to Trash. Open `l` and select Trash, the
last entry, to see deleted tasks. Press `r` to restore the selected task.
The task returns to its original list with its original status.
A restored done task stays done. It appears in the default view only if
it was completed today. Restoring a task does not reset its completion date.

Trash counts every deleted task, regardless of status. Tasks in Trash are
excluded from ordinary task views even when the query is `s:all`.
Trash keeps the task's title, description, due date, status date, and list membership.
Trash uses the heading `All tasks > Trash` and has no date-group headings.
Press Esc to return to your previous All tasks view.

## Use the mouse and copy text

Select text with the keyboard in a task field, filter input, or list-name
form, then press Ctrl+C to copy that selection. A message confirms success.
Copying preserves your unsaved text, cursor, and selection.
Ctrl+C has no Husker clipboard action outside those inputs.

If copying fails, the error leaves the selection intact. From a task editor,
press F4 and use your terminal's Copy command instead.
Text containing a null character cannot be copied through Ctrl+C.

Click a task or list to select it. Double-click the same row within 400 ms
to open it. The mouse wheel moves through the rows. In an editor, click
a field to focus it and place the text cursor.
When the picker is open, clicks and wheel movements outside the popup do
nothing. Scroll inside the picker to move through its entries.

Press F2 to turn mouse capture off when you want to select text with your
terminal. Press F2 again to restore Husker's mouse navigation.

Press F4 on a task to open its title and description as plain text.
F4 also works inside the task editor and shows your current unsaved text.
The copy view has no vertical borders or inserted line wrapping.
Drag to select text, then use your terminal's Copy command.
Use arrow keys to scroll vertically or horizontally when the text does not fit.
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
This notice has no time limit.

Other status messages disappear after five seconds. For example, `Saved`
clears after you save a task, even if you navigate to another list.
Errors also clear after five seconds. If you enter the invalid due date
`2026-02-30`, the error disappears but the editor keeps your unsaved text.
Correct the date and press Ctrl+S to try again.

If you are editing a task, refresh preserves your unsaved text.
Your save still checks the task version. If the agent changed that task,
copy your draft with F4, cancel the editor, and reopen the task before saving again.

## Use the keyboard

These are the default navigation shortcuts. Press `?` inside Husker to see
the shortcuts from your own configuration. Use Up and Down to scroll help
when all actions do not fit on screen.

| Key | Action |
| --- | --- |
| Up or `k` | Select the previous row. |
| Down or `j` | Select the next row. |
| Enter | Choose a picker entry or edit a task. |
| `n` | Create a task outside History and Trash. |
| `N` | Create a list from the picker. |
| `R` | Rename an ordinary list from the picker. |
| `l` | Open or cancel the list picker. |
| `h` | Open History, or return to the view where you opened it. |
| `t`, `T`, `w`, `x` | Set today, tomorrow, next Monday, or no due date outside Trash. |
| `/` | Edit the task filter. |
| Esc | Return from a list to All tasks, return from History, or cancel the picker. |
| Space | Toggle done status. |
| `c` | Toggle cancelled status. |
| `d` or Delete | Move a task to Trash, or review deleting a list in the picker. |
| `r` | Restore a task from Trash, or reopen a task in History. |
| Alt+Up or Alt+Down | Reorder open tasks with the same due date, or ordinary lists in the picker. |
| F2 | Toggle mouse capture. |
| F3 | Choose English or Dansk and save the interface language. |
| F4 | Open plain task text for copying. |
| F5 | Refresh database contents. |
| `?` | Open help. Esc closes help. |
| `q` | Quit. |

In the task editor, Ctrl+Enter or Ctrl+S saves. Esc or Ctrl+G asks before
discarding changed fields. An unchanged editor closes immediately.
F4 opens the copy view without discarding the draft. In the filter and list-name
forms, Enter applies the input and Esc cancels it.
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

Restart Husker to load the file. Ctrl+Q now quits, and `q` no longer quits.
Use `language = "en"` for English. Your task text and list names are never
translated. Filter operators and CLI commands stay in English.

Each shortcut array replaces all default keys for that action. For example,
`trash = ["delete"]` keeps Delete and removes `d`. Use `trash = []` to disable
both shortcuts. Actions omitted from `[keys]` keep their defaults.
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
The new action names are `history`, `reopen`, `delete_list`, and `language`.
For example, `delete_list = []` disables removal shortcuts in the picker.
Under `[keys]`, `language = ["f6"]` changes the language-picker shortcut to F6.

Key strings accept characters such as `q` or `N`, named keys such as `enter`
or `f2`, and modifiers such as `ctrl+q` or `alt+up`.
Use `plus` for the `+` key. Run this command to see every action and default:

```text
husker config --example
```

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
interface. For example:

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

Back up your profile before updating an earlier version to 0.9.0.
Follow [Back up your tasks](#back-up-your-tasks), then keep every old Husker
process closed, including MCP clients, until the upgrade is complete.

Install the current version from the repository:

```text
cargo install --path . --locked
```

Version 0.9.0 upgrades the database to schema version 4. Run `husker lists`
once before reconnecting a read-only MCP client. For a custom profile, run
`husker --profile DIRECTORY lists`. A read-only connection cannot upgrade it.

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
case-insensitive alphabetical order. Use Alt+Up and Alt+Down in the
list picker to arrange them after the upgrade.
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
