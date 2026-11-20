# Build and check Husker

This page is for contributors who need to build Husker and verify a change.

## Run the checks

Run these commands from the repository directory:

```text
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release --locked
openspec validate --all --strict
node --test web/logic.test.mjs
node --test web/app.test.mjs
```

The two `node --test` commands test the phone page. They need Node.js 20
or newer and no install step, because the tests use only modules that
ship with Node.js.

After a change to the phone page, also run the browser check. It drives
the phone page in headless Chrome against a local server and writes
numbered screenshots:

```text
cargo build --bin husker --example fake_blob
node web/browser-check.mjs
```

The browser check needs Node.js 22 or newer and Chrome or Edge. It is not
part of `cargo test`. See
[How the phone page is built](#how-the-phone-page-is-built) for its
arguments and for what it does not cover.

Before a deployment, also run the host check. It runs the Azure Functions
host on your computer with the `host.json` and `serve/function.json` that
`deploy.ps1` uploads, and `husker serve` behind the host:

```text
cargo build --bin husker --example fake_blob
node deploy/azure/check-local.mjs
```

The host check needs Node.js 22 or newer and Azure Functions Core Tools
4. It is not part of `cargo test` either. See
[How the online server is built](#how-the-online-server-is-built) for
its arguments and for what it cannot see.

The browser check and the host check each work in a directory of their
own, `target/browser-check` and `target/functions-check`. Each refuses a
directory that holds files of anything else. Neither passes a `HUSKER_*`
variable of your shell to the server that it starts, so a
`HUSKER_SYNC_URL` in your shell cannot point that server at your real
sync file. Ctrl+C stops the processes that a check started.

The test suite uses temporary profiles. It does not read or change your
`~/.husker` profile. Keep a separate profile for interactive checks too:

```text
cargo run -- --profile ./target/manual-profile
```

Run the installed copy while developing so the repository's executable stays
available for rebuilding:

```powershell
& "$env:USERPROFILE\.cargo\bin\husker.exe"
```

On Windows, `cargo run` and direct launches from `target/` lock that build
executable until the process exits. A rebuild then fails with
`Access is denied (os error 5)`.
The installed copy does not lock the repository's build executable.
Close installed Husker processes and disconnect MCP clients before replacing
the installed copy with `cargo install --path . --locked`.

Use only temporary profiles for migration tests. Version 0.15.0 uses schema
version 5. Versions 0.9.0 through 0.14.0 use schema version 4. Updating
between those versions requires no migration. Versions 0.4.0 through 0.8.0
use schema version 3.
Starting the current version against a schema version 1, 2, 3, or 4 profile
migrates it to version 5 on writable access. An older version cannot open
the upgraded profile. Read-only connections require the writable upgrade first.
Follow the [upgrade instructions](usage.md#update-an-existing-profile) for a real profile.

## Delete tasks permanently in the store

Three `Store` operations in `src/store.rs` remove task rows. Every other
task write keeps the row. That includes `set_deleted`, which moves a task
to or from Trash.

| Operation | What it does |
| --- | --- |
| `purge_task(id, version)` | Deletes one task in Trash if its stored version still equals `version`. Returns `TaskPurgeOutcome::Deleted`, `Changed(task)` with the current task, or `NotInTrash` when the task is outside Trash or no longer exists. |
| `preview_empty_trash()` | Returns a `TrashPreview` with the number of tasks in Trash and a snapshot of each task's ID and version. |
| `empty_trash(&preview)` | Deletes every task in Trash if Trash still matches the preview. Returns `EmptyTrashOutcome::Emptied(count)`, or `Changed(preview)` with a fresh preview. |

`purge_task` and `empty_trash` check and delete inside one immediate SQLite
write transaction. Another writer therefore cannot change the task or Trash
between the check and the delete. A `Changed` or `NotInTrash` result writes
nothing. Show the fresh task or preview to the user and ask for
confirmation again before you retry.

All three operations reject read-only and list-scoped stores before they
read Trash. A list-scoped store fails with
`A list-scoped store cannot delete tasks permanently.` A read-only store
fails with `This store is read-only.` Only the terminal interface calls
these operations. No CLI command or MCP tool calls them.

Task IDs come from an `AUTOINCREMENT` column, so a deleted ID is never
reused. Permanent deletion needed no schema change.

## How sync is built

Sync keeps the lists and tasks of several profiles the same through one
JSON file in Azure Blob Storage, the sync file. The
[sync guide](sync.md) describes the behavior for users. The `device-sync`
capability spec under `openspec/` holds the rules.

One sync is one call to `sync_once` in `src/sync.rs`. It runs these steps:

1. `Remote::download` fetches the sync file, unless the file still has the
   ETag stored at the last sync.
2. `Store::sync_merge` merges the download into the database in one
   immediate SQLite write transaction. It calls the pure `merge` function
   and stores the new sync state in the same transaction.
3. `Remote::upload` sends the merged snapshot if it differs from the
   download. The upload is conditional on the downloaded ETag.
4. `Store::sync_uploaded` records the uploaded snapshot as the new base.
   The base is the snapshot that the database and the sync file agreed on.

If another device replaced the sync file between steps 1 and 3, the upload
is refused and `sync_once` starts again from step 1. It gives up after five
rounds and reports the `Busy` failure kind.

Two cases need more than the base:

- An upload can reach the storage while its answer is lost. Step 2
  therefore stores the snapshot it is about to upload under the `sent` key,
  and every upload adds a random identifier to the `uploads` list in the
  sync file. When a later download lists that identifier, `sync_merge` uses
  the `sent` snapshot as the base. Without this, the device would treat its
  own upload as another device's change and undo edits made since.
- The sync file can be deleted and written again, or replaced by a file
  from another profile. The file then carries another `collection` than the
  base. `sync_merge` passes `rejoin = true` to `merge`, which then deletes
  nothing and keeps a conflict copy of every local text the file replaces.

| Module | What it holds |
| --- | --- |
| `src/sync.rs` | The `sync.toml` settings (`Settings`, `Auth`), the failure kinds, the `Report`, and `sync_once`. |
| `src/sync/snapshot.rs` | The JSON form of the sync file, its validation, and the `FORMAT` number. |
| `src/sync/merge.rs` | The three-way merge of the base, the local snapshot, and the downloaded snapshot. It is a pure function. It does no I/O and reads no clock. |
| `src/sync/remote.rs` | The `Remote` trait and `BlobRemote`, the HTTPS client for the blob. It also holds `MemoryRemote`, an in-memory sync file for unit tests. |
| `src/sync/worker.rs` | `SyncWorker`, the background thread, and its `Schedule`. `Cadence::TERMINAL` and `Cadence::MCP` hold the delays for the two kinds of process. |
| `src/store/sync.rs` | The store side: the snapshot export, the merge transaction, the sync state in the `sync_state` table, and the counters the terminal reads. |
| `tests/support/mod.rs` | `FakeBlob`, a local HTTP test double for the blob and the managed identity token endpoint. `tests/sync.rs` uses it. |

The worker thread opens its own database connection. It notices changes
from other connections through SQLite's `data_version`, so no store call
has to report its writes. The terminal reads the `commits` counter to tell
a sync commit from a commit by another process. A sync commit shows a sync
message. Any other external commit shows the external-update notice.

Follow these rules when you change sync:

- Test with `MemoryRemote` in unit tests and with `FakeBlob` in
  `tests/sync.rs`. `FakeBlob` listens on `127.0.0.1` and `tests/sync.rs`
  starts real `husker` processes against it. No test contacts a real
  storage account.
- Never print or log the sync URL. Its query string can hold a SAS token.
  `BlobRemote::scrub` removes the URL and the token from failure messages.
  The tests assert that neither appears in a failure.
- Give the sync file a new `format` number when you add a field to it.
  The snapshot types use `deny_unknown_fields`. An older Husker therefore
  rejects a file that has a field it does not know. With a new `format`
  number, the older Husker reports `The sync file needs a newer Husker.`
  Without one, it reports `The sync file is not valid.`
- Keep `merge` free of clocks and I/O. Every device must compute the same
  result from the same three snapshots.

## How the online server is built

The online server is `husker serve`. It answers MCP requests and the
requests of the phone page over HTTP, and it works on the sync file
directly. It keeps no database and is not a device: it never merges, and
it holds no base. The [online guide](online.md) describes the behavior
for users. The `online-server` and `online-mcp` capability specs under
`openspec/` hold the rules. [How the phone page is built](#how-the-phone-page-is-built)
covers the page and its JSON API.

One change is one call to `Engine::write` in `src/online/engine.rs`. It
runs these steps:

1. `fetch` downloads the sync file, unless the file still has the ETag of
   the cached download.
2. `apply` in `src/online/doc.rs` changes one list or task in a copy of the
   downloaded snapshot.
3. If the copy equals the download, `write` returns without an upload.
4. `write` serializes the copy, reads it back with `Snapshot::parse`, and
   calls `guard` to compare it with the download.
5. `Remote::upload` sends the copy with `If-Match` set to the downloaded
   ETag.

If a device replaced the sync file between steps 1 and 5, the upload is
refused and `write` starts again from step 1. It gives up after five
attempts and reports the `Busy` failure kind. If an upload gets no answer,
`write` downloads once more. A file that equals the upload counts as
success.

`apply` is a pure function. It does no I/O and reads no clock and no random
source. The MCP handler creates the `uid` of a new record and the `Stamp`,
the date and time of the request, once per tool call. A retry therefore
applies exactly the same change.

`guard` is the last check before an upload. It limits a mistake in `apply`
to one record. It refuses the upload unless all of these hold:

- `format`, `collection`, `default_list`, and `uploads` equal the download.
- Every list and task of the download is still there, with the same `id`
  and `position`.
- At most one list or task is new or changed.
- The request has write access, and the new or changed record is inside
  its list scope.

Two rules protect the devices:

- The online server never creates the sync file. A missing file is
  `EngineError::Missing`. Every upload is conditional on an existing ETag.
  A server with a wrong URL therefore cannot start a new `collection`.
- The online server never adds to `uploads`. A device finds its own upload
  in that list after a lost answer. The list holds 20 identifiers, so
  uploads by the server would push a device's identifier out.

| Module | What it holds |
| --- | --- |
| `src/online.rs` | `ServerSettings`, read from the environment variables, and `serve`, which listens on `127.0.0.1`. |
| `src/online/doc.rs` | The write side: `Access`, `Stamp`, `Op`, `apply`, `guard`, the content `version` of a task, and `resolve_list` for the list in a path. |
| `src/online/view.rs` | The read side. It gives the accessible records temporary integer IDs, calls the shared query and planning functions, and maps the results back to each `uid`. The numbers never leave the server. |
| `src/online/engine.rs` | `Engine`: the cached read, the write loop above, and one mutex that serializes the writes of a process. |
| `src/online/access.rs` | Access tokens: `Signer`, `Verifier`, the `Claims` in a token, the key file `online.toml` (`OnlineProfile`), and the address check `normalize_origin`. |
| `src/online/mcp.rs` | `OnlineServer`, the MCP handler for one request. It takes the tool names and descriptions from `available_tools` in `src/mcp.rs` and uses text IDs and versions. |
| `src/online/http.rs` | The routes: `/api/health`, `/mcp`, `/mcp/list/{name}`, the files of the phone page, and the `/api` routes of the phone page. It checks the `Origin` and `X-Forwarded-Host` headers of every request and reads no request body over 512 KiB. When `X-Forwarded-Host` holds several host names, in one header line or in several, the last one counts, because the proxy next to the server adds it. The 403 answer for another host names that host: `This address does not accept requests for the host 'NAME'.` A known path with a method that it does not accept gets the JSON error `method_not_allowed`. For MCP, it then checks the access token and the list in the path, builds one rmcp service per request, and keeps no session. |
| `src/rules.rs` | The title rule, the list name rule, and the status stamp rule. `Store` and `apply` both call them, so a change gives the same result and the same error sentence through every interface. |
| `deploy/azure/` | The deployment: `host.json` and `serve/function.json` for the Azure Functions custom handler, `main.bicep` for the function app, `code.bicep` for one code package, and `deploy.ps1`, which runs it all. `check-local.mjs` checks the two host files on your computer. |

The tests never contact a real storage account or a deployed server:

| Test or tool | What it does |
| --- | --- |
| `src/online/tests.rs` | Unit tests against hand-built snapshots and `MemoryRemote`, the in-memory sync file. They cover every operation, the guard, a randomized check that any operation changes at most one record, the views, the engine next to devices that sync through the same file, the settings, and the routes. The parity tests run each change through `Store` and through `apply` and compare the record and the error sentence. The file also holds the tests of the phone page. |
| `tests/online.rs` | Starts real `husker serve` processes against `FakeBlob`, the local test double in `tests/support/mod.rs`, next to a profile that syncs through the same file. An MCP client connects over HTTP in both lifecycles, with and without `initialize`. Two more tests run `husker pair` against such a server. |
| `examples/fake_blob.rs` | Runs `FakeBlob` until you stop it and prints the URL of its sync file. Use it for manual checks with `husker sync` and `husker serve`. The [online guide](online.md#try-the-online-server-on-your-own-computer) shows the steps. |
| `examples/package.rs` | Builds the code package for Azure Functions. It writes the zip itself, because the Linux executable must be stored with the Unix mode 0755 and zip tools on Windows store no mode. `deploy.ps1` calls it. |
| `deploy/azure/check-local.mjs` | Runs the Azure Functions host on your computer with the deployed host files and `husker serve` behind it. The next paragraphs describe it. |

`deploy/azure/check-local.mjs` checks what the Azure Functions host does
with a request before `husker serve` receives it. It is not part of
`cargo test`. Install Azure Functions Core Tools 4 with
`npm install -g azure-functions-core-tools@4`. Then build the two
executables that the script starts, and run the script:

```text
cargo build --bin husker --example fake_blob
node deploy/azure/check-local.mjs
```

The script runs two hosts in turn. Both use `host.json` and
`serve/function.json` from `deploy/azure/`, the files that `deploy.ps1`
uploads:

1. A host whose handler answers with the request that it received. This
   host shows what the Functions host forwards: the path, the query, the
   method, the body, the `Authorization`, `Cookie`, and `Origin` headers,
   and custom headers such as `X-Husker`. It also shows that the host
   adds `X-Forwarded-Host` with the address that the client used, passes
   `Set-Cookie` back, and hands the root path to the handler when
   `AzureWebJobsDisableHomepage` is set. `main.bicep` sets it.
2. A host with `husker serve` as its handler, against `fake_blob`. The
   script requests the page files, pairs with `husker pair`, reads and
   changes tasks through the cookie, signs out, sends MCP requests, and
   syncs the profile to find the change. At the end it checks that the
   output of the host holds no access token and no sync URL.

The script prints `ok` or `FAIL` for each check. At the end it prints the
number of passed checks and the work directory. It exits with one of
these codes:

| Exit code | Meaning |
| --- | --- |
| 0 | Every check passed. |
| 1 | A check failed, or the script could not finish. For example, a host gave no answer within two minutes. |
| 2 | The script cannot find an executable, or it refuses the work directory. |
| 130 | You pressed Ctrl+C. The script first stops the Functions host, the handler behind the host, and `fake_blob`. |

| Argument or variable | Default | Meaning |
| --- | --- | --- |
| First argument | `target/debug/husker`, with `.exe` on Windows | The `husker` executable. |
| Second argument | `target/debug/examples/fake_blob`, with `.exe` on Windows | The `fake_blob` executable. |
| Third argument | `target/functions-check` | The work directory. It must be new or empty, or hold the file `.husker-check` of an earlier run. The script removes only what an earlier run made there: the directories `profile`, `echo`, and `app`. It writes the output of the two hosts to `echo.log` and `app.log` there. |
| `FUNC` | On Windows, `func.exe` in the global npm directory under `%APPDATA%`. Elsewhere, `func` on the path. | The `func` executable of Core Tools. Set `FUNC` when it is installed elsewhere. |

The script marks its work directory with the file `.husker-check`. It
works in a directory that does not exist yet, in an empty directory, and
in a directory that holds this file. It refuses any other directory and
removes nothing there:

```text
<directory> holds files that this script did not make. Give it a new or an empty directory.
```

The script passes a cleaned environment to the Functions host and to the
`husker` commands that it runs. The cleaned environment holds no
variable whose name starts with `HUSKER_`, and no
`FUNCTIONS_CUSTOMHANDLER_PORT`. Core Tools lets a variable of the shell
win over `local.settings.json`. A `HUSKER_SYNC_URL` in your shell would
otherwise point the server behind the host at your real sync file. The
server therefore gets its settings only from the `local.settings.json`
that the script writes.

The host check needs Node.js 22 or newer. Nothing in it reaches Azure.
The sync storage is the local test double, and the profile is a temporary
directory in the work directory. The host check cannot see the Azure
front end before the host, the Linux build of `husker`, or the time that
a start takes in Azure.

Follow these rules when you change the online server:

- Change one list or task per upload. When you add an operation, extend
  `guard` and the randomized test with it.
- Keep `apply` free of clocks, random sources, and I/O. Pass new
  identifiers and the `Stamp` in.
- Never create the sync file from the online server, and never change
  `uploads`.
- Put a rule that the store and the online server share in `src/rules.rs`,
  and add a parity test for it. `i18n` translates the error sentences by
  their exact text, so keep the sentences as they are.
- Give the sync file a new `format` number when you add a field to it, and
  tell users to deploy the online server before they update their
  computers. An older server refuses a newer file and changes nothing.
- Never print or log an access token, the private key in `online.toml`,
  or the sync URL. A refused token always gets the same 401 answer, which
  does not say why.
- Use only `MemoryRemote`, `FakeBlob`, and temporary profiles in tests and
  manual checks.

## How the phone page is built

The phone page is the page that `husker serve` answers at `/`. It is plain
HTML, CSS, and JavaScript, with no build step and no third-party code.
The [online guide](online.md#use-the-phone-page) describes the behavior
for users. The `phone-page` capability spec under `openspec/` holds the
rules.

| File | What it holds |
| --- | --- |
| `web/index.html` | The elements of the page: the task screen, the pairing screen, the task panel (`#sheet`), the menu, and the message at the bottom (`#toast`). |
| `web/app.js` | The page script. It draws the views that the server sends, sends the changes, and keeps the state of the page. |
| `web/logic.js` | Pure functions that touch no document and no network: the phone's date, reading a pairing code, the language choice, the list filter, changing a row, counting the open tasks of each list, appending a page of rows, checking that a kept view or an answer has the shape of a view, and the ID of a new task. |
| `web/app.css` | The styles, in light and dark colors. |
| `web/sw.js` | The service worker. It keeps the page files of one release, so the page opens before the server answers. |
| `web/manifest.webmanifest` and the three PNG files | The name and the icons for the home screen of a phone. |
| `src/online/assets.rs` | `ASSETS`, the page files by request path. `WEB_KEYS`, the catalog keys of the page's text. `page_version`, the version that the service worker carries. The content security policy of `/`. |
| `src/online/api.rs` | The JSON API under `/api`: views, task changes, the session cookie, and pairing codes. |
| `src/main.rs` | The `husker pair` command, and `qr_lines`, which draws the QR code. |

`ASSETS` includes every page file with `include_bytes!`, so the files are
compiled into the `husker` executable. A change to a file in `web/`
reaches a phone only after a new build and a deployment. To add a file,
add it to `ASSETS`. Add it to `FILES` in `web/sw.js` too, so the page
has the file before the server answers. `FILES` names every page file
except the worker script itself: `/`, the styles, the two scripts,
`/strings.json`, the manifest, and all three icons. A test fails when
`FILES` names a path that the server does not serve. It also fails when
`FILES` misses `/`, the styles, a script, or `/strings.json`.

A release reaches a phone as a whole. `page_version` in
`src/online/assets.rs` hashes every page file, the page's text, and the
content security policy. The server writes that version into `/sw.js` in
place of the text `__PAGE_VERSION__`. A release that changes any of the
three therefore changes the worker script, and the browser installs the
new worker. The new worker fetches every file in `FILES` into a cache
named after the version before it replaces the old worker, so the files
of two releases never mix. If one file does not arrive, the installation
fails, and the old worker keeps answering with the files of the old
release.

When the worker of a new release takes over an open page, the page starts
again with the new files. `watchRelease` in `web/app.js` notices the new
worker. `restartWhenIdle` waits while one of these holds:

- The field for a new task or the field for a pairing code holds text.
- The task panel or the menu is open.
- A change or a new task is on its way to the server, or a pairing code
  is being checked.
- The message at the bottom shows its `Undo` button.

A new task that is marked as not sent does not hold the restart back,
because the phone keeps it. A page that becomes visible again asks the
browser to look for a new `/sw.js`.

The page holds no task rules. `GET /api/view` returns the ready groups of
All tasks, History, or Trash. `build_view` in `src/online/api.rs` calls
`visible_tasks`, `visible_history`, and `group_rows`, the functions that
the terminal uses, so the page and the terminal show the same headings,
counts, and order. The server groups the rows before it cuts a page of
History or Trash. The `count` of a group therefore covers the whole
group, also when a page holds only a part of it. Every view also carries
`session`: the ID, the name, and the expiry date of the access token.
The first line of the page's menu shows all three, in the catalog text
`web_paired_as`. The ID is the value that `-Revoked` of `deploy.ps1`
takes.

The only date logic in the page is `localDate` in `web/logic.js`. The
page sends that date as `today` with every request. The server uses
`today` for the groups, the planned date presets, and the status date. It
accepts `today` only within one day of its own UTC date.

The API checks the caller before it reads a request. The order is fixed:
the access token first, the `X-Husker` rule second, and the request body
last. A stranger therefore cannot make the server read a body. A body has
at most 512 KiB. The extractors `Caller`, `Writer`, and `FromPage` in
`src/online/api.rs` hold this order.

One request changes one task. `POST /api/tasks` adds a task under an ID
that the page created, so a repeated request after a lost answer adds
nothing. `PATCH /api/tasks/{id}` turns its members into up to three
operations and passes them to `Engine::write_all`, which uploads once. A
change to the title or the description must carry the `version` that the
page last read. A status, a planned date, a list, and Trash need no
`version`, so a tap on a checkbox works after a device changed the title.
The answer holds the row of the task and the view after the change. A
request with `view=none` gets the row alone. In All tasks, the checkbox
and the status buttons of the task panel use `view=none`, because their
row keeps its place. In History, they ask for the view, because a
reopened task leaves History.

A failed API request gets a status, a stable `code`, and a message in the
language of the request. Besides the codes for wrong input, the API
answers `method_not_allowed` (405) for a known path with a method that it
does not accept, `file_full` (507) when the sync file cannot hold the
change, and `server_error` (500) when the upload guard stopped a change
or the server failed in another way.

The task panel sends only what the user changed. A field counts as
changed when it holds another text than it showed when the panel took
the task. `fillSheet` in `web/app.js` puts a task into the panel and
keeps in `state.sheetShown` what the title, the description, and the
date field show for it. `draft` compares the three fields with
`state.sheetShown`, and not with the task. The reason is the browser. A
text area turns `\r\n` into `\n`, and a one-line field drops line
breaks. Tasks that agents wrote hold such text. Compared with the task,
an untouched panel of such a task would count as changed, and a tap on
`Today` would send the description back with other line breaks.
`shownAs` asks a field what it shows for a value and puts the user's
text back.

`Save` sends the changed fields. A button or the list picker sends its
action and the changed fields in one request. The request carries the
`version` only when the title or the description is among them. After a
`stale_version` answer, and after a request without an answer, `rebase`
requests the view again and calls `fillSheet` with the task of that view
and the fields that the user changed. Those fields keep their text. The
other fields, the status button, and `Cancel task` then show the task as
it is now. A request without an answer can have reached the server. The
view then holds the task with the user's change, so no field counts as
changed any more, and a `Save` with nothing left to send closes the
panel.

`endSheet` ends a panel whose task can no longer be saved. `rebase`
calls it with the message `web_task_left` when the view no longer holds
the task, for example because a computer moved the task to Trash.
`saveSheet` calls it with the message `web_task_missing` after a
`task_not_found` answer, so the panel does not show the server's
sentence for that code. `endSheet` hides the planned date buttons with
the date field, the list picker, and every button except `#sheet-close`.
It also clears `state.sheetRetry` and `state.sheetEditable`, so nothing
waits for `Save` and the fields stay disabled. The title and the
description stay in sight.

`state.sheetRetry` keeps the action of a button or of the list picker
whose request failed while the panel stayed open. The next `Save` sends
that action again with the changed fields. `saveSheet` sets
`state.sheetRetry` after every such failure: a `stale_version` answer,
no answer, and an error answer of the server. The exceptions are a 401,
which shows the pairing screen, and a panel that `endSheet` ends.
`sheetList` returns the list that the list picker of the panel shows:
the list of a move that waits in `state.sheetRetry`, or else the list of
the task. Another button, an accepted request, and opening a panel clear
`state.sheetRetry`. Two newer choices take a waiting action out of it.
When the user chooses the list that the task is in, the `change`
listener of the list picker removes a waiting move and sends nothing.
When the user changes the date field, `dropWaitingDate` removes a
waiting date button, so the next `Save` sends the day from the date
field. A date button that the user taps still wins over a date in the
date field.

While a request of the panel is under way, `state.saving` is set, and
`saveSheet` and `sheetChange` send nothing. `setSheetBusy` gives the form
`#sheet-form` the class `saving` and disables `Save`, the three fields,
and the list picker. Text typed during the request would not be part of
it. The styles dim every button of the panel except `#sheet-close` and
switch off its pointer events. The close button stays in reach, so a
slow connection cannot lock the panel. `markDraft` sets the text of the
close button. The text is `web_discard` while a field differs from the
task and no request is under way. During a request it is `web_close`,
because the changes are on their way. `saveSheet` also puts the task
into `state.pending`. Its row is dimmed, and no checkbox and no second
panel changes the task before the request has ended. A panel that opens
for another task meanwhile starts in the same waiting state, because
`openSheet` calls `setSheetBusy(state.saving)`. When the request fails
after the user closed the panel, the message at the bottom names the
task.

A new task stays on screen until the server has it. `sendTask` repeats
the request once when no answer comes. A task that the server did not
take stays in a row that is marked as not sent. A tap on that row sends
the task again. So does `resend`, which runs when a view arrives from the
server and when the server confirms a change. The task keeps its ID
through every attempt.

`sendTask` sorts the outcome of a request into these cases:

| Outcome | What `sendTask` does |
| --- | --- |
| An answer whose `task` has the ID of the request | The server has the task. `forget` removes the task from `state.adding` and from the phone, and the view of the answer shows. |
| The code `task_exists` | The server has the task from an earlier request whose answer was lost, and the task changed there since. `forget` removes the task, and the page requests the view. |
| A 401 | The pairing screen shows. It removes the waiting tasks. |
| A refusal: another status from 400 through 499, except 408 and 429, with the error object of the server | `forget` removes the task. The title returns to the field for a new task. When that field holds the next title already, the message names the refused title. |
| Anything else: no answer, a status from 500 on, 408, 429, an error without the error object of the server, or an answer without the task | The task stays in `state.adding` and on the phone, marked as not sent. An error answer shows its message. |

Only the server's own refusal removes a waiting task. A 502 from the
sync storage and a 503 from something in front of the server leave it
waiting.

The page also keeps these tasks on the phone. `keepUnsent` writes the
request body of every task that the server does not have yet to
`husker.unsent` in the browser's local storage. Two open pages of one
phone share that entry. `keepUnsent` therefore keeps the bodies that
another page wrote and replaces only its own. `unsentBodies` reads the
entry when the page starts. The tasks show as not sent until `resend`
sends them. `showPairing` removes the entry, so signing out and a 401
answer leave no unsent task on the phone.

Answers can arrive late, above all while the server starts. The page
numbers every request that returns a view in the order it sends them, and
it shows a view only when no view of a later request is on screen. It
also drops a view whose request was sent before the server confirmed a
change, because the server answers a read without waiting for a write.
In each doubtful case the page requests the view again. `state.sent`,
`state.shown`, and `state.floor` in `web/app.js` hold the three numbers.
`adopt` also ignores an answer that `drawable` in `web/logic.js` does not
accept as a view. The screen and the kept view then stay as they are.

A change that keeps its row asks for `view=none`, and `showRow` puts the
row of the answer into the view on screen. When another change was
confirmed since the request was sent, the two answers do not tell which
change the server saved last. `showRow` then requests the view again.

The page also counts its sessions in `state.epoch`. `newEpoch` begins a
new count. `showPairing` calls it, and so does a successful pairing, so
a sign-out, a 401 answer, and a new pairing each begin a new count.
`api` remembers the count that a request was sent under. When the count
has grown, `api` drops the answer, whatever it says, and its caller
never continues. Without the count, a refresh that was under way at a
sign-out could put the old tasks back on screen after the next pairing,
and a late 401 could end the new pairing. The requests that open and
close a session pass the option `across` and are exempt.
`newEpoch` also resets `state.loading`, `state.saving`, `state.pending`,
and the waiting state of the task panel, and it marks every new task
that was on its way as not sent. Nothing on the page waits for a dropped
answer.

A request of `api` gives up after 30 seconds, the constant `TIMEOUT`. A
stalled connection would otherwise hold a request, and a task panel that
waits for it, for minutes. A request that gave up counts as unanswered.
A browser without `AbortSignal.timeout` sends its requests without the
limit.

All text of the page comes from the catalog in `src/i18n.rs`.
`/strings.json` serves the keys in `WEB_KEYS` in English and Danish, and
`t("key")` in `web/app.js` looks a text up. Headings, date labels, and
error sentences arrive in the API answers, in the language of the `lang`
parameter. To add a text, add the key to `tr` in `src/i18n.rs` in both
languages, add the key to `WEB_KEYS`, and read it with `t`.

A pairing code hands an access token to a phone without showing the
token. `husker pair` signs an access token and sends it to
`POST /api/pairings` in the `Authorization` header. `Pairings` in
`src/online/api.rs` keeps the token in memory under a random code of 12
characters from an alphabet of 32, which gives 60 random bits. A code
works once and for 300 seconds. The server keeps at most 16 codes, and
one access token has one code at a time. The link from `husker pair`
carries the code after `#code=`, a part of the address that a browser
never sends to a server. The page exchanges the code for the session
cookie with `POST /api/session`. Codes are not shared between processes
and do not survive a restart. Pairing therefore relies on the
deployment's default of one instance.

`codeFromAddress` in `web/app.js` reads the code of a link and removes it
from the address. It runs when the page starts and when the address
changes in an open page. `useLink` then asks the server with
`GET /api/session` whether the phone is paired. A kept view cannot tell,
because a paired phone can be without one. The outcome decides what
`useLink` does:

| Outcome of `GET /api/session` | What `useLink` does |
| --- | --- |
| 200 | The phone is paired. The page asks the user before the code replaces the pairing. After a no, it requests the view and does not use the code. |
| 401 | The phone is not paired. `pair` sends the code. |
| Anything else, also no answer | The code is not used. A phone with a kept view keeps showing it. Another phone gets the pairing screen with the message `web_offline` and the code in the field. |

`pair` sends a code to `POST /api/session`. `state.pairing` is set while
a code is being checked. `useLink` and `pair` then start no second
check, so a second tap on `Pair` sends nothing. While the note of an
unfinished sign-out exists, `useLink` asks no question and `pair`
finishes the sign-out first. When the request of `pair` gets no answer
on a phone without a kept view, `pair` shows the pairing screen with the
message `web_offline` and puts the code into the field. A typed code
therefore stays in the field for the next try, as the code of a link
does.

The session cookie holds the access token. `POST /api/session` answers
with the cookie `husker_session`, whose value is the token. The cookie is
`HttpOnly`, `SameSite=Strict`, limited to `Path=/api`, and `Secure` when
the server address uses `https`. Its `Max-Age` is the time until the
token expires. `session` in `src/online/api.rs` reads the `Authorization`
header when a request has one, and the cookie otherwise. Every token rule
applies to both: the signature, the expiry, the revocation list, the list
scope, and write access. `DELETE /api/session` removes the cookie. It
does not revoke the token.

Signing out leaves no task on the phone and none on screen.
`showPairing` removes the kept view and the unsent new tasks from the
phone. It also clears the rows of the hidden task screen and empties the
field for a new task and the fields of the task panel. It clears three
more hidden places that can hold a task title or the names of the lists:
the text of the message at the bottom, the `aria-label` of the task
panel, and the entries of the three list pickers. It returns to All
tasks without a list filter, so the next pairing opens there.
The page then sends `DELETE /api/session`, because only the server can
remove the cookie. When that request gets no answer, the page keeps the
note `husker.signout` on the phone and requests no view. `finishSignOut`
sends the request again at the next start and before `pair` uses a
pairing code. One such request runs at a time, and a pairing code waits
for it. A late answer therefore cannot remove the cookie of a new
pairing.

`watchKeyboard` in `web/app.js` measures the on-screen keyboard and sets
the CSS variable `--keyboard`. The styles use it to keep the task panel,
the menu, and the message above the keyboard. The browser check sets the
variable itself and checks the styles. The measuring on a real phone is
not verified.

The row of buttons at the lower edge of the task panel is sticky. The
rule `#sheet-form > .actions` in `web/app.css` keeps the row in view
while the fields above it scroll, so `Save` stays in reach on a small
screen or with the keyboard open. No test covers this rule.

Every control has a name for assistive technology. A control with
visible text uses that text. `applyTexts` in `web/app.js` sets an
`aria-label` on the controls without text, `rowElement` names each
checkbox after its action, and `openSheet` names the task panel after its
task. The list filter and the list picker for a new task have names of
their own, the catalog texts `web_filter` and `web_add_list`. A test
fails when the two names are equal. The date field of the task panel
takes its name from the `Planned date` legend through `aria-labelledby`
in `web/index.html`. The messages carry `role="status"`. The browser
check reads Chrome's accessibility tree and fails when a control of the
task screen or of the task panel has no name. It runs no screen reader,
and it does not check the menu or the pairing screen this way.

Follow these rules when you change the phone page:

- Write task text into the page with `textContent`. Never use
  `innerHTML`. A test also refuses `outerHTML`, `insertAdjacentHTML`,
  `document.write`, `eval(`, and `new Function` in the scripts.
- Put no text into the page files. Take every text from `src/i18n.rs`
  through `/strings.json`. The name Husker and the pattern
  `XXXX-XXXX-XXXX` of a pairing code are the only exceptions. Write each
  key as a string literal in `web/app.js`, so the test that compares the
  keys with `WEB_KEYS` finds it.
- Keep `web/index.html` free of inline scripts, inline event handlers,
  and `style` attributes, and load nothing from another server. The
  content security policy `default-src 'self'` blocks them. A test also
  fails when `web/index.html` or one of the three scripts holds the text
  `http://` or `https://`, also in a comment.
- Let the service worker keep page files only. It must never keep or
  answer a request under `/api` or `/mcp`.
- Keep the text `__PAGE_VERSION__` exactly once in `web/sw.js`. The
  server replaces it with the page version, and a test fails when the
  count differs.
- Send the header `X-Husker: 1` with every request of the page. The
  server refuses a change through the cookie without that header, and a
  page of another website cannot send it.
- Keep the cookie away from MCP. The cookie is limited to `/api`, and the
  `/mcp` routes read only the `Authorization` header.
- Keep the order of the API checks: the access token, then the `X-Husker`
  rule, then the request body.
- Keep grouping, order, and date labels on the server. Do not add a
  second copy of a task rule to the page.
- Keep the page small. A test fails when the page files without the
  icons exceed 60 KiB together.

These tests and tools cover the phone page:

| Test or tool | What it does |
| --- | --- |
| `web/logic.test.mjs` | Tests the pure functions in `web/logic.js`: the phone's date around midnight, reading a pairing code from an address and from typed text, the language choice, the list filter with its new counts, finding and changing a row, counting the lists again, the shape of a view, appending a page, and the ID of a new task. |
| `web/app.test.mjs` | Loads `web/app.js` against a small stand-in for the document, the storage, and `fetch`. The tests hold answers back, refuse them, or drop them, to cover a server that starts slowly. No browser runs, so the tests do not apply the styles. The list below this table names what they cover. |
| `src/online/tests.rs` | The tests after the comment `The phone page` request every page file and `/strings.json`, check the version in the worker script, compare the keys in `web/app.js` with `WEB_KEYS`, and search the page files for forbidden calls. They compare each view with the terminal's groups. They cover adding and changing tasks, an answer with the row alone, every error code, the checks before the body is read, the cookie among other cookies, the counts of History across pages, the phone's date, pairing codes, the session cookie, and a read-only access. |
| `tests/online.rs` | Two tests run `husker pair` against a real `husker serve` process. They cover the link on standard output and the token ID on standard error, the code exchanged for a cookie, a change through the cookie that a laptop profile receives at its next sync, and `--json`. They also cover a server with another key, an address without a server, which is not stored, and an address that stays stored after a mistyped one. A last case is a server that answers with a redirect. `husker pair` reports the status and sends no second request, so the address in the redirect never gets the access token. |
| `src/main.rs` | A unit test draws the QR code with `qr_lines` and reads the drawing back with the `rqrr` crate, a test dependency. It also checks that the QR code and the two lines under it fit 24 rows and 80 columns for a link of up to 78 characters. |
| `web/browser-check.mjs` | Drives the page in headless Chrome against a local `husker serve`. The rest of this section describes it. |
| `deploy/azure/check-local.mjs` | Runs the page and the API behind the Azure Functions host on your computer. See [How the online server is built](#how-the-online-server-is-built). |

`web/app.test.mjs` covers these parts of the page script. Run
`node --test web/app.test.mjs` to see the name of each test:

- All tasks, the list filter, and the checkbox. A failed change with the
  message of the server, a change without an answer, and an error that
  is not the server's.
- A new task: an accepted task, a refused task whose title returns to
  the field or is named in the message, a task without an answer, a
  server that cannot save now, a task that the server has already, a
  task across a reload, the waiting tasks of a second open page, and a
  sign-out.
- The task panel: the buttons and the list picker, `Save` with only the
  changed fields, text that a field shows with other line breaks, a
  button with typed text, the fields and the row that wait during a
  request, the close button that reads `Close` meanwhile, the status
  buttons in All tasks and in History, a stale answer with the status
  button that follows the task, a failed button that the next `Save`
  sends again, the list that the list picker then shows, the list of
  the task and a day in the date field that replace a waiting button, a
  task that left the view or was deleted, a save whose answer was lost,
  a failure after the panel closed, and `Save` tapped twice.
- Pairing: a code in the address of a starting page and of an open page,
  a link on a paired phone with and without a kept view, a link without
  an answer from the server, a typed code without an answer, a refused
  code, typed text that holds no code, and a second tap on `Pair`.
- Sessions: signing out with and without a connection, the hidden
  places that a sign-out clears, a pairing code that waits for the
  sign-out, answers that belong to an earlier session, a late 401 for
  the pairing before, and All tasks after a new pairing.
- The kept view, a kept view that the page cannot draw, an answer that
  is not a view, a read-only session, the loading line, and a server
  that cannot be reached.
- The order of late answers: a refresh during a change, an older answer,
  a view that was read before a confirmed change, two overlapping
  changes with a view and with the row alone, and an answer for a view
  that the user left.
- The rows of History, the list chosen for a new task, a row with a
  change under way, a new release, and the keyboard.

One test checks that a request of the page carries a time limit. No test
waits for the 30 seconds.

`web/browser-check.mjs` checks the phone page in a real browser on your
computer. It is not part of `cargo test`. Build the two executables that
the script starts, then run the script:

```text
cargo build --bin husker --example fake_blob
node web/browser-check.mjs
```

The script starts `fake_blob`, the test double of the sync storage. It
fills a temporary profile through `husker` commands, starts
`husker serve` on a free port, and runs `husker pair`. Then it drives the
phone page in headless Chrome through the DevTools protocol, on a screen
of 390 by 844 pixels. Nothing leaves the computer, and the only profile
is a temporary directory under the output directory. The script passes a
cleaned environment to the `husker` commands and to `husker serve`: no
variable whose name starts with `HUSKER_`, and no
`FUNCTIONS_CUSTOMHANDLER_PORT`. The settings of the server come from the
script alone.

The browser check goes through ten steps:

1. The pairing screen without a session, and the message for a wrong
   pairing code.
2. Pairing by a link in a tab that already shows the page, the attributes
   of the cookie, the content security policy, the names of the controls
   of the task screen in Chrome's accessibility tree, and the headings of
   All tasks.
3. Adding a task with the Enter key and with the `+` button.
4. Closing a task with one tap, the row that keeps its place and its
   focus, and reopening the task.
5. The task panel: the names of its controls in the accessibility tree,
   the buttons of a waiting panel, its place above a keyboard of 300
   pixels, the planned date buttons, the date field with `Save`, the
   close button that reads `Close` or `Discard changes`, a button that
   sends typed text with its action, the list, the title and the
   description, the status buttons, and Trash with Undo. For the waiting
   panel, the check sets the class `saving` itself. It reads that `Save`
   and `Move to Trash` take no pointer events and that the close button
   does.
6. The line about the pairing in the menu, a task that leaves History
   after `Reopen task` in its panel, and `Restore` in Trash.
7. Danish, the list filter, and the dark colors. Then a pairing link on
   the paired page, which asks before it replaces the pairing.
8. The files that the service worker keeps, in one cache named after the
   page version. Then the kept view while the server is stopped, a change
   that is rolled back, and a task that is added while the server is
   stopped and sent when the server returns.
9. Signing out, the sign-out that the server confirms, the 401 that the
   browser gets afterwards, and pairing a read-only session with a typed
   pairing code.
10. `husker sync` and `husker tasks` on the profile, which must show the
    changes from the page.

The script prints each passed check and writes numbered screenshots, such
as `01-pairing.png`, into the output directory. It ends with
`ALL CHECKS PASSED` and exit code 0. After a failed check, it prints the
check, writes a screenshot named `failure`, and exits with code 1. It
also fails when the page logs a script error. It exits with one of these
codes:

| Exit code | Meaning |
| --- | --- |
| 0 | Every check passed. |
| 1 | A check failed. Or the script did not finish within five minutes: it then prints `The check did not finish within five minutes.` and stops the browser, the server, and `fake_blob`. |
| 2 | The script cannot find `husker`, `fake_blob`, or Chrome, or it refuses the output directory. |
| 130 | You pressed Ctrl+C. The script first stops the browser, the server, and `fake_blob`. |

| Argument or variable | Default | Meaning |
| --- | --- | --- |
| First argument | `target/debug/husker`, with `.exe` on Windows | The `husker` executable. |
| Second argument | `target/debug/examples/fake_blob`, with `.exe` on Windows | The `fake_blob` executable. |
| Third argument | `target/browser-check` | The output directory. It must be new or empty, or hold the file `.husker-check` of an earlier run. The script removes only what an earlier run made there: the directories `profile` and `chrome`, and its numbered PNG files. It retries the removal of the two directories, because the browser of an earlier run can hold its files for a moment after it closed. Then it writes the screenshots, the temporary profile, and the profile of Chrome into it. |
| `CHROME` | Chrome or Edge in a standard install path | The path of the Chrome or Edge executable. Set `CHROME` when the browser is installed elsewhere. |

The script marks its output directory with the file `.husker-check`, as
the host check marks its work directory. It works in a directory that
does not exist yet, in an empty directory, and in a directory that holds
this file. It refuses any other directory and removes nothing there:

```text
<directory> holds files that this script did not make. Give it a new or an empty directory.
```

The browser check needs Node.js 22 or newer and Chrome or Edge. On
Windows, keep the path of the output directory short. Chrome cannot store
a service worker under a profile path longer than about 200 characters.

The browser check does not cover Safari on an iPhone, a real phone, or
Azure. It runs desktop Chrome with a phone-sized screen, taps with mouse
events, and sets the value of a picker through script. Touch input, the
on-screen keyboard, the pickers of a phone, the camera with the QR code,
and the home screen are therefore untested. So is a screen reader: the
check reads the names of the controls and runs none.

The browser check also leaves these cases to `web/app.test.mjs`, which
runs no browser:

- A tap on a checkbox in History.
- An unsent task across a reload, and a new task that the server answers
  with an error.
- A request of the task panel that fails: a stale answer, no answer, the
  button that the next `Save` sends again, a newer choice that replaces
  that button, and a task that left the view or was deleted.
- A request of the task panel that takes long, with its waiting fields,
  its waiting row, and its close button that reads `Close`.
- A pairing link or a typed pairing code without an answer from the
  server, and a pairing link on a paired phone without a kept view.
- Answers of an earlier session, and a sign-out without a connection.

The browser check reads the pointer events of the buttons of a waiting
panel, because it sets the class `saving` itself. No check holds a
request of the panel back in a browser. The dimmed buttons and the
disabled fields of a real waiting panel are therefore unseen in a
browser. No check scrolls the task panel to see the sticky row of
buttons.

The script never contacts Azure. The host check in
[How the online server is built](#how-the-online-server-is-built) covers
the Azure Functions host on your computer. The Azure front end before
the host and a cold start in Azure stay untested.

## Checks for version 0.22.0

The change that makes Last and the Up wrap select the last open task completed these checks on November 20, 2026, on macOS 27.2.1 with Rust 1.100.0:

| Check | Result |
| --- | --- |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Unit tests | 402 passed. Two new UI tests cover Right, End, and `G` with tasks under Done today and Cancelled today, Up on the first task under Earlier, Down from the last open task, a held row after it, a list view whose first task is its only open task, the filter `s:done`, History, Trash, a list view without Earlier, and a list view where only tasks under Earlier are open. |
| Mutation checks | Each of three code paths was undone in turn: Last, the Up wrap, and the view check that keeps History and Trash on their last row. The first new test failed each time. |
| Integration tests | CLI 9, MCP 8, online 9, sync 8, terminal keys 1, and package 2 passed. |
| Pseudo-terminal run | The 0.22.0 release executable ran in a Python pseudo-terminal, 100 columns by 30 rows, against a temporary profile with two tasks under Earlier, one each under Today, Tomorrow, and Sometime, one under Done today, and one under Cancelled today. Up from the first Earlier task selected `Someday` under Sometime. Down then reached the done and cancelled tasks and wrapped to `Pay rent`. Right, End, and `G` selected `Someday`. After Space on `Someday`, the held row was skipped, and End and the Up wrap selected `Call the bank`. Help showed `End / G / Right` with `Select last item, skipping done and cancelled`. |
| Review | An independent code review found no bug. It noted that Home lands below End when only tasks under Earlier are open and tasks are closed today, that the spec contradicted itself for a view with no open task, and two test gaps. The design now records the first case, the spec was fixed, and a new test covers the gaps. The docs review fixed two spec sentences and one scenario. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.22.0. |
| OpenSpec | `openspec validate jump-to-last-open-task --strict` passed. |

Not verified:

- Windows and Linux.
- The Help label in History, which names the skip although History holds only closed tasks.

## Checks for version 0.21.1

The change that selects today's first task when no task is known completed these checks on November 19, 2026, on Windows 11 with Rust 1.91.1:

| Check | Result |
| --- | --- |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Unit tests | 400 passed. A new UI test covers startup, opening a list, each typed filter letter, Enter with and without an edit, clearing a filter with a visible task, with no result, and with a task that is gone, returning to a saved task that is visible and one that is gone, a refresh that keeps the row position of a gone task, Esc in the filter input of an empty view that receives tasks, a refresh that keeps a task, a deleted current list, and Enter that finds the current list deleted. The planned bookmark test now expects the start task when the saved task is gone. |
| Mutation checks | Each of the eight code paths was undone in turn: the start rule in `refresh`, the kept row position in `refresh`, the deleted-list case, a typed filter, Enter, Enter after a deleted list, clearing a filter, and Esc in the filter input without an earlier task. The new test failed each time. |
| Integration tests | CLI 9, MCP 8, online 9, sync 8, and package 2 passed. |
| Pseudo-terminal run | The 0.21.1 release executable ran in an isolated psmux server, 100 columns by 30 rows, against a temporary profile with three tasks under Earlier. Startup selected `Pay rent` under Today, with the Earlier tasks shown above it. Typing `work` letter by letter and Enter selected `Work soon`, and Esc kept it. Opening the Work list selected `Work soon`, and Esc returned to All tasks with `Pay rent`. A filter with no result, cleared with Esc, selected `Pay rent`. |
| Review | An independent code review found no significant bug. It found that no test kept the row position of a gone task, that Enter after a deleted list replaced the saved task of All tasks, that Esc in the filter input and a language change still fell back to row 0, and that the saved row index was unused. All were fixed, with tests for the first three. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.21.1. |
| OpenSpec | `openspec validate select-today-by-default --strict` passed. |

Not verified:

- Linux and macOS.

## Checks for version 0.21.0

The change that makes Left and the wrap of Down skip Earlier completed these checks on November 19, 2026, on Windows 11 with Rust 1.91.1:

| Check | Result |
| --- | --- |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Unit tests | 399 passed. A new UI test covers Left, Home, and `g` with tasks under Earlier, Down on the last task, Up from the start task, a list view with nothing planned today, a start task that is also the last task, a filter that leaves only Earlier, a held row under Earlier, `s:all` with a Done today row after Earlier, the Help label, History, and Trash. The existing wrap test now expects the start task in All tasks and the Work list. |
| Mutation check | With the start row forced to the first row, the new test and the existing wrap test failed. |
| Integration tests | CLI 9, MCP 8, online 9, sync 8, and package 2 passed. |
| Pseudo-terminal run | The 0.21.0 release executable ran in an isolated psmux server, 100 columns by 30 rows, against a temporary profile with two tasks under Earlier and one each under Today, Tomorrow, and Sometime. Startup selected the first Earlier task. Left, Home, `g`, and Down on the last task selected `Pay rent` under Today. Up from it reached both Earlier tasks and then wrapped to the last task. PageUp reached the first Earlier task. With `due:overdue`, Down and Left selected the first task. A task just marked done under Earlier stayed held, and Left selected `Pay rent`. Help showed `Home / g / Left` with `Select first item, skipping Earlier`. |
| Review | An independent code review found no significant issue. Its notes led to a guard against a date change at midnight in the new test, a Trash case that the All tasks rule would fail, and the `s:all` case. A docs review added the filter rule to the spec. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.21.0. |
| OpenSpec | `openspec validate jump-to-today --strict` passed. |

Not verified:

- Linux and macOS.

## Checks for version 0.20.1

The Danish day-name phrase change completed these checks on November 18, 2026, on Windows 11 with Rust 1.91.1:

| Check | Result |
| --- | --- |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Unit tests | 398 passed. A new query test covers `i mo`, `I  MO`, `i m`, `i morgen`, `i d`, `i` alone, `i i mo`, words after a phrase, quoted words, `mo i`, `buy milk`, a field term between two words, `i g` in All tasks and History, `i mo` in History, Trash in both languages, English, and queries without day names. A second test runs a pasted filter of 2,000 words. A UI test types `i mo` with Danish selected. |
| Mutation checks | Each part was removed in turn, and at least one test failed each time: the joining, the phrase match from a word start, the literal check, and the position check. With the old cubic search restored, the long-filter test had not finished after 120 seconds. |
| Integration tests | CLI 9, MCP 8, online 9, sync 8, and package 2 passed. |
| Pseudo-terminal run | The 0.20.1 release executable ran in an isolated psmux server, 100 columns by 30 rows, against a temporary profile with Dansk chosen through F3. All 7 checks passed. `i` listed the tasks under I dag and I morgen. `i mo` left only `Pay rent` under I morgen and hid `Configure model source`, which is under I dag and contains `mo`. `buy milk` found `Buy oat milk`. |
| Review | An independent code review and a docs review found three issues, all fixed with tests: a field term between two words did not keep them apart, every view checked all five names, so Trash and History joined words for headings they never show, and the run search was cubic, so a pasted filter of 1,000 words took 1.9 seconds per rebuild. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.20.1. |
| OpenSpec | `openspec validate match-day-name-phrases --strict` passed. |

Not verified:

- Linux and macOS.

## Checks for version 0.20.0

The filter day-name and picker border change completed these checks on November 18, 2026, on Windows 11 with Rust 1.91.1:

| Check | Result |
| --- | --- |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Unit tests | 395 passed. A new query test covers `to`, `tod`, `ear`, `some`, `soon`, `next`, `day`, two terms, quoted terms, the Danish names, exact text for callers without day names, a closed task planned tomorrow, History by closing day in English and Danish, and Trash. New UI tests cover typing `to`, `tod`, and `tom`, a held row under `tom`, a quoted `"to"`, and a language change that rebuilds an applied filter and keeps the selected task. The picker tests check that the hint appears only in the footer and that the popup's bottom border row holds no text. |
| Mutation checks | Each part was removed in turn, and at least one test failed each time: the day names in `App::rebuild_visible`, the rebuild in `App::set_language`, the kept selection after it, the literal flag of quoted terms, the History and Trash rules, and the empty picker border. |
| Integration tests | CLI 9, MCP 8, online 9, sync 8, and package 2 passed. |
| Pseudo-terminal run | The 0.20.0 release executable ran in an isolated psmux server, 100 columns by 30 rows, against a temporary profile. All 21 checks passed. `to` listed today's, tomorrow's, and the toaster task, and `tod` left only today's task. A task completed under `tom` stayed under Tomorrow. The list picker's border was empty and the footer started with `Type to filter`; `hu` showed `Filter: hu`. In History, `tod` listed the task closed today and `tom` listed nothing. `dag` listed nothing in English and today's task after choosing Dansk with F3. `husker tasks to` listed only the toaster task. |
| Review | An independent review found no code defect. It found that History matched the planned date while its headings show the closing day, so History now matches Today and Yesterday from the closing day and Trash matches no names. The spec now says wholly quoted, and the tests gained the History, Trash, and border-row checks. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.20.0. |
| OpenSpec | `openspec validate match-date-words-in-filter --strict` passed, and `openspec validate --specs --strict` passed all 10 capabilities. |

Not verified:

- The editor's List picker and the move-task picker in a live terminal. Unit tests cover their shared border drawing.
- Linux and macOS.

## Checks for version 0.19.0

The filter, refresh, and group-color change completed these checks on November 18, 2026, on Windows 11 with Rust 1.91.1:

| Check | Result |
| --- | --- |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Unit tests | 392 passed. New tests cover `r` refreshing in All tasks and restoring in Trash, the external-update notice naming `r / F5` in All tasks and only F5 in the task editor, the filter narrowing rows while typing, a red border and kept rows for an unclosed quote, Esc restoring the earlier filter, selected task, and held rows, and a refresh that leaves a deleted list while the filter input is open. Others cover the group styles read from the rendered buffer, a held row under the Today heading, typing in the list picker, the editor list picker, and the move-task picker, Danish names, a click on a filtered row, reordering blocked while filtering, a stale scroll offset after typing, and the filter surviving Help, the language picker, and an external refresh. The config tests cover the Trash mode, printable keys in the pickers, and the `r` conflict. 25 older tests changed because letters now type in the pickers. |
| Mutation checks | Each review fix was removed in turn. The tests failed without the held-row restore, without the query taken from the applied filter text, and without the scroll reset. Clearing the saved filter state when a refresh leaves a deleted list is defensive: no test fails without it, because the query is already taken from the applied filter text. |
| Integration tests | CLI 9, MCP 8, online 9, sync 8, and package 2 passed. |
| Pseudo-terminal run | The 0.19.0 release executable ran in a Windows pseudo-terminal of 100 columns and 30 rows with a temporary profile. All 19 checks passed. The Today heading was black on yellow and bold, and its row bold. Earlier and the done row were dark gray, Done today green, and Tomorrow yellow. After `husker add` from another process, the notice named `r / F5`, and `r` refreshed. Typing `mi` after `/` narrowed the rows before Enter, an unclosed quote kept them, and Esc brought back every row. In the list picker the footer showed `Ctrl+N New list` and `F1 Help`, `hu` left only Husker, and Enter opened it. The move-task picker filtered to Shopping, and Esc cleared the filter. `tra` and Enter opened Trash, where `r` restored the task. |
| Review | An independent review found no high defect. It found three real defects, all fixed with tests: typing in a scrolled picker kept the old scroll offset and hid matches above the selection, Esc in the filter input after a refresh left a deleted list could keep a filter that the screen did not show, and Esc did not bring back held rows. Smaller points were fixed too: the spec now says that Backspace to an empty picker filter keeps the selection, the Help label for typed text covers all characters, and the guide explains that Ctrl+Alt keys count as printable. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.19.0. |
| OpenSpec | `openspec validate improve-filtering-and-highlights --strict` passed, and `openspec validate --specs --strict` passed all 10 capabilities. |

Not verified:

- The colors in a real terminal window. The pseudo-terminal run read the
  colors that Husker sends, and a headless browser drew them with the
  Windows Terminal palette.
- Mouse input in the pseudo-terminal.
- Linux and macOS, including Terminal.app, where `K` and `J` no longer
  reorder lists in the picker.

## Checks for version 0.18.2

The Option+arrow fix completed these checks on November 17, 2026, on macOS 27.2.1 with Rust 1.100.0:

| Check | Result |
| --- | --- |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Unit tests | 384 passed. 13 new tests in `src/input.rs` cover the join with scripted events that arrive after scripted delays. The four arrows join after `[` and after `O`. An arrow joins when it follows the Esc at once, after 5 milliseconds, and after 10. An arrow after 11 milliseconds stays apart, and an Esc alone waits exactly 10 milliseconds. A plain arrow key in two reads does not join, also when only its letter comes later, and 21 other inputs after Esc stay in order. A failed look-ahead loses no key. The joining tests fail when `Input::read` skips either join, and the timing tests fail when the wait is 50 milliseconds or also applies to the letter. |
| Terminal keys test | `tests/terminal_keys.rs` passed in 20 of 20 runs after the last change. It puts a pseudo-terminal in the place of standard input, writes the bytes of 17 keys in one write or in two, and compares the keys that `Input` reads through crossterm. Without the join it fails, because `ESC ESC [ A` arrives as Esc. When the look-ahead reads without polling, the test fails after 30 seconds instead of blocking forever. |
| Integration tests | CLI 9, MCP 8, online 9, sync 8, and package 2 passed in 8 of 9 full runs: 3 before and 5 after the review fixes. The first full run stopped in `tests/online.rs`, where `serve_ignores_the_profile_and_uses_the_handler_port` failed with `the online server did not start`. The same test passed in the 11 runs after that. The online server does not use the changed code. |
| Pseudo-terminal run | Versions 0.18.1 and 0.18.2 ran in a pseudo-terminal of 100 columns and 30 rows with temporary profiles. On 0.18.1, `ESC ESC [ A` and `ESC ESC [ B` moved nothing, cleared a filter, opened `Discard unsaved changes?` in a changed editor, and closed the list picker. On 0.18.2 the same bytes moved the task, also in a filtered view, moved the list in the picker, and left the editor open. `ESC ESC O A`, a held key, and `ESC [ 1 ; 3 A` moved the task too. An Esc alone still cleared the filter. An Esc followed later by `[` and `A` or by a plain Up moved nothing. |
| Two writes | On 0.18.2, the Esc byte and the arrow were sent in two writes. Each pause between the writes was tried with 20 presses. With 0, 0.2, 1, and 5 milliseconds, all 20 moved the task. With 30 and 100 milliseconds, none of the 20 did, as for an Esc and an arrow that a person presses in a row. |
| Screen and tmux | On 0.18.2 inside GNU screen 4.00.03 and tmux 3.1c with default settings, Esc and Up 50 milliseconds apart moved the task in 5 of 5 runs each. Both programs hold the Esc and deliver it together with the arrow. With `maptimeout 10` in screen and `set -s escape-time 10` in tmux, none of 5 runs moved it, and `ESC ESC [ A` still moved it in 5 of 5. |
| Terminal.app settings | Read from the preferences of Terminal.app 2.15 (488.9): the profile had Use Option as Meta set to Both Option keys and no key mapping for Option+Up or Option+Down. |
| Key press | The user pressed Option+Up and Option+Down in Terminal.app 2.15 with those settings and the installed 0.18.2. The selected task moved. |
| Review | An independent review found no high defect and one medium defect. The first version joined only keys that were ready together. It missed the key when crossterm read the Esc byte in a read of its own. That happened in 9 of 600 pairs of writes without a pause and in 300 of 300 with 0.2 milliseconds between them. The 10 millisecond wait for a plain arrow fixes it. Four smaller findings were fixed too: a failed look-ahead lost the Esc, 5 of 20 code mutations passed the tests, the terminal keys test could block forever, and one sentence about undo did not hold for lists. A second pass of the same review checked the wait. It found that nothing is lost, repeated, moved, or blocked in 299,593 sequences, 1,628,718 splits, and 200,000 runs with random arrival. It found one more medium point: inside screen and tmux, an Esc and an arrow pressed in a row count as Alt+arrow. That is now in the design and the usage guide. Its smaller points were fixed too: two more code mutations passed the tests, and two sentences claimed more than was measured. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.18.2. |
| OpenSpec | `openspec validate fix-option-arrows --strict` passed. |

Not verified:

- The bytes that Terminal.app sends for Option+Up and Option+Down, and
  whether it sends them in one write or in two. The agent session could
  not press keys, and the bytes of the user's key press were not captured.
  Husker reads both forms.
- Terminal.app with Use Option as Meta set to None, and the two key
  mappings of the usage guide in Terminal.app 2.15.
- iTerm2 and other terminals that send Alt+arrow with an Esc prefix.
- Windows and Linux. Only the macOS target was built.

## Checks for version 0.18.1

The sync fix completed these checks on November 16, 2026, on Windows 11 with Rust 1.91.1:

| Check | Result |
| --- | --- |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Unit tests | 371 passed. Five new tests cover the fix. A conflict copy follows its original for 32 task identifiers, half of which sort after the copy's identifier. A copy follows an original that the other device moved, and no task the other device added at the same position comes between them. A worker sends a task committed before it started, reports a failed start check through a sync, and makes no request at start when nothing is unsent. Each of the five fails without its fix, except the last, which guards against a sync at start. |
| Integration tests | CLI 9, MCP 8, online 9, and package 2 passed. |
| Sync integration tests | `cargo test --test sync` passed all 8 tests in 40 of 40 runs: 20 before and 20 after the review fixes. On 0.18.0 it failed in 4 of 6 runs. `two_profiles_show_the_same_tasks_after_husker_sync` now expects `Received 2, sent 1` and checks that the copy sits directly after task 3. |
| Cause of the timeout | Temporary logging in the worker showed that in each failed run the worker's first data version already held the agent's task, so no change was ever seen. In each passing run the data version moved once and the sync followed 2 seconds later. |
| Review | An independent review found one medium defect: a conflict copy kept the local position, so it could land before an original that the other device moved. It also found that a task at the same position could come between the original and its copy, that the spec did not name a failed start check, and that one error branch had no test. All four were fixed with tests and spec text, and the checks above were run again. |
| OpenSpec | `openspec validate fix-flaky-sync --strict` passed. `openspec validate --specs --strict` passed all 10 capabilities after `add-phone-page` was archived. |

Not verified:

- A real MCP client, such as Claude Code, against a writable server with
  sync. The integration test drives `husker mcp` through the rmcp client.
- Linux and macOS.

## Checks for version 0.18.0

The phone page update completed these checks on November 16, 2026, on Windows 11 with Rust 1.91.1, Node 24.11.1, Chrome 157, and Azure Functions Core Tools 4.16.0:

| Check | Result |
| --- | --- |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Unit tests | 366 passed. 73 of them cover the online server. The new ones cover the views against the terminal's headings, each write and each error code of the API, the order of the checks on a request, the body limit, the cookie among other cookies, `view=none`, the counts of 150 tasks closed on one day across pages, pairing codes, the session cookie, the date rule at one and two days from the server's date, the last name in `X-Forwarded-Host`, and the page files with the page version in `/sw.js`. One more test, in `src/main.rs`, draws the QR code and reads it back with a QR decoder. |
| Online integration tests | 9 passed. Two of them run `husker pair` against a real `husker serve` process: the link, `--json`, the token ID on stderr, a server with another key, an address where nothing answers, an address that stays stored after a mistyped one, a server that redirects, and a full pairing down to a task that a computer receives. |
| Package, CLI, and MCP tests | 2, 9, and 8 passed. |
| Sync integration tests | `two_profiles_show_the_same_tasks_after_husker_sync` and `an_agent_on_a_new_machine_reads_and_writes_a_synced_list` failed in 6 of 8 runs on the final code. The first printed `sent 1` where the test expects `sent 2`, and the second timed out waiting for the agent's upload. On the unchanged 0.17.0 source the same two tests failed the same way in 7 of 8 runs, so the failures don't come from this change. This change doesn't touch sync. The other 6 tests passed in every run. |
| Page tests | `node --test web/logic.test.mjs` passed 8 tests. `node --test web/app.test.mjs` passed 54 tests of the page script against a stand-in document, with answers that the test holds back, refuses, or drops. |
| Broken guards | A script removed one guard at a time from a copy of `web/app.js`, 76 guards in all, and ran the page tests. 75 removals made a test fail. The last guard has a second one behind it, and the page still behaved correctly. |
| Browser check | `node web/browser-check.mjs` passed 65 checks in headless Chrome at 390 by 844 pixels against `husker serve` and the local test double: pairing by link and by typed code, the cookie attributes, the question before a link replaces a pairing, adding with Enter and with the button, a task added while the server was stopped, closing and reopening, every action of a task's panel, Trash with Undo, History, Trash, Danish, the list filter, light and dark colors, the saved view with the server stopped, the names of the controls in Chrome's accessibility tree, signing out, a read-only pairing, and `husker sync` on the computer. The screenshots of the steps were checked by eye. |
| Page update | Two builds that differ in one page file ran one after the other on one address. Chrome opened the page with the files of the first build and started it again by itself 3.0 seconds later with the files of the second, all from one page version. |
| Functions host | `node deploy/azure/check-local.mjs` passed 38 checks. The Azure Functions host of Core Tools ran `husker serve` as its custom handler with the deployed `host.json` and `function.json`. The host passed the path, the query, the method, the body, `Authorization`, `Cookie`, `Origin`, and custom headers on unchanged, added `X-Forwarded-Host` with the address the client used, and passed `Set-Cookie` back. The root path reached the server. `husker serve` answered behind the host after about 4 seconds. |
| Review | Two independent code and security reviews. Neither found a hole in the server. The first found five medium defects: a second Save after a refused one overwrote text of another computer, a title that was not saved disappeared while the next one was typed, a failed save was reported in a closed panel, one test failed whenever the local date differs from the UTC date, and the browser check deleted its output directory. The second found one high and three medium defects in the page: a field that the browser shows with other line breaks counted as changed, a server error removed a waiting new task, answers from before a sign-out counted for the next pairing, and text typed while a save ran was dropped. All were fixed with tests, and so were the smaller findings of both reviews. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.18.0. |
| Linux build | `cargo zigbuild --release --locked --target x86_64-unknown-linux-musl` produced a statically linked executable of 14.5 MB. Under WSL 2 it reported version 0.18.0, added and listed a task in a temporary profile, and printed a public key. |
| OpenSpec | `openspec validate add-phone-page --strict` and `openspec validate add-online-server --strict` passed. |

### Deployment of version 0.18.0

Version 0.18.0 was deployed to Azure Functions on the Flex Consumption plan on November 16, 2026. `deploy/azure/deploy.ps1` ran twice: first with `-CreatePolicy` and read-only, then with `-AllowWrites`. Both runs passed their health check. The checks below ran from a computer with Node's `fetch` and with the installed `husker` 0.18.0 on the same sync file:

| Check | Result |
| --- | --- |
| Health | `/api/health` answered `{"format":1,"name":"husker","version":"0.18.0"}`. |
| Page files | `/`, `/app.js`, `/sw.js`, `/strings.json`, and `/manifest.webmanifest` answered 200 with their content types. `/` carried the content security policy, `nosniff`, `no-referrer`, and `no-cache`. |
| Refusals | Without an access token, `GET /api/view`, `POST /api/tasks`, `GET /mcp`, and `POST /mcp` answered 401. A forged access token got 401 too. |
| Read-only server | The All tasks view of a read-only access token held 28 tasks. They matched the 28 of `husker tasks` on the computer by title, status, date, and list, and the counts per list matched. The online MCP at `/mcp/list/Husker` offered the 3 read tools, and `list_tasks` returned the same 3 tasks as the computer's Husker list. `add_task` was refused as an unknown tool. |
| Writes | With writes allowed, the online MCP offered 8 tools. `add_task` answered in 81 ms and `set_task_status` in 51 ms. A change with the version from before the status change was refused as stale. The computer's next `husker sync` printed `Received 1, sent 0` and showed the task done in the Husker list. |
| Phone | The owner paired a phone with the QR code of `husker pair` from a terminal window. The page opened All tasks in the read-only deployment. Pairing proves that `Origin`, `X-Husker`, and `X-Forwarded-Host` reach the server in Azure and that `Set-Cookie` reaches the browser. |
| Cold start | The first request after 25 idle minutes took 3.4 seconds. The next two took 124 ms and 33 ms. The local Functions host took about 4 seconds to start `husker serve`. |

Not verified:

- The phone checklist, tasks 7.1 and 7.2 of `add-phone-page`: the page
  added to the home screen, Safari's keyboard over a panel, the date
  picker, changes while the phone is offline, and Danish on the phone.
- Claude Code and Codex as clients of the deployed server. The checks above
  sent the MCP requests directly, not through either client.

Later on November 16, 2026, the owner confirmed on an iPhone that the
page added to the home screen stays paired (task 7.1) and that the page
works and does what it should (task 7.2). No timing or result per step
was recorded.

## Checks for version 0.17.0

The online server update completed these checks on November 15, 2026:

| Check | Result |
| --- | --- |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Unit tests | 352 passed. 59 of them cover the online server: every change to a list or task, the upload guard, a property test of 600 random changes, parity with the store for results and error sentences, the engine next to devices on the in-memory sync file, access tokens, the MCP tools, and the HTTP routes. |
| Online integration tests | 7 passed. They start a real `husker serve` process next to a laptop profile and the local test double, and connect the rmcp HTTP client in the `initialize` and the discover lifecycle. |
| Package tests | 2 passed for the zip writer in `examples/package.rs`. |
| CLI integration tests | 9 passed against isolated profiles. |
| MCP integration tests | 8 passed through child processes. |
| Sync integration tests | `two_profiles_show_the_same_tasks_after_husker_sync` failed in 4 of 9 runs, and `an_agent_on_a_new_machine_reads_and_writes_a_synced_list` in 1 of 9. On the unchanged 0.16.0 source the same two tests failed in 5 of 6 runs, so the failures don't come from this change. The other runs passed all 8. |
| Review | An independent code and security review found no critical or high defect. It found three medium ones: a stored access policy created again under an old name would revive a leaked SAS, a misspelled revocation list revoked nothing, and text had no size limit. All three were fixed, with the smaller findings: one rule for tasks whose list is missing, a stricter upload guard, the outcome of an upload without an answer, and the token check before the path check. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.17.0. |
| Linux build | `cargo zigbuild --release --locked --target x86_64-unknown-linux-musl` produced a statically linked executable of 14.6 MB on Windows. |
| Package | `cargo run --example package` wrote the zip. Python's `zipfile` read it back without errors and showed `husker` with mode 0755 and the Unix host marker. |
| Templates | `az bicep build` accepted `main.bicep` and `code.bicep`, with one type warning for the `onedeploy` resource. |
| OpenSpec | `openspec validate add-online-server --strict` passed. |

Not verified in this session:

- The deployment to Azure. The agent session was not allowed to run
  `deploy.ps1`, so nothing ran against Azure: the script itself, the
  request proxy of Azure Functions, the headers that reach the server, and
  the cold start time are untested.
- Claude Code and Codex against a deployed server. The rmcp client covered
  both protocol lifecycles against a local server.
- A manual run with `examples/fake_blob.rs`, two profiles, and `curl`. The
  online integration tests cover the same path with real processes.
- Linux and macOS as places to run the terminal. The Linux executable was
  built, not started.

## Checks for version 0.16.0

The suggested-list update completed these checks on November 14, 2026:

| Check | Result |
| --- | --- |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Unit tests | 286 passed. The suggested-list tests cover the newest task winning over a list with 9 of the 10 newest tasks, input order, Trash, done, cancelled, and moved tasks, inaccessible lists, and the default-list fallback. The editor test gives Private more tasks and Work the newest task, and `n` in All tasks opens Work. |
| Review | An independent review found no code defects. It found that the inaccessible-list test also passed when the list check ran after picking the newest task, and an ambiguous spec sentence about synced task IDs. Both were fixed. |
| CLI integration tests | 9 passed against isolated profiles. |
| MCP integration tests | 8 passed through child processes. |
| Sync integration tests | `two_profiles_show_the_same_tasks_after_husker_sync` and `an_agent_on_a_new_machine_reads_and_writes_a_synced_list` failed in 2 of 3 runs. They failed the same way in 3 of 4 runs of the unchanged 0.15.1 source, so the failures don't come from this change. The third run passed all 8. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.16.0. |
| Live terminal | The release executable ran in an 80 by 24 Windows pseudo-terminal against a temporary profile with 9 Labs tasks and a newer Private task. `n` showed Private in List, and Enter saved the task to Private. After `husker add --list Labs`, `n` showed Labs. After that task went to Trash, `n` showed Private again. All 7 checks passed. The installed 0.15.1 failed 3 of them, because it suggested Labs. |
| OpenSpec | The change validated strictly before archiving. |

The Danish interface and mouse input were not driven, because no labels
changed. Linux and macOS were not tested in this session.

## Checks for version 0.15.0

The sync update completed these checks on November 13, 2026:

| Check | Result |
| --- | --- |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Unit tests | 285 passed. New tests cover the three-way merge, conflict copies, list merging, position renumbering, the schema 5 migration, the sync transaction, two and three simulated devices against an in-memory sync file, the worker schedule and thread, and the terminal's sync messages and notice rule. |
| CLI integration tests | 9 passed against isolated profiles. |
| MCP integration tests | 8 passed through child processes. |
| Sync integration tests | 8 passed. Real `husker` processes sync through the real HTTP client against a local test double: two profiles, a title conflict, a new profile whose `--list` exists only in the sync file, an agent's `add_task`, a read-only server, managed identity with a token double, refused uploads, and failures that must not show the URL. |
| Review | An independent review ran a randomized test of three devices over 750 seeds and found no lost task, no diverging device, and no ID that changed its task. It reported three defects, all fixed and covered by tests: an upload whose answer is lost made the next sync undo newer edits, a sync file written again after deletion overrode unsent changes, and a script that adds and syncs within one second hid the external-update notice. It also led to the named default list in the sync file, the range check on IDs and positions, and the rule that a reorder does not keep a task another device deleted. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.15.0. |
| Live terminal | The release executable ran in a 100 by 24 Windows pseudo-terminal against a local stand-in for the blob, with a second profile driven by commands. All 19 checks passed: the start sync showed the other profile's task and `Synced changes from another device.` without the external-update notice, F5 fetched a new task, a task typed in the terminal was uploaded by the exit sync after `Syncing...`, an exit without changes made no request, a failing storage showed the Danish failure message once, and the exit then printed the Danish warning on stderr. No output showed the token. Typing 43 characters took 0.3 seconds with sync on, the same as the installed 0.14.0 in the same harness. |
| Azure Blob Storage | Two temporary profiles synced through a real storage account with a SAS URL for one blob. The first sync created the blob, an unchanged sync transferred nothing, the second profile received the lists and tasks with the same IDs, and a title conflict produced one `#conflict` copy on both. A wrong token reported `HTTP 403, AuthenticationFailed` without the URL. The blob answered 409 `BlobAlreadyExists` and 412 `ConditionNotMet` to uploads whose precondition did not hold. Blob versioning kept each upload. The `az` commands in `docs/sync.md` for the resource group, storage account, versioning, lifecycle rule, container, access policy, and SAS URL were run as written, with `northeurope` as the region. |
| OpenSpec | The change validated strictly, and all seven capability specs passed strict validation after archiving. |

Not verified: managed identity on a real Azure VM. The token request and
the bearer header were tested against a local double only, and the
`az vm identity assign` and `az role assignment create` commands in
`docs/sync.md` were not run. Linux and macOS were not tested in this
session. Mouse input was not driven in the pseudo-terminal.

## Checks for version 0.14.0

The task-capture update completed these checks on November 11, 2026:

| Check | Result |
| --- | --- |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Unit tests | 234 passed. New tests cover the suggested list: the most common of the 10 newest tasks, ties, the window after Trash filtering, closed and moved tasks, inaccessible lists, and the default-list fallback. Others cover Tab and Shift+Tab order with wrapping, the field positions and mouse hits, the two-Tab capture with `t`, and the `Sometime` and `Engang` placeholders. |
| Review | An independent review found no code defects. It found two rules that no test checked, a task-storage spec sentence that the change contradicted, and three inaccurate sentences in the spec delta, the guide, and the README. All were fixed. Removing either rule from the code now fails a test. |
| CLI integration tests | 9 passed against isolated profiles. |
| MCP integration tests | 8 passed through child processes. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.14.0. |
| Manual CLI check | In a temporary profile, `husker add` without `--list` still created the task in Inbox, list 1. |
| Live terminal | The release executable ran in an 80 by 24 Windows pseudo-terminal against a temporary profile. With 3 of the 6 newest tasks in Work, `n` opened the editor with Work in List and `Sometime` in Planned date. Tab focused List, a second Tab focused Planned date and showed the preset hint, and `t` and Enter created the task under Today in Work. Shift+Tab from Title focused Description. The Danish run showed `Liste`, `Planlagt dato`, and `Engang`. `q` exited. |
| OpenSpec | The change validated strictly, and all six capability specs passed strict validation after archiving. |

Mouse clicks were covered by automated tests only. Linux and macOS were not tested in this session.

## Checks for version 0.13.0

The permanent-deletion update completed these checks on November 9, 2026:

| Check | Result |
| --- | --- |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Unit tests | 225 passed. They cover deleting one Trash task, emptying Trash, stale and restored tasks, a changed Trash with an equal count, ID reuse, read-only and list-scoped stores, both confirmations with No as the default, refresh while a confirmation is open, the picker's Trash entry, undo after deletion, long titles, footer hints, Help, and the `D` key conflict. |
| CLI integration tests | 9 passed against isolated profiles. |
| MCP integration tests | 8 passed through child processes. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.13.0. |
| Manual CLI check | A temporary profile showed `Trash (1)` after `trash`, `config --example` listed `empty_trash = ["D"]`, and `new_task = ["D"]` failed with the `D` conflict until `empty_trash = []` was added. |
| Rendered screens | The Trash footer, both confirmations in English and Danish, the emptied Trash, and the picker on Trash were rendered at 80 by 24 cells and inspected. |
| OpenSpec | The change validated strictly, and all six capability specs passed strict validation after archiving. |

No live pseudo-terminal session was driven. Linux and macOS were not tested in this session.

## Checks for version 0.12.1

The heading-count update completed these checks on November 9, 2026:

| Check | Result |
| --- | --- |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Unit tests | 214 passed, including a rendered check that every heading in task views and History ends with its row count in English and Danish, that a held row counts under its old heading, that a filter changes the counts, and that Trash shows no headings. |
| CLI integration tests | 9 passed against isolated profiles. |
| MCP integration tests | 8 passed through child processes. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.12.1. |
| OpenSpec | The change validated strictly, and all six capability specs passed strict validation after archiving. |

No live pseudo-terminal session was driven. Linux and macOS were not tested in this session.

## Checks for version 0.12.0

Version 0.12.0 completed these checks on November 9, 2026:

| Check | Result |
| --- | --- |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Unit tests | 213 passed, including key display names, the new actions and their conflicts, list-only and status-restore mutations, held rows and their ordering, undo of every recorded action, the move picker, Enter saving, filter clearing, paging, confirmation letters, footer packing, and grouped Help. |
| Review | An independent review reported eight findings. Hold ordering for chained rows, reorders next to held rows, hold release at a date change, paging by task rows, undo of editor saves, and a Help row that depends on the Enter binding were fixed and covered by tests. |
| CLI integration tests | 9 passed against isolated profiles. |
| MCP integration tests | 8 passed through child processes. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.12.0. |
| OpenSpec | The change validated strictly before and after implementation. All six capability specs passed strict validation after archiving. |
| CLI examples | The release executable created lists and tasks, completed a task, printed JSON, and printed the example configuration against an isolated profile. A configuration binding `history` to `m` was rejected with the conflict message, and adding `move_task = []` made it load. |

The terminal screens were rendered through the test backend at 80 by 24
cells with sample tasks in every date group, in English and Danish. The
inspected screens were All tasks with a task held after Space, the filter
input with its syntax hint, the move picker, Help from navigation and from
the picker, the cleared-date and Undone messages, the new-task editor, the
discard prompt, the list picker, the list-deletion confirmation, a list
view, History, and Trash. Footers fit in two lines at 80 columns in both
languages for All tasks. The Danish list view drops the last footer entry at
exactly 80 columns, as the packing rule allows.

No live pseudo-terminal session was driven in this session, so physical
key delivery of Ctrl+C, PageUp, PageDown, and Shift letters was not tested
on Windows. Mouse gestures were covered by automated tests only. Linux and
macOS were not tested in this session.

## Checks for version 0.11.1

The Left/Right alias update passed all 213 tests on November 9, 2026:
196 unit tests, 9 CLI tests, and 8 MCP tests. Formatting, all-target Clippy
with warnings denied, and all six strict OpenSpec validations passed.
The debug executable reported 0.11.1. Isolated CLI checks confirmed the
new defaults, Left/Right conflicts, and the documented override remedies.
All 15 local documentation links and 5 JSON/TOML examples passed validation.

An 80 by 24 Windows pseudo-terminal check confirmed that Right selected the
last task and Left selected the first. In Description, Left and Right still
moved the text cursor. Typing `ABC`, Left, `X`, Right, and `Y` produced
`ABXCY`. Ctrl+S saved that text, and a separate CLI read confirmed version 2.
Quitting restored the terminal. These checks used isolated profiles.
Linux and macOS were not tested in this session.

## Checks for version 0.11.0

Version 0.11.0 completed these checks on November 9, 2026:

| Check | Result |
| --- | --- |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.11.0. |
| OpenSpec | All six capability specs passed strict validation after archiving the planning and navigation change. No active changes remain. |
| Unit tests | 196 passed, including keyboard wrapping, boundary jumps, History date display, configuration, and bounded movement regressions. |
| CLI integration tests | 9 passed against isolated profiles. |
| MCP integration tests | 8 passed through child processes. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Guide examples | Documented task/list commands, stale-version guards, strict external date input, and MCP help passed against an isolated profile. |
| Shortcut compatibility | Verified First/Last defaults, conflicts with custom `g`, `G`, Home, and End bindings, and the documented rebinding or disabled-action remedies. Existing `h`, `l`, and date-preset defaults stayed unchanged. |
| Date compatibility | JSON retained `due`. `due:overdue` matched both open and completed tasks with past planned dates. `due:earlier`, `due:tidligere`, and `planned:today` were rejected without task changes. |
| Documentation | All 15 local links and anchors resolved. All 5 JSON and TOML examples parsed. |

An 80 by 24 Windows pseudo-terminal check displayed Earlier and Planned
date in English and Tidligere and Planlagt dato in Danish. Task selection
wrapped with `k` and `j`. Home, End, and `G` selected boundary tasks.
History showed the completion-day heading without the future planned-date
suffix. Opening that task's editor still showed `2026-11-13`, although the
task was completed on `2026-11-09`.

In the navigation picker, `G` selected Trash and `j` wrapped to All tasks.
The editor picker wrapped through ordinary lists with `j` and `k`.
The language picker wrapped with `k` and `j`, and `G` followed by Enter
saved Danish. Quitting restored the terminal. A separate CLI read confirmed
the original task JSON and version 2 were unchanged, and configuration
contained `language = "da"`.

These checks used an isolated Windows profile. Mouse interaction was not
checked manually. Linux and macOS were not tested in this session.

## Checks for version 0.10.0

Version 0.10.0 completed these checks on November 8, 2026:

| Check | Result |
| --- | --- |
| Unit tests | 188 passed, including date parsing, dated headings, list-choice history, modal state, and mouse hit regions. |
| CLI integration tests | 9 passed against isolated profiles. |
| MCP integration tests | 8 passed through child processes. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.10.0. |
| Guide examples | Task commands, version checks, Trash/restore, list rename identity, picker bindings, and MCP help passed against an isolated profile. |
| Strict external dates | CLI add/edit and query input rejected `20261109`, `261109`, and `1109` without changing or creating tasks. The existing `due:soon` and `due:next` boundaries passed. |
| Profile compatibility | Opening a version 0.9.0 scratch profile with 0.10.0 preserved every task JSON field. Both versions use schema 4. |
| OpenSpec | All six capability specs passed strict validation after archiving the date and list editing change. No active changes remain. |
| Documentation | All 15 local links and anchors resolved. All 5 JSON and TOML examples parsed. |

An 80 by 24 Windows pseudo-terminal displayed actual dates on Today,
Tomorrow, and Done today headings. Future tasks appeared under separate
November 10 and November 20 headings without Soon or Next labels.
The empty Due date placeholder and the full list of accepted date formats
fit in the editor.

Entering `1112` and pressing Tab displayed `2026-11-12`. Enter on List
opened a popup with ordinary lists and the default marker. The popup
excluded All tasks and Trash. Choosing Home changed only the draft.
A separate CLI read confirmed that the stored task still belonged to Work,
had no due date, and retained version 1 before Save.

The terminal checks also verified English and Danish date headings, the
placeholder and format hint, and normalization from both four-digit and
six-digit entries. Ctrl+Z undid a List choice. A temporary configuration
bound Save to F6 for a persistence check. The following CLI read confirmed
the selected list and due date were saved together.

A separate Danish editor check entered raw `20261114` and pressed the
default Ctrl+S shortcut without leaving Due date first. The editor returned
to navigation. A CLI read confirmed `2026-11-14`, the selected Home list,
and the unchanged title, description, and status. All three compact input
lengths were exercised manually: four and six digits normalized on Tab,
and eight digits saved directly. Danish History showed I dag with its actual date.

All terminal processes exited cleanly. These checks used Windows and
isolated profiles. Linux and macOS were not tested in this session.

## Checks for version 0.9.0

Version 0.9.0 passed these checks on November 8, 2026:

| Check | Result |
| --- | --- |
| Unit tests | 170 passed, including calendar presets, due-only writes, whole-field undo, timestamp transitions and sorting, schema migration, and task-list labels. |
| CLI integration tests | 9 passed against isolated profiles. |
| MCP integration tests | 8 passed through child processes. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.9.0. |
| Guide examples | Verified preset defaults and overrides, conflicting Due date bindings, timestamp-aware ordering, repeated status, due edits, Trash, reopening, and MCP help. |
| Migration | An isolated profile created by version 0.8.0 upgraded from schema 3 to schema 4 with its existing fields, task versions, and list order unchanged. |
| OpenSpec | All six capability specs passed strict validation after archiving the presets change. No active changes remain. |
| Documentation | All 15 local links and anchors resolved. All 7 JSON and TOML examples parsed. |

The migration check created open, done, and trashed tasks with the old
executable. Read-only version 0.9.0 refused the old schema without changing
it. A writable `lists` command completed the upgrade. Every migrated task
received a null timestamp, including tasks with known completion dates.
The old executable then refused to open the upgraded profile.

Separate CLI completions recorded a lower-ID task after a higher-ID task
on the same local day. The newer completion appeared first, followed by
the older recorded time and the migrated task with an unknown time.
Repeating done preserved the original timestamp. Due-date edits and
Trash/restore also preserved it. Changing done to cancelled recorded a new
timestamp, and reopening cleared both status-change fields.

An 80 by 24 Windows pseudo-terminal check exercised `t`, `T`, `w`, and `x`
against an isolated profile. A preset that repeated the stored date kept
the task version unchanged. Editor presets supported one-step undo, and
Ctrl+S persisted the resulting date. English Due date hints were visually
inspected.

Alt+Down on a closed task left its position, version, and completion timestamp
unchanged. All tasks and all-list History displayed list prefixes, while the
Work view omitted them. Same-day closed tasks appeared newest first despite
their reversed task IDs.

The terminal check switched the interface to Danish, used `w` in the editor,
and saved. A release CLI read confirmed the due date was November 9.
Danish preset hints were checked by unit tests. They were not visually
inspected in this terminal session.

These checks used Windows and isolated profiles. Linux and macOS were not
tested in this session.

## Checks for version 0.8.0

Version 0.8.0 passed these checks on November 8, 2026:

| Check | Result |
| --- | --- |
| Unit tests | 149 passed, including finished-task groups, History, list-deletion previews and retention, shortcut compatibility, and language persistence. |
| CLI integration tests | 8 passed against isolated profiles. |
| MCP integration tests | 8 passed through child processes. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.8.0. |
| OpenSpec | All six capability specs passed strict validation after archiving the lifecycle change. No active changes remain. |
| Guide examples | Verified default and overridden shortcuts, duplicate root-action rejection, old and unknown-date completions, shared ordering, due filters, clearing dates, reopening, Trash, list JSON, and MCP help. |
| Documentation | All 15 local links and anchors resolved. All 7 JSON and TOML examples parsed. |

The CLI guide checks used a new scratch profile. Two fixture completion dates
were set to yesterday and unknown to verify retained records without waiting
for another day. Explicit status filters returned both records. Default
results excluded them. Clearing a completed task's due date preserved its
status and status-change date.

Interactive checks ran in an 80 by 24 Windows pseudo-terminal with isolated
profiles. The initial English view displayed All tasks, Today, Done today,
and Cancelled today. F3 opened the language picker. Choosing Dansk changed
labels and date text immediately and created `config.toml` with `language = "da"`.
Quitting and reopening Husker retained Danish. History opened with `h` and
displayed localized labels.

The Danish list-deletion dialog showed one unfinished task, one cancelled
task, and one task in Trash, with No selected initially. Confirming deletion
moved all three tasks to Inbox. CLI reads confirmed their statuses, dates,
and Trash flags were preserved and their versions increased.

A separate run used an explicit `--config shared.toml` file.
F3 switched from English to Danish while a task editor contained the unsaved
title `Preserved draft æøå`. The editor returned with that exact draft.
The shared file changed only its language value and retained both test
comments and the Ctrl+Q override. The profile's default configuration stayed
unchanged. Ctrl+S saved the exact Unicode title, and Ctrl+Q quit.

Linux and macOS were not tested in this session.

## Checks for version 0.7.0

Version 0.7.0 passed these checks on November 8, 2026:

| Check | Result |
| --- | --- |
| Unit tests | 120 passed, including list picker navigation, saved Planned state, list forms, help return, and modal mouse handling. |
| CLI integration tests | 8 passed against isolated profiles. |
| MCP integration tests | 7 passed through child processes. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.7.0. |
| OpenSpec | All six capability specs passed strict validation after archiving the navigation change. No active changes remain. |
| Guide examples | Verified default and rebound picker shortcuts, ordinary list ordering and JSON fields, task commands, and MCP help. The virtual All tasks entry did not appear in CLI list output. |
| Documentation | All 15 local links and anchors resolved. All 7 JSON and TOML examples parsed. |

Interactive checks ran in an 80 by 24 pseudo-terminal with isolated profiles.
The English check opened Work and Trash through the picker and showed their
`Planned > Work` and `Planned > Trash` headings. Returning to a filtered
Planned view restored the same task and scroll position. Trash kept its
ungrouped task rows.

Help and a canceled list-name form returned to the scrolled picker.
Creating and renaming a list kept the saved list selected in the picker.
A separate CLI read confirmed the renamed list existed and the canceled
draft list did not. Discarding changed task text returned to Work.
A CLI read confirmed that task's description and version stayed unchanged.

The Danish check displayed `Planlagt`, `Vælg en liste`, and `Alle opgaver`.
Choosing Work displayed `Planlagt > Work`, and Esc returned to Planlagt.
The footer used Danish text. Both terminal checks exited cleanly.

The checks ran on Windows. Mouse behavior was checked by automated tests.
Physical mouse gestures were not tested in this session. Linux and macOS
were not tested in this session.

## Checks for version 0.6.0

Version 0.6.0 passed these checks on November 8, 2026:

| Check | Result |
| --- | --- |
| Unit tests | 112 passed, including exact selected-text copying, all input modes, clipboard failures, NUL rejection, preserved editing state, and shortcut configuration. |
| CLI integration tests | 8 passed against isolated profiles. |
| MCP integration tests | 7 passed through child processes. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.6.0. |
| OpenSpec | All six capability specs passed strict validation after archiving the clipboard change. No active changes remain. |
| Guide examples | Verified default, rebound, and disabled copy shortcuts, conflicting Ctrl+C bindings, task commands, no-date filters, and MCP help. |
| Native clipboard | A Windows clipboard read matched the selected multiline text exactly, including LF newlines, Danish letters, and a joined emoji. |

The native clipboard check ran against a scratch profile in the terminal.
Ctrl+A selected Description and Ctrl+C displayed `Copied selection` while
keeping the editor open. `Get-Clipboard -Raw` returned the expected text,
including `Køb æbler` and `👩‍💻`. Esc then closed the unchanged editor without
a discard prompt. The test harness restored the previous clipboard text
after the check.

Automated UI tests use recording and failing clipboard implementations.
They do not read or overwrite the system clipboard. The discard-confirmation
tests also cover both terminal event encodings of Shift+Tab.

The checks ran on Windows with Rust 1.91.1 and isolated profiles.
Linux and macOS native clipboard behavior was not tested in this session.
No SSH clipboard forwarding was tested or added.

## Checks for version 0.5.0

Version 0.5.0 passed these checks on November 8, 2026:

| Check | Result |
| --- | --- |
| Unit tests | 105 passed, including Unicode word movement, selection, replacement, undo/redo, discard confirmation, task IDs, planning order, and reorder boundaries. |
| CLI integration tests | 8 passed against isolated profiles. |
| MCP integration tests | 7 passed through child processes, including scoped pagination with open tasks first. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.5.0. |
| OpenSpec | All six capability specs passed strict validation after archiving the editor controls change. |
| Guide examples | Verified task commands, open-first ordering, both no-date filters, the stable `undated` CLI token, versioned edits, Trash, list ordering, and default keys. |

Interactive checks ran in an 80 by 24 pseudo-terminal with a scratch profile
created by version 0.4.0. Sometime showed an open task before done and cancelled
tasks. Opening the task displayed `Edit task #2` in the heading.

Injected control-key sequences moved by word and selected `beta` in Description.
Typing replaced the selection. Ctrl+Z and Ctrl+Y undid and restored the edit.
Esc opened a confirmation with No selected. Enter returned to the unchanged
draft. Ctrl+S saved the expected description and incremented the task version.
A CLI read confirmed the saved text, including its newline and Danish characters.

Ctrl+A selected the whole Description field for replacement. Esc, Right, and
Enter explicitly discarded that later change. A CLI read confirmed the stored
text and version stayed unchanged. Reopening the task and pressing Esc without
editing closed the editor immediately. Quit restored the terminal settings.

Unit tests verified that one paste event is one undo step. The pseudo-terminal
injection did not establish that paste behavior, so the interactive checks
cover typed edits only.
The checks ran on Windows with Rust 1.91.1 and isolated profiles.
Physical keyboard delivery of modified shortcuts, Linux, and macOS were not
tested in this session.

## Checks for version 0.4.0

Version 0.4.0 passed these checks on November 8, 2026:

| Check | Result |
| --- | --- |
| Unit tests | 87 passed, including atomic migrations, saved list order, wrapped text and cursor movement, Unicode, mouse positions, heading styles, and message expiration. |
| CLI integration tests | 7 passed against isolated profiles. |
| MCP integration tests | 6 passed through child processes. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.4.0. |
| OpenSpec | All six capability specs passed strict validation after archiving the terminal layout change. |
| Guide examples | Verified help, default shortcuts, task commands, filters, status changes, Trash, versioned edits, list ordering, and the default list after a rename. |

Interactive checks ran in an 80 by 24 pseudo-terminal with a scratch profile
created by version 0.3.0. Migration preserved a task's full description,
due date, and version. Its description wrapped into three displayed rows,
followed by a paragraph separated by an existing newline.

An injected mouse click placed the cursor on a wrapped row. Ctrl+S saved the
inserted text at that position. A CLI read confirmed the exact description,
its original newline, and the new task version.
Clicking the Tomorrow heading left the selected task unchanged.
Unit tests checked full-width task rows and heading styles at 140 columns.

The migrated lists appeared as Inbox, Alpha, Beta, Zulu, and Trash.
Alt+Down on Alpha moved it below Beta and kept Alpha selected.
Attempts to move Inbox or Trash did nothing. Restarting Husker preserved
the order Inbox, Beta, Alpha, Zulu, and Trash.
Quit restored the terminal's normal screen and input settings.

F5 displayed a refresh result. Navigating to the list chooser did not extend
the message's lifetime. The footer cleared during six seconds of idle time.
A task added by a separate CLI process triggered an external-update notice.
The notice remained through more than twelve seconds of idle time.
F5 acknowledged that notice, and the replacement refresh result then expired.

The checks ran on Windows with Rust 1.91.1 and isolated profiles.
Mouse checks used injected terminal events, not physical mouse gestures.
Linux and macOS were not tested in this session.

## Checks for version 0.3.0

Version 0.3.0 passed these checks on November 8, 2026:

| Check | Result |
| --- | --- |
| Unit tests | 68 passed, including schema migration, stale edits, local status dates, filters, sorting, mouse hit regions, copy text, and refresh notices. |
| CLI integration tests | 7 passed against isolated profiles. |
| MCP integration tests | 6 passed through child processes. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.3.0. |
| OpenSpec | All six capability specs passed strict validation after archiving the workflow change. |
| Guide examples | Verified help, default shortcuts, task commands, literal hashtag searches, status visibility, and list ordering against the debug executable. |

Interactive checks ran in an 80 by 24 pseudo-terminal with a scratch profile
created by version 0.2.0. Migration preserved the description and appended
`#shopping`. The changed task received a new version.

Ctrl+S saved an added description line. Esc discarded a later unsaved change.
CLI reads confirmed both results. F4 displayed plain task text and disabled
mouse capture. A task completed by another connection remained visible with
an `[x]` marker and an external-update notice. F5 acknowledged the notice.

Injected terminal mouse events selected the drawn task and opened its editor
on a double-click. Lists appeared as Alpha, Inbox, Work, and Trash.
Returning from Work with Esc kept Work selected. Quit restored the main screen
and disabled raw input and mouse capture.

The checks ran on Windows with Rust 1.91.1. Tests used isolated profiles.
The installed app and personal profiles were not upgraded during development.
Physical mouse selection into the system clipboard was not tested.
Linux and macOS were not tested in this session.

## Checks for version 0.2.0

The MCP version passed these checks on November 8, 2026:

| Check | Result |
| --- | --- |
| Unit tests | 52 passed, including list restrictions, read-only storage, and list identity after a rename. |
| CLI integration tests | 6 passed against isolated profiles. |
| MCP integration tests | 6 passed through child processes. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Release build | `cargo build --release --locked` passed. The release executable reports version 0.2.0 and includes the MCP options. |
| OpenSpec | All six capability specs passed strict validation after archiving the MCP change. |
| Setup guide | Verified MCP help and list commands against the debug executable. Verified Codex and Claude Code registration syntax with their local help and official guides. |

The MCP tests use the official Rust SDK as a client.
They cover both the `initialize` lifecycle and the newer `discover` lifecycle.
The tests check restricted list reads and writes, read-only access, paginated searches,
and task creation through completion, Trash, and restoration.
They also check stale versions and tasks moved to another list by a separate connection.
Protocol checks verify JSON-RPC output and shutdown after standard input closes.

The MCP checks ran on Windows with Rust 1.91.1.
Codex and Claude Code registration commands were checked without changing client settings.
Linux, macOS, and an interactive MCP session inside either client have not been tested.

## Checks for version 0.1.0

The first working version passed these checks on November 8, 2026:

| Check | Result |
| --- | --- |
| Unit tests | 45 passed. Covers storage, planning, search, configuration, localization, and terminal actions. |
| CLI integration tests | 6 passed. Runs commands against isolated profiles. |
| Formatting | `cargo fmt --check` passed. |
| Clippy | All targets passed with warnings denied. |
| Release build | `cargo build --release --locked` passed. |
| Local installation | `cargo install --path . --locked --offline --root .scratch/install` passed. |
| Interactive terminal | English launch and quit passed. Danish task creation, multiline text, due date, description focus, discard, and help scrolling passed at 80 by 24 cells. |
| Persistence | The CLI returned the exact Danish title, two description lines, and date saved through the terminal. |
| OpenSpec | All five capability specs passed strict validation. |

The checks ran on Windows build 26300 with Rust 1.91.1. Linux and macOS
were not tested in this session. AltGr handling has a regression test based
on Crossterm's Windows key events. A physical Danish keyboard was not tested.

## Performance sample from version 0.1.0

The release executable searched 10,000 tasks and printed all matches as
JSON in a median 73.5 ms across five warm runs. This is a local measurement,
not a latency guarantee.

The fixture used one list and 10,000 open tasks outside Trash. Every task
had the `shopping` tag and the description `Fixture description with Danish: Køb æbler`.
Titles were `Task 1` through `Task 10000`. Two thirds of tasks had dates
from five days before today through 24 days after today. The rest were undated.

The measured command was:

```text
husker --profile FIXTURE_DIRECTORY --json tasks "#shopping æbler"
```

Timing included process startup, database reads, Unicode search, sorting,
and JSON output captured in memory. Terminal drawing and fixture creation
were outside the measurement.

| Run | Elapsed time |
| --- | --- |
| First run | 71.5 ms |
| Median of five later runs | 73.5 ms |
| Fastest later run | 62.5 ms |
| Slowest later run | 75.3 ms |

The machine reported `Intel64 Family 6 Model 189 Stepping 1, GenuineIntel`
and `Microsoft Windows NT 10.0.26300.0`. The benchmark used a local SQLite
database and a release build with thin link-time optimization.
