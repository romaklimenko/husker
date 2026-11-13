//! Sync through real `husker` processes and the real HTTP client, against a local test double.

mod support;

use std::{
    path::Path,
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};

use husker::{
    model::TaskInput,
    store::Store,
    sync::{
        Auth, Downloaded, FailureKind, Settings,
        remote::{BlobRemote, Remote, Stored},
        sync_once,
    },
};
use rmcp::{
    ClientLifecycleMode, ClientServiceExt, RoleClient, model::CallToolRequestParams,
    service::RunningService, transport::TokioChildProcess,
};
use serde_json::{Value, json};
use support::FakeBlob;
use tokio::time::timeout;

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
    String::from_utf8(output.stdout).unwrap()
}

fn json_output(profile: &Path, arguments: &[&str]) -> Value {
    let mut all = vec!["--json"];
    all.extend_from_slice(arguments);
    serde_json::from_str(&ok(profile, &all)).unwrap()
}

/// A profile directory whose `sync.toml` points at `url`.
fn profile(url: &str) -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("sync.toml"),
        format!("url = \"{url}\"\n"),
    )
    .unwrap();
    directory
}

fn sas(url: String) -> Settings {
    Settings {
        url,
        auth: Auth::Sas,
    }
}

fn add(store: &mut Store, title: &str) {
    store
        .create_task(&TaskInput {
            list_id: 1,
            title: title.into(),
            ..TaskInput::default()
        })
        .unwrap();
}

#[test]
fn the_blob_client_sends_conditions_and_keeps_the_sas_query() {
    let blob = FakeBlob::start();
    let remote = BlobRemote::new(&sas(blob.url()));
    let first = tempfile::tempdir().unwrap();
    let mut laptop = Store::open(&first.path().join("tasks.db")).unwrap();
    add(&mut laptop, "Buy milk");

    // First sync: the file does not exist, so the upload must not replace anything.
    let report = sync_once(&mut laptop, &remote).unwrap();
    assert_eq!((report.received, report.sent), (0, 2));
    let requests = blob.requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].target, "/husker/tasks.json?sig=S3CRET");
    assert_eq!(requests[0].header("if-none-match"), None);
    assert!(requests[0].header("x-ms-version").is_some());
    assert_eq!(requests[1].method, "PUT");
    assert_eq!(requests[1].target, "/husker/tasks.json?sig=S3CRET");
    assert_eq!(requests[1].header("if-none-match"), Some("*"));
    assert_eq!(requests[1].header("x-ms-blob-type"), Some("BlockBlob"));
    assert_eq!(requests[1].header("content-type"), Some("application/json"));
    assert_eq!(requests[1].header("authorization"), None);
    assert_eq!(blob.file().unwrap()["format"], 1);

    // Nothing changed: one conditional download that answers 304, and no upload.
    blob.clear_requests();
    let report = sync_once(&mut laptop, &remote).unwrap();
    assert_eq!((report.received, report.sent), (0, 0));
    let requests = blob.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].header("if-none-match"), Some("\"0x1\""));

    // A second device downloads the file and uploads its own task with If-Match.
    let second = tempfile::tempdir().unwrap();
    let mut desktop = Store::open(&second.path().join("tasks.db")).unwrap();
    add(&mut desktop, "Call the bank");
    blob.clear_requests();
    let report = sync_once(&mut desktop, &remote).unwrap();
    assert_eq!(report.conflicts, 0);
    let requests = blob.requests();
    assert_eq!(requests[1].method, "PUT");
    assert_eq!(requests[1].header("if-match"), Some("\"0x1\""));
    assert_eq!(blob.titles(), ["Buy milk", "Call the bank"]);

    sync_once(&mut laptop, &remote).unwrap();
    let mut titles: Vec<String> = laptop
        .tasks()
        .unwrap()
        .into_iter()
        .map(|task| task.title)
        .collect();
    titles.sort();
    assert_eq!(titles, ["Buy milk", "Call the bank"]);
}

#[test]
fn the_blob_client_reports_a_file_that_another_device_changed() {
    let blob = FakeBlob::start();
    let remote = BlobRemote::new(&sas(blob.url()));
    let Stored::Done { etag: first } = remote.upload(b"{}", None).unwrap() else {
        panic!("the first upload creates the file");
    };
    // Creating the file again answers 409, and replacing an old version answers 412.
    assert_eq!(remote.upload(b"{}", None).unwrap(), Stored::Changed);
    let Stored::Done { etag: second } = remote.upload(b"{}", Some(&first)).unwrap() else {
        panic!("the upload replaces the version it names");
    };
    assert_ne!(first, second);
    assert_eq!(remote.upload(b"{}", Some(&first)).unwrap(), Stored::Changed);
    assert_eq!(
        remote.download(Some(&second)).unwrap(),
        Downloaded::Unchanged
    );
    let methods: Vec<String> = blob
        .requests()
        .into_iter()
        .map(|request| request.method)
        .collect();
    assert_eq!(methods, ["PUT", "PUT", "PUT", "PUT", "GET"]);
}

#[test]
fn a_managed_identity_sends_a_cached_bearer_token_and_no_secret() {
    let blob = FakeBlob::start();
    blob.require_authorization("Bearer vm-token");
    let settings = Settings {
        url: blob.bare_url(),
        auth: Auth::ManagedIdentity {
            client_id: Some("11111111-2222-3333-4444-555555555555".into()),
        },
    };
    let remote = BlobRemote::with_metadata_endpoint(&settings, &blob.token_url());
    let directory = tempfile::tempdir().unwrap();
    let mut store = Store::open(&directory.path().join("tasks.db")).unwrap();
    add(&mut store, "From the VM");
    sync_once(&mut store, &remote).unwrap();
    sync_once(&mut store, &remote).unwrap();
    assert_eq!(blob.titles(), ["From the VM"]);

    let requests = blob.requests();
    let tokens: Vec<_> = requests
        .iter()
        .filter(|request| request.target.starts_with("/metadata/"))
        .collect();
    assert_eq!(tokens.len(), 1, "the token is reused");
    assert_eq!(tokens[0].header("metadata"), Some("true"));
    assert!(
        tokens[0]
            .target
            .contains("resource=https%3A%2F%2Fstorage.azure.com%2F")
    );
    assert!(
        tokens[0]
            .target
            .contains("client_id=11111111-2222-3333-4444-555555555555")
    );
    let blobs: Vec<_> = requests
        .iter()
        .filter(|request| request.target.starts_with("/husker/"))
        .collect();
    assert_eq!(blobs.len(), 3);
    for request in blobs {
        assert_eq!(request.target, "/husker/tasks.json");
        assert_eq!(request.header("authorization"), Some("Bearer vm-token"));
    }

    // A SAS client without the token is refused, and the failure hides the URL.
    let denied = BlobRemote::new(&sas(blob.url()))
        .download(None)
        .unwrap_err();
    assert_eq!(denied.kind, FailureKind::Denied);
    assert!(denied.detail.contains("AuthorizationFailure"), "{denied}");
    assert!(!denied.detail.contains("S3CRET"), "{denied}");
}

#[test]
fn failures_have_a_kind_and_never_show_the_url() {
    let blob = FakeBlob::start();
    let remote = BlobRemote::new(&sas(blob.url()));
    for (status, kind) in [
        (401, FailureKind::Denied),
        (403, FailureKind::Denied),
        (500, FailureKind::Rejected),
        (301, FailureKind::Rejected),
    ] {
        blob.force_status(Some(status));
        for failure in [
            remote.download(None).unwrap_err(),
            remote.upload(b"{}", None).unwrap_err(),
        ] {
            assert_eq!(failure.kind, kind, "{failure}");
            assert!(failure.detail.contains(&status.to_string()), "{failure}");
            assert!(!failure.detail.contains("S3CRET"), "{failure}");
            assert!(!failure.detail.contains("127.0.0.1"), "{failure}");
        }
    }
    assert!(
        blob.requests()
            .iter()
            .all(|request| !request.target.starts_with("/elsewhere")),
        "a redirect is not followed, so the token goes nowhere else"
    );
    let offline = BlobRemote::new(&sas(FakeBlob::closed_url()))
        .download(None)
        .unwrap_err();
    assert_eq!(offline.kind, FailureKind::Offline);
    assert!(!offline.detail.contains("S3CRET"), "{offline}");
}

#[test]
fn two_profiles_show_the_same_tasks_after_husker_sync() {
    let blob = FakeBlob::start();
    let laptop = profile(&blob.url());
    let desktop = profile(&blob.url());

    assert_eq!(
        ok(laptop.path(), &["sync"]),
        "Synced. Received 0, sent 1.\n"
    );
    ok(laptop.path(), &["list-add", "Work"]);
    ok(
        laptop.path(),
        &["add", "Buy milk #shopping", "--due", "2026-11-20"],
    );
    ok(laptop.path(), &["add", "Write report", "--list", "Work"]);
    ok(laptop.path(), &["done", "1"]);
    assert_eq!(
        ok(laptop.path(), &["sync"]),
        "Synced. Received 0, sent 3.\n"
    );
    assert_eq!(
        ok(laptop.path(), &["sync"]),
        "Synced. Received 0, sent 0.\n"
    );

    // The second computer starts empty and receives the lists and tasks with the same IDs.
    let report = json_output(desktop.path(), &["sync"]);
    assert_eq!(report, json!({"received": 4, "sent": 0, "conflicts": 0}));
    let strip = |mut tasks: Value| -> Value {
        for task in tasks.as_array_mut().unwrap() {
            task.as_object_mut().unwrap().remove("version");
        }
        tasks
    };
    assert_eq!(
        strip(json_output(desktop.path(), &["tasks", "s:all"])),
        strip(json_output(laptop.path(), &["tasks", "s:all"]))
    );
    assert_eq!(
        json_output(desktop.path(), &["lists"]),
        json_output(laptop.path(), &["lists"])
    );
    let milk = json_output(desktop.path(), &["show", "1"]);
    assert_eq!(milk["status"], "done");
    assert_eq!(milk["due"], "2026-11-20");
    assert!(milk.get("uid").is_none());

    // A change on the second computer reaches the first.
    ok(desktop.path(), &["add", "Call the bank"]);
    ok(desktop.path(), &["reopen", "1"]);
    assert_eq!(
        ok(desktop.path(), &["sync"]),
        "Synced. Received 0, sent 2.\n"
    );
    assert_eq!(
        ok(laptop.path(), &["sync"]),
        "Synced. Received 2, sent 0.\n"
    );
    assert_eq!(json_output(laptop.path(), &["show", "1"])["status"], "open");
    assert_eq!(
        json_output(laptop.path(), &["show", "3"])["title"],
        "Call the bank"
    );

    // Both computers rename one task before syncing: both titles survive.
    ok(
        laptop.path(),
        &["edit", "3", "--title", "Call the bank today"],
    );
    ok(desktop.path(), &["edit", "3", "--title", "Email the bank"]);
    ok(laptop.path(), &["sync"]);
    assert_eq!(
        ok(desktop.path(), &["sync"]),
        "Synced. Received 2, sent 2. Kept both versions of 1.\n"
    );
    ok(laptop.path(), &["sync"]);
    for computer in [&laptop, &desktop] {
        assert_eq!(
            json_output(computer.path(), &["show", "3"])["title"],
            "Call the bank today"
        );
        let copies = json_output(computer.path(), &["tasks", "#conflict"]);
        assert_eq!(copies.as_array().unwrap().len(), 1);
        assert_eq!(copies[0]["title"], "Email the bank #conflict");
    }
}

#[test]
fn husker_sync_reports_missing_settings_bad_settings_and_no_network() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("never-created");
    let output = husker(&missing, &["sync"]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("sync.toml"));
    assert!(!missing.exists(), "a failed sync creates no profile");

    let insecure = profile("http://example.com/tasks.json?sig=S3CRET");
    let output = husker(insecure.path(), &["sync"]);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(!output.status.success());
    assert!(stderr.contains("sync.toml"), "{stderr}");
    assert!(!stderr.contains("S3CRET"), "{stderr}");
    assert!(!stderr.contains("example.com"), "{stderr}");
    assert!(!insecure.path().join("tasks.db").exists());

    // Without a network the task stays local, and other commands still work.
    let offline = profile(&FakeBlob::closed_url());
    ok(offline.path(), &["add", "Saved without a network"]);
    let output = husker(offline.path(), &["sync"]);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!stderr.contains("S3CRET"), "{stderr}");
    assert_eq!(
        json_output(offline.path(), &["tasks"])[0]["title"],
        "Saved without a network"
    );
}

type Client = RunningService<RoleClient, ()>;

async fn connect(profile: &Path, arguments: &[&str]) -> Client {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_husker"));
    command
        .arg("--profile")
        .arg(profile)
        .arg("mcp")
        .args(arguments)
        .stderr(Stdio::null());
    let transport = TokioChildProcess::new(command).unwrap();
    timeout(
        WAIT,
        ().serve_with_lifecycle(transport, ClientLifecycleMode::Initialize),
    )
    .await
    .expect("MCP handshake timed out")
    .expect("MCP handshake failed")
}

async fn tool(client: &Client, name: &str, arguments: Value) -> Value {
    let request = CallToolRequestParams::new(name.to_owned())
        .with_arguments(arguments.as_object().unwrap().clone());
    let result = timeout(WAIT, client.call_tool(request))
        .await
        .expect("Tool call timed out")
        .unwrap();
    assert_ne!(result.is_error, Some(true), "{result:?}");
    result.structured_content.unwrap()
}

fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + WAIT;
    while !condition() {
        assert!(Instant::now() < deadline, "timed out waiting until {what}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_agent_on_a_new_machine_reads_and_writes_a_synced_list() {
    let blob = FakeBlob::start();
    let laptop = profile(&blob.url());
    ok(laptop.path(), &["list-add", "Husker"]);
    ok(laptop.path(), &["add", "Design sync", "--list", "Husker"]);
    ok(laptop.path(), &["add", "Private errand"]);
    ok(laptop.path(), &["sync"]);

    // The VM profile is new. The list exists only in the sync file until the server starts.
    let vm = profile(&blob.url());
    let client = connect(vm.path(), &["--list", "Husker"]).await;
    let tasks = tool(&client, "list_tasks", json!({})).await;
    assert_eq!(tasks["total"], 1);
    assert_eq!(tasks["tasks"][0]["title"], "Design sync");
    assert_eq!(tasks["tasks"][0]["id"], 1);

    tool(&client, "add_task", json!({"title": "Added by the agent"})).await;
    wait_until("the agent's task is uploaded", || {
        blob.titles().contains(&"Added by the agent".to_owned())
    });
    assert_eq!(
        ok(laptop.path(), &["sync"]),
        "Synced. Received 1, sent 0.\n"
    );
    let added = json_output(laptop.path(), &["tasks", "agent"]);
    assert_eq!(added[0]["title"], "Added by the agent");
    assert_eq!(added[0]["list_id"], 2);

    // A task from the laptop reaches a server that starts later.
    timeout(WAIT, client.cancel()).await.unwrap().unwrap();
    ok(laptop.path(), &["add", "Second task", "--list", "Husker"]);
    ok(laptop.path(), &["sync"]);
    let client = connect(vm.path(), &["--list", "Husker"]).await;
    let tasks = tool(&client, "list_tasks", json!({})).await;
    assert_eq!(tasks["total"], 3);
    timeout(WAIT, client.cancel()).await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_read_only_server_makes_no_request_and_an_offline_server_serves_local_tasks() {
    let blob = FakeBlob::start();
    let laptop = profile(&blob.url());
    ok(laptop.path(), &["add", "Local task"]);
    ok(laptop.path(), &["sync"]);
    blob.clear_requests();
    let client = connect(laptop.path(), &["--read-only"]).await;
    let tasks = tool(&client, "list_tasks", json!({})).await;
    assert_eq!(tasks["total"], 1);
    timeout(WAIT, client.cancel()).await.unwrap().unwrap();
    assert!(blob.requests().is_empty(), "a read-only server never syncs");

    let offline = profile(&FakeBlob::closed_url());
    ok(offline.path(), &["add", "Kept locally"]);
    let client = connect(offline.path(), &[]).await;
    let tasks = tool(&client, "list_tasks", json!({})).await;
    assert_eq!(tasks["tasks"][0]["title"], "Kept locally");
    timeout(WAIT, client.cancel()).await.unwrap().unwrap();
}
