//! The online server: Husker over HTTP on top of the sync file.
//!
//! The server keeps no database. `doc` changes one record of a downloaded
//! snapshot, `engine` uploads it with `If-Match`, and `mcp` and `http` serve
//! the result to clients that hold a signed access token.

use std::{net::Ipv4Addr, sync::Arc};

use anyhow::{Context, Result, anyhow, bail};
use chrono_tz::Tz;

use crate::sync::{Auth, Settings, remote::BlobRemote};

pub mod access;
pub mod api;
pub mod assets;
pub mod doc;
pub mod engine;
pub mod http;
pub mod mcp;
pub mod view;

#[cfg(test)]
mod tests;

/// The port `husker serve` listens on when nothing else names one.
pub const DEFAULT_PORT: u16 = 8787;

/// The environment variable Azure Functions sets to the port of a custom handler.
const HANDLER_PORT: &str = "FUNCTIONS_CUSTOMHANDLER_PORT";

/// The settings of the online server, read from environment variables.
pub struct ServerSettings {
    pub sync: Settings,
    pub verifier: access::Verifier,
    pub origin: String,
    pub zone: Tz,
    pub read_only: bool,
}

impl ServerSettings {
    pub fn from_env() -> Result<Self> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Reads the settings through `lookup`, which returns the value of a variable.
    ///
    /// Each error names the variable and never repeats the sync URL.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let value = |name: &str| lookup(name).filter(|value| !value.trim().is_empty());
        let required = |name: &str| value(name).with_context(|| format!("{name} is not set."));

        let sync = Settings::new(&required("HUSKER_SYNC_URL")?, Auth::Sas)
            .map_err(|error| anyhow!("HUSKER_SYNC_URL: {error}"))?;
        let origin = access::normalize_origin(&required("HUSKER_ONLINE_ORIGIN")?)
            .map_err(|error| anyhow!("HUSKER_ONLINE_ORIGIN: {error}"))?;
        let revoked = access::parse_revoked(&value("HUSKER_ONLINE_REVOKED").unwrap_or_default())
            .map_err(|error| anyhow!("HUSKER_ONLINE_REVOKED: {error}"))?;
        let verifier = access::Verifier::new(&required("HUSKER_ONLINE_KEYS")?, &origin, revoked)
            .map_err(|error| anyhow!("HUSKER_ONLINE_KEYS: {error}"))?;
        let zone = match value("HUSKER_ONLINE_TZ") {
            Some(name) => name.trim().parse::<Tz>().map_err(|_| {
                anyhow!("HUSKER_ONLINE_TZ: Use an IANA time zone name such as Europe/Copenhagen.")
            })?,
            None => Tz::UTC,
        };
        let read_only = match value("HUSKER_ONLINE_READ_ONLY").as_deref().map(str::trim) {
            None | Some("false") => false,
            Some("true") => true,
            Some(_) => bail!("HUSKER_ONLINE_READ_ONLY: Use true or false."),
        };
        Ok(Self {
            sync,
            verifier,
            origin,
            zone,
            read_only,
        })
    }

    /// Builds the shared state of the request handlers.
    pub fn into_state(self) -> http::AppState {
        http::AppState {
            engine: Arc::new(engine::Engine::new(Box::new(
                BlobRemote::with_bundled_roots(&self.sync),
            ))),
            verifier: Arc::new(self.verifier),
            origin: self.origin.into(),
            zone: self.zone,
            read_only: self.read_only,
            pairings: Arc::default(),
        }
    }
}

/// Runs the online server on the loopback address until the process stops.
pub fn serve(port: Option<u16>) -> Result<()> {
    let state = ServerSettings::from_env()?.into_state();
    let port = match port {
        Some(port) => port,
        None => match std::env::var(HANDLER_PORT) {
            Ok(value) => value
                .trim()
                .parse()
                .map_err(|_| anyhow!("{HANDLER_PORT} is not a port number."))?,
            Err(_) => DEFAULT_PORT,
        },
    };
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async move {
            let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port))
                .await
                .with_context(|| format!("Cannot listen on 127.0.0.1:{port}."))?;
            eprintln!("Husker online server listening on 127.0.0.1:{port}.");
            axum::serve(listener, http::router(state)).await?;
            Ok(())
        })
}
