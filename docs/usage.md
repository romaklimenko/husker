# Use Husker

This guide is for people who want to capture and manage tasks from a terminal.

## Capture your first task

Run `husker` in an interactive terminal. Press `n`, enter a task title, and
press Esc to save. For example, enter `Buy milk`.
The task appears in Maybe because it has no due date and is not marked ready.

Press `a` on an undated task when you are ready to pick it up. The task moves
to Anytime. Press `m` to move a task to Maybe. Both actions remove an existing
due date.

Press Enter on a task to open its editor at Description. New tasks start at
Title. Use Tab and Shift+Tab to move between the fields:

| Field | What to enter |
| --- | --- |
| Title | A nonempty task title, such as `Buy milk`. |
| Description | Plain text. Press Enter here to add another line. |
| Due date | A date such as `2026-11-09`, or leave the field empty. |
| List | An existing list name, such as `Inbox`. |
| Tags | Space-separated names, such as `shopping weekly`. A leading `#` is optional. |
| Readiness | `maybe` or `anytime`. A due date takes precedence over this field. |

Press Esc to save every field together and return to the task list.
Press Ctrl+G to discard the changes. Navigation shortcuts such as `q`, `d`,
and `l` are ordinary text while you are editing.

If you remove an existing due date, the task moves to Anytime by default.
Edit the Readiness field to `maybe` in the same save to choose Maybe instead.

If a value is invalid, the editor stays open. Correct the value and press
Esc again. For example, replace `2026-02-30` with a date that exists.

If another Husker process changes the task while you edit, your save fails
and your text stays in the editor. Copy any text you need to keep before
discarding. Press Ctrl+G, then reopen the task to load its current values.

## Organize tasks into lists

Press `l` to open the list chooser. Press `N` to create a list, type its name,
and press Enter. For example, create `Private`.
Press Esc instead to cancel the list-name form.

Select a list and press Enter to see its planned tasks. New tasks created in
that view belong to the selected list. Press Esc to return to the list
chooser, or `p` to show planned tasks from every list.

Press `R` on a list to rename it. Enter the new name and press Enter.
The tasks stay in the same list. Names are unique without regard to case,
so `Private` and `PRIVATE` cannot be separate lists.
`Trash` and `Papirkurv` are reserved names.

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
| Anytime | No due date, marked ready. |
| Maybe | No due date, not marked ready. |

If today is November 8, Soon covers November 10 through November 15.
Next starts on November 16. Done, cancelled, and trashed tasks stay out of
the default planned view.

Date labels include the weekday and ISO week number:
`2026-11-08 Sun W45`. Danish uses `2026-11-08 søn W45`.
At a year boundary, `2027-01-01 Fri 2026-W53` shows that January 1 belongs
to the final ISO week of 2026.

Use Alt+Up or Alt+Down to change the order of tasks sharing the same due
date. Undated tasks can move within Anytime or within Maybe.
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
| `#shopping` | Tasks tagged `shopping`. |
| `l:private milk #shopping` | Tasks matching all three terms. |
| `due:today` | Tasks due today. |
| `due:2026-11-08` | Tasks due on that exact date. |
| `due:none` | Tasks with no due date. |
| `s:done` | Completed tasks. |
| `s:cancelled` | Cancelled tasks. |
| `s:all` | Tasks of every status in the current view. |

Use any of these date-group tokens with `due:`: `overdue`, `today`,
`tomorrow`, `soon`, `next`, `anytime`, or `maybe`.
For example, `due:anytime #shopping` finds ready, undated shopping tasks.

List names and tags match whole names. Text matches part of a title or
description. These comparisons ignore case and preserve accents.
For example, `ÆBLE` matches `Køb æbler`, but does not match `Køb abler`.

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
The task leaves the default open-task view after either action.

Apply `s:done` to see completed tasks, then press Space on a task to reopen
it. Apply `s:cancelled` and press `c` to reopen a cancelled task.
You can also use `s:all` to review every status.

Press `d` or Delete to move a task to Trash. Open `l` and select Trash, the
last entry, to see deleted tasks. Press `r` to restore the selected task.
The task returns to its original list with its original status.
A restored done task stays done and does not appear in the default open view.

Trash counts every deleted task, regardless of status. Tasks in Trash are
excluded from planned views even when the query is `s:all`.
Trash keeps the task's title, description, due date, tags, and list membership.

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
| `a` | Remove the due date and move to Anytime. |
| `m` | Remove the due date and move to Maybe. |
| `d` or Delete | Move a task to Trash. |
| `r` | Restore a task from Trash. |
| Alt+Up or Alt+Down | Reorder tasks within the same date or undated group. |
| `?` | Open help. Esc closes help. |
| `q` | Quit. |

In the task editor, Esc saves and Ctrl+G discards. In the filter and list-name
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
translated. Filter operators, readiness values, and CLI commands stay in English.

Each shortcut array replaces all default keys for that action. For example,
`trash = ["delete"]` keeps Delete and removes `d`. Use `trash = []` to disable
both shortcuts. Actions omitted from `[keys]` keep their defaults.

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
husker add "Buy milk" --list Private --description "Two cartons" --tag shopping
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

`edit` changes only supplied fields. Use `--clear-tags` to remove all tags,
or repeat `--tag` to replace them. Use `--clear-due` to remove the due date
and mark the task ready for Anytime.
Add `--readiness maybe` to choose Maybe explicitly:

```text
husker edit 1 --clear-due --readiness maybe
```

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

Use `husker lists` to see list IDs and counts. Rename a list with
`husker list-rename 2 Home`, replacing `2` with the list's ID.
Every command accepts `--profile DIRECTORY` to select its task collection.

## Back up your tasks

Close every Husker instance that uses the profile, including any running
commands. Copy the whole profile directory to your backup location.
For the default profile, copy `~/.husker`, including `tasks.db`, configuration,
and any database companion files that remain.

To restore a backup without replacing your current tasks, copy the backup
to a separate directory and open that directory as a profile:

```text
husker --profile ./restored-tasks
```

Keep the original profile until you have checked the restored tasks.
