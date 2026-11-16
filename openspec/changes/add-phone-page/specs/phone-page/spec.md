## ADDED Requirements

### Requirement: The online server serves the page files

The online server SHALL serve the phone page at `/` and its files without an access token.

| Path | Content |
| --- | --- |
| `/` | The page, as HTML. |
| `/app.css`, `/app.js`, `/logic.js` | The style sheet and the two scripts. |
| `/sw.js` | The service worker. |
| `/manifest.webmanifest` | The web app manifest. |
| `/icon-192.png`, `/icon-512.png`, `/apple-touch-icon.png` | The icons. |
| `/strings.json` | The text of the page in English and Danish. |

The files hold no task data. The page loads no file from another server.
The answer for `/` carries a `Content-Security-Policy` header that allows scripts, styles, images, and connections from the online server only, and forbids framing by other pages.
The page writes task text into the document as text and never as markup.

#### Scenario: Open the page before pairing

- **WHEN** a browser without a session cookie requests `/`
- **THEN** the online server answers 200 with the page
- **AND** the storage that holds the sync file receives no request

### Requirement: The service worker keeps one version of the page files

The service worker at `/sw.js` SHALL keep the page files of one page version and answer requests for page files from them.
The page version is a value that changes when a page file, a text of the page, or the content security policy changes. The online server writes it into `/sw.js`, so a release with a changed page changes `/sw.js`.
A service worker fetches every page file when it installs, into a cache named after its page version. It takes over only after all files arrived, and then removes the caches of other page versions.
The service worker never answers and never keeps a request under `/api` or `/mcp`.
When the service worker of a new release takes over an open page, the page starts again with the new files. It waits while a text field holds text, a sheet or the menu is open, a change is under way, a pairing code is checked, or the Undo message shows.
A page that becomes visible again asks the browser to look for a new `/sw.js`.

#### Scenario: Open the page while the server is away

- **WHEN** the page was opened once, and the user opens it again while the online server cannot be reached
- **THEN** the page files come from the service worker, and the page shows the kept view

#### Scenario: Receive a new release

- **WHEN** the online server serves a release with a changed page file, and the user opens the page
- **THEN** the page opens with the kept files of the old release
- **AND** after the new service worker fetched every file, the page starts again with the files of the new release, all of the same page version

#### Scenario: Keep the old release when a file does not arrive

- **WHEN** a new service worker installs and one page file cannot be fetched
- **THEN** the old service worker keeps answering with the files of the old release

#### Scenario: Do not interrupt typing

- **WHEN** the service worker of a new release takes over while the field for a new task holds text
- **THEN** the page does not start again until the field is empty and nothing else is under way

### Requirement: The page text comes from the catalog

The online server SHALL answer `GET /strings.json` with a JSON object that has the members `en` and `da`. Each member maps every text key of the page to its text in that language.
The texts come from the Husker catalog, which also holds the terminal's text. The page holds no text of its own except the name Husker and the pattern `XXXX-XXXX-XXXX` of a pairing code.
The page shows Danish when the browser's language starts with `da`, and English otherwise. A language choice in the page's menu overrides the browser's language and is kept on the phone.
Headings, date labels, and error messages from the API follow the `lang` parameter of the request, which is `en` or `da`. Another value counts as `en`.

#### Scenario: Show Danish on a Danish phone

- **WHEN** the browser's language is `da-DK` and the user made no choice
- **THEN** the page requests views with `lang=da` and shows the Danish heading `I dag` for today's group

### Requirement: A view returns ready groups

The online server SHALL answer `GET /api/view` with the rows of one view, grouped under the headings the terminal shows for that view.

| Parameter | Value |
| --- | --- |
| `view` | `all`, `history`, or `trash`. The default is `all`. |
| `today` | The phone's local date as `YYYY-MM-DD`. The requirement `The phone supplies the date` defines it. |
| `lang` | `en` or `da`. |
| `offset`, `limit` | For `history` and `trash` only. `offset` starts at 0. `limit` is from 1 through 200 and 100 by default. |

| View | Rows | Grouping |
| --- | --- | --- |
| `all` | The rows of the terminal's All tasks view without a filter: open tasks, and tasks done or cancelled today. | By planned date, then by status, as the terminal's All tasks view. |
| `history` | Done and cancelled tasks outside Trash, newest first. | By the day the task was closed, as the terminal's History view. |
| `trash` | Tasks in Trash. | One group without a label. |

The planning-search spec owns the order and the date groups. The terminal-interface spec owns the heading text and the rule for when a row shows its planned date.
The access of the request limits the rows, as the online-server requirement `A token limits lists and writes` defines.

The answer is a JSON object with these members.

| Member | Value |
| --- | --- |
| `view`, `today` | The view and the date the answer was built for. |
| `write` | True when the access allows changes. |
| `session` | The access token of the request: `id`, `name`, `expires` (the expiry date), `write`, and `list`. |
| `lists` | The accessible lists in display order. Each has `id`, `name`, `count` (open tasks outside Trash), and `default` (true for the default list). |
| `suggested_list` | The `id` of the list a new task gets by default: the list of the newest task outside Trash, or else the first list. Null when no list is accessible. |
| `trash_count` | The number of accessible tasks in Trash. |
| `total` | The number of rows of the view before `offset` and `limit`. |
| `next_offset` | The `offset` of the next page, or null. Always null for `all`. |
| `groups` | The groups that have a row on this page, in order. Each has `key`, `label` (null in Trash), `count`, and `rows`. |

The `count` of a group is the number of rows of the whole group. A page of `history` or `trash` can hold a part of a group, and the group still has the count of all its rows. A group that continues on the next page appears there with the same `key` and the same `count`.

A row has the members of an online task (`id`, `list_id`, `title`, `description`, `due`, `status`, `status_changed_on`, `status_changed_at`, `deleted`, `position`, `version`) and three more.

| Member | Value |
| --- | --- |
| `list_name` | The name of the task's list. |
| `due_label` | The planned date as the terminal writes it, for example `2026-11-20 Fri W47`, or null. |
| `show_due` | True when the row shows its planned date because its heading does not. |

A group `key` is a stable English word or a date: `overdue`, `today`, `tomorrow`, a `YYYY-MM-DD` date, `undated`, `done_today`, `cancelled_today`, `done`, `cancelled`, `unknown`, or `trash`.

The newest task is the one a computer would call newest. When two computers each created a task before they synced with each other, the sync file cannot tell which is newer, and `suggested_list` can differ from the list that one of those computers suggests.

#### Scenario: Group All tasks like the terminal

- **WHEN** the sync file holds an open task planned for 2026-11-10 and one planned for 2026-11-15, and the page requests `view=all&today=2026-11-15&lang=en`
- **THEN** the answer has a group with `key` `overdue` and `label` `Earlier`, then a group with `key` `today` and `label` `Today - 2026-11-15 Sun W46`
- **AND** each group has `count` 1, and the row under Earlier has `show_due` true

#### Scenario: Page through History

- **WHEN** 150 tasks were closed on one day and the page requests `view=history` without `offset` and `limit`
- **THEN** the answer holds 100 rows in one group, `total` 150, and `next_offset` 100
- **AND** the group has `count` 150

#### Scenario: Continue a group on the next page

- **WHEN** the page then requests `view=history&offset=100`
- **THEN** the answer holds the other 50 rows in a group with the same `key` and `count` 150, and `next_offset` is null

### Requirement: The phone supplies the date

The online server SHALL use the `today` parameter of an API request as the current date for groups, planned date presets, and the status date of a change.
`today` is valid when it is a `YYYY-MM-DD` date at most one day before or after the online server's UTC date. Another value fails with status 400 and the code `invalid_today`. The message tells a person to check the date of the device, in the language of the request.
Without `today`, the online server uses the date in its configured time zone, as the online-mcp requirement `Dates follow the configured time zone` defines.

#### Scenario: Close a task late in the evening

- **WHEN** the phone's local date is 2026-11-16, the UTC date is 2026-11-15, and the user closes a task
- **THEN** the task's `status_changed_on` is 2026-11-16

#### Scenario: Refuse a date from a wrong clock

- **WHEN** a request carries `today=2026-11-17` while the UTC date is 2026-11-15
- **THEN** the online server answers 400 with the code `invalid_today`

### Requirement: The page adds a task

The online server SHALL create a task for `POST /api/tasks` with a JSON body of these members.

| Member | Required | Value |
| --- | --- | --- |
| `id` | Yes | The `uid` of the new task: 32 lowercase hexadecimal characters that the page takes from 16 random bytes. |
| `title` | Yes | The title. |
| `list_id` | No | The `id` of the list. Without it, the task goes to the default list of the access. |
| `description` | No | The description. |
| `due` | No | A planned date as `YYYY-MM-DD`. |
| `preset` | No | `today`, `tomorrow`, or `next_monday`, resolved from the request's `today` as the planning-search requirement `Resolve common due-date presets` defines. Not allowed together with `due`. |

The answer has the members `task` and `view`.
`task` is the row of the new task as a view holds it, without `show_due`: the members of an online task, `list_name`, and `due_label`.
`view` is the view the request's `view` parameter names, built after the change. With `view=none` the answer holds no `view`.
A second request with the same `id`, title, description, list, and planned date returns the task that exists and creates nothing. A request with an `id` that another task has fails with the code `task_exists`.
The online-server requirements `One upload changes one list or task` and `Online changes follow the task storage rules` own the upload and the task rules.

#### Scenario: Add a task for tomorrow

- **WHEN** the page posts `{"id": "0123456789abcdef0123456789abcdef", "title": "Call Ann", "preset": "tomorrow"}` with `today=2026-11-15`
- **THEN** the sync file holds an open task `Call Ann` planned for 2026-11-16 in the default list

#### Scenario: Send the same task twice

- **WHEN** the page posts the same body twice, because the first answer never arrived
- **THEN** the sync file holds one task, and both answers return it

### Requirement: The page changes a task

The online server SHALL change one task for `PATCH /api/tasks/ID`, where `ID` is the task's `id`, with a JSON body that holds at least one of these members.

| Member | Value |
| --- | --- |
| `title`, `description` | New text. A request with either needs `version`. |
| `version` | The `version` the page last read. Required for `title` and `description`, optional otherwise. |
| `list_id` | The `id` of the list to move the task to. |
| `due` | A planned date as `YYYY-MM-DD`. |
| `clear_due` | True removes the planned date. |
| `preset` | `today`, `tomorrow`, or `next_monday`. |
| `status` | `open`, `done`, or `cancelled`. |
| `deleted` | True moves the task to Trash. False restores it. |

At most one of `due`, `clear_due`, and `preset` is allowed.
The online server applies all members in one upload: first the text, the list, and the planned date, then the status, then Trash.
A supplied `version` that differs from the task's current `version` fails with status 409 and the code `stale_version`, and nothing changes. That holds for every member, not only for text.
A request with `title` or `description` and without `version` fails with status 400 and the code `version_required`.
Without `version`, the change applies to the newest sync file. A status, a planned date, a list, or Trash can therefore change although a computer changed the task's text a moment ago.
The answer has the members `task` and `view`, as for a new task. With `view=none` the answer holds no `view`.

#### Scenario: Close a task that a computer just renamed

- **WHEN** a computer syncs a new title for a task, and the page then sends `{"status": "done"}` for it without `version`
- **THEN** the task is done and keeps the computer's title

#### Scenario: Refuse an edit of stale text

- **WHEN** a computer syncs a new title for a task, and the page then sends a new description with the `version` it read before
- **THEN** the online server answers 409 with the code `stale_version` and the task keeps the computer's title and its old description

#### Scenario: Move and plan in one request

- **WHEN** the page sends `{"list_id": ID, "preset": "today"}` for a task
- **THEN** the sync file changes once, and the task is in that list and planned for the request's date

#### Scenario: Answer a tap with the row alone

- **WHEN** the page sends `{"status": "done"}` with `view=none`
- **THEN** the answer holds `task` with the status `done` and holds no `view`

### Requirement: The API checks the caller before it reads a request

The online server SHALL check an API request in this order, and stop at the first check that fails: the access token, then the `X-Husker` rule of the online-server requirement `The session cookie is an access token`, then the request itself.
The online server reads the request body only after the first two checks passed. A request body can have at most 512 KiB.
A request to a known path with a method that the path does not accept fails with the code `method_not_allowed`. A task `ID` that is not text, such as a broken percent code, names no task and fails with the code `task_not_found`. Both answers are JSON error objects.
The session cookie is found among other cookies of the browser, also when another cookie has a value outside ASCII. The first cookie with the name `husker_session` counts.

#### Scenario: Refuse a stranger before reading the body

- **WHEN** a request without an access token posts a body of 600 KiB to `/api/tasks`
- **THEN** the online server answers 401 with the code `unauthorized`, and not 413

#### Scenario: Refuse a session without the header next

- **WHEN** a request with a valid session cookie and without `X-Husker` posts to `/api/tasks`
- **THEN** the online server answers 403 with the code `forbidden`

#### Scenario: Refuse a body over the limit

- **WHEN** a request with a valid access token posts a body of 600 KiB to `/api/tasks`
- **THEN** the online server answers 413 with the code `too_long`, and the storage that holds the sync file receives no request

#### Scenario: Find the cookie next to another one

- **WHEN** a browser sends `Cookie: theme=blå; husker_session=TOKEN` with a valid access token
- **THEN** the online server answers the request for the access of that access token

### Requirement: API failures have a code and a message

The online server SHALL answer a failed API request with a JSON error object: one member `error` with the members `code` and `message`.
`code` is a stable English word. `message` is a sentence in the language of the request's `lang` parameter where the catalog has a translation, and in English otherwise.

| Status | `code` | Failure |
| --- | --- | --- |
| 400 | `invalid_request` | The body is not valid JSON, has an unknown member, or asks for no change. Or a parameter in the address is not valid. |
| 400 | `invalid_today` | `today` is not a valid date for the request. |
| 400 | `invalid_date` | `due` is not a date, or `preset` is unknown. |
| 400 | `empty_title` | The title is empty. |
| 400 | `too_long` | A text is longer than the online server accepts. |
| 400 | `version_required` | A text change has no `version`. |
| 401 | `unauthorized` | The request has no valid access token. |
| 403 | `forbidden` | A change through the session cookie lacks the `X-Husker` header, or the request comes from another origin. |
| 403 | `read_only` | The access allows no changes. |
| 403 | `restricted` | The change would leave the list scope. |
| 404 | `task_not_found` | No accessible task has that `id`. |
| 404 | `unknown_list` | No list has that `id`. |
| 404 | `unknown_code` | The pairing code is unknown, used, or expired. |
| 405 | `method_not_allowed` | The path does not accept the method. |
| 409 | `stale_version` | The `version` is not current. |
| 409 | `task_exists` | Another task has the `id` of a new task. |
| 413 | `too_long` | The request body has more than 512 KiB. |
| 500 | `server_error` | The online server stopped a change that it built wrongly, or failed in another way. |
| 502 | `storage_unavailable` | The storage that holds the sync file failed. |
| 503 | `busy` | The sync file kept changing, or does not exist. |
| 507 | `file_full` | The sync file cannot hold the change: it would be larger than the online server uploads, or it has no room for another record. |

#### Scenario: Report an empty title in Danish

- **WHEN** the page posts a task whose title is spaces, with `lang=da`
- **THEN** the online server answers 400 with the code `empty_title` and a Danish message

#### Scenario: Answer a wrong method with the error object

- **WHEN** a client with a valid access token sends `PUT /api/view`
- **THEN** the online server answers 405 with the code `method_not_allowed`

### Requirement: Pairing codes hand an access token to a phone once

The online server SHALL store the access token of a `POST /api/pairings` request under a new pairing code and answer with the members `code` and `expires_in`, the lifetime in seconds.
The request carries the access token in the `Authorization` header. The online server stores exactly that access token.
A pairing code is 12 characters from the digits and the uppercase letters without `I`, `L`, `O`, and `U`, taken from random bytes.
A pairing code works once and for 300 seconds. The online server keeps at most 16 pairing codes and drops the oldest for a new one.
One access token has one pairing code at a time. A new pairing code for an access token replaces the older pairing code of that access token, so a caller with one access token cannot push out the pairing codes of other access tokens.
Pairing codes live in memory only. A restart of the online server loses them.

`POST /api/session` with the body `{"code": CODE}` exchanges a pairing code for a session, as the requirement `The session cookie holds the access token` defines.
The online server reads a code without regard to letter case, spaces, and hyphens, and reads `O` as `0` and `I` and `L` as `1`.
An unknown, used, or expired code fails with status 404 and the code `unknown_code`.

#### Scenario: Use a pairing code once

- **WHEN** `husker pair` created a pairing code and the page sends it to `POST /api/session` twice
- **THEN** the first request gets a session cookie and the second answers 404 with the code `unknown_code`

#### Scenario: Let a pairing code expire

- **WHEN** the page sends a pairing code 301 seconds after it was created
- **THEN** the online server answers 404 with the code `unknown_code`

#### Scenario: Keep the code of another access token

- **WHEN** one access token creates a pairing code, and another access token then creates 40 pairing codes
- **THEN** the pairing code of the first access token still works

### Requirement: The session cookie holds the access token

The online server SHALL answer a successful `POST /api/session` with a `Set-Cookie` header for the cookie `husker_session`, whose value is the access token.
The body of the request is `{"code": CODE}` with a pairing code, or `{"token": TOKEN}` with an access token.

| Cookie attribute | Value |
| --- | --- |
| `HttpOnly` | Always. Page scripts cannot read the cookie. |
| `SameSite` | `Strict`. |
| `Path` | `/api`. |
| `Max-Age` | The seconds until the access token expires. |
| `Secure` | When `HUSKER_ONLINE_ORIGIN` uses `https`. |

The answer body has the members `id`, `name`, `expires` (the expiry date), `write`, and `list` of the access token.
`GET /api/session` returns the same members for the current session, or answers 401.
`DELETE /api/session` answers 204 with a `Set-Cookie` header that removes the cookie.
The online-server requirement `The session cookie is an access token` defines how API requests use the cookie.

#### Scenario: Pair and read tasks

- **WHEN** the page exchanges a pairing code and then requests `GET /api/view` with the cookie
- **THEN** the online server answers the view for the access of the paired access token

#### Scenario: Sign out

- **WHEN** the page sends `DELETE /api/session` and then requests `GET /api/view`
- **THEN** the second request answers 401

### Requirement: The page shows All tasks first and changes tasks with a tap

The phone page SHALL open on the view All tasks and offer the actions in this table.

| Action | How |
| --- | --- |
| Filter by list | A list selector above the rows. The page hides the rows of other lists and counts each group again. |
| Add a task | A text field and a list selector above the rows. The list selector starts on the filtered list, or else on `suggested_list`. A list the user chooses there stays chosen until the filter or the view changes. |
| Close or reopen a task | A tap on the row's checkbox, or a button in the sheet. |
| Open a task | A tap on the row. A sheet shows the title, the description, the planned date, and the list. The sheet opens without a text field in focus, so the phone's keyboard stays closed. |
| Set the planned date | In the sheet: a button for Today, Tomorrow, Next Monday, or no date. Or a date in the date field, sent with Save. |
| Move to another list | In the sheet: a list selector. |
| Edit the text | In the sheet: the title and description fields and Save. |
| Cancel a task | A button in the sheet. |
| Move a task to Trash | A button in the sheet. The page then offers Undo for a few seconds. |
| See History and Trash | In the menu. Trash restores a task. History opens a task's sheet. |
| Show more rows | In History and Trash, a button under the rows requests the next page. |
| Refresh | A button. The page also refreshes when it becomes visible again and when the phone reports a connection again. |
| Change the language, sign out | In the menu. The menu also names the pairing: the name, the expiry date, and the ID of the access token. |

The page shows a status change in All tasks before the request returns. When the request fails, the page restores the row and shows a message. Every other change dims its row until the answer arrives.
In All tasks, a task that the user closes, cancels, or reopens stays in its row until the next refresh, whether the user tapped the checkbox or a button in the sheet. In History, a reopened task leaves the view at once.
A change that keeps the row in its place requests `view=none` and puts the `task` of the answer into the row. The page then counts the open tasks of each list again.
A row with a change under way is dimmed. The page accepts no second change of that row and does not open its sheet until the answer arrives.
The page keeps the last answer for All tasks on the phone and shows it at once when it opens. It then requests the current view. A kept view that the page cannot draw is dropped.
In History and Trash, a refresh and a change request as many rows as the page shows, up to 200, so the rows that Show more loaded stay.
When the access allows no changes, the page shows the rows, disables the checkboxes, and hides the other controls that change tasks.
When a change gets no answer, the page restores the row, says that the change may not be saved, and requests the view again.

#### Scenario: Close a task with one tap

- **WHEN** the user taps the checkbox of an open task
- **THEN** the row shows as done at once, and the page sends `{"status": "done"}` for the task with `view=none`
- **AND** the row stays under its heading until the next refresh

#### Scenario: Close a task from its sheet

- **WHEN** the user opens a task in All tasks and taps the button that marks it done
- **THEN** the sheet closes, and the row shows as done under the heading it had

#### Scenario: Undo a move to Trash

- **WHEN** the user moves a task to Trash and taps Undo
- **THEN** the page sends `{"deleted": false}` for the task and the row returns

#### Scenario: Show the last view while the server starts

- **WHEN** the user opens the page after a pause in which the online server stopped
- **THEN** the page shows the kept view at once and replaces it when the current view arrives

#### Scenario: Keep the rows of History after a refresh

- **WHEN** History shows 130 rows after Show more, and the user refreshes
- **THEN** the page requests `limit=130` and shows 130 rows

### Requirement: The sheet sends what the user changed

The phone page SHALL send, from a task's sheet, only the fields that the user changed: the title, the description, and the date field. Save sends them. The Enter key in the title field saves too.
A field counts as changed when it holds another text than it showed for the task. A browser can show a text with other line breaks than the task has, and that difference is no change.
A request with a title or a description carries the `version` of the task. A request with the date field alone carries no `version`.
A button or the list selector in the sheet sends its action together with the changed fields, in one request. A tapped planned date wins over a date in the date field.
The sheet stays open until the online server accepted changed fields. With no changed field, the sheet closes at once and the action follows.
While a request of the sheet is under way, Save and the buttons of the sheet send nothing and show that they wait, and the fields take no text. The button that closes the sheet still works. The row of the task waits too, as a row with a change under way does.
The button that closes the sheet says that it discards changes while a field differs from the task. While a request of the sheet is under way it says only that it closes, because the changes are on their way.
When the online server refuses the text with the code `stale_version`, the page reads the task again, keeps the fields the user changed, and shows the task as it is now in the other fields and on the status button. The next Save carries the current `version` and only the fields the user changed.
When the request of a button fails and the sheet stays open, the next Save sends the action of that button again with the changed fields, so the tap is not lost. The list selector then shows the list that the Save moves the task to.
A newer choice replaces an action that waits for Save: another button, a day in the date field in place of a tapped date, and the list of the task in place of a waiting move.
When a request of the sheet gets no answer, the page reads the task again, as after `stale_version`, because the online server may have saved the change. A Save with nothing left to send closes the sheet.
When the task is no longer in the view after it was read again, or the online server answers `task_not_found`, the sheet ends: it shows a message, keeps the text in sight, and offers only the button that closes it.
When a request of the sheet fails after the user closed the sheet, the page shows the failure in a message that names the task.

#### Scenario: Save the title with the Enter key

- **WHEN** the user changes the title in the sheet and presses Enter in the title field
- **THEN** the page sends the title with the task's `version`, without the description, and closes the sheet

#### Scenario: Plan a task for a date

- **WHEN** the user sets the date field of the sheet to 2026-12-24 and taps Save
- **THEN** the page sends `{"due": "2026-12-24"}` without `version`

#### Scenario: Tap a button after typing

- **WHEN** the user changes the title in the sheet and taps Today
- **THEN** the page sends the title, `preset` `today`, and the `version` in one request
- **AND** the sheet stays open until the online server answered

#### Scenario: Save again after another computer changed the task

- **WHEN** a computer renamed a task, the user typed a description for it and tapped Save, and the online server answered `stale_version`
- **THEN** the sheet shows the computer's title and the user's description
- **AND** the next Save sends the description with the current `version` and does not send the title

#### Scenario: Save again after a button was refused

- **WHEN** the user changed the title and tapped Today, and the online server answered `stale_version`
- **THEN** the sheet stays open with the message
- **AND** the next Save sends the title, `preset` `today`, and the current `version`

#### Scenario: Open a task that an agent wrote with other line breaks

- **WHEN** a task has the description `Line one\r\nLine two`, and the user opens its sheet and taps Today without typing
- **THEN** the page sends `{"preset": "today"}` alone, without a description and without `version`

#### Scenario: Wait for a slow save

- **WHEN** the user taps Save and the online server takes ten seconds to answer
- **THEN** the fields take no text and the buttons do not react for those ten seconds
- **AND** the button that closes the sheet closes it

#### Scenario: Lose the answer to a save

- **WHEN** the online server saves a new title but its answer does not arrive, and the user taps Save again
- **THEN** the sheet closes without a second request and without a message about a change from elsewhere

#### Scenario: Edit a task that a computer moved to Trash

- **WHEN** the user saves a description, the online server answers `stale_version`, and the task is no longer in the view
- **THEN** the sheet says that the task is no longer in this view and that the changes may not be saved
- **AND** the sheet shows the typed description and no Save button

#### Scenario: Close the sheet before a save failed

- **WHEN** the user taps Save, closes the sheet, and the request then fails
- **THEN** the page shows a message with the task's title and the failure

### Requirement: The page keeps a new task on screen until the server has it

The phone page SHALL empty the field for a new task when it sends the task, and show the task in a row above the groups until the answer arrives.
The page repeats the request once when it gets no answer. The task has the same `id` in every request.
A task that still got no answer stays on screen, marked as not sent. The page sends it again when the user taps it, and when the online server answers a request for a view or confirms a change.
The page keeps the tasks that the online server does not have yet on the phone. After a reload they show as not sent and go out in the same way. Signing out and an answer of 401 remove them from the phone.
When the online server refuses a task, the page removes its row and shows the message of the online server. The title returns to the field. When the field holds the next title already, the message names the refused title.
A refusal is an answer with a status from 400 through 499, except 408 and 429, and with the error object of the online server. Any other failure leaves the task waiting: no answer, a status from 500 on, and an answer that does not hold the task.
The code `task_exists` for a task that waits means that the online server has it, from an earlier request whose answer was lost. The page then removes the row and requests the view.
When two pages of one phone are open, each keeps its own waiting tasks and leaves those of the other in place.

#### Scenario: Type the next task while one is sent

- **WHEN** the user sends a task and the online server takes five seconds to answer
- **THEN** the field is empty at once, and the task shows in a row above the groups until its answer arrives

#### Scenario: Add a task without a connection

- **WHEN** the user sends a task while the online server cannot be reached
- **THEN** the task stays on screen, marked as not sent
- **AND** when the page next receives a view, it sends the task with the same `id`

#### Scenario: Reload the page with a task that waits

- **WHEN** a task got no answer, and the browser unloads the page and loads it again
- **THEN** the task shows as not sent, and the page sends it with the same `id` when the first view arrives

#### Scenario: Keep the tasks while the storage is away

- **WHEN** three tasks wait, and the online server answers each of them with 502
- **THEN** all three stay on screen and on the phone, marked as not sent

#### Scenario: Learn which task was refused

- **WHEN** the online server refuses a task while the user types the next title
- **THEN** the field keeps the typed text, and a message names the refused title and the reason

### Requirement: The page shows views in the order of their requests

The phone page SHALL number every request that returns a view in the order it sends them, and SHALL show a view only when no view of a later request is on screen.
A view whose request was sent before the online server confirmed a change is not shown, whenever it arrives. The online server answers a read without waiting for a write, so such a view can be older than the change. The page requests the current view again instead.
An answer to a change that arrives after a later view, or after the user moved to another view, is not shown. The page requests the current view again instead.
An answer to a request for the view that arrives after a later view, or after the user moved to another view, is dropped.
An answer that does not have the members of a view is not shown, and the screen stays as it is.
When a view arrives while a change of a row is under way, that row keeps the values the user chose.
When a change fails, the page restores only the values that the change set, and only on that row.
A change that keeps its row requests the view again when another change was confirmed since its request was sent.
The page counts its sessions. A sign-out, an answer of 401, and a new pairing each begin a new count. The answer to a request of an earlier count is dropped, whatever it says, and nothing on the page waits for it. The requests that open and close a session are the exception.
While the pairing screen shows, the page keeps no view on the phone and sends no request for a view.

#### Scenario: Refresh while a change is under way

- **WHEN** the user closes a task, and a refresh answers before the change does
- **THEN** the row still shows as closed
- **AND** when the change then fails, the row shows as open and the other rows keep the values of the refresh

#### Scenario: Drop a late answer

- **WHEN** the user refreshes twice and the first answer arrives after the second
- **THEN** the page keeps the view of the second answer

#### Scenario: Drop a view that was read before a change

- **WHEN** a refresh is sent, the user then closes a task, the change is confirmed, and the refresh then answers with the task still open
- **THEN** the row stays closed, and the page requests the view once more

#### Scenario: Settle two overlapping changes

- **WHEN** two changes with a view in their answers overlap
- **THEN** the page shows the view of the first answer and requests the current view again after the second

#### Scenario: Move to another view while a change is under way

- **WHEN** the user moves a task to Trash in All tasks, opens History, and the answer to the move then arrives
- **THEN** the page does not show the rows of All tasks under History
- **AND** the page requests History again

#### Scenario: Sign out while a request is under way

- **WHEN** the user signs out while a request for the view is under way, and the answer then arrives
- **THEN** the page stays on the pairing screen and keeps no view on the phone

#### Scenario: Pair again while old requests are under way

- **WHEN** the user signs out, pairs again, and an answer of the session before then arrives
- **THEN** the page keeps the view of the new session
- **AND** an answer of 401 for the session before does not show the pairing screen

### Requirement: The page shows that it waits for the server

The phone page SHALL show the task screen at once when it opens with a kept view or with a pairing code, and SHALL mark the refresh button as busy while a request for the view is under way.
Without a kept view, the page shows a loading line in place of the rows until the first view arrives.
When the online server cannot be reached, the page shows a message that it has no connection. With a kept view on screen, the message also says that the page shows saved tasks. The message shows on a phone with read-only access too, in place of the note that the access is read-only.
When an error answer holds no error object of the online server, the page shows a text of its own with the status. Such an answer comes from something in front of the online server.
A request that gets no answer within 30 seconds counts as unanswered.
The page shows kept text at once and requests `/strings.json` in the background. It waits for `/strings.json` only when it has no kept text.

#### Scenario: Open the page for the first time

- **WHEN** the page opens with a session and without a kept view, and the online server takes five seconds to start
- **THEN** the page shows the task screen with a loading line for those five seconds, and then the rows

#### Scenario: Open the page without a connection

- **WHEN** the page opens with a kept view and the online server cannot be reached
- **THEN** the page shows the kept view and the message `No connection to the server. Showing saved tasks.`

#### Scenario: Show an error from a gateway

- **WHEN** a change is answered with status 503 and a body that is not JSON
- **THEN** the page shows `The server answered with an error (503). Try again in a moment.`

### Requirement: The page pairs, stays paired, and signs out

The phone page SHALL show a pairing screen when it has no session: a field for a pairing code and a button.
The page reads a pairing code from the address when it follows `#code=`, both when the page opens and when the address changes while the page is open. The page removes the code from the address.
Before the page uses a pairing code from the address, it asks the online server with `GET /api/session` whether the phone is paired. A phone that is paired is asked whether to pair again, so a link cannot replace a pairing without a question. A refusal leaves the pairing and the code as they are.
When that request gets no answer, the page does not use the code. A phone without a kept view shows the pairing screen with the code in its field, so the user can try again. A typed code that gets no answer stays in the field in the same way.
While a pairing code is checked, a second code is not sent.
When the online server refuses a pairing code, the page requests the view. A phone that is paired already then shows its tasks and the refusal in a message. Another phone shows the pairing screen with the refusal.
An answer of 401 removes the kept view and shows the pairing screen. A phone that was paired is told that it no longer is.
Signing out removes the tasks from the screen and the kept view from the phone at once, and shows the pairing screen. The hidden part of the page holds no row, no text of a task, no name of a list, and no message either. The page then sends `DELETE /api/session`, because only the online server can remove the cookie.
When that request gets no answer, the page keeps a note on the phone and says that the sign-out finishes later. While the note is kept, the page requests no view and asks no question about a pairing. At its next start, and before it uses a pairing code, the page sends the request again. One such request runs at a time, and a pairing code waits for it, so a late answer cannot remove a new cookie.
The pairing after a sign-out opens on All tasks.

#### Scenario: Pair from a link in the open page

- **WHEN** the pairing screen shows and the address of the page changes to end in `#code=` and a pairing code
- **THEN** the page exchanges the code for a session, removes the code from the address, and shows All tasks

#### Scenario: Ask before a link replaces a pairing

- **WHEN** a paired phone opens an address that ends in `#code=` and a pairing code
- **THEN** the page asks whether to pair again
- **AND** after a refusal the page sends no pairing request and shows its tasks

#### Scenario: Ask a paired phone that keeps no tasks

- **WHEN** a paired phone without a kept view opens a pairing link
- **THEN** the page asks whether to pair again, because the online server says that the phone is paired

#### Scenario: Open a pairing link without a connection

- **WHEN** a phone without a kept view opens a pairing link while the online server cannot be reached
- **THEN** the page shows the pairing screen with a message, and the code is in the code field

#### Scenario: Open a used link on a paired phone

- **WHEN** a paired phone opens a pairing link whose code was used, and the user agrees to pair again
- **THEN** the page shows its tasks and a message that the code is wrong, used, or expired
- **AND** the kept view stays on the phone

#### Scenario: Sign out without a connection

- **WHEN** the user signs out while the online server cannot be reached
- **THEN** the page shows the pairing screen and a message that the sign-out finishes later
- **AND** at the next start the page sends `DELETE /api/session` before any other API request, and requests no view while that fails

### Requirement: The page keeps its sheets above the keyboard

The phone page SHALL keep an open sheet, the menu, and its message above the on-screen keyboard of the phone, where the browser reports the visible part of the page.
The page measures the keyboard only while the page is not zoomed.

#### Scenario: Type a description

- **WHEN** the user taps the description field of a sheet and the keyboard covers the lower 320 pixels of the page
- **THEN** the sheet ends 320 pixels above the bottom of the page

### Requirement: The page names its controls and announces its messages

The phone page SHALL give every control without visible text a name for assistive technology, name the sheet after its task and the menu as the menu, and mark its messages so that assistive technology announces them.
The list filter and the list selector for a new task have names of their own.
When the page draws the rows again, the control of a row that had the focus gets the focus back.

#### Scenario: Close a task with a screen reader

- **WHEN** a screen reader user activates the checkbox of a task
- **THEN** the checkbox has a name that says what it does, and the focus stays on the checkbox of that task after the row is drawn again
