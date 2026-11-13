//! A local stand-in for the sync file in Azure Blob Storage.
//!
//! It implements the parts of the Blob service that Husker uses: a conditional
//! download, a conditional upload, and the managed identity token endpoint.

#![allow(dead_code)]

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug)]
pub struct Recorded {
    pub method: String,
    pub target: String,
    pub headers: Vec<(String, String)>,
}

impl Recorded {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

#[derive(Default)]
struct State {
    file: Option<Vec<u8>>,
    version: u64,
    requests: Vec<Recorded>,
    /// The `Authorization` value every blob request must carry.
    required_authorization: Option<String>,
    /// Answers every blob request with this status.
    forced_status: Option<u16>,
}

pub struct FakeBlob {
    port: u16,
    state: Arc<Mutex<State>>,
}

impl FakeBlob {
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let state = Arc::new(Mutex::new(State::default()));
        let shared = Arc::clone(&state);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let state = Arc::clone(&shared);
                std::thread::spawn(move || {
                    let _ = serve(stream, &state);
                });
            }
        });
        Self { port, state }
    }

    /// The sync file URL with a SAS-style query string.
    pub fn url(&self) -> String {
        format!(
            "http://127.0.0.1:{}/husker/tasks.json?sig=S3CRET",
            self.port
        )
    }

    /// The sync file URL without a token, for managed identity.
    pub fn bare_url(&self) -> String {
        format!("http://127.0.0.1:{}/husker/tasks.json", self.port)
    }

    pub fn token_url(&self) -> String {
        format!(
            "http://127.0.0.1:{}/metadata/identity/oauth2/token",
            self.port
        )
    }

    /// A URL on this host that nothing listens on.
    pub fn closed_url() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        format!("http://127.0.0.1:{port}/husker/tasks.json?sig=S3CRET")
    }

    pub fn file(&self) -> Option<serde_json::Value> {
        self.state
            .lock()
            .unwrap()
            .file
            .as_ref()
            .map(|body| serde_json::from_slice(body).unwrap())
    }

    pub fn titles(&self) -> Vec<String> {
        let mut titles: Vec<String> = self
            .file()
            .map(|file| {
                file["tasks"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|task| task["title"].as_str().unwrap().to_owned())
                    .collect()
            })
            .unwrap_or_default();
        titles.sort();
        titles
    }

    pub fn requests(&self) -> Vec<Recorded> {
        self.state.lock().unwrap().requests.clone()
    }

    pub fn clear_requests(&self) {
        self.state.lock().unwrap().requests.clear();
    }

    pub fn require_authorization(&self, value: &str) {
        self.state.lock().unwrap().required_authorization = Some(value.to_owned());
    }

    pub fn force_status(&self, status: Option<u16>) {
        self.state.lock().unwrap().forced_status = status;
    }
}

fn serve(stream: TcpStream, state: &Mutex<State>) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut writer = stream;
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let target = parts.next().unwrap_or_default().to_owned();
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            headers.push((name.trim().to_owned(), value.trim().to_owned()));
        }
    }
    let request = Recorded {
        method,
        target,
        headers,
    };
    if request
        .header("expect")
        .is_some_and(|value| value.eq_ignore_ascii_case("100-continue"))
    {
        writer.write_all(b"HTTP/1.1 100 Continue\r\n\r\n")?;
    }
    let length: usize = request
        .header("content-length")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;

    let (status, extra, payload) = answer(&request, body, state);
    let reason = match status {
        200 => "OK",
        201 => "Created",
        304 => "Not Modified",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        412 => "Precondition Failed",
        _ => "Error",
    };
    let mut response = format!("HTTP/1.1 {status} {reason}\r\nConnection: close\r\n");
    for (name, value) in extra {
        response.push_str(&format!("{name}: {value}\r\n"));
    }
    if status != 304 {
        response.push_str(&format!("Content-Length: {}\r\n", payload.len()));
    }
    response.push_str("\r\n");
    writer.write_all(response.as_bytes())?;
    if status != 304 {
        writer.write_all(&payload)?;
    }
    writer.flush()
}

type Answer = (u16, Vec<(&'static str, String)>, Vec<u8>);

fn answer(request: &Recorded, body: Vec<u8>, state: &Mutex<State>) -> Answer {
    let mut state = state.lock().unwrap();
    state.requests.push(request.clone());
    if request
        .target
        .starts_with("/metadata/identity/oauth2/token")
    {
        if request.header("metadata") != Some("true") {
            return (400, vec![], b"missing Metadata header".to_vec());
        }
        let expires = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600;
        let token = format!(
            r#"{{"access_token":"vm-token","expires_on":"{expires}","token_type":"Bearer"}}"#
        );
        return (200, vec![], token.into_bytes());
    }
    let denied = |code: &str| -> Answer {
        (
            403,
            vec![("x-ms-error-code", code.to_owned())],
            b"<Error/>".to_vec(),
        )
    };
    if let Some(status) = state.forced_status {
        let mut headers = vec![("x-ms-error-code", "Forced".to_owned())];
        if (300..400).contains(&status) {
            headers.push(("Location", "/elsewhere".to_owned()));
        }
        return (status, headers, vec![]);
    }
    if let Some(required) = &state.required_authorization
        && request.header("authorization") != Some(required.as_str())
    {
        return denied("AuthorizationFailure");
    }
    let etag = |version: u64| format!("\"0x{version:X}\"");
    let current = state.file.as_ref().map(|_| etag(state.version));
    match request.method.as_str() {
        "GET" => match (&state.file, &current) {
            (Some(_), Some(current)) if request.header("if-none-match") == Some(current) => {
                (304, vec![("ETag", current.clone())], vec![])
            }
            (Some(file), Some(current)) => (200, vec![("ETag", current.clone())], file.clone()),
            _ => (
                404,
                vec![("x-ms-error-code", "BlobNotFound".to_owned())],
                vec![],
            ),
        },
        "PUT" => {
            if request.header("x-ms-blob-type") != Some("BlockBlob") {
                return (400, vec![], b"missing x-ms-blob-type".to_vec());
            }
            if request.header("if-none-match") == Some("*") {
                if current.is_some() {
                    return (
                        409,
                        vec![("x-ms-error-code", "BlobAlreadyExists".to_owned())],
                        vec![],
                    );
                }
            } else if request.header("if-match") != current.as_deref() {
                return (
                    412,
                    vec![("x-ms-error-code", "ConditionNotMet".to_owned())],
                    vec![],
                );
            }
            state.version += 1;
            state.file = Some(body);
            (201, vec![("ETag", etag(state.version))], vec![])
        }
        _ => (405, vec![], vec![]),
    }
}
