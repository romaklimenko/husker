## Why

In the terminal filter, words in a row that together start a Danish day name match as one phrase. `i mo` lists the I morgen tasks and tasks whose text contains `i mo`.
Version 0.20.0 matched `i` and `mo` as separate words. `i` matched I dag, and `mo` matched a title such as `Configure model source`. So the filter `i mo` also listed that task, which is planned for today. The user expected the filter to match the string `i mo`.

## What changes

1. Two or more adjacent unquoted text terms form one phrase term when their text, joined by single spaces, starts a day name that the current view shows, in the selected language. The longest run from the left wins. A field term between two words keeps them apart.
2. A phrase term matches a day name that starts with it, or a title or description that contains it.
3. Only Danish has day names of more than one word: I dag, I morgen, and I går. English filters do not change.
4. Other words stay separate terms, so `buy milk` still finds `Buy oat milk`.
5. `husker tasks`, the MCP servers, and the online server do not change.

## Capabilities

### New capabilities

None.

### Modified capabilities

- `terminal-interface`: the requirement Match day names in the task filter gains the phrase rule.

## Impact

| File | Change |
| --- | --- |
| `src/query.rs` | Text terms record their position. `visible_tasks` and `visible_history` join adjacent unquoted terms that start a day name of the view. A phrase matches a day name from a word start. |
| `docs/usage.md`, `README.md` | The phrase rule for Danish names |
| `docs/development.md` | The checks for version 0.20.1 |
| `Cargo.toml`, `Cargo.lock` | Version 0.20.1 |

Compatibility effects:

1. A Danish filter such as `i mo` or `i d` can list fewer tasks than in 0.20.0. Quoting each word, as in `"i" "mo"`, matches the words separately in task text only.
2. No configuration, database, sync format, CLI, MCP, online server, or phone page change is needed. No migration is needed.
