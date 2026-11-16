//! The files of the phone page, embedded in the executable.
//!
//! The page has no build step. `web/` holds plain HTML, CSS, and JavaScript,
//! and this module serves those files and the page's text from the catalog.

use axum::{
    http::{
        HeaderValue,
        header::{CACHE_CONTROL, CONTENT_TYPE},
    },
    response::{IntoResponse, Response},
};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

use crate::i18n::{Language, tr};

/// One file of the page.
pub struct Asset {
    pub path: &'static str,
    pub content_type: &'static str,
    pub body: &'static [u8],
}

/// The files of the page by request path.
pub const ASSETS: &[Asset] = &[
    Asset {
        path: "/",
        content_type: "text/html; charset=utf-8",
        body: include_bytes!("../../web/index.html"),
    },
    Asset {
        path: "/app.css",
        content_type: "text/css; charset=utf-8",
        body: include_bytes!("../../web/app.css"),
    },
    Asset {
        path: "/app.js",
        content_type: "text/javascript; charset=utf-8",
        body: include_bytes!("../../web/app.js"),
    },
    Asset {
        path: "/logic.js",
        content_type: "text/javascript; charset=utf-8",
        body: include_bytes!("../../web/logic.js"),
    },
    Asset {
        path: "/sw.js",
        content_type: "text/javascript; charset=utf-8",
        body: include_bytes!("../../web/sw.js"),
    },
    Asset {
        path: "/manifest.webmanifest",
        content_type: "application/manifest+json",
        body: include_bytes!("../../web/manifest.webmanifest"),
    },
    Asset {
        path: "/icon-192.png",
        content_type: "image/png",
        body: include_bytes!("../../web/icon-192.png"),
    },
    Asset {
        path: "/icon-512.png",
        content_type: "image/png",
        body: include_bytes!("../../web/icon-512.png"),
    },
    Asset {
        path: "/apple-touch-icon.png",
        content_type: "image/png",
        body: include_bytes!("../../web/apple-touch-icon.png"),
    },
];

/// The page may load scripts, styles, images, and data from this server only.
const CONTENT_SECURITY_POLICY: &str = "default-src 'self'; base-uri 'none'; form-action 'none'; \
     frame-ancestors 'none'; object-src 'none'";

/// The catalog keys whose text the page shows.
pub const WEB_KEYS: &[&str] = &[
    "all_tasks",
    "history",
    "trash",
    "no_tasks",
    "trash_empty",
    "new_task",
    "title",
    "description",
    "due",
    "today",
    "tomorrow",
    "next_monday",
    "clear_due",
    "action_reopen",
    "action_trash",
    "action_restore",
    "action_move_task",
    "action_refresh",
    "undo_short",
    "deleted",
    "web_mark_done",
    "web_cancel_task",
    "web_save",
    "web_close",
    "web_add",
    "web_add_list",
    "web_filter",
    "web_menu",
    "web_show_more",
    "web_read_only",
    "web_offline",
    "web_saved_view",
    "web_loading",
    "web_not_saved",
    "web_not_sent",
    "web_server_error",
    "web_stale",
    "web_discard",
    "web_task_missing",
    "web_task_left",
    "web_pair_title",
    "web_pair_hint",
    "web_pair_code",
    "web_pair_button",
    "web_pair_failed",
    "web_unauthorized",
    "web_sign_out",
    "web_sign_out_later",
    "web_pair_replace",
    "web_paired_as",
    "web_lang_en",
    "web_lang_da",
];

/// The text of the page in both languages, as the body of `/strings.json`.
pub fn strings_json() -> String {
    let language = |language: Language| -> Value {
        WEB_KEYS
            .iter()
            .map(|key| ((*key).to_owned(), Value::from(tr(language, key))))
            .collect::<Map<String, Value>>()
            .into()
    };
    serde_json::json!({ "en": language(Language::En), "da": language(Language::Da) }).to_string()
}

/// The path of the service worker.
const WORKER_PATH: &str = "/sw.js";

/// The text in `web/sw.js` that the server replaces with the page version.
const VERSION_MARK: &str = "__PAGE_VERSION__";

/// A version of the page. It changes when a page file or a text of the page changes.
///
/// The service worker carries it. A changed worker script makes a browser fetch
/// the page files of a new release, all of them or none.
pub fn page_version() -> &'static str {
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION.get_or_init(|| {
        let mut hash = Sha256::new();
        for asset in ASSETS {
            hash.update(asset.path.as_bytes());
            hash.update((asset.body.len() as u64).to_be_bytes());
            hash.update(asset.body);
        }
        hash.update(strings_json().as_bytes());
        // The policy travels with `/`, which the service worker keeps.
        hash.update(CONTENT_SECURITY_POLICY.as_bytes());
        hash.finalize()[..8]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    })
}

/// The service worker script with the page version in place.
pub fn worker_script() -> String {
    let source = ASSETS
        .iter()
        .find(|asset| asset.path == WORKER_PATH)
        .map(|asset| String::from_utf8_lossy(asset.body))
        .unwrap_or_default();
    source.replace(VERSION_MARK, page_version())
}

/// The answer for one file of the page.
pub fn serve(asset: &'static Asset) -> Response {
    let mut response = if asset.path == WORKER_PATH {
        worker_script().into_response()
    } else {
        asset.body.into_response()
    };
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(asset.content_type));
    // The service worker keeps the files. The browser asks again, so an update arrives.
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    if asset.path == "/" {
        headers.insert(
            "content-security-policy",
            HeaderValue::from_static(CONTENT_SECURITY_POLICY),
        );
        headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    }
    response
}

/// The answer for `/strings.json`.
pub fn serve_strings() -> Response {
    let mut response = strings_json().into_response();
    let headers = response.headers_mut();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}
