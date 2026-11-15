use std::{
    io::IsTerminal,
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Local, NaiveDate, Utc};
use clap::{Args, Parser, Subcommand};
use husker::{
    config::Config,
    mcp,
    model::{Status, Task, TaskInput, TaskList},
    online::{
        self,
        access::{KEY_FILE, OnlineProfile},
    },
    query::{Query, visible_tasks},
    store::Store,
    sync::{
        self, SETTINGS_FILE, Settings,
        remote::BlobRemote,
        worker::{Cadence, SyncWorker},
    },
    ui,
};
use serde::Serialize;

#[derive(Parser)]
#[command(version, about = "Manage local tasks from your terminal.")]
struct Cli {
    /// Store tasks in this directory instead of ~/.husker.
    #[arg(long, global = true)]
    profile: Option<PathBuf>,
    /// Read settings from an existing TOML file.
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    /// Print data as JSON with stable English field names.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Serve task tools to an MCP client over standard input and output.
    Mcp {
        /// Restrict every tool to this existing list for the server's lifetime.
        #[arg(long)]
        list: Option<String>,
        /// Expose only read tools and open an existing database read-only.
        #[arg(long)]
        read_only: bool,
    },
    /// Create a task with an optional due date.
    Add {
        title: String,
        #[arg(long)]
        list: Option<String>,
        #[arg(long, default_value = "")]
        description: String,
        #[arg(long, value_parser = parse_date)]
        due: Option<NaiveDate>,
    },
    /// Find tasks, for example: tasks 'l:private milk #shopping'.
    Tasks {
        #[arg(default_value = "")]
        query: String,
        #[arg(long)]
        trash: bool,
    },
    /// Show every field of one task.
    Show { id: i64 },
    /// Change supplied fields. Unspecified fields keep their values.
    Edit {
        #[command(flatten)]
        target: Target,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        list: Option<String>,
        #[arg(long, value_parser = parse_date, conflicts_with = "clear_due")]
        due: Option<NaiveDate>,
        #[arg(long)]
        clear_due: bool,
    },
    /// Mark a task done.
    Done(Target),
    /// Mark a task cancelled.
    Cancel(Target),
    /// Mark a task open.
    Reopen(Target),
    /// Move a task to recoverable Trash.
    Trash(Target),
    /// Restore a task to its original list and status.
    Restore(Target),
    /// Show lists with open-task counts, followed by Trash.
    Lists,
    /// Create a list.
    ListAdd { name: String },
    /// Rename a list without moving its tasks.
    ListRename { id: i64, name: String },
    /// Sync this profile with its sync file now. Needs sync.toml in the profile.
    Sync,
    /// Run the online server. Its settings come from environment variables.
    Serve {
        /// Listen on this port instead of FUNCTIONS_CUSTOMHANDLER_PORT or 8787.
        #[arg(long)]
        port: Option<u16>,
    },
    /// Sign an access token for the online server.
    Token {
        /// A label for the token, such as the device or agent that uses it.
        #[arg(long)]
        name: Option<String>,
        /// Limit the token to this list. Without it the token covers every list.
        #[arg(long)]
        list: Option<String>,
        /// Sign a token that cannot change tasks.
        #[arg(long)]
        read_only: bool,
        /// The lifetime in days, from 1 through 400.
        #[arg(long, default_value_t = 365)]
        days: u32,
        /// The public address of the online server. Husker remembers it.
        #[arg(long)]
        url: Option<String>,
        /// Print the public key for the server settings instead of a token.
        #[arg(long, conflicts_with_all = ["name", "list", "read_only", "days"])]
        public_key: bool,
    },
    /// Print settings without opening a database.
    Config {
        /// Print a complete default configuration.
        #[arg(long, required = true)]
        example: bool,
    },
}

#[derive(Args)]
struct Target {
    id: i64,
    /// Refuse the change unless the task still has this version.
    #[arg(long)]
    if_version: Option<i64>,
}

/// How long an MCP server waits at exit to upload unsent changes.
const EXIT_SYNC_TIMEOUT: Duration = Duration::from_secs(10);

fn parse_date(value: &str) -> Result<NaiveDate, String> {
    husker::query::parse_date(value).map_err(|error| error.to_string())
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    if matches!(cli.command, Some(Command::Config { .. })) {
        print!("{}", Config::example());
        return Ok(());
    }
    if let Some(Command::Serve { port }) = &cli.command {
        // The online server reads no profile, no settings file, and no database.
        return online::serve(*port);
    }
    if cli.command.is_none()
        && (!std::io::stdin().is_terminal() || !std::io::stdout().is_terminal())
    {
        bail!(
            "The task interface needs an interactive terminal. Use 'husker tasks' or 'husker --help'."
        );
    }
    let profile = match cli.profile {
        Some(path) => path,
        None => dirs::home_dir()
            .context("Cannot find your home directory. Set --profile DIRECTORY.")?
            .join(".husker"),
    };
    if let Some(Command::Token {
        name,
        list,
        read_only,
        days,
        url,
        public_key,
    }) = &cli.command
    {
        return token(
            &profile,
            cli.json,
            TokenRequest {
                name: name.as_deref(),
                list: list.as_deref(),
                read_only: *read_only,
                days: *days,
                url: url.as_deref(),
                public_key: *public_key,
            },
        );
    }
    let config_path = cli
        .config
        .clone()
        .unwrap_or_else(|| profile.join("config.toml"));
    let config = Config::load(&config_path, cli.config.is_some())?;
    let database = profile.join("tasks.db");
    if let Some(Command::Mcp { list, read_only }) = &cli.command {
        let mut worker = None;
        if !read_only {
            std::fs::create_dir_all(&profile)
                .with_context(|| format!("Cannot create profile {}", profile.display()))?;
            // A read-only server never syncs, so it never writes to the profile.
            if let Some(settings) = Settings::load(&profile)? {
                let remote = BlobRemote::new(&settings);
                // Sync first, so --list can name a list that another device created.
                let mut store = Store::open(&database)?;
                if let Err(failure) = sync::sync_once(&mut store, &remote) {
                    eprintln!("Sync failed: {failure} Serving the local tasks.");
                }
                drop(store);
                worker = Some(SyncWorker::spawn(
                    database.clone(),
                    Box::new(remote),
                    Cadence::MCP,
                    false,
                    true,
                ));
            }
        }
        let store = Store::open_for_mcp(&database, list.as_deref(), *read_only)?;
        let served = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(mcp::serve(store));
        if let Some(worker) = worker
            && let Some(failure) = worker.finish(EXIT_SYNC_TIMEOUT)
        {
            eprintln!("Sync failed: {failure} The changes stay on this computer.");
        }
        return served;
    }
    // Check the settings before creating a profile so a missing file has no side effects.
    let sync_settings = if matches!(cli.command, Some(Command::Sync)) {
        Some(Settings::load(&profile)?.with_context(|| {
            format!(
                "Sync is not set up. Create {} first. See docs/sync.md.",
                profile.join(SETTINGS_FILE).display()
            )
        })?)
    } else {
        None
    };
    // Parse queries before creating a profile so invalid input has no side effects.
    let query = if let Some(Command::Tasks { query, .. }) = &cli.command {
        Some(Query::parse(query)?)
    } else {
        None
    };
    std::fs::create_dir_all(&profile)
        .with_context(|| format!("Cannot create profile {}", profile.display()))?;
    let mut store = Store::open(&database)?;
    let Some(command) = cli.command else {
        let worker = Settings::load(&profile)?.map(|settings| {
            SyncWorker::spawn(
                database,
                Box::new(BlobRemote::new(&settings)),
                Cadence::TERMINAL,
                true,
                false,
            )
        });
        return ui::run(&mut store, &config, &config_path, worker);
    };
    match command {
        Command::Mcp { .. } => unreachable!("MCP handled before ordinary commands"),
        Command::Add {
            title,
            list,
            description,
            due,
        } => {
            let task = store.create_task(&TaskInput {
                list_id: find_list(&store, list.as_deref())?.id,
                title,
                description,
                due,
            })?;
            print_task(&task, cli.json)?;
        }
        Command::Tasks { trash, .. } => {
            let lists = store.lists()?;
            let tasks = visible_tasks(
                &store.tasks()?,
                &lists,
                &query.expect("task query parsed"),
                None,
                trash,
                Local::now().date_naive(),
            );
            if cli.json {
                print_json(&tasks)?;
            } else if tasks.is_empty() {
                println!("No tasks match.");
            } else {
                for task in tasks {
                    print_task(&task, false)?;
                }
            }
        }
        Command::Show { id } => {
            let task = store.task(id)?;
            if cli.json {
                print_json(&task)?;
            } else {
                print_task(&task, false)?;
                println!(
                    "List: {}\nVersion: {}\nTrash: {}",
                    task.list_id, task.version, task.deleted
                );
                if let Some(date) = task.status_changed_on {
                    println!("Status changed on: {date}");
                }
                if !task.description.is_empty() {
                    println!("\n{}", task.description);
                }
            }
        }
        Command::Edit {
            target,
            title,
            description,
            list,
            due,
            clear_due,
        } => {
            let original = store.task(target.id)?;
            let mut input = TaskInput::from(&original);
            if let Some(title) = title {
                input.title = title;
            }
            if let Some(description) = description {
                input.description = description;
            }
            if let Some(list) = list {
                input.list_id = find_list(&store, Some(&list))?.id;
            }
            if let Some(due) = due {
                input.due = Some(due);
            }
            if clear_due {
                input.due = None;
            }
            let task = store.update_task(
                target.id,
                target.if_version.unwrap_or(original.version),
                &input,
            )?;
            print_task(&task, cli.json)?;
        }
        Command::Done(target) => change_status(&mut store, target, Status::Done, cli.json)?,
        Command::Cancel(target) => change_status(&mut store, target, Status::Cancelled, cli.json)?,
        Command::Reopen(target) => change_status(&mut store, target, Status::Open, cli.json)?,
        Command::Trash(target) => change_deleted(&mut store, target, true, cli.json)?,
        Command::Restore(target) => change_deleted(&mut store, target, false, cli.json)?,
        Command::Lists => {
            let mut lists = store.lists()?;
            lists.push(TaskList {
                id: 0,
                name: "Trash".into(),
                count: store.tasks()?.iter().filter(|task| task.deleted).count(),
            });
            if cli.json {
                print_json(&lists)?;
            } else {
                for list in lists {
                    println!("{}\t{} ({})", list.id, list.name, list.count);
                }
            }
        }
        Command::ListAdd { name } => print_list(&store.create_list(&name)?, cli.json)?,
        Command::ListRename { id, name } => print_list(&store.rename_list(id, &name)?, cli.json)?,
        Command::Sync => {
            let settings = sync_settings.expect("sync settings loaded");
            let report = sync::sync_once(&mut store, &BlobRemote::new(&settings))?;
            if cli.json {
                print_json(&report)?;
            } else {
                print!(
                    "Synced. Received {}, sent {}.",
                    report.received, report.sent
                );
                if report.conflicts > 0 {
                    print!(" Kept both versions of {}.", report.conflicts);
                }
                println!();
            }
        }
        Command::Config { .. } => unreachable!("configuration handled before opening a profile"),
        Command::Serve { .. } | Command::Token { .. } => {
            unreachable!("online commands handled before opening a profile")
        }
    }
    Ok(())
}

struct TokenRequest<'a> {
    name: Option<&'a str>,
    list: Option<&'a str>,
    read_only: bool,
    days: u32,
    url: Option<&'a str>,
    public_key: bool,
}

/// Signs an access token with the profile's key, or prints the public key.
fn token(profile: &Path, json: bool, request: TokenRequest<'_>) -> Result<()> {
    std::fs::create_dir_all(profile)
        .with_context(|| format!("Cannot create profile {}", profile.display()))?;
    let mut online = OnlineProfile::open(profile)?;
    if let Some(url) = request.url {
        online.set_url(profile, url)?;
    }
    if request.public_key {
        println!("{}", online.signer().public_key());
        return Ok(());
    }
    let name = request
        .name
        .context("Name the token with --name NAME, for example --name phone.")?;
    let origin = online.url.clone().with_context(|| {
        format!(
            "Give the address of the online server once with --url URL. Husker stores it in {KEY_FILE}."
        )
    })?;
    let list = match request.list {
        Some(list) => {
            // A token names a list by its sync identifier, which is equal on every device.
            let store = Store::open_for_mcp(&profile.join("tasks.db"), None, true)?;
            let id = find_list(&store, Some(list))?.id;
            Some(store.list_uid(id)?.with_context(|| {
                format!("List '{list}' has no sync identifier yet. Run 'husker sync' first.")
            })?)
        }
        None => None,
    };
    let (claims, token) = online.signer().mint(
        &origin,
        name,
        list,
        !request.read_only,
        i64::from(request.days),
        Utc::now().timestamp(),
    )?;
    let expires = DateTime::from_timestamp(claims.exp, 0)
        .map(|time| time.date_naive().to_string())
        .unwrap_or_default();
    if json {
        print_json(&serde_json::json!({
            "id": claims.id,
            "name": claims.name,
            "list": claims.list,
            "write": claims.write,
            "expires": expires,
            "token": token,
        }))
    } else {
        println!("{token}");
        eprintln!("Token {} expires on {expires}.", claims.id);
        Ok(())
    }
}

fn find_list(store: &Store, name: Option<&str>) -> Result<TaskList> {
    let lists = store.lists()?;
    match name {
        Some(name) => lists
            .into_iter()
            .find(|list| list.name.to_lowercase() == name.trim().to_lowercase())
            .with_context(|| {
                format!("List '{name}' does not exist. Create it with 'husker list-add'.")
            }),
        None => lists
            .into_iter()
            .min_by_key(|list| list.id)
            .context("No list exists. Create one with 'husker list-add'."),
    }
}

fn change_status(store: &mut Store, target: Target, status: Status, json: bool) -> Result<()> {
    let version = target.if_version.unwrap_or(store.task(target.id)?.version);
    print_task(&store.set_status(target.id, version, status)?, json)
}

fn change_deleted(store: &mut Store, target: Target, deleted: bool, json: bool) -> Result<()> {
    let version = target.if_version.unwrap_or(store.task(target.id)?.version);
    print_task(&store.set_deleted(target.id, version, deleted)?, json)
}

fn print_task(task: &Task, json: bool) -> Result<()> {
    if json {
        return print_json(task);
    }
    let due = task
        .due
        .map(|date| date.to_string())
        .unwrap_or_else(|| "undated".to_owned());
    println!("{}\t{}\t{}\t{}", task.id, task.status, due, task.title);
    Ok(())
}

fn print_list(list: &TaskList, json: bool) -> Result<()> {
    if json {
        print_json(list)
    } else {
        println!("{}\t{} ({})", list.id, list.name, list.count);
        Ok(())
    }
}

fn print_json(value: &impl Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
