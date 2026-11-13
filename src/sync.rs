//! Sync between devices through one JSON file in Azure Blob Storage.
//!
//! `sync_once` downloads the file when it changed, merges it into the local
//! database, and uploads the result when the database holds changes the file
//! lacks. The worker in `worker` runs it on a schedule.

use std::{fmt, path::Path};

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;

use crate::store::Store;

pub mod merge;
pub mod remote;
pub mod snapshot;
pub mod worker;

use remote::{Remote, Stored};
use snapshot::Snapshot;

/// The name of the settings file in the profile directory.
pub const SETTINGS_FILE: &str = "sync.toml";

/// How many refused uploads one sync accepts before it gives up.
const UPLOAD_ATTEMPTS: usize = 5;

/// Why a sync failed. The terminal shows a localized sentence for each kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureKind {
    /// No connection, DNS failure, or timeout.
    Offline,
    /// The storage service refused the credentials.
    Denied,
    /// Any other unexpected answer from the storage service.
    Rejected,
    /// The sync file was written by a newer Husker.
    Incompatible,
    /// The sync file failed validation.
    Invalid,
    /// Other devices kept replacing the file during one sync.
    Busy,
    /// The local database could not be read or written.
    Local,
}

/// A failed sync. The detail never contains the sync URL or a token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Failure {
    pub kind: FailureKind,
    pub detail: String,
}

impl Failure {
    pub fn new(kind: FailureKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }

    fn local(error: anyhow::Error) -> Self {
        Self::new(FailureKind::Local, format!("{error:#}"))
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.detail)
    }
}

impl std::error::Error for Failure {}

/// What one download returned.
#[derive(Clone, Debug, PartialEq)]
pub enum Downloaded {
    /// The file still has the ETag of the stored base.
    Unchanged,
    /// The file does not exist.
    Missing,
    File {
        snapshot: Snapshot,
        etag: String,
    },
}

/// How Husker signs in to the storage service.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Auth {
    /// The URL carries a SAS token.
    Sas,
    /// A token from the Azure Instance Metadata Service of the VM.
    ManagedIdentity { client_id: Option<String> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Settings {
    pub url: String,
    pub auth: Auth,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsFile {
    url: String,
    auth: Option<String>,
    client_id: Option<String>,
}

impl Settings {
    /// Reads `sync.toml` from the profile. Returns `None` when the file does not exist.
    ///
    /// Errors name the file and never repeat the URL, because its query string is a secret.
    pub fn load(profile: &Path) -> Result<Option<Self>> {
        let path = profile.join(SETTINGS_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error).with_context(|| format!("Cannot read {}", path.display()));
            }
        };
        Self::parse(&text)
            .with_context(|| format!("Invalid sync settings in {}", path.display()))
            .map(Some)
    }

    fn parse(text: &str) -> Result<Self> {
        // The TOML error can quote the offending line, which may hold the URL.
        let file: SettingsFile = toml::from_str(text).map_err(|error| {
            anyhow::anyhow!(
                "{}. Use the keys url, auth, and client_id.",
                error.message()
            )
        })?;
        let url = file.url.trim().to_owned();
        let (scheme, rest) = url
            .split_once("://")
            .context("url must start with https://")?;
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        let host = authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host);
        let host = match host.strip_prefix('[') {
            Some(bracketed) => bracketed.split(']').next().unwrap_or_default(),
            None => host.split(':').next().unwrap_or_default(),
        };
        ensure!(!host.is_empty(), "url has no host name.");
        match scheme {
            "https" => {}
            "http" => ensure!(
                matches!(host, "localhost" | "127.0.0.1" | "::1"),
                "url must use https. http is accepted only for localhost."
            ),
            _ => bail!("url must start with https://"),
        }
        let auth = match file.auth.as_deref() {
            None | Some("sas") => {
                ensure!(
                    file.client_id.is_none(),
                    "client_id needs auth = \"managed-identity\"."
                );
                Auth::Sas
            }
            Some("managed-identity") => Auth::ManagedIdentity {
                client_id: file.client_id,
            },
            Some(_) => bail!("auth must be \"sas\" or \"managed-identity\"."),
        };
        Ok(Self { url, auth })
    }
}

/// The result of one completed sync.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Serialize)]
pub struct Report {
    /// Local lists and tasks inserted, changed, or deleted.
    pub received: usize,
    /// Lists and tasks the upload added, changed, or removed in the file.
    pub sent: usize,
    /// Conflict copies created.
    pub conflicts: usize,
}

/// Runs one sync: download when changed, merge, and upload when needed.
///
/// A refused upload means another device replaced the file after the download.
/// The merged tasks stay in the database, and the loop downloads again.
pub fn sync_once(store: &mut Store, remote: &dyn Remote) -> Result<Report, Failure> {
    let mut report = Report::default();
    for _ in 0..UPLOAD_ATTEMPTS {
        let since = store.sync_etag().map_err(Failure::local)?;
        let downloaded = remote.download(since.as_deref())?;
        let Some(outcome) = store
            .sync_merge(since.as_deref(), downloaded)
            .map_err(Failure::local)?
        else {
            // Another process synced between the download and the merge.
            continue;
        };
        report.received += outcome.received;
        report.conflicts += outcome.conflicts;
        let Some(upload) = outcome.upload else {
            return Ok(report);
        };
        match remote.upload(&upload.to_json(), outcome.replaces.as_deref())? {
            Stored::Done { etag } => {
                store
                    .sync_uploaded(outcome.replaces.as_deref(), &upload, &etag)
                    .map_err(Failure::local)?;
                report.sent = outcome.sent;
                return Ok(report);
            }
            Stored::Changed => {}
        }
    }
    Err(Failure::new(
        FailureKind::Busy,
        "The sync file kept changing. Husker will try again.",
    ))
}

#[cfg(test)]
mod tests;
