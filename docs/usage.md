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

Remove an existing due date to move the task to Sometime.
Write `#shopping` in the title or description if you want to find that text later.
Husker has no separate tags or readiness fields.

If a value is invalid, the editor stays open. Correct the value and press
Ctrl+S again. For example, replace `2026-02-30` with a date that exists.

If another Husker process changes the task while you edit, your save fails
and your text stays in the editor. Press F4 to copy any text you need to keep.
Return from the copy view with Esc. Press Esc again and confirm Yes to discard
the changed draft, then reopen the task to load its current values.

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
Use [F4 and your terminal's Copy command](#use-the-mouse-and-copy-text) to copy task text.

## Organize tasks into lists

Press `l` to open the list chooser. Press `N` to create a list, type its name,
and press Enter. For example, create `Private`.
Press Esc instead to cancel the list-name form.

Select a list and press Enter to see its planned tasks. New tasks created in
that view belong to the selected list. Press Esc to return to the list
chooser with that list still selected, or `p` to show tasks from every list.
The default list, initially Inbox, stays first. Trash stays last.
New tasks in the global view use the default list.

Select another list and press Alt+Up or Alt+Down to change its position.
Husker saves this order for the next session. Lists cannot move above the
default list or below Trash. New lists appear immediately before Trash.

Press `R` on a list to rename it. Enter the new name and press Enter.
The tasks stay in the same list. Names are unique without regard to case,
so `Private` and `PRIVATE` cannot be separate lists.
`Trash` and `Papirkurv` are reserved names.
Renaming a list keeps its position. Renaming the default list keeps that
same list first and keeps it as the default destination for new tasks.

The number beside an ordinary list counts its open tasks outside Trash.
For example, `Private (4)` means that list has four open tasks.
Done and cancelled tasks do not contribute to that count.

## Plan by due date

Set a task's Due date in the editor to place the task in a dated group.
Husker uses your computer's local calendar date to calculate these groups:

| Group | Tasks shown |
| --- | --- |
| Overdue | Due before today. |
| Today | Due today. |
| Tomorrow | Due tomorrow. |
| Soon | Due from two days through seven days after today, inclusive. |
| Next | Due eight or more days after today. |
| Sometime | No due date. Shown as Engang in Danish. |

If today is November 8, Soon covers November 10 through November 15.
Next starts on November 16. Open tasks appear before done and cancelled tasks
within each group. Within those two sets, tasks follow due-date order.
For example, in Soon, an open task due November 15 comes before a done task
due November 10. An Overdue task still appears before every Today task.
Sometime is the final group. Tasks completed or cancelled today remain visible.
Trash is always separate from the planned view.

Task rows use the full navigation width. The selected task has no side
description preview. Press Enter to open its Description field, or F4 to
read and copy its plain text.
Date-group headings are labels. They do not receive selection highlighting,
and keyboard navigation skips them.

Date labels include the weekday and ISO week number:
`2026-11-08 Sun W45`. Danish uses `2026-11-08 søn W45`.
At a year boundary, `2027-01-01 Fri 2026-W53` shows that January 1 belongs
to the final ISO week of 2026.

Use Alt+Up or Alt+Down to change the order of tasks sharing the same due
date. Tasks without dates can move among other tasks in Sometime.
Open tasks move only among other open tasks. Done and cancelled tasks can
move among each other, below the open tasks for that date.
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

## Complete, cancel, and restore tasks

Press Space to mark a task done. Press `c` to cancel a task.
The task stays visible for the rest of the local day with its new status.
It moves below the open tasks within its date group.
Tomorrow, the default view hides it. Use `s:open` to hide completed and
cancelled tasks immediately.

Apply `s:done` to see completed tasks, then press Space on a task to reopen
it. Apply `s:cancelled` and press `c` to reopen a cancelled task.
You can also use `s:all` to review every status.

Press `d` or Delete to move a task to Trash. Open `l` and select Trash, the
last entry, to see deleted tasks. Press `r` to restore the selected task.
The task returns to its original list with its original status.
A restored done task stays done. It appears in the default view only if
it was completed today. Restoring a task does not reset its completion date.

Trash counts every deleted task, regardless of status. Tasks in Trash are
excluded from planned views even when the query is `s:all`.
Trash keeps the task's title, description, due date, status date, and list membership.

## Use the mouse and copy text

Click a task or list to select it. Double-click the same row within 400 ms
to open it. The mouse wheel moves through the rows. In an editor, click
a field to focus it and place the text cursor.

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
| Enter | Open a list or edit a task. |
| `n` | Create a task in a task view. |
| `N` | Create a list from the list chooser. |
| `R` | Rename a list from the list chooser. |
| `l` | Show lists. |
| `p` | Show planned tasks from all lists. |
| `/` | Edit the task filter. |
| Esc | Return to the list chooser. |
| Space | Toggle done status. |
| `c` | Toggle cancelled status. |
| `d` or Delete | Move a task to Trash. |
| `r` | Restore a task from Trash. |
| Alt+Up or Alt+Down | Reorder tasks within the same date or undated group, or reorder lists in the chooser. |
| F2 | Toggle mouse capture. |
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

Create `config.toml` in your profile directory to change the interface.
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
and place the task in Sometime:

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
Rename a list with `husker list-rename 2 Home`, replacing `2` with the list's ID.
The renamed list keeps its position.
Every command accepts `--profile DIRECTORY` to select its task collection.

## Update an existing profile

Back up your profile before updating an earlier version to 0.5.0.
Follow [Back up your tasks](#back-up-your-tasks), then keep every old Husker
process closed, including MCP clients, until the upgrade is complete.

Install the current version from the repository:

```text
cargo install --path . --locked
```

Version 0.5.0 uses the same database schema as 0.4.0. A profile from 0.4.0
needs no database conversion, and its saved tasks and list order stay unchanged.
The steps below apply when upgrading older profiles or old shortcut overrides.

If your `config.toml` overrides the old `anytime` or `maybe` actions, remove
those lines. If you set `save = ["esc"]`, remove that override to use the new
save and cancel defaults. Esc now belongs to the discard action by default.
Old save and discard overrides can conflict until you update them together.

For a profile created before 0.4.0, run `husker lists` once before reconnecting
a read-only MCP client. For a custom profile, run `husker --profile DIRECTORY lists`.
This writable command upgrades the database to schema version 3.
A read-only MCP connection cannot perform the upgrade.

The upgrade adds saved list positions.
The default list moves to the top. Other existing lists start in
case-insensitive alphabetical order. Use Alt+Up and Alt+Down in the
list chooser to arrange them after the upgrade.
For a profile from version 0.3.0, every task field and task version stays unchanged.

For profiles from version 0.1 or 0.2, the upgrade also simplifies task fields.
It preserves task IDs, lists, due dates, and statuses.
Existing description text stays intact. Existing tags become `#words` in a
final description paragraph.
Tasks that gain this text get a new version, so an agent must read them again
before saving an edit prepared before the upgrade.
Readiness is removed, so every task without a date appears in Sometime.
Older completed tasks have no recorded completion date and stay hidden from
the default view. Use `s:done`, `s:cancelled`, or `s:all` to find them.

Update scripts that use `--tag`, `--clear-tags`, `--anytime`, or `--readiness`.
Those options now produce argument errors. Write `#words` in the title or
description instead. Replace `due:anytime` and `due:maybe` with `due:none`.
MCP clients must refresh their tool definitions after the upgrade.

Start Husker and reconnect MCP clients after the upgrade.
Husker versions before 0.4.0 cannot open a profile migrated to schema version 3.

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
