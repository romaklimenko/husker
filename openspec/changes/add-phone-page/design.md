## Context

The online server (`husker serve`) reads and writes the sync file and offers the MCP tools over HTTP. It checks access tokens that `husker token` signs. The terminal groups rows under headings in `planning::group_rows`, which the change `add-online-server` moved out of the renderer.

The user wants a simple page for a phone: start from the planned tasks, filter by list, and add, move, plan, delete, close, and reopen tasks. The server scales to zero, so the first request after a pause takes a few seconds.

| Fact | Evidence |
| --- | --- |
| A browser never sends the part of a URL after `#` to a server. | Read from the URL standard. |
| Safari limits storage that scripts write to seven days without a visit. A cookie that a server sets has no such limit. | Read from WebKit's tracking prevention notes. |
| An installed page on an iPhone has its own storage, separate from Safari. | Read from WebKit bug reports. Not checked on a phone yet: the phone check in `tasks.md` covers it. |
| A QR code for a 340-byte link needs about 39 terminal rows. A link of up to 78 characters with a 12-character code needs 21. | Computed from the QR code capacity tables. A test draws the code and reads it back. |
| The Azure Functions host passes the path, the query, the method, the body, `Authorization`, `Cookie`, `Origin`, and custom headers to a custom handler unchanged. It adds `X-Forwarded-Host` with the address the client used. It passes `Set-Cookie` and the other answer headers back. With the homepage setting off, the root path reaches the handler. | Measured on this computer with Core Tools 4.16.0 and host 4.1054.250, by `deploy/azure/check-local.mjs`. The Azure front end before the host is not measured. |
| The date field of a phone can report a date while the user still turns the month or the year. | Not measured. The review of this change raised it. The page does not depend on it: the date field needs Save. |

## Goals / Non-Goals

**Goals:**

- Show the views of the terminal with the same headings, counts, and order.
- Make every listed action one or two taps.
- Feel fast although the server may start cold: show the last view at once, change the screen before the request returns.
- Keep the access token out of reach of page scripts.
- Keep the page free of a build step and of third-party code.

**Non-Goals:**

- Changes of existing tasks while the phone has no connection. A new task is the exception: it waits on the phone.
- Permanent deletion, list management, reordering, and search filters on the page.
- Push notifications and reminders.
- Sign-in with an identity provider.

## Decisions

### 1. The server builds the groups

`GET /api/view` returns the groups of a view with their labels, counts, and rows. It calls `query::visible_tasks`, `query::visible_history`, and `planning::group_rows`, as the terminal does. The page renders what it gets.

Reason: one implementation of the grouping, the date labels, and the planning order. The page holds no date arithmetic except its own local date.

Rejected: sending raw tasks and grouping in JavaScript. That would be a second copy of the rules.

### 2. The phone sends its local date

Every API request carries `today` as the phone's local date. The server accepts it within one day of its own UTC date and uses it for groups, presets, and the status date.

Reason: planned dates are local calendar dates, and the phone knows its time zone. The bound keeps a wrong clock from writing odd dates.

### 3. One request changes one task

`PATCH /api/tasks/ID` can set several fields of one task. The server applies them as one upload through `Engine::write_all`. The upload guard still allows only one record to differ.

A title or description change needs the `version` the page last read. Status, planned date, list, and Trash apply to the newest file without a version.

Reason: a tap on a checkbox should not fail because a computer changed the title a minute ago. Text is different: a stale edit would overwrite newer text.

### 4. The page creates the identifier of a new task

`POST /api/tasks` carries the `uid` of the new task. A repeat with the same `uid` and the same content returns the task that exists.

Reason: a request that got no answer can be sent again without a duplicate.

### 5. Pairing with a short code

`husker pair` signs an access token for the phone and hands it to the server with `POST /api/pairings`. The server keeps it in memory under a random pairing code of 12 characters for five minutes. The terminal shows the address with the code as a QR code and as text. The page sends the code to `POST /api/session` once and gets the session cookie.

Reason: the QR code stays small enough for a terminal of 24 rows. The long-lived access token never appears on the screen. A code that someone photographs is useless after one use or five minutes. The user can type the code in an installed page that cannot scan.

Rejected: the access token in the link. The QR code would need about 39 rows, and the link would work until the token expires.

The codes live in memory only. The server runs as one instance, and a restart within five minutes means running `husker pair` again.

One access token has one code at a time. A new code for a token replaces its older one. An agent with a list token can therefore fill one of the 16 places, and cannot push out the code that the user just made for the phone.

The terminal draws the QR code black on white with explicit colors. A code drawn in the colors of a light terminal theme would be inverted on a dark one, and some cameras refuse an inverted code. A test reads the drawn code back with a QR decoder.

### 6. The session is an HttpOnly cookie

`POST /api/session` answers with the cookie `husker_session`, which holds the access token. The cookie is `HttpOnly`, `SameSite=Strict`, limited to `/api`, and `Secure` when the server address uses `https`. Its lifetime is the token's.

Reason: page scripts cannot read it, so an injected script cannot take the token away. A server-set cookie also survives Safari's seven-day limit for script storage.

A change through the cookie must carry the header `X-Husker: 1`. Another site cannot add that header without permission from this server, and the cookie is not sent across sites.

The server checks the access token first, the header second, and reads the request body last. A stranger gets 401 for any request and cannot make the server read 512 KiB. axum runs extractors in order, so the two checks are extractors before the body.

### 7. No build step, no third-party code

The page is six text files and three icons in `web/`. `src/online/assets.rs` embeds them with `include_bytes!`. The page sets text with `textContent` only. The server sends a content security policy that allows scripts, styles, and connections from the server itself and nothing else.

Reason: nothing to install, nothing to keep up to date, and task text that an agent wrote cannot become markup.

### 8. Strings come from the catalog

`GET /strings.json` returns the page's text in English and Danish, built from `i18n::tr`. It needs no access token, because the pairing screen needs text too. A test checks that every key the page uses exists in both languages.

### 9. Cached shell, cached last view

A service worker keeps the page files, so the page opens without waiting for the server. It never handles `/api`. The page keeps the last view of All tasks in `localStorage` and shows it at once, then loads the current one. The page keeps its text there too, and waits for `/strings.json` only when it has no kept text.

Reason: a cold start then delays only the refresh, not the first paint.

Without a kept view the page shows a loading line, and the refresh button turns while a request for the view is under way. A blank screen for the seconds of a cold start would look broken.

The server replaces a mark in `web/sw.js` with a version: a hash of every page file and of the page's text. The worker names its cache after the version, fetches all files at installation, and answers page files from that cache only. The old worker and its cache serve the page until the new one has every file.

Reason: a release then reaches the phone as a whole. A worker that renews each file on its own can leave a new `app.js` next to an old `logic.js` after one failed fetch, and the page then does not start.

### 10. Changes show before the answer

The page changes the row first and sends the request. On a failure it puts the row back and shows a message. A closed task stays in its row until the next refresh, as it does in the terminal.

The checkbox and the status buttons of the sheet behave the same in All tasks: the row keeps its place. Such a change asks for `view=none`. The answer then holds the row alone, a few hundred bytes, where the whole view of 200 tasks with long descriptions has about 460 KB.

### 11. Views show in the order of their requests

A cold start makes answers late, so a tap often overlaps the first refresh. The page numbers every request that returns a view when it sends it. It shows a view only when no view of a later request is on screen.

| Late answer | What the page does |
| --- | --- |
| A refresh arrives while a change of a row is under way. | The row keeps the values the user chose. |
| A change fails after a refresh. | Only the values that the change set go back, on that row. |
| An older refresh arrives after a newer view. | The page drops it. |
| A view arrives whose request was sent before a change was confirmed. | The page does not show it and requests the current view again. |
| The answer to a change arrives after a newer view, or after the user moved to another view. | The page does not show it and requests the current view again. |
| An answer arrives after signing out, after a lost session, or after a new pairing. | The page drops it and keeps nothing. Nothing waits for it. |

Reason: a server answers a read without waiting for a write that is under way, so the order of the answers says nothing about which view is newer. The sending order is the only order the page knows. A second request settles the doubtful cases with one extra read.

The page counts its sessions for the last row. A request remembers the count it was sent under, and its answer is dropped when the count has grown. Without the count, a refresh that was under way at a sign-out could put the old tasks back on screen after the next pairing, and a late 401 could end a pairing that had just begun.

A request gives up after 30 seconds. A stalled connection would otherwise hold a sheet that waits for its answer.

Rejected: blocking the screen while a request is under way. The page would feel slow on every cold start.

### 12. The sheet sends what changed

The title, the description, and the date field are fields: the user changes them and taps Save. The buttons and the list selector act at once. A button sends the changed fields in the same request, so a tap on Today after typing a title loses nothing. The sheet stays open until the server accepted changed fields.

Save sends only the fields that changed. A field is compared with what it showed when the sheet took the task, not with the task: a browser turns `\r\n` in a text area into `\n` and drops line breaks in a one-line field, and tasks from agents hold such text. Compared with the task, an untouched sheet would count as changed.

After a `stale_version` answer the page reads the task again and shows its current values in the untouched fields. The second Save then carries the current version and cannot put an old title back that the user never touched. A button whose request failed goes out again with that Save.

While a request of the sheet runs, the fields and buttons wait, because text typed then would not be part of the request. The close button stays in reach.

The date field needs Save and does not act on every change. A date picker can report a date while the user still turns the month or the year, and a sheet that closes in that moment would take a date nobody chose.

Rejected: Save that always sends title and description. After a stale answer it overwrote the other computer's text with the copy the sheet had loaded.

### 13. A new task stays on screen until the server has it

The field empties when the task is sent, so several tasks can be typed during a cold start. The task shows in a row above the groups. Without an answer it stays there, marked as not sent, and goes out again on a tap or when the next view arrives. The page made its `id`, so a repeat creates no second task.

The page keeps the tasks that wait on the phone, because a phone browser unloads a page in the background at any time. After a reload they show as not sent and go out with the next answer. Signing out removes them.

Only a refusal with the server's own error object removes a waiting task. A 502 from the storage, a 503 from a gateway, and an answer without the task leave it waiting.

This is the only change the page makes without a connection. A change of an existing task still needs the server.

### 14. A pairing link asks, and a sign-out finishes later

A pairing code in the address needs a yes from the user when the phone is paired. Otherwise anyone with a valid access token, also a narrow one, could make a code and replace the phone's pairing with one click on a link. The page asks the server whether the phone is paired. A kept view cannot tell: Safari removes it after seven days without a visit, while the cookie stays.

Page script cannot remove an `HttpOnly` cookie. Signing out therefore removes the tasks from the screen and the phone at once, and asks the server to remove the cookie. Without a connection the page keeps a note, shows no task while the note exists, and finishes at the next start. One such request runs at a time and a new pairing waits for it, so its late answer cannot remove the new cookie.

### 15. A browser check drives the real page

`web/browser-check.mjs` starts the test double of the sync storage, `husker serve`, and headless Chrome at phone size. It pairs, adds, closes, plans, moves, edits, trashes, restores, changes the language and the list filter, stops the server, signs out, and checks that a computer receives the changes. It saves one screenshot per step. It is not part of `cargo test`, because it needs Chrome.

Reason: `web/app.test.mjs` runs the page script against a stand-in document. It cannot find a covered button, a keyboard that opens over the sheet, a cookie attribute, or a service worker that does not install.

### 16. The Functions host runs on the computer before a deployment

`deploy/azure/check-local.mjs` starts the Azure Functions host of Core Tools with the deployed `host.json` and `function.json` and `husker serve` as its custom handler. It checks the page files, the cookie, each method, and MCP through the host. A second handler answers with the request it received, which shows what the host forwards.

Reason: the proxy behaviour was read from the host's source in the first change. Measuring it needs no Azure.

## Risks / Trade-offs

- [The installed page on an iPhone does not share Safari's cookie] → The pairing screen takes a typed code, so the installed page pairs on its own.
- [The cached view shows task text on a lost phone] → Signing out and a 401 answer clear the cache. The phone's own lock protects the rest.
- [A code is guessed] → 60 bits, one use, five minutes, and at most 16 codes at a time.
- [An injected script acts as the user] → A strict content security policy, no inline script, `textContent` only, and no third-party code.
- [The keyboard covers Save on a phone] → The page reads the visible part of the page from the browser and lifts the sheets above the keyboard. A test covers the arithmetic. No phone has shown it yet.
- [The phone holds tasks after it is lost] → The kept view stays on the phone until the page next reaches the server and gets 401. Revoking the access token stops new reads, not the kept ones. The phone's own lock protects them.
- [The page is not tested in Safari] → The grouping is server code with tests, the page script runs under `node --test` against a stand-in document, the browser check runs it in Chrome, and a checklist covers a real phone.
- [The page files of two releases mix] → The server writes a version of all page files into the service worker script. A new release changes the script, and the new worker fetches every file into its own cache before it replaces the old one. A fetch that fails leaves the old worker and its files in place.
- [The phone shows the old page after a deployment] → The page opens from the kept files once more while the new worker installs. The next opening shows the new page.

## Migration Plan

1. Deploy the server with `deploy/azure/deploy.ps1`.
2. Run `husker pair` and scan the code with the phone.
3. Add the page to the home screen and go through the phone checklist in `tasks.md`.

Rollback: deploy the previous release. The page keeps no data on the server.

## Open Questions

None.
