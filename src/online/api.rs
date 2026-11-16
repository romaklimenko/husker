//! The JSON API of the phone page: views, task changes, sessions, and pairing codes.
//!
//! Views come from the functions the terminal uses, so the page holds no
//! grouping or date logic. A change goes through `Engine::write_all`, one
//! upload for one task.

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Json,
    extract::{
        FromRequestParts, Path, Query, State,
        rejection::{JsonRejection, PathRejection, QueryRejection},
    },
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{AUTHORIZATION, COOKIE, SET_COOKIE},
        request::Parts,
    },
    response::{IntoResponse, Response},
};
use chrono::{DateTime, NaiveDate, Utc};
use ring::rand::{SecureRandom, SystemRandom};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    access::Claims,
    doc::{Access, Changed, DocError, Op, Stamp},
    engine::{EngineError, storage_key},
    http::{AppState, authenticate, error, unauthorized},
    mcp::stamp_at,
    view::{Projection, TaskView},
};
use crate::{
    i18n::{Language, error_message, tr},
    model::{Status, Task},
    planning::{DuePreset, GroupLayout, date_label, group_rows, suggested_list_id},
    query::{Query as Filter, parse_date, visible_history, visible_tasks},
    sync::{FailureKind, snapshot::Snapshot},
};

/// The cookie that holds the access token of the phone page.
pub const SESSION_COOKIE: &str = "husker_session";

/// How long a pairing code works.
pub const PAIRING_SECONDS: u64 = 300;

/// How many pairing codes the server keeps. A new one drops the oldest.
///
/// One access token holds one code at a time, so a caller with one token fills one place.
const MAX_PAIRINGS: usize = 16;

/// Digits and uppercase letters without I, L, O, and U, which are easy to misread.
const CODE_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Pairing codes and the access tokens they hand out. They live in memory only.
#[derive(Default)]
pub struct Pairings(Mutex<Vec<Pairing>>);

struct Pairing {
    code: String,
    token: String,
    /// The ID of the access token.
    id: String,
    created: Instant,
}

impl Pairings {
    /// Stores an access token under a new random code and returns the code.
    ///
    /// `id` is the ID of the access token. A new code replaces the older code of the same token.
    pub fn create(&self, token: String, id: &str, now: Instant) -> anyhow::Result<String> {
        let mut random = [0_u8; 12];
        SystemRandom::new()
            .fill(&mut random)
            .map_err(|_| anyhow::anyhow!("The system has no random numbers."))?;
        // Five bits of each byte pick a character: 60 random bits in 12 characters.
        let code: String = random
            .iter()
            .map(|byte| char::from(CODE_ALPHABET[usize::from(byte & 31)]))
            .collect();
        let mut pairings = self.0.lock().unwrap_or_else(|lock| lock.into_inner());
        pairings.retain(|pairing| is_live(pairing, now) && pairing.id != id);
        if pairings.len() >= MAX_PAIRINGS {
            pairings.remove(0);
        }
        pairings.push(Pairing {
            code: code.clone(),
            token,
            id: id.to_owned(),
            created: now,
        });
        Ok(code)
    }

    /// Returns the access token of a code and forgets the code. A code works once.
    pub fn take(&self, code: &str, now: Instant) -> Option<String> {
        let code = normalize_code(code);
        let mut pairings = self.0.lock().unwrap_or_else(|lock| lock.into_inner());
        pairings.retain(|pairing| is_live(pairing, now));
        let index = pairings.iter().position(|pairing| pairing.code == code)?;
        Some(pairings.remove(index).token)
    }
}

fn is_live(pairing: &Pairing, now: Instant) -> bool {
    now.saturating_duration_since(pairing.created) <= Duration::from_secs(PAIRING_SECONDS)
}

/// Reads a typed code: any letter case, no spaces or hyphens, and the usual misreadings.
pub fn normalize_code(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace() && *character != '-')
        .map(|character| match character.to_ascii_uppercase() {
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        })
        .collect()
}

/// A failed API request: a status, a stable English code, and a sentence for a person.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request", message)
    }

    #[cfg(test)]
    pub(super) fn parts(&self) -> (StatusCode, &'static str, &str) {
        (self.status, self.code, &self.message)
    }

    /// The failure of a read or write of the sync file, in the language of the request.
    fn from_engine(failure: &EngineError, language: Language) -> Self {
        let sentence = |text: String| error_message(language, &anyhow::anyhow!(text));
        match failure {
            EngineError::Doc(doc) => {
                let (status, code) = match doc {
                    DocError::ReadOnly => (StatusCode::FORBIDDEN, "read_only"),
                    DocError::Restricted => (StatusCode::FORBIDDEN, "restricted"),
                    DocError::EmptyTitle => (StatusCode::BAD_REQUEST, "empty_title"),
                    DocError::TooLong { .. } => (StatusCode::BAD_REQUEST, "too_long"),
                    // The sync file is full. The request was fine.
                    DocError::OutOfRange | DocError::FileTooLarge => {
                        (StatusCode::INSUFFICIENT_STORAGE, "file_full")
                    }
                    // The upload check stopped a change that the server built wrongly.
                    DocError::Guard(_) => (StatusCode::INTERNAL_SERVER_ERROR, "server_error"),
                    DocError::TaskNotFound { .. } => (StatusCode::NOT_FOUND, "task_not_found"),
                    DocError::ListNotFound(_) | DocError::NoAccessibleList => {
                        (StatusCode::NOT_FOUND, "unknown_list")
                    }
                    DocError::Stale(_) => (StatusCode::CONFLICT, "stale_version"),
                    DocError::TaskExists(_) => (StatusCode::CONFLICT, "task_exists"),
                    _ => (StatusCode::BAD_REQUEST, "invalid_request"),
                };
                Self::new(status, code, sentence(doc.to_string()))
            }
            EngineError::Missing => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "busy",
                tr(language, "online_missing"),
            ),
            EngineError::Storage(FailureKind::Busy) => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "busy",
                tr(language, "sync_busy"),
            ),
            EngineError::Storage(kind) => Self::new(
                StatusCode::BAD_GATEWAY,
                "storage_unavailable",
                tr(language, storage_key(*kind)),
            ),
            EngineError::Unknown => Self::new(
                StatusCode::BAD_GATEWAY,
                "storage_unavailable",
                tr(language, "online_unknown"),
            ),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        error(self.status, self.code, &self.message)
    }
}

/// The query parameters every API request can carry.
#[derive(Debug, Default, Deserialize)]
pub struct Params {
    view: Option<String>,
    today: Option<String>,
    lang: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
}

impl Params {
    fn language(&self) -> Language {
        match self.lang.as_deref() {
            Some("da") => Language::Da,
            _ => Language::En,
        }
    }

    fn kind(&self) -> Result<ViewKind, ApiError> {
        match self.view.as_deref() {
            None | Some("all") => Ok(ViewKind::All),
            Some("history") => Ok(ViewKind::History),
            Some("trash") => Ok(ViewKind::Trash),
            Some(_) => Err(ApiError::invalid("view must be all, history, or trash.")),
        }
    }

    /// Whether the answer to a change holds a view. `view=none` asks for the task alone.
    fn wants_view(&self) -> bool {
        self.view.as_deref() != Some("none")
    }

    /// The date and time of the request. The phone's date counts when it is near the server's.
    pub(super) fn stamp(&self, state: &AppState, now: DateTime<Utc>) -> Result<Stamp, ApiError> {
        let Some(text) = &self.today else {
            return Ok(stamp_at(now, state.zone));
        };
        // A person reads this message when the clock of the phone is wrong.
        let invalid = || {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_today",
                tr(self.language(), "online_wrong_date"),
            )
        };
        let today = parse_date(text).map_err(|_| invalid())?;
        if today
            .signed_duration_since(now.date_naive())
            .num_days()
            .abs()
            > 1
        {
            return Err(invalid());
        }
        Ok(Stamp { today, now })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewKind {
    All,
    History,
    Trash,
}

impl ViewKind {
    fn name(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::History => "history",
            Self::Trash => "trash",
        }
    }
}

/// Builds the answer of `GET /api/view` from a snapshot.
pub fn build_view(
    snapshot: &Snapshot,
    access: &Access,
    kind: ViewKind,
    today: NaiveDate,
    language: Language,
    offset: usize,
    limit: usize,
) -> Value {
    let projection = Projection::new(snapshot, access);
    let everything = Filter::default();
    let (visible, layout) = match kind {
        ViewKind::All => (
            visible_tasks(
                &projection.tasks,
                &projection.lists,
                &everything,
                None,
                false,
                today,
            ),
            GroupLayout::Planned,
        ),
        ViewKind::History => (
            visible_history(
                &projection.tasks,
                &projection.lists,
                &everything,
                None,
                today,
            ),
            GroupLayout::History,
        ),
        ViewKind::Trash => (
            visible_tasks(
                &projection.tasks,
                &projection.lists,
                &everything,
                None,
                true,
                today,
            ),
            GroupLayout::Trash,
        ),
    };
    let total = visible.len();
    // All tasks is never cut. History and Trash answer one page of their rows.
    let (start, end) = match kind {
        ViewKind::All => (0, total),
        _ => (offset.min(total), offset.saturating_add(limit).min(total)),
    };
    let next_offset = (end < total).then_some(end);

    // The rows are grouped before the page is cut, so a heading counts its whole group
    // although the page may hold only a part of it.
    let mut position = 0;
    let mut groups: Vec<Value> = Vec::new();
    for group in group_rows(&visible, &|_| false, layout, today, language) {
        let first = position;
        position += group.rows.len();
        let rows: Vec<Value> = group
            .rows
            .iter()
            .enumerate()
            .filter(|(index, _)| (start..end).contains(&(first + index)))
            .map(|(_, row)| {
                let mut value = task_row(&projection, &visible[row.index], language);
                value["show_due"] = row.show_date.into();
                value
            })
            .collect();
        if !rows.is_empty() {
            groups.push(json!({
                "key": group.key,
                "label": group.label,
                "count": group.rows.len(),
                "rows": rows,
            }));
        }
    }

    let lists: Vec<Value> = projection
        .list_views()
        .into_iter()
        .map(|list| {
            json!({
                "id": list.id,
                "name": list.name,
                "count": list.count,
                "default": list.id == snapshot.default_list,
            })
        })
        .collect();
    let suggested = suggested_list_id(&projection.tasks, &projection.lists)
        .and_then(|number| projection.list_uid(number));

    json!({
        "view": kind.name(),
        "today": today,
        "write": !access.read_only,
        "lists": lists,
        "suggested_list": suggested,
        "trash_count": projection.trash_count(),
        "total": total,
        "next_offset": next_offset,
        "groups": groups,
    })
}

/// A task as a row of the page: the online task, the name of its list, and its date label.
fn task_row(projection: &Projection, task: &Task, language: Language) -> Value {
    let mut value = serde_json::to_value(projection.task_view(task)).unwrap_or_else(|_| json!({}));
    value["list_name"] = projection
        .lists
        .iter()
        .find(|list| list.id == task.list_id)
        .map(|list| list.name.as_str())
        .into();
    value["due_label"] = task.due.map(|date| date_label(date, language)).into();
    value
}

/// The access token of a request: the `Authorization` header, or else the session cookie.
pub fn session(state: &AppState, headers: &HeaderMap) -> Option<Claims> {
    if headers.contains_key(AUTHORIZATION) {
        return authenticate(state, headers);
    }
    let token = cookie(headers, SESSION_COOKIE)?;
    state.verifier.verify(token, Utc::now().timestamp()).ok()
}

/// The value of the first cookie with this name.
///
/// The header is read as bytes. Another cookie with a value outside ASCII, which
/// the browser may hold for this address, then hides nothing.
fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(COOKIE)
        .iter()
        .flat_map(|value| value.as_bytes().split(|byte| *byte == b';'))
        .find_map(|pair| {
            let pair = pair.trim_ascii();
            let split = pair.iter().position(|byte| *byte == b'=')?;
            (&pair[..split] == name.as_bytes()).then_some(&pair[split + 1..])
        })
        .and_then(|value| std::str::from_utf8(value).ok())
}

/// A change through the cookie needs a header that another site cannot send.
fn changes_allowed(headers: &HeaderMap) -> Result<(), ApiError> {
    let from_page = headers
        .get("x-husker")
        .is_some_and(|value| value.as_bytes() == b"1");
    if headers.contains_key(AUTHORIZATION) || from_page {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "forbidden",
            "A change needs the X-Husker header.",
        ))
    }
}

/// The caller of a request that reads. The access token is checked before anything else.
pub struct Caller(Claims);

impl FromRequestParts<AppState> for Caller {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Response> {
        session(state, &parts.headers)
            .map(Self)
            .ok_or_else(unauthorized)
    }
}

/// The caller of a request that changes something.
///
/// The access token is checked first and the `X-Husker` rule second. Both are
/// checked before the request body is read, so a stranger cannot make the
/// server read a body.
pub struct Writer(Claims);

impl FromRequestParts<AppState> for Writer {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Response> {
        let claims = session(state, &parts.headers).ok_or_else(unauthorized)?;
        changes_allowed(&parts.headers).map_err(IntoResponse::into_response)?;
        Ok(Self(claims))
    }
}

/// A request that the page or a program sent: it carries `X-Husker` or `Authorization`.
///
/// The session routes use it, where no access token exists yet.
pub struct FromPage;

impl FromRequestParts<AppState> for FromPage {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &AppState) -> Result<Self, ApiError> {
        changes_allowed(&parts.headers).map(|()| Self)
    }
}

fn access_of(state: &AppState, claims: &Claims) -> Access {
    Access {
        list: claims.list.clone(),
        read_only: !claims.write || state.read_only,
    }
}

fn params(query: Result<Query<Params>, QueryRejection>) -> Result<Params, ApiError> {
    query
        .map(|Query(params)| params)
        .map_err(|_| ApiError::invalid("The address has a parameter that is not valid."))
}

fn body<T>(body: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
    body.map(|Json(body)| body).map_err(|rejection| {
        if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
            ApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "too_long",
                "The request body is too large.",
            )
        } else {
            ApiError::invalid("The request body is not valid.")
        }
    })
}

fn page_bounds(params: &Params) -> Result<(usize, usize), ApiError> {
    let limit = params.limit.unwrap_or(100);
    if !(1..=200).contains(&limit) {
        return Err(ApiError::invalid("limit must be from 1 through 200."));
    }
    Ok((params.offset.unwrap_or(0), limit))
}

/// Runs blocking storage work off the async threads.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, ApiError> + Send + 'static,
) -> Result<T, ApiError> {
    tokio::task::spawn_blocking(work).await.unwrap_or_else(|_| {
        Err(ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "server_error",
            "The request failed.",
        ))
    })
}

pub async fn view(
    State(state): State<AppState>,
    Caller(claims): Caller,
    query: Result<Query<Params>, QueryRejection>,
) -> Response {
    let answer = async {
        let params = params(query)?;
        let (kind, language) = (params.kind()?, params.language());
        let today = params.stamp(&state, Utc::now())?.today;
        let (offset, limit) = page_bounds(&params)?;
        let access = access_of(&state, &claims);
        let session = session_info(&claims);
        let engine = Arc::clone(&state.engine);
        blocking(move || {
            let snapshot = engine
                .read()
                .map_err(|failure| ApiError::from_engine(&failure, language))?;
            let mut view = build_view(&snapshot, &access, kind, today, language, offset, limit);
            view["session"] = session;
            Ok(view)
        })
        .await
    };
    match answer.await {
        Ok(view) => Json(view).into_response(),
        Err(failure) => failure.into_response(),
    }
}

/// The planned date a request asks for: `None` leaves it, `Some(None)` clears it.
fn planned(
    due: Option<&str>,
    clear_due: bool,
    preset: Option<&str>,
    today: NaiveDate,
) -> Result<Option<Option<NaiveDate>>, ApiError> {
    let invalid = |message: &str| ApiError::new(StatusCode::BAD_REQUEST, "invalid_date", message);
    if usize::from(due.is_some()) + usize::from(clear_due) + usize::from(preset.is_some()) > 1 {
        return Err(ApiError::invalid(
            "Use only one of due, clear_due, and preset.",
        ));
    }
    if clear_due {
        return Ok(Some(None));
    }
    if let Some(due) = due {
        let date = parse_date(due).map_err(|failure| invalid(&failure.to_string()))?;
        return Ok(Some(Some(date)));
    }
    let Some(preset) = preset else {
        return Ok(None);
    };
    let preset = match preset {
        "today" => DuePreset::Today,
        "tomorrow" => DuePreset::Tomorrow,
        "next_monday" => DuePreset::NextMonday,
        _ => return Err(invalid("preset must be today, tomorrow, or next_monday.")),
    };
    preset
        .resolve(today)
        .map(Some)
        .map_err(|failure| invalid(&failure.to_string()))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddBody {
    id: String,
    title: String,
    list_id: Option<String>,
    #[serde(default)]
    description: String,
    due: Option<String>,
    preset: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChangeBody {
    title: Option<String>,
    description: Option<String>,
    version: Option<String>,
    list_id: Option<String>,
    due: Option<String>,
    #[serde(default)]
    clear_due: bool,
    preset: Option<String>,
    status: Option<Status>,
    deleted: Option<bool>,
}

fn is_uid(text: &str) -> bool {
    text.len() == 32
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Applies changes to one task and answers with the task and the view after the change.
///
/// `task` is the row of the task as a view holds it, without `show_due`. With
/// `view=none` the answer holds no view: a tap on a checkbox needs the row only.
async fn change(
    state: AppState,
    claims: Claims,
    params: Params,
    stamp: Stamp,
    ops: Vec<Op>,
) -> Result<Value, ApiError> {
    let language = params.language();
    let view = if params.wants_view() {
        Some((params.kind()?, page_bounds(&params)?))
    } else {
        None
    };
    let access = access_of(&state, &claims);
    let session = session_info(&claims);
    blocking(move || {
        let (changed, snapshot) = state
            .engine
            .write_all(&access, &ops, &stamp)
            .map_err(|failure| ApiError::from_engine(&failure, language))?;
        let Changed::Task(task) = changed else {
            return Err(ApiError::invalid("The request changed no task."));
        };
        let projection = Projection::new(&snapshot, &access);
        let row = projection
            .tasks
            .iter()
            .find(|candidate| projection.record(candidate).uid == task.uid)
            .map(|candidate| task_row(&projection, candidate, language))
            .unwrap_or_else(|| json!(TaskView::from(&task)));
        let mut answer = json!({ "task": row });
        if let Some((kind, (offset, limit))) = view {
            let mut view = build_view(
                &snapshot,
                &access,
                kind,
                stamp.today,
                language,
                offset,
                limit,
            );
            view["session"] = session;
            answer["view"] = view;
        }
        Ok(answer)
    })
    .await
}

pub async fn add_task(
    State(state): State<AppState>,
    Writer(claims): Writer,
    query: Result<Query<Params>, QueryRejection>,
    request: Result<Json<AddBody>, JsonRejection>,
) -> Response {
    let answer = async {
        let params = params(query)?;
        let request = body(request)?;
        let stamp = params.stamp(&state, Utc::now())?;
        if !is_uid(&request.id) {
            return Err(ApiError::invalid(
                "id must be 32 lowercase hexadecimal characters.",
            ));
        }
        let due = planned(
            request.due.as_deref(),
            false,
            request.preset.as_deref(),
            stamp.today,
        )?;
        let op = Op::AddTask {
            uid: request.id,
            list: request.list_id,
            title: request.title,
            description: request.description,
            due: due.flatten(),
        };
        change(state, claims, params, stamp, vec![op]).await
    };
    match answer.await {
        Ok(value) => Json(value).into_response(),
        Err(failure) => failure.into_response(),
    }
}

pub async fn change_task(
    State(state): State<AppState>,
    Writer(claims): Writer,
    id: Result<Path<String>, PathRejection>,
    query: Result<Query<Params>, QueryRejection>,
    request: Result<Json<ChangeBody>, JsonRejection>,
) -> Response {
    let answer = async {
        let params = params(query)?;
        // An ID that is not text, such as a broken percent code, names no task.
        let Ok(Path(id)) = id else {
            return Err(ApiError::new(
                StatusCode::NOT_FOUND,
                "task_not_found",
                error_message(params.language(), &anyhow::anyhow!("Task not found.")),
            ));
        };
        let request = body(request)?;
        let stamp = params.stamp(&state, Utc::now())?;
        let due = planned(
            request.due.as_deref(),
            request.clear_due,
            request.preset.as_deref(),
            stamp.today,
        )?;
        let changes_text = request.title.is_some() || request.description.is_some();
        if changes_text && request.version.is_none() {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "version_required",
                "A change to the title or description needs the version of the task.",
            ));
        }
        // The version guards the first change. The later ones follow it in the same upload.
        let mut version = request.version;
        let mut ops = Vec::new();
        if changes_text || request.list_id.is_some() || due.is_some() {
            ops.push(Op::UpdateTask {
                uid: id.clone(),
                version: version.take(),
                list: request.list_id,
                title: request.title,
                description: request.description,
                due,
            });
        }
        if let Some(status) = request.status {
            ops.push(Op::SetStatus {
                uid: id.clone(),
                version: version.take(),
                status,
            });
        }
        if let Some(deleted) = request.deleted {
            ops.push(Op::SetDeleted {
                uid: id.clone(),
                version: version.take(),
                deleted,
            });
        }
        if ops.is_empty() {
            return Err(ApiError::invalid("The request asks for no change."));
        }
        change(state, claims, params, stamp, ops).await
    };
    match answer.await {
        Ok(value) => Json(value).into_response(),
        Err(failure) => failure.into_response(),
    }
}

fn session_info(claims: &Claims) -> Value {
    json!({
        "id": claims.id,
        "name": claims.name,
        "expires": DateTime::from_timestamp(claims.exp, 0).map(|time| time.date_naive()),
        "write": claims.write,
        "list": claims.list,
    })
}

fn session_cookie(state: &AppState, token: &str, seconds: i64) -> Option<HeaderValue> {
    let secure = if state.origin.starts_with("https://") {
        "; Secure"
    } else {
        ""
    };
    HeaderValue::from_str(&format!(
        "{SESSION_COOKIE}={token}; Path=/api; HttpOnly; SameSite=Strict; Max-Age={seconds}{secure}"
    ))
    .ok()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionBody {
    code: Option<String>,
    token: Option<String>,
}

pub async fn open_session(
    State(state): State<AppState>,
    _page: FromPage,
    request: Result<Json<SessionBody>, JsonRejection>,
) -> Response {
    let answer = (|| {
        let request = body(request)?;
        let token = match (request.code, request.token) {
            (Some(code), None) => state.pairings.take(&code, Instant::now()).ok_or_else(|| {
                ApiError::new(
                    StatusCode::NOT_FOUND,
                    "unknown_code",
                    "The pairing code is unknown, used, or expired.",
                )
            })?,
            (None, Some(token)) => token,
            _ => return Err(ApiError::invalid("Send either code or token.")),
        };
        let now = Utc::now().timestamp();
        let claims = state.verifier.verify(&token, now).map_err(|_| {
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "A valid access token is required.",
            )
        })?;
        let cookie = session_cookie(&state, &token, claims.exp - now)
            .ok_or_else(|| ApiError::invalid("The access token cannot be stored."))?;
        Ok((cookie, session_info(&claims)))
    })();
    match answer {
        Ok((cookie, info)) => {
            let mut response = Json(info).into_response();
            response.headers_mut().insert(SET_COOKIE, cookie);
            response
        }
        Err(failure) => failure.into_response(),
    }
}

pub async fn read_session(Caller(claims): Caller) -> Response {
    Json(session_info(&claims)).into_response()
}

pub async fn close_session(State(state): State<AppState>, _page: FromPage) -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    if let Some(cookie) = session_cookie(&state, "", 0) {
        response.headers_mut().insert(SET_COOKIE, cookie);
    }
    response
}

/// Stores the caller's access token under a new pairing code. `husker pair` calls this.
pub async fn create_pairing(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, token)| token.trim().to_owned());
    let (Some(token), Some(claims)) = (token, authenticate(&state, &headers)) else {
        return unauthorized();
    };
    match state.pairings.create(token, &claims.id, Instant::now()) {
        Ok(code) => Json(json!({ "code": code, "expires_in": PAIRING_SECONDS })).into_response(),
        Err(_) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "server_error",
            "The request failed.",
        ),
    }
}
