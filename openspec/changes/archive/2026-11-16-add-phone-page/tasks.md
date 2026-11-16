## 1. Specification

- [x] 1.1 Write the proposal, design, the `phone-page` spec, and the `command-line` and `online-server` deltas. Run `openspec validate add-phone-page --strict`.
- [x] 1.2 Bring the spec, the design, and the proposal in line with the changes that the browser check and the review led to.

## 2. JSON API

- [x] 2.1 Add `src/online/api.rs`: `GET /api/view` for the three views with groups from `planning::group_rows`, `POST /api/tasks`, `PATCH /api/tasks/ID`, the `today` rule, and the error codes with messages in the request's language. Tests in `src/online/tests.rs` cover the answer of each view against the terminal's headings, pagination, each write, each error code, a repeated add, a stale version, and a status change without a version after a device changed the title.
- [x] 2.2 Add the session cookie and pairing codes to `src/online/api.rs` and `src/online/http.rs`: `POST`, `GET`, and `DELETE /api/session`, `POST /api/pairings`, the `X-Husker` rule, and the cookie attributes. Tests cover a code used twice, an expired code, the limit of 16 codes, the cookie on `/api` and not on `/mcp`, and a change without the header.
- [x] 2.3 Check the caller before the request: the access token, then the `X-Husker` rule, then the body, with a body limit of 512 KiB. Answer a wrong method and an unreadable task ID with the JSON error object. Read the cookie header as bytes. Tests cover the order, the limit, both answers, and a cookie next to one with a letter outside ASCII.
- [x] 2.4 Answer a change with the row of the task and, unless the request says `view=none`, the view. Count each group of History and Trash before the page is cut. Add `session` to a view. Tests cover `view=none`, the row, a stale version on a status change, and the counts of 150 tasks closed on one day across pages.
- [x] 2.5 Keep one pairing code per access token. Answer a full sync file with `file_full` and a change the upload check stopped with `server_error`. Give `invalid_today` a message for a person in both languages. Tests cover 40 codes of one access token and the date rule at one and two days from the server's date.

## 3. Page

- [x] 3.1 Add `web/index.html`, `web/app.css`, `web/app.js`, `web/logic.js`, `web/sw.js`, `web/manifest.webmanifest`, and three icons, and embed them in `src/online/assets.rs` with the content security policy. A test requests each file and checks its content type and that `/` needs no access token and no storage request.
- [x] 3.2 Add the page text to `src/i18n.rs` in English and Danish and serve it as `/strings.json`. A test checks that every key `web/app.js` uses is in both languages, that the page uses every key, and that the page files contain no `innerHTML`.
- [x] 3.3 Add `web/logic.test.mjs` for the pure functions of `web/logic.js`: the local date around midnight, reading a pairing code, the list filter with new counts, patching a row and counting the lists again, appending a page, a kept view of another shape, and the language choice. `node --test web/logic.test.mjs` passes.
- [x] 3.4 Add `web/app.test.mjs`: the page script against a stand-in document, with answers that the test holds back, refuses, or drops. `node --test web/app.test.mjs` passes.
- [x] 3.5 Write a page version into `/sw.js` and keep the page files of one version. Start the page again when the worker of a new release takes over and nothing is typed or under way. Tests cover the version in the served script, the file list of the worker, and the restart.
- [x] 3.6 Show views in the order of their requests: drop late answers, drop a view that was read before a confirmed change, keep the values of a row with a change under way, restore only what a failed change set, and show nothing after signing out.
- [x] 3.7 Send from the sheet what the user changed: only changed fields, the version with text only, a button with the changed fields in one request, the date field with Save, the same in-place behaviour for the checkbox and the status buttons, the warning on the close button, the rebase after a stale answer, and a message when a save fails after the sheet closed.
- [x] 3.8 Keep a new task on screen and on the phone until the server has it: empty the field at once, show the task above the groups, mark it when no answer came, keep it through a reload and through a server error, and send it again on a tap or when the server answers.
- [x] 3.9 Ask the server whether the phone is paired, and ask the user before a pairing link replaces a pairing. Leave a paired phone paired after a refused code. Finish a sign-out later when it got no answer. Name a lost connection on a read-only phone. Show a text with the status for an error that is not the server's. Lift the sheets above the keyboard. Name the controls and announce the messages.
- [x] 3.10 Count the sessions of the page and drop the answers of an earlier one. Compare the fields of the sheet with what they showed for the task. Lock the fields and buttons of the sheet while its request runs, and keep the close button. Give every request a time limit of 30 seconds.

## 4. Pairing command

- [x] 4.1 Add `husker pair` in `src/main.rs` with the `qrcode` dependency. Tests in `tests/online.rs` cover the link for redirected output, `--json`, a server with another key, a server that cannot be reached, and a full pairing: the code from `husker pair` exchanged for a cookie that reads a view.
- [x] 4.2 Draw the QR code black on white, and read the drawn code back in a test with the `rqrr` test dependency. Store `--url` after the server answered. Write the token ID to stderr when stdout is redirected. Check the server's certificate against the certificate store of the computer. A test covers a server that redirects: the address in the redirect gets no request.

## 5. Documentation and release

- [x] 5.1 Add the phone sections to `docs/online.md` and update `README.md`, `docs/development.md`, and `AGENTS.md`.
- [x] 5.2 Bump `Cargo.toml` to 0.18.0.

## 6. Verification

- [x] 6.1 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `node --test web/logic.test.mjs`, `node --test web/app.test.mjs`, and a locked release build. Two sync tests that fail now and then on unchanged code failed here too. `docs/development.md` has the results.
- [x] 6.2 Add `web/browser-check.mjs` and run it: `husker serve` against the local test double, with the page in headless Chrome at phone size. It pairs by link and by typed code, adds, closes, reopens, plans, moves, edits, cancels, uses Trash with Undo, opens History and Trash, changes the language and the colors, filters by list, adds a task while the server is stopped, signs out, reads the names of the controls from the accessibility tree, and checks that a computer receives the changes.
- [x] 6.3 Add `deploy/azure/check-local.mjs` and run it: the Azure Functions host of Core Tools on this computer with the deployed `host.json` and `function.json`, and `husker serve` behind it. It checks what the host forwards, the page files, the cookie, each method, and MCP.
- [x] 6.4 Delegate a code review and a security review of the API, the cookie, pairing, and the page, address the findings, and rerun the affected checks. Two reviews ran, and the findings of both are worked in.
- [x] 6.5 Remove one guard at a time from a copy of `web/app.js` and run the page tests. 75 of 76 removals make a test fail. The last guard has a second one behind it.

## 7. Phone check

- [x] 7.1 Deploy, run `husker pair`, and scan the code with the phone. Add the page to the home screen and check that the installed page is paired, or pair it with a typed code.
- [x] 7.2 On the phone: reopen the page after the server stopped, and time the first refresh. Add five tasks in a row. Tap five checkboxes quickly. Use each sheet action. Type a description and check that Save stays above the keyboard. Choose a date in the date field and check that the sheet stays open until Save. Undo a move to Trash. Turn on airplane mode, add a task, turn it off, and check that the task goes out. Check dark mode and Danish. The owner confirmed on November 16, 2026 that the page works on the phone and does what it should. No timing or result per step was recorded.
- [x] 7.3 Archive the change and strictly validate all canonical capabilities.
