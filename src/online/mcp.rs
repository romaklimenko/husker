//! The Husker MCP tools on top of the sync file.
//!
//! The tools have the names, descriptions, and hints of the local server in
//! `crate::mcp`. IDs are the `uid` of a record and versions are content
//! tokens, because a server without a database has no stable integers.

use std::sync::Arc;

use anyhow::{Result, bail, ensure};
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
        ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
    },
    schemars::JsonSchema,
    service::RequestContext,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

use super::{
    access::random_hex,
    doc::{Access, Changed, Op, Stamp},
    engine::Engine,
    view::{self, ListView, TaskView},
};
use crate::{
    mcp::{SHARED_INSTRUCTIONS, available_tools as local_tools},
    model::Status,
    query::{Query, parse_date},
};

const ADD_TASK: &str = "Create an open task. Omit list_id to use the restricted list or the default list. \
     Dates use YYYY-MM-DD. Hashtags belong in the title or description.";

/// One MCP server for one request. Its tools and instructions match the access.
#[derive(Clone)]
pub struct OnlineServer {
    engine: Arc<Engine>,
    access: Access,
    zone: Tz,
    tools: Arc<Vec<Tool>>,
    instructions: String,
}

impl OnlineServer {
    pub fn new(engine: Arc<Engine>, access: Access, zone: Tz) -> Self {
        let scope = match &access.list {
            Some(uid) => format!(
                "This server is restricted to list ID {uid}. Other lists are inaccessible. \
                 Request another server configuration if you need a different list."
            ),
            None => "This server has access to every list in the Husker sync file.".into(),
        };
        let mode = if access.read_only {
            "This server is read-only. Only list_lists, list_tasks, and get_task are available."
        } else {
            "This server permits writes through its advertised tools."
        };
        let instructions = format!(
            "{scope} {mode} {SHARED_INSTRUCTIONS} \
             Task IDs, list IDs, and task versions are text. \
             They differ from the numbers a computer shows. \
             A version changes when any device changes the task. \
             Computers receive a change within about a minute. \
             Dates use the time zone {zone}."
        );
        Self {
            tools: Arc::new(online_tools(&access)),
            engine,
            access,
            zone,
            instructions,
        }
    }

    pub fn instructions(&self) -> &str {
        &self.instructions
    }
}

impl ServerHandler for OnlineServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("husker", env!("CARGO_PKG_VERSION")))
            .with_instructions(self.instructions.clone())
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        if request.is_some_and(|params| params.cursor.is_some()) {
            return Err(ErrorData::invalid_params("Invalid tools cursor.", None));
        }
        Ok(ListToolsResult::with_all_items(self.tools.as_ref().clone()))
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tools.iter().find(|tool| tool.name == name).cloned()
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        if self.get_tool(&request.name).is_none() {
            return Err(ErrorData::invalid_params(
                format!("Unknown or unavailable tool '{}'.", request.name),
                None,
            ));
        }
        let engine = Arc::clone(&self.engine);
        let access = self.access.clone();
        let zone = self.zone;
        // The storage client blocks, so the tool runs off the async threads.
        let result = tokio::task::spawn_blocking(move || {
            execute(
                &engine,
                &access,
                &stamp_at(Utc::now(), zone),
                &request.name,
                Value::Object(request.arguments.unwrap_or_default()),
            )
        })
        .await
        .map_err(|_| ErrorData::internal_error("The tool worker failed.", None))?;
        let result = match result {
            Ok(value) => CallToolResult::structured(value),
            Err(error) => CallToolResult::error(vec![ContentBlock::text(error.to_string())]),
        };
        Ok(result.into())
    }
}

/// The date and time of a request that arrives at `now`. The date is the one in `zone`.
pub fn stamp_at(now: DateTime<Utc>, zone: Tz) -> Stamp {
    Stamp {
        today: now.with_timezone(&zone).date_naive(),
        now,
    }
}

/// The local tools of this access, with the online argument and result schemas.
fn online_tools(access: &Access) -> Vec<Tool> {
    local_tools(access.list.as_ref().map(|_| 0), access.read_only)
        .into_iter()
        .map(|tool| match tool.name.as_ref() {
            "list_lists" => tool
                .with_input_schema::<EmptyArgs>()
                .with_output_schema::<ListsResult>(),
            "list_tasks" => tool
                .with_input_schema::<ListTasksArgs>()
                .with_output_schema::<TasksResult>(),
            "get_task" => tool
                .with_input_schema::<TaskIdArgs>()
                .with_output_schema::<TaskResult>(),
            "add_task" => {
                let mut tool = tool
                    .with_input_schema::<AddTaskArgs>()
                    .with_output_schema::<TaskResult>();
                tool.description = Some(ADD_TASK.into());
                tool
            }
            "update_task" => tool
                .with_input_schema::<UpdateTaskArgs>()
                .with_output_schema::<TaskResult>(),
            "set_task_status" => tool
                .with_input_schema::<SetTaskStatusArgs>()
                .with_output_schema::<TaskResult>(),
            "trash_task" | "restore_task" => tool
                .with_input_schema::<TaskVersionArgs>()
                .with_output_schema::<TaskResult>(),
            "create_list" => tool
                .with_input_schema::<CreateListArgs>()
                .with_output_schema::<ListResult>(),
            "rename_list" => tool
                .with_input_schema::<RenameListArgs>()
                .with_output_schema::<ListResult>(),
            name => unreachable!("the local tool '{name}' has no online schema"),
        })
        .collect()
}

/// Runs one tool against the sync file. `stamp` is the date and time of the request.
pub fn execute(
    engine: &Engine,
    access: &Access,
    stamp: &Stamp,
    name: &str,
    arguments: Value,
) -> Result<Value> {
    match name {
        "list_lists" => {
            let _: EmptyArgs = parse_arguments(arguments)?;
            let snapshot = engine.read()?;
            let (lists, trash_count) = view::lists(&snapshot, access);
            Ok(serde_json::to_value(ListsResult { lists, trash_count })?)
        }
        "list_tasks" => {
            let args: ListTasksArgs = parse_arguments(arguments)?;
            ensure!(
                (1..=200).contains(&args.limit),
                "limit must be from 1 through 200."
            );
            let query = Query::parse(&args.query)?;
            let snapshot = engine.read()?;
            let tasks = view::tasks(&snapshot, access, &query, args.trash, stamp.today);
            let total = tasks.len();
            let page: Vec<TaskView> = tasks
                .into_iter()
                .skip(args.offset)
                .take(args.limit)
                .collect();
            let end = args.offset.saturating_add(page.len());
            Ok(serde_json::to_value(TasksResult {
                tasks: page,
                total,
                offset: args.offset,
                next_offset: (end < total).then_some(end),
            })?)
        }
        "get_task" => {
            let args: TaskIdArgs = parse_arguments(arguments)?;
            let snapshot = engine.read()?;
            task_result(view::task(&snapshot, access, &args.id)?)
        }
        "add_task" => {
            let args: AddTaskArgs = parse_arguments(arguments)?;
            let op = Op::AddTask {
                uid: random_hex(16)?,
                list: args.list_id,
                title: args.title,
                description: args.description,
                due: args.due.as_deref().map(parse_date).transpose()?,
            };
            changed_task(engine, access, stamp, &op)
        }
        "update_task" => {
            let args: UpdateTaskArgs = parse_arguments(arguments)?;
            ensure!(
                !(args.due.is_some() && args.clear_due),
                "Do not supply due with clear_due=true."
            );
            let due = if args.clear_due {
                Some(None)
            } else {
                args.due.as_deref().map(parse_date).transpose()?.map(Some)
            };
            let op = Op::UpdateTask {
                uid: args.id,
                version: Some(args.version),
                list: args.list_id,
                title: args.title,
                description: args.description,
                due,
            };
            changed_task(engine, access, stamp, &op)
        }
        "set_task_status" => {
            let args: SetTaskStatusArgs = parse_arguments(arguments)?;
            let op = Op::SetStatus {
                uid: args.id,
                version: Some(args.version),
                status: args.status,
            };
            changed_task(engine, access, stamp, &op)
        }
        "trash_task" | "restore_task" => {
            let args: TaskVersionArgs = parse_arguments(arguments)?;
            let op = Op::SetDeleted {
                uid: args.id,
                version: Some(args.version),
                deleted: name == "trash_task",
            };
            changed_task(engine, access, stamp, &op)
        }
        "create_list" => {
            let args: CreateListArgs = parse_arguments(arguments)?;
            let op = Op::CreateList {
                uid: random_hex(16)?,
                name: args.name,
            };
            changed_list(engine, access, stamp, &op)
        }
        "rename_list" => {
            let args: RenameListArgs = parse_arguments(arguments)?;
            let op = Op::RenameList {
                uid: args.id,
                name: args.name,
            };
            changed_list(engine, access, stamp, &op)
        }
        _ => bail!("Unknown or unavailable tool '{name}'."),
    }
}

fn changed_task(engine: &Engine, access: &Access, stamp: &Stamp, op: &Op) -> Result<Value> {
    match engine.write(access, op, stamp)?.0 {
        Changed::Task(task) => task_result(TaskView::from(&task)),
        Changed::List(_) => bail!("The tool returned a list instead of a task."),
    }
}

fn changed_list(engine: &Engine, access: &Access, stamp: &Stamp, op: &Op) -> Result<Value> {
    let (changed, snapshot) = engine.write(access, op, stamp)?;
    let Changed::List(list) = changed else {
        bail!("The tool returned a task instead of a list.");
    };
    // The count comes from the file the change produced.
    let list = view::list(&snapshot, access, &list.uid).unwrap_or(ListView {
        id: list.uid,
        name: list.name,
        count: 0,
    });
    Ok(serde_json::to_value(ListResult { list })?)
}

fn parse_arguments<T: DeserializeOwned>(arguments: Value) -> Result<T> {
    serde_json::from_value(arguments).map_err(|error| anyhow::anyhow!("Invalid arguments: {error}"))
}

fn task_result(task: TaskView) -> Result<Value> {
    Ok(serde_json::to_value(TaskResult { task })?)
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EmptyArgs {}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ListTasksArgs {
    #[serde(default)]
    query: String,
    #[serde(default)]
    trash: bool,
    #[serde(default)]
    offset: usize,
    #[serde(default = "default_limit")]
    #[schemars(range(min = 1, max = 200))]
    limit: usize,
}

fn default_limit() -> usize {
    100
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskIdArgs {
    /// The text ID of the task.
    id: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct TaskVersionArgs {
    /// The text ID of the task.
    id: String,
    /// The version text from the last read of the task.
    version: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AddTaskArgs {
    title: String,
    /// The text ID of the list.
    list_id: Option<String>,
    #[serde(default)]
    description: String,
    due: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct UpdateTaskArgs {
    /// The text ID of the task.
    id: String,
    /// The version text from the last read of the task.
    version: String,
    title: Option<String>,
    /// The text ID of the list.
    list_id: Option<String>,
    description: Option<String>,
    due: Option<String>,
    #[serde(default)]
    clear_due: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SetTaskStatusArgs {
    /// The text ID of the task.
    id: String,
    /// The version text from the last read of the task.
    version: String,
    status: Status,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CreateListArgs {
    name: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RenameListArgs {
    /// The text ID of the list.
    id: String,
    name: String,
}

#[derive(Serialize, JsonSchema)]
struct ListsResult {
    lists: Vec<ListView>,
    trash_count: usize,
}

#[derive(Serialize, JsonSchema)]
struct TasksResult {
    tasks: Vec<TaskView>,
    total: usize,
    offset: usize,
    next_offset: Option<usize>,
}

#[derive(Serialize, JsonSchema)]
struct TaskResult {
    task: TaskView,
}

#[derive(Serialize, JsonSchema)]
struct ListResult {
    list: ListView,
}
