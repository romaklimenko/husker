# Husker

Husker is a local Rust terminal todo app. Keep the interface quick to use with
the keyboard. Support English and Danish from the first working version.

## Working agreement

- Do not commit or push until the user explicitly asks after reviewing changes.
- Keep this file current when the workflow, commands, or project structure change.
- Keep shared instructions here. `CLAUDE.md` contains only `@AGENTS.md`.
- Read applicable lessons at the start of work. Follow the user's shared lessons
  workflow. Never stage `LESSONS.md` without an explicit request.
- Do not add secrets, personal task databases, or real configuration values.

## Draft inbox

`DRAFT.md` is the user's inbox. `ACTIVE.md` is the agent's work queue. Both are
ignored. Treat `DRAFTS.md` as a spelling variant, not a second inbox.

1. Read the inbox and copy selected thoughts verbatim to `ACTIVE.md` before work.
2. Record progress and unresolved work in `ACTIVE.md`, not in the inbox.
3. After a batch, reread `DRAFT.md`. Remove only completed thoughts whose exact
   text still matches the captured copy. Keep new or changed text untouched.
4. If the user is editing the same thought, leave it in the inbox. An extra copy
   is preferable to losing a note. Never replace the inbox from an old snapshot.
5. Remove completed work from `ACTIVE.md` after its result is recorded in the
   repository or reported to the user. Keep unfinished work explicit.

## OpenSpec

- Use OpenSpec for substantial features. Create proposal, design, capability
  specs, and tasks before implementation. Validate with
  `openspec validate <change> --strict`.
- Treat `openspec/changes/` as proposed behavior and `openspec/specs/` as the
  behavior of completed changes. Do not claim a proposal is implemented.
- Follow the artifact rules in `openspec/config.yaml`.
- Keep task checkboxes accurate. Archive only after implementation and checks
  are complete. Replace generated `TBD` purpose text when archiving.
- Existing user authorization also covers routine implementation decisions.
  Do not add a separate approval gate unless the user asks for one.

## Documentation

- Use `.claude/agents/docs-writer.md` proactively for README, guides, and spec
  reviews. Claude Code can invoke the named agent. Codex reads that definition
  and delegates the same task to a subagent.
- Update `README.md` when usage, setup, or behavior changes.
- Write guides for someone managing tasks in a terminal. Give exact keys,
  commands, inputs, and results. Verify runnable examples.
- Write specs for maintainers. One rule per requirement, with SHALL or MUST in
  the first paragraph and at least one concrete scenario. Preserve every case
  when rewriting specs for clarity.
- Use short sentences, sentence case headings, and one name per concept.
  Do not use long dashes or decorative bold labels.

## Implementation and checks

- Rust is the application language. Keep storage, task rules, and terminal
  rendering separate so command line automation can reuse the same behavior.
- Use SQLite transactions for writes. Keep task data out of this repository.
- Treat due dates as local calendar dates. Inject the current date into task
  grouping tests. Do not infer elapsed hours from a date.
- Keep UI strings in English and Danish catalogs. Stable CLI tokens and JSON
  fields stay in English.
- Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo test` after implementation. Run the relevant OpenSpec validation too.
- Exercise the CLI against a temporary profile and inspect the terminal UI.
  Report any platform or interaction that was not verified.
- Before reporting completion, delegate a review, address its findings, and
  rerun the checks affected by any fixes.
