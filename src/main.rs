use std::{
    io::IsTerminal,
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Local, NaiveDate, Utc};
use clap::{Args, Parser, Subcommand};
use crossterm::style::{Color, Colors, Print, ResetColor, SetColors};
use husker::{
    config::Config,
    mcp,
    model::{Status, Task, TaskInput, TaskList},
    online::{
        self,
        access::{Claims, KEY_FILE, OnlineProfile, normalize_origin},
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
    /// Pair a phone with the online server: show a QR code with a pairing code that works once.
    Pair {
        /// A label for the phone's access token.
        #[arg(long, default_value = "phone")]
        name: String,
        /// Limit the phone to this list. Without it the phone reaches every list.
        #[arg(long)]
        list: Option<String>,
        /// Sign an access token that cannot change tasks.
        #[arg(long)]
        read_only: bool,
        /// The lifetime of the access token in days, from 1 through 400.
        #[arg(long, default_value_t = 365)]
        days: u32,
        /// The public address of the online server. Husker remembers it.
        #[arg(long)]
        url: Option<String>,
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
    if let Some(Command::Pair {
        name,
        list,
        read_only,
        days,
        url,
    }) = &cli.command
    {
        return pair(
            &profile,
            cli.json,
            TokenRequest {
                name: Some(name),
                list: list.as_deref(),
                read_only: *read_only,
                days: *days,
                url: url.as_deref(),
                public_key: false,
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
        Command::Serve { .. } | Command::Token { .. } | Command::Pair { .. } => {
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
    let (_, claims, token) = sign(profile, &online, &request)?;
    let expires = expiry_date(&claims);
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

fn expiry_date(claims: &Claims) -> String {
    DateTime::from_timestamp(claims.exp, 0)
        .map(|time| time.date_naive().to_string())
        .unwrap_or_default()
}

/// Signs an access token for the server address of the profile. Returns the address with it.
fn sign(
    profile: &Path,
    online: &OnlineProfile,
    request: &TokenRequest<'_>,
) -> Result<(String, Claims, String)> {
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
    Ok((origin, claims, token))
}

/// How long `husker pair` waits for the online server, which may have to start first.
const PAIR_TIMEOUT: Duration = Duration::from_secs(40);

/// Signs an access token for a phone and hands it to the online server under a pairing code.
///
/// The access token never reaches the screen. The phone gets it from the
/// server in exchange for the code, once and within five minutes.
fn pair(profile: &Path, json: bool, request: TokenRequest<'_>) -> Result<()> {
    std::fs::create_dir_all(profile)
        .with_context(|| format!("Cannot create profile {}", profile.display()))?;
    let mut online = OnlineProfile::open(profile)?;
    // A new address is stored after the server answered, so a typing mistake does not stay.
    let stored = online.url.clone();
    if let Some(url) = request.url {
        online.url = Some(normalize_origin(url)?);
    }
    let (origin, claims, token) = sign(profile, &online, &request)?;

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(PAIR_TIMEOUT))
        .http_status_as_error(false)
        // A redirect would send the access token to another address.
        .max_redirects(0)
        // The certificate store of this computer, as `husker sync` uses it.
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into();
    let mut response = agent
        .post(format!("{origin}/api/pairings"))
        .header("Authorization", format!("Bearer {token}"))
        .send_empty()
        .map_err(|error| anyhow::anyhow!("Cannot reach the online server at {origin}: {error}."))?;
    match response.status().as_u16() {
        200 => {}
        401 => bail!(
            "The online server at {origin} refused the access token. \
             The server may hold another public key than this profile's. \
             Deploy the server from this computer, or copy {KEY_FILE} from the computer that deployed it."
        ),
        status => bail!("The online server at {origin} answered with status {status}."),
    }
    let answer: serde_json::Value = response
        .body_mut()
        .read_to_vec()
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .context("The online server sent an answer that Husker cannot read.")?;
    let code = answer["code"]
        .as_str()
        .filter(|code| code.len() == 12 && code.bytes().all(|byte| byte.is_ascii_alphanumeric()))
        .context("The online server sent no pairing code.")?;
    let seconds = answer["expires_in"].as_u64().unwrap_or(300);
    let link = format!("{origin}/#code={code}");
    if online.url != stored {
        online.url = stored;
        online.set_url(profile, &origin)?;
    }

    if json {
        return print_json(&serde_json::json!({
            "code": code,
            "link": link,
            "expires_in": seconds,
            "id": claims.id,
            "expires": expiry_date(&claims),
        }));
    }
    if !std::io::stdout().is_terminal() {
        // The link alone goes to the output. The token ID, which revokes the phone later,
        // goes to the error stream, as `husker token` writes it.
        println!("{link}");
        eprintln!("Token {} expires on {}.", claims.id, expiry_date(&claims));
        return Ok(());
    }
    let mut out = std::io::stdout();
    for line in qr_lines(&link)? {
        // Black on white on every terminal theme: a camera needs dark squares on a light ground.
        crossterm::execute!(
            out,
            SetColors(Colors::new(Color::Black, Color::White)),
            Print(line),
            ResetColor,
            Print("\n"),
        )?;
    }
    let grouped = [&code[..4], &code[4..8], &code[8..]].join("-");
    println!("Scan the code with the phone, or open {origin} and type {grouped}.");
    println!(
        "The code works once, for {} minutes. Token {} expires on {}.",
        seconds / 60,
        claims.id,
        expiry_date(&claims)
    );
    Ok(())
}

/// The QR code of a link as lines of half-block characters, with its empty border.
///
/// One character is one square wide and two squares high. A filled half is a
/// dark square, so the lines must be printed dark on light.
fn qr_lines(link: &str) -> Result<Vec<String>> {
    // The lowest error correction keeps the code small. A screen has no dirt to correct.
    let code = qrcode::QrCode::with_error_correction_level(link.as_bytes(), qrcode::EcLevel::L)
        .context("Cannot draw the QR code.")?;
    let drawing = code
        .render::<qrcode::render::unicode::Dense1x2>()
        .quiet_zone(true)
        .build();
    Ok(drawing.lines().map(str::to_owned).collect())
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

#[cfg(test)]
mod tests {
    use super::qr_lines;

    /// Reads the printed QR code as a camera sees it when the lines are printed dark on light.
    fn read_back(lines: &[String]) -> String {
        // A camera needs several pixels for one square of the code.
        const SCALE: usize = 4;
        let width = lines[0].chars().count();
        let mut dark = vec![vec![false; width]; lines.len() * 2];
        for (row, line) in lines.iter().enumerate() {
            assert_eq!(line.chars().count(), width, "every line has the same width");
            for (column, character) in line.chars().enumerate() {
                let (top, bottom) = match character {
                    ' ' => (false, false),
                    '\u{2580}' => (true, false),
                    '\u{2584}' => (false, true),
                    '\u{2588}' => (true, true),
                    other => panic!("unexpected character {other:?}"),
                };
                dark[row * 2][column] = top;
                dark[row * 2 + 1][column] = bottom;
            }
        }
        let mut image = rqrr::PreparedImage::prepare_from_greyscale(
            width * SCALE,
            dark.len() * SCALE,
            |x, y| if dark[y / SCALE][x / SCALE] { 0 } else { 255 },
        );
        let grids = image.detect_grids();
        assert_eq!(grids.len(), 1, "one QR code is found");
        grids[0].decode().expect("the code can be read").1
    }

    #[test]
    fn the_printed_qr_code_reads_back_as_the_link() {
        let link = "https://husker-online-0a1b2c3d.azurewebsites.net/#code=K7QF2M9XTB4H";
        let lines = qr_lines(link).unwrap();
        assert_eq!(read_back(&lines), link);

        // The code and the two lines under it fit a terminal of 24 rows and 80 columns.
        assert!(lines.len() + 2 <= 24, "{} lines", lines.len());
        assert!(lines[0].chars().count() <= 80);

        // The border is empty. Printed dark on light, it is the light frame a camera needs.
        assert!(lines[0].chars().all(|character| character == ' '));
        assert!(
            lines
                .iter()
                .all(|line| line.starts_with("    ") && line.ends_with("    "))
        );

        // A local address reads back too.
        let local = "http://127.0.0.1:8787/#code=0123456789AB";
        assert_eq!(read_back(&qr_lines(local).unwrap()), local);

        // The longest link of that size has 78 characters. One more needs a larger code.
        let long = format!(
            "https://{}.azurewebsites.net/#code=K7QF2M9XTB4H",
            "h".repeat(33)
        );
        assert_eq!(long.len(), 78);
        let lines = qr_lines(&long).unwrap();
        assert_eq!(read_back(&lines), long);
        assert!(lines.len() + 2 <= 24, "{} lines", lines.len());
    }
}
