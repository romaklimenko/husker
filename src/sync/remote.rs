//! The sync file in Azure Blob Storage, reached over HTTPS.

use std::{
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use ureq::{
    Agent,
    http::Response,
    tls::{RootCerts, TlsConfig},
};

use super::{Auth, Downloaded, Failure, FailureKind, Settings, snapshot::Snapshot};

/// Every request gives up after this long.
pub const TIMEOUT: Duration = Duration::from_secs(10);

/// The Blob service version Husker speaks. Bearer tokens need 2017-11-09 or later.
const SERVICE_VERSION: &str = "2021-08-06";

/// The token endpoint of the Azure Instance Metadata Service on a VM.
const METADATA_ENDPOINT: &str = "http://169.254.169.254/metadata/identity/oauth2/token";

/// A sync file larger than this is refused instead of read into memory.
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;

/// A token is replaced this long before it expires.
const TOKEN_MARGIN: Duration = Duration::from_secs(300);

/// What one upload returned.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Stored {
    Done {
        etag: String,
    },
    /// Another device replaced or created the file after the download.
    Changed,
}

/// The place the sync file lives.
pub trait Remote: Send {
    /// Downloads the file unless it still has the ETag `since`.
    fn download(&self, since: Option<&str>) -> Result<Downloaded, Failure>;

    /// Replaces the file if it still has the ETag `replaces`, or creates it when `replaces` is `None`.
    fn upload(&self, body: &[u8], replaces: Option<&str>) -> Result<Stored, Failure>;
}

pub struct BlobRemote {
    agent: Agent,
    url: String,
    auth: Auth,
    metadata_endpoint: String,
    token: Mutex<Option<(String, Instant)>>,
}

impl BlobRemote {
    pub fn new(settings: &Settings) -> Self {
        Self::with_metadata_endpoint(settings, METADATA_ENDPOINT)
    }

    /// Uses another token endpoint. Tests point this at a local server.
    pub fn with_metadata_endpoint(settings: &Settings, endpoint: &str) -> Self {
        let agent = Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .http_status_as_error(false)
            // A redirect would send the SAS token or bearer token to another host.
            .max_redirects(0)
            .tls_config(
                TlsConfig::builder()
                    .root_certs(RootCerts::PlatformVerifier)
                    .build(),
            )
            .build()
            .into();
        Self {
            agent,
            url: settings.url.clone(),
            auth: settings.auth.clone(),
            metadata_endpoint: endpoint.to_owned(),
            token: Mutex::new(None),
        }
    }

    /// Removes the sync URL and its query string from a message.
    fn scrub(&self, text: &str) -> String {
        let mut text = text.replace(&self.url, "[sync URL]");
        if let Some((_, query)) = self.url.split_once('?')
            && !query.is_empty()
        {
            text = text.replace(query, "[token]");
        }
        text
    }

    fn transport(&self, error: ureq::Error) -> Failure {
        let kind = match error {
            ureq::Error::Io(_)
            | ureq::Error::Timeout(_)
            | ureq::Error::HostNotFound
            | ureq::Error::ConnectionFailed => FailureKind::Offline,
            _ => FailureKind::Rejected,
        };
        Failure::new(
            kind,
            self.scrub(&format!("Cannot reach the sync storage: {error}.")),
        )
    }

    fn status<T>(&self, response: &Response<T>, action: &str) -> Failure {
        let status = response.status().as_u16();
        let code = response
            .headers()
            .get("x-ms-error-code")
            .and_then(|value| value.to_str().ok())
            .map(|code| format!(", {code}"))
            .unwrap_or_default();
        let kind = if matches!(status, 401 | 403) {
            // A refused token is not tried again. The next request asks for a new one.
            *self.token.lock().unwrap_or_else(|lock| lock.into_inner()) = None;
            FailureKind::Denied
        } else {
            FailureKind::Rejected
        };
        Failure::new(
            kind,
            self.scrub(&format!(
                "The sync storage refused the {action} (HTTP {status}{code})."
            )),
        )
    }

    /// Returns the `Authorization` header value, or `None` for a SAS URL.
    fn authorization(&self) -> Result<Option<String>, Failure> {
        let Auth::ManagedIdentity { client_id } = &self.auth else {
            return Ok(None);
        };
        let mut cached = self.token.lock().unwrap_or_else(|lock| lock.into_inner());
        if let Some((token, expires)) = cached.as_ref()
            && Instant::now() < *expires
        {
            return Ok(Some(format!("Bearer {token}")));
        }
        let mut url = format!(
            "{}?api-version=2018-02-01&resource=https%3A%2F%2Fstorage.azure.com%2F",
            self.metadata_endpoint
        );
        if let Some(client_id) = client_id {
            if !client_id
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-')
            {
                return Err(Failure::new(
                    FailureKind::Denied,
                    "client_id in sync.toml must contain only letters, digits, and hyphens.",
                ));
            }
            url.push_str("&client_id=");
            url.push_str(client_id);
        }
        let denied = |detail: String| Failure::new(FailureKind::Denied, detail);
        let mut response = self
            .agent
            .get(&url)
            .header("Metadata", "true")
            .call()
            .map_err(|error| {
                denied(format!(
                    "Cannot get a managed identity token. Is this an Azure VM with an identity? ({error})"
                ))
            })?;
        if response.status().as_u16() != 200 {
            return Err(denied(format!(
                "The managed identity endpoint refused the token request (HTTP {}).",
                response.status().as_u16()
            )));
        }
        let body: serde_json::Value = response
            .body_mut()
            .read_to_vec()
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .ok_or_else(|| denied("The managed identity token answer is not valid JSON.".into()))?;
        let token = body["access_token"]
            .as_str()
            .ok_or_else(|| denied("The managed identity answer holds no token.".into()))?
            .to_owned();
        let expires_on = match &body["expires_on"] {
            serde_json::Value::String(text) => text.parse::<u64>().ok(),
            value => value.as_u64(),
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        let lifetime = Duration::from_secs(expires_on.map_or(0, |at| at.saturating_sub(now)));
        *cached = Some((
            token.clone(),
            Instant::now() + lifetime.saturating_sub(TOKEN_MARGIN),
        ));
        Ok(Some(format!("Bearer {token}")))
    }

    fn etag<T>(&self, response: &Response<T>) -> Result<String, Failure> {
        response
            .headers()
            .get("etag")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
            .ok_or_else(|| {
                Failure::new(
                    FailureKind::Rejected,
                    "The sync storage sent no ETag for the sync file.",
                )
            })
    }
}

impl Remote for BlobRemote {
    fn download(&self, since: Option<&str>) -> Result<Downloaded, Failure> {
        let mut request = self
            .agent
            .get(&self.url)
            .header("x-ms-version", SERVICE_VERSION);
        if let Some(authorization) = self.authorization()? {
            request = request.header("Authorization", authorization);
        }
        if let Some(since) = since {
            request = request.header("If-None-Match", since);
        }
        let mut response = request.call().map_err(|error| self.transport(error))?;
        match response.status().as_u16() {
            200 => {
                let etag = self.etag(&response)?;
                let body = response
                    .body_mut()
                    .with_config()
                    .limit(MAX_FILE_BYTES)
                    .read_to_vec()
                    .map_err(|error| self.transport(error))?;
                Ok(Downloaded::File {
                    snapshot: Snapshot::parse(&body)?,
                    etag,
                })
            }
            304 if since.is_some() => Ok(Downloaded::Unchanged),
            404 => Ok(Downloaded::Missing),
            _ => Err(self.status(&response, "download")),
        }
    }

    fn upload(&self, body: &[u8], replaces: Option<&str>) -> Result<Stored, Failure> {
        let mut request = self
            .agent
            .put(&self.url)
            .header("x-ms-version", SERVICE_VERSION)
            .header("x-ms-blob-type", "BlockBlob")
            .header("Content-Type", "application/json");
        if let Some(authorization) = self.authorization()? {
            request = request.header("Authorization", authorization);
        }
        request = match replaces {
            Some(etag) => request.header("If-Match", etag),
            None => request.header("If-None-Match", "*"),
        };
        let response = request.send(body).map_err(|error| self.transport(error))?;
        match response.status().as_u16() {
            200 | 201 => Ok(Stored::Done {
                etag: self.etag(&response)?,
            }),
            412 => Ok(Stored::Changed),
            // The file appeared after a download that found none.
            409 if replaces.is_none() => Ok(Stored::Changed),
            _ => Err(self.status(&response, "upload")),
        }
    }
}

/// An in-memory sync file that several test devices share.
#[cfg(test)]
pub(crate) mod testing {
    use std::sync::{Arc, Mutex};

    use super::*;

    type Hook = Box<dyn FnOnce() + Send>;

    #[derive(Default)]
    pub struct MemoryRemote {
        file: Mutex<Option<(Vec<u8>, u64)>>,
        pub failure: Mutex<Option<Failure>>,
        /// The next upload is stored, but the device gets a failure instead of the answer.
        pub lose_next_answer: Mutex<bool>,
        /// Every upload is refused as if another device had just replaced the file.
        pub always_refuse: Mutex<bool>,
        pub downloads: Mutex<usize>,
        pub full_downloads: Mutex<usize>,
        pub uploads: Mutex<usize>,
        /// Runs once at the start of the next upload, before the ETag is checked.
        pub before_upload: Mutex<Option<Hook>>,
    }

    impl MemoryRemote {
        pub fn shared() -> Arc<Self> {
            Arc::new(Self::default())
        }

        pub fn snapshot(&self) -> Option<Snapshot> {
            self.file
                .lock()
                .unwrap()
                .as_ref()
                .map(|(body, _)| Snapshot::parse(body).unwrap())
        }

        pub fn replace(&self, body: Option<Vec<u8>>) {
            let mut file = self.file.lock().unwrap();
            let next = file.as_ref().map_or(1, |(_, version)| version + 1);
            *file = body.map(|body| (body, next + 100));
        }

        pub fn fail(&self, kind: FailureKind) {
            *self.failure.lock().unwrap() = Some(Failure::new(kind, "test failure"));
        }

        pub fn recover(&self) {
            *self.failure.lock().unwrap() = None;
        }

        pub fn counts(&self) -> (usize, usize, usize) {
            (
                *self.downloads.lock().unwrap(),
                *self.full_downloads.lock().unwrap(),
                *self.uploads.lock().unwrap(),
            )
        }
    }

    impl Remote for Arc<MemoryRemote> {
        fn download(&self, since: Option<&str>) -> Result<Downloaded, Failure> {
            if let Some(failure) = self.failure.lock().unwrap().clone() {
                return Err(failure);
            }
            *self.downloads.lock().unwrap() += 1;
            let file = self.file.lock().unwrap();
            match file.as_ref() {
                None => Ok(Downloaded::Missing),
                Some((_, version)) if since == Some(version.to_string().as_str()) => {
                    Ok(Downloaded::Unchanged)
                }
                Some((body, version)) => {
                    *self.full_downloads.lock().unwrap() += 1;
                    Ok(Downloaded::File {
                        snapshot: Snapshot::parse(body)?,
                        etag: version.to_string(),
                    })
                }
            }
        }

        fn upload(&self, body: &[u8], replaces: Option<&str>) -> Result<Stored, Failure> {
            if let Some(failure) = self.failure.lock().unwrap().clone() {
                return Err(failure);
            }
            let hook = self.before_upload.lock().unwrap().take();
            if let Some(hook) = hook {
                hook();
            }
            let mut file = self.file.lock().unwrap();
            let current = file.as_ref().map(|(_, version)| version.to_string());
            if current.as_deref() != replaces || *self.always_refuse.lock().unwrap() {
                return Ok(Stored::Changed);
            }
            *self.uploads.lock().unwrap() += 1;
            let next = file.as_ref().map_or(1, |(_, version)| version + 1);
            *file = Some((body.to_vec(), next));
            if std::mem::take(&mut *self.lose_next_answer.lock().unwrap()) {
                return Err(Failure::new(FailureKind::Offline, "the answer was lost"));
            }
            Ok(Stored::Done {
                etag: next.to_string(),
            })
        }
    }
}
