## Why

The terminal task filter matches the day names that the terminal headings show.
Typing `to` lists today's and tomorrow's tasks, along with tasks whose text contains `to`. Typing `tod` narrows the rows to today's tasks.
A request in the Husker task inbox asked for this. Before this change, the user has to know and type `due:today` or `due:tomorrow` to filter by day.
The list pickers also stop showing the `Type to filter` hint in their bottom border. The user asked for this because the hint crowds the small popup, and the footer already shows it.

## What changes

1. In the terminal task filter, an unquoted text term also matches a task when a word of the day name in the task's heading starts with the term. Case is ignored.
2. In All tasks and list views, the names are the date group headings: Earlier, Today, Tomorrow, and Sometime in English, and Tidligere, I dag, I morgen, and Engang in Danish. Soon and Next have no name to match, because their headings show dates such as `2026-11-20 Fri W47`.
3. In All tasks and list views, the date group comes from the task's planned date for every status, so a completed task keeps matching the name of its held row's heading. In History, the names are Today and Yesterday, from the day the task was closed, as History's headings show. Trash has no headings, so no name matches there.
4. Only the names of the selected interface language match. A language change applies to an applied filter at once.
5. Wholly quoted terms and `#` terms match only titles and descriptions, as before. So `"to"` finds only task text.
6. The `husker tasks` command, the local MCP server, and the online server match text terms against titles and descriptions only, as before.
7. The bottom border of the three list pickers stays empty until the user types. Then it shows the filter text, such as `Filter: hu`. The footer keeps its `Type to filter` entry.

## Capabilities

### New capabilities

None.

### Modified capabilities

- `terminal-interface`: a new requirement, Match day names in the task filter, and an empty picker border in the requirement Filter list pickers by typing.
- `planning-search`: the requirement Combined filters points to the terminal rule and states that `husker tasks`, the local MCP server, and the online server match text against titles and descriptions only.

## Impact

| File | Change |
| --- | --- |
| `src/query.rs` | Text terms record whether they are quoted. `Query::with_group_names` lets unquoted terms also match the day name of each view's headings. |
| `src/ui.rs` | Task rows are filtered with the group names of the interface language. A language change rebuilds the rows. The picker border drops its hint. |
| `README.md`, `docs/usage.md`, `docs/mcp.md` | The date words in the filter, the text-only matching of agent searches, and the empty picker border |
| `docs/development.md` | The checks for version 0.20.0 |
| `Cargo.toml`, `Cargo.lock` | Version 0.20.0 |

Compatibility effects:

1. A terminal filter such as `to` or `some` can list more tasks than before. Quoting the term, as in `"to"`, gives the old result.
2. The configuration, the database, the sync format, the CLI, the MCP tools, the online server, and the phone page do not change. No migration is needed.
