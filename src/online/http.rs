//! The HTTP routes of the online server.
//!
//! Every request is checked before the sync file is read: the caller's address
//! headers first, then the access token.

use std::sync::Arc;

use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Path, Request, State, rejection::PathRejection},
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{AUTHORIZATION, CACHE_CONTROL, COOKIE, ORIGIN, WWW_AUTHENTICATE},
    },
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{any, get, patch, post},
};
use chrono::Utc;
use chrono_tz::Tz;
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::never::NeverSessionManager,
};
use serde_json::json;
use tower::ServiceExt;

use super::{
    access::{Claims, Verifier, origin_host},
    api::{self, Pairings},
    assets,
    doc::{Access, resolve_list},
    engine::{Engine, EngineError},
    mcp::OnlineServer,
};
use crate::sync::{FailureKind, snapshot::FORMAT};

/// The largest request body, for MCP and for the API. A request with the longest accepted texts
/// fits several times.
const MAX_REQUEST_BYTES: usize = 512 * 1024;

/// What every request handler shares.
#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<Engine>,
    pub verifier: Arc<Verifier>,
    /// The public address of the server, as `normalize_origin` returns it.
    pub origin: Arc<str>,
    /// The time zone that decides "today" for MCP calls.
    pub zone: Tz,
    /// Removes write access from every request.
    pub read_only: bool,
    /// The pairing codes of phones that have not fetched their access token yet.
    pub pairings: Arc<Pairings>,
}

pub fn router(state: AppState) -> Router {
    let mut router = Router::new()
        .route("/api/health", get(health))
        .route("/api/view", get(api::view))
        .route("/api/tasks", post(api::add_task))
        .route("/api/tasks/{id}", patch(api::change_task))
        .route(
            "/api/session",
            post(api::open_session)
                .get(api::read_session)
                .delete(api::close_session),
        )
        .route("/api/pairings", post(api::create_pairing))
        .route("/strings.json", get(|| async { assets::serve_strings() }))
        .route("/mcp", any(mcp_root))
        .route("/mcp/list/{name}", any(mcp_list));
    // The files of the phone page. They hold no task data and need no access token.
    for asset in assets::ASSETS {
        router = router.route(asset.path, get(move || async move { assets::serve(asset) }));
    }
    router
        .fallback(not_found)
        .method_not_allowed_fallback(method_not_allowed)
        // The API reads no larger body. MCP has the same limit in its own service.
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES))
        .layer(middleware::from_fn_with_state(state.clone(), check_request))
        .with_state(state)
}

/// The answer for a known path with a method it does not accept.
async fn method_not_allowed() -> Response {
    error(
        StatusCode::METHOD_NOT_ALLOWED,
        "method_not_allowed",
        "This address does not accept that method.",
    )
}

/// A JSON error answer. `code` is a stable English token.
pub fn error(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(json!({ "error": { "code": code, "message": message } })),
    )
        .into_response()
}

/// The answer for a failed read or write of the sync file.
pub fn engine_error(failure: &EngineError) -> Response {
    let (status, code) = match failure {
        EngineError::Doc(_) => (StatusCode::BAD_REQUEST, "invalid_request"),
        EngineError::Missing | EngineError::Storage(FailureKind::Busy) => {
            (StatusCode::SERVICE_UNAVAILABLE, "busy")
        }
        EngineError::Storage(_) | EngineError::Unknown => {
            (StatusCode::BAD_GATEWAY, "storage_unavailable")
        }
    };
    error(status, code, &failure.message())
}

/// Refuses a request that another web page or another host name sent, and marks answers as private.
async fn check_request(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let headers = request.headers();
    let origin_matches = headers.get(ORIGIN).is_none_or(|origin| {
        origin.to_str().is_ok_and(|origin| {
            origin
                .trim_end_matches('/')
                .eq_ignore_ascii_case(&state.origin)
        })
    });
    if !origin_matches {
        return error(
            StatusCode::FORBIDDEN,
            "forbidden",
            "This address does not accept requests from that origin.",
        );
    }
    // A proxy in front of the server names the host the client used. Each proxy on the way
    // can add a name, so the last one counts: it comes from the proxy next to the server.
    let forwarded = headers.get_all("x-forwarded-host").iter().next_back();
    if let Some(value) = forwarded {
        let host = value
            .to_str()
            .unwrap_or_default()
            .rsplit(',')
            .next()
            .unwrap_or_default()
            .trim();
        if !same_host(host, origin_host(&state.origin)) {
            // The answer names the host. A first deployment behind another proxy then shows
            // at once which name arrived.
            return error(
                StatusCode::FORBIDDEN,
                "forbidden",
                &format!("This address does not accept requests for the host '{host}'."),
            );
        }
    }
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    // Task data is never stored by a browser or a proxy. The page files set their own rule.
    headers
        .entry(CACHE_CONTROL)
        .or_insert(HeaderValue::from_static("no-store"));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    headers.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    response
}

/// Compares two `host[:port]` values without regard to case or a default port.
fn same_host(left: &str, right: &str) -> bool {
    fn bare(host: &str) -> String {
        let host = host.trim().to_lowercase();
        host.strip_suffix(":443")
            .or_else(|| host.strip_suffix(":80"))
            .unwrap_or(&host)
            .to_owned()
    }
    bare(left) == bare(right)
}

/// Checks the bearer token of a request. `None` means the caller gets `unauthorized`.
pub fn authenticate(state: &AppState, headers: &HeaderMap) -> Option<Claims> {
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            let (scheme, token) = value.split_once(' ')?;
            scheme.eq_ignore_ascii_case("bearer").then(|| token.trim())
        })?;
    state.verifier.verify(token, Utc::now().timestamp()).ok()
}

/// The answer for a missing or refused token. It never says why the token was refused.
pub fn unauthorized() -> Response {
    let mut response = error(
        StatusCode::UNAUTHORIZED,
        "unauthorized",
        "A valid access token is required.",
    );
    response
        .headers_mut()
        .insert(WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
    response
}

async fn health() -> Response {
    Json(json!({
        "name": "husker",
        "version": env!("CARGO_PKG_VERSION"),
        "format": FORMAT,
    }))
    .into_response()
}

async fn not_found() -> Response {
    error(StatusCode::NOT_FOUND, "not_found", "Not found.")
}

async fn mcp_root(State(state): State<AppState>, request: Request) -> Response {
    serve_mcp(state, None, request).await
}

async fn mcp_list(
    State(state): State<AppState>,
    name: Result<Path<String>, PathRejection>,
    request: Request,
) -> Response {
    // The token comes first: a caller without one learns nothing, not even that the path is malformed.
    if authenticate(&state, request.headers()).is_none() {
        return unauthorized();
    }
    match name {
        Ok(Path(name)) => serve_mcp(state, Some(name), request).await,
        Err(_) => error(
            StatusCode::NOT_FOUND,
            "unknown_list",
            "The list in the address does not exist.",
        ),
    }
}

/// Answers one MCP request with a server built for the caller's access.
async fn serve_mcp(state: AppState, list: Option<String>, mut request: Request) -> Response {
    let Some(claims) = authenticate(&state, request.headers()) else {
        return unauthorized();
    };
    let mut access = Access {
        list: claims.list,
        read_only: !claims.write || state.read_only,
    };
    if let Some(name) = list {
        let engine = Arc::clone(&state.engine);
        let resolved = tokio::task::spawn_blocking(move || {
            engine.read().map(|snapshot| resolve_list(&snapshot, &name))
        })
        .await;
        let uid = match resolved {
            Ok(Ok(uid)) => uid,
            Ok(Err(failure)) => return engine_error(&failure),
            Err(_) => {
                return error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "The request failed.",
                );
            }
        };
        match (&access.list, uid) {
            // The path cannot widen a token. A token for one list learns nothing about other names.
            (Some(allowed), uid) if uid.as_ref() != Some(allowed) => {
                return error(
                    StatusCode::FORBIDDEN,
                    "forbidden",
                    "This token is limited to another list.",
                );
            }
            (_, Some(uid)) => access.list = Some(uid),
            (None, None) => {
                return error(
                    StatusCode::NOT_FOUND,
                    "unknown_list",
                    "The list in the address does not exist.",
                );
            }
            (Some(_), None) => unreachable!("handled by the first arm"),
        }
    }

    // rmcp hands the request parts to tool code. The credentials stop here.
    request.headers_mut().remove(AUTHORIZATION);
    request.headers_mut().remove(COOKIE);

    let server = OnlineServer::new(Arc::clone(&state.engine), access, state.zone);
    let config = StreamableHttpServerConfig::default()
        // No sessions: any process answers any request.
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_sse_keep_alive(None)
        .with_max_request_body_bytes(MAX_REQUEST_BYTES)
        .with_allowed_hosts([
            "localhost".to_owned(),
            "127.0.0.1".to_owned(),
            "::1".to_owned(),
            origin_host(&state.origin).to_owned(),
        ]);
    let service = StreamableHttpService::new(
        move || Ok(server.clone()),
        Arc::new(NeverSessionManager::default()),
        config,
    );
    match service.oneshot(request).await {
        Ok(response) => response.map(Body::new),
        Err(never) => match never {},
    }
}

#[cfg(test)]
mod tests {
    use super::same_host;

    #[test]
    fn hosts_compare_without_case_or_default_port() {
        assert!(same_host("Tasks.Example.net", "tasks.example.net"));
        assert!(same_host("tasks.example.net:443", "tasks.example.net"));
        assert!(same_host("127.0.0.1:8787", "127.0.0.1:8787"));
        assert!(!same_host("tasks.example.net:8443", "tasks.example.net"));
        assert!(!same_host("other.example.net", "tasks.example.net"));
    }
}
