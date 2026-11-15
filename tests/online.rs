//! The online server as a real `husker serve` process, next to a laptop profile, against a local test double.

mod support;

use std::{
    net::TcpListener,
    path::Path,
    process::{Child, Command, Output, Stdio},
    time::{Duration, Instant},
};

use rmcp::{
    ClientLifecycleMode, ClientServiceExt, RoleClient,
    model::{CallToolRequestParams, CallToolResult, ProtocolVersion},
    service::RunningService,
    transport::{
        StreamableHttpClientTransport, streamable_http_client::StreamableHttpClientTransportConfig,
    },
};
use serde_json::{Value, json};
use support::FakeBlob;
use tokio::time::timeout;

type Client = RunningService<RoleClient, ()>;
const WAIT: Duration = Duration::from_secs(20);

fn husker(profile: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_husker"))
        .arg("--profile")
        .arg(profile)
        .args(arguments)
        .output()
        .unwrap()
}

fn ok(profile: &Path, arguments: &[&str]) -> String {
    let output = husker(profile, arguments);
    assert!(
        output.status.success(),
        "{arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn fails(profile: &Path, arguments: &[&str]) -> String {
    let output = husker(profile, arguments);
    assert!(!output.status.success(), "{arguments:?} succeeded");
    assert!(output.stdout.is_empty(), "{arguments:?} printed data");
    String::from_utf8(output.stderr).unwrap()
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn serve_command(port: u16) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_husker"));
    command.args(["serve", "--port", &port.to_string()]);
    for name in [
        "HUSKER_SYNC_URL",
        "HUSKER_ONLINE_KEYS",
        "HUSKER_ONLINE_ORIGIN",
        "HUSKER_ONLINE_TZ",
        "HUSKER_ONLINE_REVOKED",
        "HUSKER_ONLINE_READ_ONLY",
        "FUNCTIONS_CUSTOMHANDLER_PORT",
    ] {
        command.env_remove(name);
    }
    command
}

/// A `husker serve` process that stops when the test ends, also after a failed assertion.
struct Running(Child);

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// One HTTP answer: status, headers in lower case, and the body.
struct Answer {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

impl Answer {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

fn http(method: &str, url: &str, headers: &[(&str, &str)], body: Option<&str>) -> Answer {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(WAIT))
        .build()
        .into();
    let mut response = match body {
        Some(body) => {
            let mut request = agent.post(url);
            for (name, value) in headers {
                request = request.header(*name, *value);
            }
            assert_eq!(method, "POST");
            request.send(body).unwrap()
        }
        None => {
            let mut request = match method {
                "DELETE" => agent.delete(url),
                _ => agent.get(url),
            };
            for (name, value) in headers {
                request = request.header(*name, *value);
            }
            request.call().unwrap()
        }
    };
    Answer {
        status: response.status().as_u16(),
        headers: response
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_lowercase(),
                    value.to_str().unwrap_or_default().to_owned(),
                )
            })
            .collect(),
        body: response.body_mut().read_to_string().unwrap_or_default(),
    }
}

/// A laptop profile, a sync file with two lists, and a running online server.
struct Online {
    blob: FakeBlob,
    laptop: tempfile::TempDir,
    server: Child,
    origin: String,
}

impl Online {
    fn start() -> Self {
        Self::start_with(&[])
    }

    fn start_with(settings: &[(&str, &str)]) -> Self {
        let blob = FakeBlob::start();
        let laptop = tempfile::tempdir().unwrap();
        std::fs::write(
            laptop.path().join("sync.toml"),
            format!("url = \"{}\"\n", blob.url()),
        )
        .unwrap();
        ok(laptop.path(), &["list-add", "Work"]);
        ok(laptop.path(), &["add", "Buy milk"]);
        ok(laptop.path(), &["add", "Write report", "--list", "Work"]);
        ok(laptop.path(), &["sync"]);
        let key = ok(laptop.path(), &["token", "--public-key"]);

        let port = free_port();
        let origin = format!("http://127.0.0.1:{port}");
        let server = serve_command(port)
            .env("HUSKER_SYNC_URL", blob.url())
            .env("HUSKER_ONLINE_KEYS", key)
            .env("HUSKER_ONLINE_ORIGIN", &origin)
            .envs(settings.iter().copied())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let online = Self {
            blob,
            laptop,
            server,
            origin,
        };
        let started = Instant::now();
        loop {
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .http_status_as_error(false)
                .timeout_global(Some(Duration::from_secs(1)))
                .build()
                .into();
            if agent
                .get(format!("{}/api/health", online.origin))
                .call()
                .is_ok_and(|response| response.status() == 200)
            {
                break;
            }
            assert!(started.elapsed() < WAIT, "the online server did not start");
            std::thread::sleep(Duration::from_millis(50));
        }
        online
    }

    fn profile(&self) -> &Path {
        self.laptop.path()
    }

    /// Signs a token on the laptop. `options` are extra `husker token` options.
    fn token(&self, options: &[&str]) -> String {
        let mut arguments = vec!["token", "--name", "test", "--url", &self.origin];
        arguments.extend_from_slice(options);
        ok(self.profile(), &arguments)
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.origin)
    }

    async fn connect(&self, path: &str, token: &str, discover: bool) -> Client {
        let config = StreamableHttpClientTransportConfig::with_uri(self.url(path))
            .auth_header(token.to_owned());
        let transport = StreamableHttpClientTransport::from_config(config);
        let lifecycle = if discover {
            ClientLifecycleMode::Discover {
                preferred_versions: vec![ProtocolVersion::V_2026_07_28],
            }
        } else {
            ClientLifecycleMode::Initialize
        };
        timeout(WAIT, ().serve_with_lifecycle(transport, lifecycle))
            .await
            .expect("MCP handshake timed out")
            .expect("MCP handshake failed")
    }
}

impl Drop for Online {
    fn drop(&mut self) {
        let _ = self.server.kill();
        let _ = self.server.wait();
    }
}

async fn call(client: &Client, name: &str, arguments: Value) -> CallToolResult {
    let request = CallToolRequestParams::new(name.to_owned())
        .with_arguments(arguments.as_object().unwrap().clone());
    timeout(WAIT, client.call_tool(request))
        .await
        .expect("Tool call timed out")
        .unwrap()
}

async fn data(client: &Client, name: &str, arguments: Value) -> Value {
    let result = call(client, name, arguments).await;
    assert_ne!(result.is_error, Some(true), "{result:?}");
    let structured = result.structured_content.unwrap();
    let content = serde_json::to_value(&result.content).unwrap();
    let text: Value = serde_json::from_str(content[0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(
        text, structured,
        "Text fallback must match structured output"
    );
    structured
}

async fn error_text(client: &Client, name: &str, arguments: Value) -> String {
    let result = call(client, name, arguments).await;
    assert_eq!(result.is_error, Some(true), "{result:?}");
    serde_json::to_value(&result.content).unwrap()[0]["text"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn close(client: Client) {
    timeout(WAIT, client.cancel())
        .await
        .expect("MCP shutdown timed out")
        .unwrap();
}

#[tokio::test]
async fn both_client_lifecycles_read_and_change_the_sync_file() {
    let online = Online::start();
    let token = online.token(&[]);

    for discover in [false, true] {
        let client = online.connect("/mcp", &token, discover).await;
        assert_eq!(client.list_all_tools().await.unwrap().len(), 10);
        let instructions = client.peer_info().unwrap().instructions.clone().unwrap();
        assert!(instructions.contains("every list"), "{instructions}");
        assert!(instructions.contains("IDs, list IDs, and task versions are text"));

        let lists = data(&client, "list_lists", json!({})).await;
        let names: Vec<&str> = lists["lists"]
            .as_array()
            .unwrap()
            .iter()
            .map(|list| list["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["Inbox", "Work"]);

        let title = format!("From the online server {discover}");
        let added = data(&client, "add_task", json!({ "title": title })).await;
        let task = &added["task"];
        assert_eq!(task["id"].as_str().unwrap().len(), 32);
        let finished = data(
            &client,
            "set_task_status",
            json!({ "id": task["id"], "version": task["version"], "status": "done" }),
        )
        .await;
        assert_eq!(finished["task"]["status"], "done");
        let stale = error_text(
            &client,
            "trash_task",
            json!({ "id": task["id"], "version": task["version"] }),
        )
        .await;
        assert!(stale.contains("Reload it before saving."), "{stale}");
        close(client).await;

        // The laptop receives the task at its next sync and has nothing to send.
        let synced = ok(online.profile(), &["sync"]);
        assert_eq!(synced, "Synced. Received 1, sent 0.");
        let tasks = ok(online.profile(), &["tasks", "s:done"]);
        assert!(tasks.contains(&title), "{tasks}");
    }

    // Every upload of the server was conditional, and none created the file.
    let puts: Vec<_> = online
        .blob
        .requests()
        .into_iter()
        .filter(|request| request.method == "PUT")
        .collect();
    let by_server = puts
        .iter()
        .filter(|request| request.header("if-match").is_some())
        .count();
    assert_eq!(by_server, 4, "two adds and two status changes");
    assert_eq!(puts.len(), 5, "plus the first upload by the laptop");

    // The server recorded no upload: the history still holds the laptop's single identifier.
    assert_eq!(
        online.blob.file().unwrap()["uploads"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn tokens_and_paths_limit_the_lists_and_the_tools() {
    let online = Online::start();
    let all = online.token(&[]);
    let work = online.token(&["--list", "work"]);
    let reader = online.token(&["--read-only"]);

    // A token for one list.
    let client = online.connect("/mcp", &work, true).await;
    assert_eq!(client.list_all_tools().await.unwrap().len(), 8);
    let lists = data(&client, "list_lists", json!({})).await;
    assert_eq!(lists["lists"].as_array().unwrap().len(), 1);
    assert_eq!(lists["lists"][0]["name"], "Work");
    let tasks = data(&client, "list_tasks", json!({ "query": "s:all" })).await;
    assert_eq!(tasks["total"], 1);
    assert_eq!(tasks["tasks"][0]["title"], "Write report");
    let added = data(&client, "add_task", json!({ "title": "Scoped" })).await;
    assert_eq!(added["task"]["list_id"], lists["lists"][0]["id"]);
    close(client).await;

    // A token for every list on a list path behaves the same.
    let client = online.connect("/mcp/list/Work", &all, false).await;
    assert_eq!(client.list_all_tools().await.unwrap().len(), 8);
    let instructions = client.peer_info().unwrap().instructions.clone().unwrap();
    assert!(
        instructions.contains("restricted to list ID"),
        "{instructions}"
    );
    let tasks = data(&client, "list_tasks", json!({ "query": "l:inbox s:all" })).await;
    assert_eq!(tasks["total"], 0, "a filter cannot leave the list");
    close(client).await;

    // A read-only token.
    let client = online.connect("/mcp", &reader, true).await;
    assert_eq!(client.list_all_tools().await.unwrap().len(), 3);
    let request = CallToolRequestParams::new("add_task".to_owned())
        .with_arguments(json!({ "title": "x" }).as_object().unwrap().clone());
    let direct = timeout(WAIT, client.call_tool(request)).await.unwrap();
    assert!(direct.is_err() || direct.unwrap().is_error == Some(true));
    close(client).await;

    // The path cannot widen a token.
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }).to_string();
    let post = |path: &str, token: &str| {
        http(
            "POST",
            &online.url(path),
            &[
                ("authorization", &format!("Bearer {token}")),
                ("content-type", "application/json"),
                ("accept", "application/json, text/event-stream"),
                ("mcp-protocol-version", "2025-11-25"),
            ],
            Some(&body),
        )
    };
    assert_eq!(post("/mcp/list/Inbox", &work).status, 403);
    assert_eq!(post("/mcp/list/Missing", &work).status, 403);
    assert_eq!(post("/mcp/list/Missing", &all).status, 404);
    assert_eq!(post("/mcp/list/Work", &work).status, 200);
    assert_eq!(
        post("/mcp/list/W%4Frk", &all).status,
        200,
        "the name is percent-decoded"
    );
}

#[test]
fn requests_are_checked_before_the_storage_is_asked() {
    let online = Online::start();
    let token = online.token(&[]);
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }).to_string();
    let mcp_headers = [
        ("content-type", "application/json"),
        ("accept", "application/json, text/event-stream"),
        ("mcp-protocol-version", "2025-11-25"),
    ];

    online.blob.clear_requests();
    let health = http("GET", &online.url("/api/health"), &[], None);
    assert_eq!(health.status, 200);
    let health: Value = serde_json::from_str(&health.body).unwrap();
    assert_eq!(
        health,
        json!({ "name": "husker", "version": env!("CARGO_PKG_VERSION"), "format": 1 })
    );

    let anonymous = http("POST", &online.url("/mcp"), &mcp_headers, Some(&body));
    assert_eq!(anonymous.status, 401);
    assert_eq!(anonymous.header("www-authenticate"), Some("Bearer"));
    let mut forged = token.clone();
    forged.pop();
    forged.push(if token.ends_with('A') { 'B' } else { 'A' });
    let mut headers = mcp_headers.to_vec();
    let bearer = format!("Bearer {forged}");
    headers.push(("authorization", &bearer));
    assert_eq!(
        http("POST", &online.url("/mcp/list/Work"), &headers, Some(&body)).status,
        401
    );
    assert_eq!(http("GET", &online.url("/nothing"), &[], None).status, 404);
    assert!(
        online.blob.requests().is_empty(),
        "no request reached the storage"
    );

    let bearer = format!("Bearer {token}");
    let mut headers = mcp_headers.to_vec();
    headers.push(("authorization", &bearer));
    for method in ["GET", "DELETE"] {
        assert_eq!(
            http(method, &online.url("/mcp"), &headers, None).status,
            405
        );
    }
    let mut foreign = headers.clone();
    foreign.push(("origin", "https://evil.example"));
    assert_eq!(
        http("POST", &online.url("/mcp"), &foreign, Some(&body)).status,
        403
    );
    let listed = http("POST", &online.url("/mcp"), &headers, Some(&body));
    assert_eq!(listed.status, 200);
    assert!(listed.header("mcp-session-id").is_none());
    assert!(
        listed
            .header("content-type")
            .unwrap()
            .starts_with("application/json")
    );
    let listed: Value = serde_json::from_str(&listed.body).unwrap();
    assert_eq!(listed["result"]["tools"].as_array().unwrap().len(), 10);
}

#[tokio::test]
async fn storage_failures_and_settings_never_show_the_sync_url() {
    let online = Online::start();
    let token = online.token(&[]);
    let client = online.connect("/mcp", &token, true).await;

    online.blob.force_status(Some(500));
    let failed = error_text(&client, "list_lists", json!({})).await;
    assert_eq!(failed, "The sync storage gave an unexpected answer.");
    online.blob.force_status(Some(403));
    let denied = error_text(&client, "add_task", json!({ "title": "x" })).await;
    assert_eq!(denied, "The sync storage refused access.");
    online.blob.force_status(None);
    close(client).await;

    // A server whose sync file does not exist reports it and never creates the file.
    let empty = FakeBlob::start();
    let port = free_port();
    let origin = format!("http://127.0.0.1:{port}");
    let key = ok(online.profile(), &["token", "--public-key"]);
    let second = Running(
        serve_command(port)
            .env("HUSKER_SYNC_URL", empty.url())
            .env("HUSKER_ONLINE_KEYS", &key)
            .env("HUSKER_ONLINE_ORIGIN", &origin)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let token = ok(
        online.profile(),
        &["token", "--name", "second", "--url", &origin],
    );
    let started = Instant::now();
    let answer = loop {
        let body = json!({
            "jsonrpc": "2.0", "id": 1, "method": "tools/call",
            "params": { "name": "add_task", "arguments": { "title": "First" } }
        })
        .to_string();
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(2)))
            .build()
            .into();
        let sent = agent
            .post(format!("{origin}/mcp"))
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("mcp-protocol-version", "2025-11-25")
            .send(body);
        if let Ok(mut response) = sent {
            break response.body_mut().read_to_string().unwrap();
        }
        assert!(started.elapsed() < WAIT, "the second server did not start");
        std::thread::sleep(Duration::from_millis(50));
    };
    drop(second);
    assert!(
        answer.contains("The sync file does not exist. Sync a computer first."),
        "{answer}"
    );
    assert!(empty.file().is_none(), "the server created no sync file");
    assert!(
        empty
            .requests()
            .iter()
            .all(|request| request.method == "GET")
    );

    // Start failures name the variable and hide the URL.
    for (name, value) in [
        (
            "HUSKER_SYNC_URL",
            "http://example.com/tasks.json?sig=S3CRET",
        ),
        ("HUSKER_ONLINE_KEYS", "not-a-key"),
        ("HUSKER_ONLINE_ORIGIN", "tasks.example.net"),
        ("HUSKER_ONLINE_TZ", "Mars/Base"),
    ] {
        let output = serve_command(free_port())
            .env("HUSKER_SYNC_URL", online.blob.url())
            .env("HUSKER_ONLINE_KEYS", &key)
            .env("HUSKER_ONLINE_ORIGIN", &origin)
            .env(name, value)
            .output()
            .unwrap();
        assert!(!output.status.success(), "{name}");
        let message = String::from_utf8_lossy(&output.stderr);
        assert!(message.contains(name), "{message}");
        assert!(!message.contains("S3CRET"), "{message}");
        assert!(!message.contains("example.com"), "{message}");
    }
    let missing = serve_command(free_port()).output().unwrap();
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("HUSKER_SYNC_URL"));
}

#[test]
fn the_read_only_setting_and_a_revoked_token_are_enforced() {
    let online = Online::start();
    let profile = online.profile();
    let key = ok(profile, &["token", "--public-key"]);
    let port = free_port();
    let origin = format!("http://127.0.0.1:{port}");

    // Two tokens from the same key for a second server. The first one gets revoked there.
    let old = ok(
        profile,
        &["--json", "token", "--name", "old", "--url", &origin],
    );
    let old: Value = serde_json::from_str(&old).unwrap();
    assert_eq!(old["name"], "old");
    assert_eq!(old["list"], Value::Null);
    assert_eq!(old["write"], true);
    assert_eq!(old["expires"].as_str().unwrap().len(), 10);
    assert_eq!(old["id"].as_str().unwrap().len(), 12);
    let new = ok(profile, &["token", "--name", "new"]);

    let second = Running(
        serve_command(port)
            .env("HUSKER_SYNC_URL", online.blob.url())
            .env("HUSKER_ONLINE_KEYS", key)
            .env("HUSKER_ONLINE_ORIGIN", &origin)
            .env("HUSKER_ONLINE_READ_ONLY", "true")
            .env("HUSKER_ONLINE_REVOKED", old["id"].as_str().unwrap())
            .env("HUSKER_ONLINE_TZ", "Europe/Copenhagen")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let post = |token: &str, message: Value| {
        let started = Instant::now();
        loop {
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .http_status_as_error(false)
                .timeout_global(Some(Duration::from_secs(2)))
                .build()
                .into();
            let sent = agent
                .post(format!("{origin}/mcp"))
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .send(message.to_string());
            if let Ok(mut response) = sent {
                let status = response.status().as_u16();
                let body = response.body_mut().read_to_string().unwrap_or_default();
                break (status, body);
            }
            assert!(started.elapsed() < WAIT, "the second server did not start");
            std::thread::sleep(Duration::from_millis(50));
        }
    };
    let initialize = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {
            "protocolVersion": "2025-11-25",
            "capabilities": {},
            "clientInfo": { "name": "test", "version": "1" }
        }
    });
    let revoked = post(old["token"].as_str().unwrap(), initialize.clone());
    let accepted = post(&new, initialize);
    let tools = post(
        &new,
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
    );
    drop(second);

    assert_eq!(revoked.0, 401, "{}", revoked.1);
    assert_eq!(accepted.0, 200, "{}", accepted.1);
    let accepted: Value = serde_json::from_str(&accepted.1).unwrap();
    let instructions = accepted["result"]["instructions"].as_str().unwrap();
    assert!(instructions.contains("Europe/Copenhagen"), "{instructions}");
    assert!(instructions.contains("read-only"), "{instructions}");
    let tools: Value = serde_json::from_str(&tools.1).unwrap();
    assert_eq!(
        tools["result"]["tools"].as_array().unwrap().len(),
        3,
        "the read-only setting removes the mutation tools from a write token"
    );
}

#[test]
fn the_token_command_signs_scoped_tokens_and_reports_each_error() {
    let online = Online::start();
    let profile = online.profile();

    // The key is created once and stays.
    let key = ok(profile, &["token", "--public-key"]);
    assert_eq!(key.len(), 43);
    assert_eq!(ok(profile, &["token", "--public-key"]), key);

    // The address is remembered after the first use.
    let first = online.token(&["--list", " WORK ", "--read-only", "--days", "7"]);
    assert!(first.starts_with("hsk1_"));
    assert_eq!(first.lines().count(), 1);
    let payload = first
        .strip_prefix("hsk1_")
        .unwrap()
        .split('.')
        .next()
        .unwrap();
    let payload: Value = serde_json::from_slice(&decode_base64url(payload)).unwrap();
    assert_eq!(payload["aud"], online.origin);
    assert_eq!(payload["write"], false);
    assert_eq!(payload["name"], "test");
    assert_eq!(payload["list"].as_str().unwrap().len(), 32);
    assert_eq!(
        payload["exp"].as_i64().unwrap() - payload["iat"].as_i64().unwrap(),
        7 * 86_400
    );
    let work = online.blob.file().unwrap()["lists"]
        .as_array()
        .unwrap()
        .iter()
        .find(|list| list["name"] == "Work")
        .unwrap()["uid"]
        .clone();
    assert_eq!(payload["list"], work);
    let later = ok(profile, &["token", "--name", "again"]);
    assert!(later.starts_with("hsk1_"), "the stored address is used");

    let output = husker(profile, &["token", "--name", "phone"]);
    let note = String::from_utf8(output.stderr).unwrap();
    assert!(
        note.starts_with("Token ") && note.contains(" expires on "),
        "{note}"
    );

    for (arguments, expected) in [
        (vec!["token"], "--name"),
        (vec!["token", "--name", " "], "1 to 64 characters"),
        (vec!["token", "--name", "x", "--days", "0"], "1 through 400"),
        (
            vec!["token", "--name", "x", "--days", "401"],
            "1 through 400",
        ),
        (
            vec!["token", "--name", "x", "--list", "Nowhere"],
            "does not exist",
        ),
        (
            vec![
                "token",
                "--name",
                "x",
                "--url",
                "https://tasks.example.net/path",
            ],
            "without a path",
        ),
        (
            vec!["token", "--name", "x", "--url", "http://tasks.example.net"],
            "https",
        ),
        (
            vec!["token", "--public-key", "--name", "x"],
            "cannot be used with",
        ),
    ] {
        let message = fails(profile, &arguments);
        assert!(message.contains(expected), "{arguments:?}: {message}");
    }

    // A new profile has no address yet, and a list without a sync identifier needs a sync first.
    let fresh = tempfile::tempdir().unwrap();
    let message = fails(fresh.path(), &["token", "--name", "x"]);
    assert!(message.contains("--url"), "{message}");
    assert!(fresh.path().join("online.toml").exists());
    ok(fresh.path(), &["list-add", "Later"]);
    let token = ok(
        fresh.path(),
        &[
            "token",
            "--name",
            "x",
            "--list",
            "Later",
            "--url",
            "https://tasks.example.net",
        ],
    );
    assert!(
        token.starts_with("hsk1_"),
        "a new list has an identifier at once"
    );

    // A damaged key file is named, and its content is never printed.
    std::fs::write(profile.join("online.toml"), "key = \"S3CRETKEY\"\n").unwrap();
    let message = fails(profile, &["token", "--name", "x"]);
    assert!(message.contains("online.toml"), "{message}");
    assert!(!message.contains("S3CRETKEY"), "{message}");
}

/// Decodes base64url without padding. The tests need no extra crate for it.
fn decode_base64url(text: &str) -> Vec<u8> {
    let value = |byte: u8| -> u32 {
        match byte {
            b'A'..=b'Z' => u32::from(byte - b'A'),
            b'a'..=b'z' => u32::from(byte - b'a') + 26,
            b'0'..=b'9' => u32::from(byte - b'0') + 52,
            b'-' => 62,
            b'_' => 63,
            _ => panic!("not base64url"),
        }
    };
    let mut bytes = Vec::new();
    for chunk in text.as_bytes().chunks(4) {
        let mut buffer = 0_u32;
        for (index, byte) in chunk.iter().enumerate() {
            buffer |= value(*byte) << (18 - 6 * index);
        }
        for index in 0..chunk.len() - 1 {
            bytes.push((buffer >> (16 - 8 * index)) as u8);
        }
    }
    bytes
}

#[test]
fn serve_ignores_the_profile_and_uses_the_handler_port() {
    let online = Online::start();
    let key = ok(online.profile(), &["token", "--public-key"]);
    let port = free_port();
    let missing_profile = online.profile().join("does-not-exist");
    let server = Running(
        Command::new(env!("CARGO_BIN_EXE_husker"))
            .arg("--profile")
            .arg(&missing_profile)
            .arg("serve")
            .env("FUNCTIONS_CUSTOMHANDLER_PORT", port.to_string())
            .env("HUSKER_SYNC_URL", online.blob.url())
            .env("HUSKER_ONLINE_KEYS", key)
            .env("HUSKER_ONLINE_ORIGIN", format!("http://127.0.0.1:{port}"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let started = Instant::now();
    let status = loop {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(1)))
            .build()
            .into();
        if let Ok(response) = agent
            .get(format!("http://127.0.0.1:{port}/api/health"))
            .call()
        {
            break response.status().as_u16();
        }
        if started.elapsed() > WAIT {
            break 0;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    drop(server);
    assert_eq!(status, 200);
    assert!(!missing_profile.exists(), "serve creates no profile");
}
