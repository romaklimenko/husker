use std::{io::IsTerminal, path::PathBuf, process::ExitCode};

use anyhow::{Context, Result, bail};
use chrono::{Local, NaiveDate};
use clap::{Args, Parser, Subcommand, ValueEnum};
use husker::{
    config::Config,
    model::{Status, Task, TaskInput, TaskList},
    query::{Query, visible_tasks},
    store::Store,
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
    /// Create a task. Undated tasks start in Maybe unless --anytime is set.
    Add {
        title: String,
        #[arg(long)]
        list: Option<String>,
        #[arg(long, default_value = "")]
        description: String,
        #[arg(long, value_parser = parse_date)]
        due: Option<NaiveDate>,
        #[arg(long)]
        anytime: bool,
        #[arg(long = "tag")]
        tags: Vec<String>,
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
        #[arg(long, value_enum)]
        readiness: Option<Readiness>,
        /// Replace tags with these values. Repeat --tag for each tag.
        #[arg(long = "tag", conflicts_with = "clear_tags")]
        tags: Vec<String>,
        #[arg(long)]
        clear_tags: bool,
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

#[derive(Clone, Copy, ValueEnum)]
enum Readiness {
    Maybe,
    Anytime,
}

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
    let config_path = cli
        .config
        .clone()
        .unwrap_or_else(|| profile.join("config.toml"));
    let config = Config::load(&config_path, cli.config.is_some())?;
    // Parse queries before creating a profile so invalid input has no side effects.
    let query = if let Some(Command::Tasks { query, .. }) = &cli.command {
        Some(Query::parse(query)?)
    } else {
        None
    };
    std::fs::create_dir_all(&profile)
        .with_context(|| format!("Cannot create profile {}", profile.display()))?;
    let mut store = Store::open(&profile.join("tasks.db"))?;
    let Some(command) = cli.command else {
        return ui::run(&mut store, &config);
    };
    match command {
        Command::Add {
            title,
            list,
            description,
            due,
            anytime,
            tags,
        } => {
            let task = store.create_task(&TaskInput {
                list_id: find_list(&store, list.as_deref())?.id,
                title,
                description,
                due,
                ready: anytime,
                tags,
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
                    "List: {}\nVersion: {}\nReadiness: {}\nTrash: {}",
                    task.list_id,
                    task.version,
                    if task.ready { "anytime" } else { "maybe" },
                    task.deleted
                );
                if !task.tags.is_empty() {
                    println!("Tags: {}", task.tags.join(" "));
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
            readiness,
            tags,
            clear_tags,
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
                input.ready = true;
            }
            if let Some(readiness) = readiness {
                input.ready = matches!(readiness, Readiness::Anytime);
            }
            if clear_tags {
                input.tags.clear();
            }
            if !tags.is_empty() {
                input.tags = tags;
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
        Command::Config { .. } => unreachable!("configuration handled before opening a profile"),
    }
    Ok(())
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
            .next()
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
        .unwrap_or_else(|| if task.ready { "anytime" } else { "maybe" }.to_owned());
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
