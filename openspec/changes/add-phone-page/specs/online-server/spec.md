## ADDED Requirements

### Requirement: The session cookie is an access token

The online server SHALL accept the access token of an API request from the cookie `husker_session` when the request has no `Authorization` header.
The cookie holds an access token, and every rule for access tokens applies to it: the checks of the requirement `Access tokens are signed on the user's computer`, the list scope, and the write permission.
The paths under `/mcp` never read the cookie. They need the `Authorization` header.

A request that changes data through the cookie SHALL carry the header `X-Husker: 1`. That covers every request under `/api` whose method is not `GET` or `HEAD`, except a request with an `Authorization` header.
Without the header, the online server answers 403 with the code `forbidden` and changes nothing.
Another web page cannot send that header to the online server, and a browser does not send the cookie with a request from another site.

A request under `/api` without a valid access token answers 401 with a JSON error object, without a request to the storage that holds the sync file. The access token is checked before the `X-Husker` header, and both before the request body is read.
`POST /api/session`, `DELETE /api/session`, and the page files need no access token. The two session requests need the `X-Husker` header or an `Authorization` header.

#### Scenario: Read tasks with the cookie

- **WHEN** a browser sends `GET /api/view` with a valid `husker_session` cookie and no `Authorization` header
- **THEN** the online server answers the view for the access of that access token

#### Scenario: Refuse a change without the header

- **WHEN** a browser sends `PATCH /api/tasks/ID` with a valid cookie and without `X-Husker`
- **THEN** the online server answers 403 with the code `forbidden` and the sync file is unchanged

#### Scenario: Refuse a stranger before the header counts

- **WHEN** a request without an access token and without `X-Husker` posts to `/api/tasks`
- **THEN** the online server answers 401, not 403

#### Scenario: Ignore the cookie for MCP

- **WHEN** a client posts to `/mcp` with a valid `husker_session` cookie and no `Authorization` header
- **THEN** the online server answers 401
