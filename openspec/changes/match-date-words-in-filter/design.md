## Context

Read from source: `Query::matches` in `src/query.rs` lowercases each text term and tests whether the title or description contains it.
Read from source: `Query::parse` already tells quoted terms and `#` terms apart from other text, and stores all of them as plain strings.
Read from source: `Bucket::for_task` in `src/planning.rs` classifies a task by its stored due date for every status. `tr(language, bucket.key())` gives the names Earlier, Today, Tomorrow, Soon, Next, and Sometime.
Read from source: `history_group_label` names History headings by `status_changed_on`: Today, Yesterday, a date, or Unknown date. Trash has no headings.
Read from source: terminal headings for Soon and Next show exact dates, such as `2026-11-20 Fri W47`, and never the words Soon or Next.
Read from source: `App::rebuild_visible` in `src/ui.rs` is the only place the terminal filters task rows. `App::set_language` runs at the start of each frame and key.
Read from source: `husker tasks`, the local MCP server in `src/mcp.rs`, and the online server in `src/online/` reach the same `visible_tasks` function.

## Goals / Non-goals

Goal: a user filters by day with the words the terminal shows, and short prefixes such as `to` work while typing.
Goal: automation keeps exact text matching.

Out of scope:

1. Weekday names, dates, and week numbers in headings.
2. Status headings such as Done today and Cancelled today.
3. The names Soon, Snart, Next, and Senere, which no terminal heading shows.
4. History headings that show dates or Unknown date.
5. Fuzzy matching and names from the language that is not selected.

## Decisions

### Match the day name of each view's headings

An unquoted text term matches a task when a word of the task's day name starts with the term. `Query::matches` receives the view as a `GroupLayout`, and `day_name` picks the name.
In All tasks and list views, the name comes from `Bucket::for_task`, so it follows the planned date for every status, as `due:` filters do.
A task completed today that was planned for tomorrow therefore still matches `tom`. Its held row stays in place when the user completes it under a `tom` filter.
In History, the name comes from `status_changed_on`: Today or Yesterday, as History's headings show. A review of the first version found that the planned date there contradicts the headings: `tod` listed a task under a dated heading and hid a task under Today.
Trash has no headings, so it has no names.
Rejected: matching the full heading text in All tasks and list views. A completed task moves from Tomorrow to Done today, so a held row would disappear under `tom`.
Rejected: the planned date in every view. In History it contradicts the headings on screen.

### Match the start of a word

The term must start a word of the name. Spaces separate the words, so I dag has the words I and dag.
`to` matches Today and Tomorrow, `dag` matches I dag, and `me` matches no name.
Rejected: substring matching. `me` would list every Sometime task while the user types `meeting`.

### Use only the selected language

The names come from the interface language, so the filter matches what the screen shows.
`App::rebuild_visible` passes `App::language` through `Query::with_group_names`. `App::set_language` rebuilds the rows when the language changes and keeps the selected task.
Rejected: matching both languages. `i` in English would list today's and tomorrow's tasks through the Danish names.

### Keep the match in the terminal

`Query` gets an optional group-name language, which is `None` after parsing. Only the terminal sets it.
`husker tasks`, MCP clients, and the online server keep exact text matching. Assumed: agents expect `today` to find the word in task text, because the MCP tool description presents `due:today` for dates. Their output has no group headings.
Rejected: a new `visible_tasks` parameter. Every other caller would have to pass `None`.

### The picker border shows only typed text

`draw_list_popup` adds a bottom title only when the picker filter has text.
The footer already starts with `Type to filter`, so the hint stays on screen once.
Rejected: removing the footer entry too. The footer is the only place that tells the user that letters type in the picker.

### Quoting turns the match off

Wholly quoted terms and `#` terms match only titles and descriptions. A partly quoted term such as `to"d"` is ordinary text, as `Query::parse` already treats it. `"to"` gives the result that `to` gave before this change.
`Query::parse` already treats both kinds of term as literal text, so quoting is the existing way to ask for exact text. The parser records that as the `literal` flag of each text term.

## Risks / Trade-offs

Short terms list more tasks. `to` lists every task due today or tomorrow. The added tasks belong to the groups the user named, and quoting the term restores the old result.
In Danish, `i` lists every task due today or tomorrow, because both names start with the word I. `dag` and `morgen` are ordinary Danish words, so a search for `dag` while looking for `fødselsdag` also lists today's tasks.
A held row can disappear after a language change. A row held under `tom` in English does not match any Danish name, so switching to Danish drops it from the results.

Rolling back means reinstalling 0.19.0. No data changes.
