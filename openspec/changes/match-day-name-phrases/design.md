## Context

Read from source: `Query::parse` in `src/query.rs` splits unquoted text at whitespace into separate text terms, and every term must match.
Read from source: `Query::with_group_names` is called only by `App::rebuild_visible` in `src/ui.rs`, with the interface language.
Read from source: the day names come from `tr` in `src/i18n.rs`. The Danish names I dag, I morgen, and I går have two words. All English names have one word.
Observed by the user in 0.20.0: `i mo` listed the I morgen tasks and also tasks under Tidligere, I dag, Engang, and Udført i dag whose text contained both `i` and `mo`.

## Goals / Non-goals

Goal: a Danish user can type a day name as it reads, such as `i mo`, and see only that day's tasks plus text matches of the same phrase.
Goal: every other filter keeps matching word by word.

Out of scope:

1. Matching the whole filter as one string. `buy milk` would stop finding `Buy oat milk`.
2. Phrases in `husker tasks`, MCP, and the online server, which do not match day names.
3. Phrases that start in the middle of a day name, such as `dag x`.

## Decisions

### Join words only when they start a day name

`Query::with_group_names` reads the text terms from the left. At each unquoted term it looks for the longest run of two or more unquoted terms whose text, joined by single spaces, starts one of the language's day names. Such a run becomes one unquoted term.
The day names checked are the names the current view shows: Earlier, Today, Tomorrow, and Sometime in All tasks and list views, Today and Yesterday in History, and none in Trash. `day_name_keys` lists them, and `visible_tasks` and `visible_history` join the phrases once per call through `Query::with_phrases`.
Only terms at consecutive positions in the filter text join. `Query::parse` records each text term's position among all terms, so a field term such as `l:home` between two words keeps them apart.
Rejected: checking all five names in every view. `i g` would form a phrase for I går in All tasks, which has no such heading, and Trash results would differ between Danish and English although Trash has no headings. A review of the first draft found both effects.
Only names with two or more words can start with a run of two words, so the rule affects only Danish.
Rejected: matching the whole filter text as one phrase. Common searches with two words in any order would break.
Rejected: matching both the phrase and the separate words. `i mo` would still list the today task, which is the confusion the user reported.

### A phrase matches a day name from a word start

`day_name_matches` tests whether the name, from the start of any of its words, starts with the term. For a one-word term that is the same test as before.
Titles and descriptions use the existing substring test, so a phrase matches text that contains it with one space between the words.

## Risks / Trade-offs

A Danish filter in which `i` is followed by the start of dag, morgen, or går no longer matches the two words separately. For example, `i m` no longer lists a task under I dag whose title contains `m`. The user reported that result as confusing.
Results narrow steadily while the user types `i mo`. After the space, each step is already a phrase: `i m` lists only the I morgen tasks and text containing `i m`.

Rolling back means reinstalling 0.20.0. No data changes.
