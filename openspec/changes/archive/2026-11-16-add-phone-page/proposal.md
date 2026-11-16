## What Changes

1. The online server serves a phone page at `/`. The page is plain HTML, CSS, and JavaScript inside the `husker` executable.
2. The page opens on All tasks with the groups, headings, and counts of the terminal's first screen. A list filter narrows it to one list.
3. The page adds a task, closes and reopens a task, cancels a task, sets or clears the planned date, moves a task to another list, moves a task to Trash with an undo, and edits the title and description.
4. The page has two more views: History and Trash. Trash restores a task.
5. The online server gains a JSON API under `/api` for the page.
6. A new command, `husker pair`, signs an access token for the phone and shows a QR code. The QR code holds a pairing code that works once and for five minutes. The page exchanges the pairing code for a session cookie that page scripts cannot read.
7. The page shows English or Danish text from the Husker catalog.
8. The rows of a view are grouped under headings by one function for the terminal and the page.

## Why

The user manages tasks in the terminal and has no way to see or change them away from the keyboard.
The online server already reads and writes the sync file. A small page on top of it reaches the phone without an app store and without a second copy of the task rules.

## Capabilities

### New Capabilities

- `phone-page`: The page at `/`, its views and actions, the JSON API under `/api`, pairing codes, the session cookie, and the page's languages.

### Modified Capabilities

- `command-line`: The `pair` command.
- `online-server`: A request can carry its access token in the session cookie. Changes through the cookie need a request header that other web pages cannot send.

## Impact

Computers need no change. The sync file format stays 1.
The MCP endpoint, its tools, and access tokens are unchanged. An access token works for the page and for MCP alike.
The page needs a network connection for every change. It shows the last loaded view without one. A task added without a connection stays on screen and on the phone, and is sent when the server answers again.
A change on the page is in the sync file at once and wins over a change to the same field that a computer has not uploaded yet, as every change through the online server does.
The online server keeps pairing codes in memory for five minutes. A restart within that time loses them, and the user runs `husker pair` again.

New dependency: `qrcode`, for the QR code in the terminal. New test dependency: `rqrr`, which reads the drawn QR code back.

Affected files:

| File | Change |
| --- | --- |
| `web/*` | New. The page: `index.html`, `app.css`, `app.js`, `logic.js`, `sw.js`, `manifest.webmanifest`, and three icons. |
| `src/online/api.rs`, `src/online/assets.rs` | New. The JSON API and the embedded page files. |
| `src/online/http.rs`, `src/online/view.rs`, `src/online.rs` | Routes for the page and the API, the session cookie, and pairing codes. |
| `src/main.rs` | The `pair` command. |
| `src/i18n.rs` | The text of the page in English and Danish. |
| `web/logic.test.mjs`, `web/app.test.mjs` | New. Tests of the page's pure functions and of the page script against a stand-in document, run with `node --test`. |
| `web/browser-check.mjs` | New. Drives the page in headless Chrome against a local server. Not part of `cargo test`. |
| `deploy/azure/check-local.mjs`, `deploy/azure/deploy.ps1` | New: runs the Azure Functions host on the computer with `husker serve` behind it. Changed: the closing hint names `husker pair`. |
| `README.md`, `docs/online.md`, `docs/development.md`, `AGENTS.md` | Documentation. |
