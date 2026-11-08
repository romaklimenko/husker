use std::{path::Path, process::Stdio, time::Duration};

use husker::{
    model::{Status, Task, TaskInput},
    store::Store,
};
use rmcp::{
    ClientLifecycleMode, ClientServiceExt, RoleClient,
    model::{CallToolRequestParams, CallToolResult, ProtocolVersion},
    service::RunningService,
    transport::TokioChildProcess,
};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    time::timeout,
};

type Client = RunningService<RoleClient, ()>;
const WAIT: Duration = Duration::from_secs(15);

struct Fixture {
    directory: tempfile::TempDir,
    store: Store,
    private: i64,
    work: i64,
    visible: Task,
    hidden: Task,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let mut store = Store::open(&directory.path().join("tasks.db")).unwrap();
        let private = store.create_list("Private").unwrap().id;
        let work = store.create_list("Work").unwrap().id;
        let visible = store
            .create_task(&TaskInput {
                list_id: private,
                title: "Køb æbler".into(),
                description: "#shopping".into(),
                ..TaskInput::default()
            })
            .unwrap();
        let hidden = store
            .create_task(&TaskInput {
                list_id: work,
                title: "Hidden work task".into(),
                ..TaskInput::default()
            })
            .unwrap();
        for (list_id, title) in [(private, "Private trash"), (work, "Hidden trash")] {
            let task = store
                .create_task(&TaskInput {
                    list_id,
                    title: title.into(),
                    ..TaskInput::default()
                })
                .unwrap();
            store.set_deleted(task.id, task.version, true).unwrap();
        }
        Self {
            directory,
            store,
            private,
            work,
            visible,
            hidden,
        }
    }

    fn path(&self) -> &Path {
        self.directory.path()
    }
}

async fn connect(profile: &Path, args: &[&str], discover: bool) -> Client {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_husker"));
    command.arg("--profile").arg(profile).arg("mcp").args(args);
    let transport = TokioChildProcess::new(command).unwrap();
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

async fn rejected(client: &Client, name: &str, arguments: Value) {
    let request = CallToolRequestParams::new(name.to_owned())
        .with_arguments(arguments.as_object().unwrap().clone());
    let result = timeout(WAIT, client.call_tool(request))
        .await
        .expect("Rejected call timed out");
    assert!(
        result.is_err() || result.unwrap().is_error == Some(true),
        "Unexpected successful call: {name}"
    );
}

async fn close(client: Client) {
    timeout(WAIT, client.cancel())
        .await
        .expect("MCP shutdown timed out")
        .unwrap();
}

#[tokio::test]
async fn legacy_client_cannot_read_or_write_outside_its_list() {
    let mut fixture = Fixture::new();
    let client = connect(fixture.path(), &["--list", "private"], false).await;
    let tools = timeout(WAIT, client.list_all_tools())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(tools.len(), 8);
    assert!(
        !tools
            .iter()
            .any(|tool| tool.name == "create_list" || tool.name == "rename_list")
    );
    assert!(tools.iter().all(|tool| tool.output_schema.is_some()));
    let instructions = client.peer_info().unwrap().instructions.clone().unwrap();
    assert!(instructions.contains(&fixture.private.to_string()));
    let lists = data(&client, "list_lists", json!({})).await;
    assert_eq!(lists["lists"].as_array().unwrap().len(), 1);
    assert_eq!(lists["lists"][0]["id"], fixture.private);
    assert_eq!(lists["trash_count"], 1);
    let matches = data(&client, "list_tasks", json!({"query":"ÆBLER #shopping"})).await;
    assert_eq!(matches["tasks"][0]["id"], fixture.visible.id);
    assert_eq!(
        data(
            &client,
            "list_tasks",
            json!({"query":"l:Work s:all", "trash":true})
        )
        .await["total"],
        0
    );
    let outside = call(&client, "get_task", json!({"id":fixture.hidden.id})).await;
    let missing = call(&client, "get_task", json!({"id":99999})).await;
    assert_eq!(outside.is_error, Some(true));
    assert_eq!(outside.content, missing.content);
    rejected(
        &client,
        "update_task",
        json!({"id":fixture.visible.id,"version":1,"list_id":fixture.work}),
    )
    .await;
    rejected(
        &client,
        "add_task",
        json!({"title":"Escape", "list_id":fixture.work}),
    )
    .await;
    rejected(
        &client,
        "rename_list",
        json!({"id":fixture.private,"name":"Escape"}),
    )
    .await;
    for name in ["set_task_status", "trash_task", "restore_task"] {
        let mut arguments = json!({"id":fixture.hidden.id,"version":fixture.hidden.version});
        if name == "set_task_status" {
            arguments["status"] = json!("done");
        }
        rejected(&client, name, arguments).await;
    }
    fixture.store.rename_list(fixture.private, "Home").unwrap();
    fixture.store.create_list("Private").unwrap();
    let renamed = data(&client, "list_lists", json!({})).await;
    assert_eq!(renamed["lists"][0]["id"], fixture.private);
    assert_eq!(renamed["lists"][0]["name"], "Home");
    assert_eq!(
        fixture.store.task(fixture.hidden.id).unwrap(),
        fixture.hidden
    );
    close(client).await;
}

#[tokio::test]
async fn current_discovery_handles_versioned_task_workflow_and_pagination() {
    let fixture = Fixture::new();
    let client = connect(fixture.path(), &["--list", "Private"], true).await;
    let task = data(
        &client,
        "add_task",
        json!({"title":"New task", "due":"2026-11-08", "description":"#Shopping"}),
    )
    .await["task"]
        .clone();
    let id = task["id"].as_i64().unwrap();
    assert_eq!(task["list_id"], fixture.private);
    let edited = data(
        &client,
        "update_task",
        json!({"id":id,"version":1,"description":"Two\nlines","clear_due":true}),
    )
    .await["task"]
        .clone();
    assert_eq!(edited["description"], "Two\nlines");
    assert!(edited["due"].is_null());
    assert!(edited.get("ready").is_none());
    assert!(edited.get("tags").is_none());
    rejected(
        &client,
        "update_task",
        json!({"id":id,"version":1,"title":"Stale"}),
    )
    .await;
    rejected(
        &client,
        "update_task",
        json!({"id":id,"title":"Missing version"}),
    )
    .await;
    let done = data(
        &client,
        "set_task_status",
        json!({"id":id,"version":2,"status":"done"}),
    )
    .await["task"]
        .clone();
    assert_eq!(done["status"], "done");
    assert!(done["status_changed_on"].is_string());
    assert_eq!(data(&client, "list_tasks", json!({})).await["total"], 2);
    assert_eq!(
        data(&client, "list_tasks", json!({"query":"s:open"})).await["total"],
        1
    );
    let deleted = data(&client, "trash_task", json!({"id":id,"version":3})).await["task"].clone();
    assert_eq!(deleted["deleted"], true);
    let restored =
        data(&client, "restore_task", json!({"id":id,"version":4})).await["task"].clone();
    assert_eq!(restored["status"], "done");
    assert_eq!(restored["version"], 5);
    let first = data(&client, "list_tasks", json!({"query":"s:all","limit":1})).await;
    assert_eq!(first["total"], 2);
    assert_eq!(first["next_offset"], 1);
    let second = data(
        &client,
        "list_tasks",
        json!({"query":"s:all","limit":1,"offset":1}),
    )
    .await;
    assert!(second["next_offset"].is_null());
    assert_ne!(first["tasks"][0]["id"], second["tasks"][0]["id"]);
    assert_eq!(
        data(&client, "list_tasks", json!({"offset":1000})).await["tasks"],
        json!([])
    );
    for args in [
        json!({"limit":0}),
        json!({"limit":201}),
        json!({"offset":-1}),
        json!({"profile":"other"}),
    ] {
        rejected(&client, "list_tasks", args).await;
    }
    rejected(&client, "get_task", json!({"id":id,"scope":"all"})).await;
    for args in [
        json!({"title":"Removed field", "tags":["old"]}),
        json!({"title":"Removed field", "readiness":"maybe"}),
    ] {
        rejected(&client, "add_task", args).await;
    }
    let reopened = data(
        &client,
        "set_task_status",
        json!({"id":id,"version":5,"status":"open"}),
    )
    .await;
    assert!(reopened["task"]["status_changed_on"].is_null());
    close(client).await;
}

#[tokio::test]
async fn read_only_discovery_and_direct_calls_cannot_mutate() {
    let fixture = Fixture::new();
    let client = connect(fixture.path(), &["--list", "Private", "--read-only"], false).await;
    let mut names = client
        .list_all_tools()
        .await
        .unwrap()
        .into_iter()
        .map(|tool| tool.name.to_string())
        .collect::<Vec<_>>();
    names.sort();
    assert_eq!(names, ["get_task", "list_lists", "list_tasks"]);
    assert!(
        client
            .peer_info()
            .unwrap()
            .instructions
            .as_ref()
            .unwrap()
            .to_lowercase()
            .contains("read-only")
    );
    assert_eq!(
        data(&client, "get_task", json!({"id":fixture.visible.id})).await["task"]["title"],
        "Køb æbler"
    );
    let baseline = fixture.store.tasks().unwrap();
    let lists_before = fixture.store.lists().unwrap();
    for (name, arguments) in [
        ("add_task", json!({"title":"New task"})),
        (
            "update_task",
            json!({"id":fixture.visible.id,"version":1,"title":"Overwrite"}),
        ),
        (
            "set_task_status",
            json!({"id":fixture.visible.id,"version":1,"status":"done"}),
        ),
        ("trash_task", json!({"id":fixture.visible.id,"version":1})),
        ("restore_task", json!({"id":fixture.visible.id,"version":1})),
        ("create_list", json!({"name":"New"})),
        (
            "rename_list",
            json!({"id":fixture.private,"name":"Renamed"}),
        ),
    ] {
        rejected(&client, name, arguments).await;
    }
    assert_eq!(fixture.store.tasks().unwrap(), baseline);
    assert_eq!(fixture.store.lists().unwrap(), lists_before);
    close(client).await;
}

#[tokio::test]
async fn unscoped_server_can_manage_lists_and_live_moves_cannot_bypass_scope() {
    let mut fixture = Fixture::new();
    let unrestricted = connect(fixture.path(), &[], true).await;
    assert_eq!(unrestricted.list_all_tools().await.unwrap().len(), 10);
    let created =
        data(&unrestricted, "create_list", json!({"name":"Agent list"})).await["list"].clone();
    let renamed = data(
        &unrestricted,
        "rename_list",
        json!({"id":created["id"],"name":"Agent work"}),
    )
    .await;
    assert_eq!(renamed["list"]["name"], "Agent work");
    let created_id = created["id"].as_i64().unwrap();
    fixture.store.move_list(created_id, false).unwrap();
    let list_result = data(&unrestricted, "list_lists", json!({})).await;
    let ids = list_result["lists"]
        .as_array()
        .unwrap()
        .iter()
        .map(|list| list["id"].as_i64().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(ids, [1, fixture.private, created_id, fixture.work]);
    let default_task = data(&unrestricted, "add_task", json!({"title":"Default list"})).await;
    assert_eq!(default_task["task"]["list_id"], 1);
    let scoped = connect(fixture.path(), &["--list", "Private"], false).await;
    let mut fields = TaskInput::from(&fixture.visible);
    fields.list_id = fixture.work;
    let moved = fixture
        .store
        .update_task(fixture.visible.id, fixture.visible.version, &fields)
        .unwrap();
    for version in [fixture.visible.version, moved.version] {
        rejected(
            &scoped,
            "update_task",
            json!({"id":moved.id,"version":version,"title":"Must not save"}),
        )
        .await;
    }
    assert_eq!(fixture.store.task(moved.id).unwrap(), moved);
    close(scoped).await;
    close(unrestricted).await;
}

#[tokio::test]
async fn startup_errors_stay_on_stderr_and_do_not_create_readonly_profiles() {
    let fixture = Fixture::new();
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_husker"))
        .arg("--profile")
        .arg(fixture.path())
        .args(["mcp", "--list", "Missing"])
        .output()
        .await
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
    let missing = fixture.path().join("missing");
    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_husker"))
        .arg("--profile")
        .arg(&missing)
        .args(["mcp", "--read-only"])
        .output()
        .await
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!missing.exists());
}

#[tokio::test]
async fn stdout_is_jsonrpc_and_server_exits_after_stdin_closes() {
    let fixture = Fixture::new();
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_husker"))
        .arg("--profile")
        .arg(fixture.path())
        .args(["mcp", "--list", "Private"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let request = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"husker-test","version":"1"}
    }});
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(format!("{request}\n").as_bytes())
        .await
        .unwrap();
    let mut line = String::new();
    timeout(WAIT, stdout.read_line(&mut line))
        .await
        .unwrap()
        .unwrap();
    let response: Value = serde_json::from_str(&line).expect("stdout must contain JSON-RPC only");
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert!(response["result"]["capabilities"]["tools"].is_object());
    drop(child.stdin.take());
    let status = timeout(WAIT, child.wait())
        .await
        .expect("Server did not exit at EOF")
        .unwrap();
    assert!(status.success());
    assert_eq!(
        fixture.store.task(fixture.visible.id).unwrap().status,
        Status::Open
    );
}
