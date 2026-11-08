## Why

Show each future task's actual due date in its heading and make dates and list membership quicker to enter.
Broad Soon and Next headings require users to inspect task rows to find a particular day.
The editor currently requires a complete ISO date and a typed list name.

## What Changes

- Replace both Soon and Next terminal headings with one heading per due date.
- Add actual dates to Today, Tomorrow, Done today, Cancelled today, and History's Today and Yesterday headings.
- Replace the editor's typed List field with a selector and a modal picker of ordinary lists.
- Show `YYYY-MM-DD` as a visual placeholder when Due date is empty.
- Accept `YYYYMMDD`, `YYMMDD`, and `MMDD` in the editor alongside strict `YYYY-MM-DD`.
- Normalize valid dates when leaving the Due date field. Preserve invalid text for correction and validate it on Save.
- Preserve existing due filters, CLI/MCP date syntax, stored task fields, and database schema version 4.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `planning-search`: Distinguish stable due buckets and ordering from terminal date headings.
- `terminal-interface`: Define dated headings, compact editor dates, an empty-date placeholder, and a draft list selector with its picker.
- `profiles-localization`: Define focused List and editor-list-picker modes using existing actions and localized calendar labels.

## Impact

Release 0.10.0 keeps database schema version 4. No migration is required.
Due dates remain local calendar dates. Status dates and completion timestamps retain their existing behavior.
The stable `due:soon`, `due:next`, and other filter values keep their meanings.
CLI, MCP, and query date input remain strict `YYYY-MM-DD`.
The List selector uses a stable list ID, so an external rename cannot redirect the draft to another list.
No installed executable or personal task data changes during development.

## Scope

No new planning property, recurrence, reminder, list-management action, status ordering, or typing mask is included.
The editor picker cannot create, rename, reorder, or delete lists.
