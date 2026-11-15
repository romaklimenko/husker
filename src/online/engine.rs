//! Reads and writes the sync file for the online server.
//!
//! A read reuses the last download when the file still has its ETag. A write
//! changes one record and uploads with `If-Match`. When a device replaced the
//! file in between, the write downloads again and applies the same change.

use std::{
    fmt,
    sync::{Arc, Mutex},
};

use super::doc::{Access, Changed, DocError, Op, Stamp, apply, guard};
use crate::sync::{
    Downloaded, Failure, FailureKind,
    remote::{Remote, Stored},
    snapshot::Snapshot,
};

/// How many refused uploads one write accepts before it gives up.
const UPLOAD_ATTEMPTS: usize = 5;

/// The largest sync file the online server uploads.
///
/// Every write sends the whole file, and every device downloads it again. A
/// client must not be able to grow it until devices can no longer sync.
pub const MAX_FILE_BYTES: usize = 16 * 1024 * 1024;

/// Why a read or write failed. No message contains the sync URL.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EngineError {
    /// The change was refused by a task rule, the access, or the upload guard.
    Doc(DocError),
    /// The storage could not be reached, refused the request, or holds a file this Husker cannot read.
    Storage(FailureKind),
    /// The sync file does not exist. The online server never creates it.
    Missing,
    /// An upload got no answer, and the file could not be read to see whether it arrived.
    Unknown,
}

impl EngineError {
    /// The sentence for a client. Storage sentences come from the sync catalog.
    pub fn message(&self) -> String {
        use crate::i18n::{Language, tr};
        match self {
            Self::Doc(error) => error.to_string(),
            Self::Missing => tr(Language::En, "online_missing").to_owned(),
            Self::Unknown => tr(Language::En, "online_unknown").to_owned(),
            Self::Storage(kind) => tr(Language::En, storage_key(*kind)).to_owned(),
        }
    }
}

/// The catalog key of the sentence for a storage failure.
pub fn storage_key(kind: FailureKind) -> &'static str {
    match kind {
        FailureKind::Offline => "sync_offline",
        FailureKind::Denied => "sync_denied",
        FailureKind::Rejected => "sync_rejected",
        FailureKind::Incompatible => "sync_incompatible",
        FailureKind::Invalid => "sync_invalid",
        FailureKind::Busy => "sync_busy",
        FailureKind::Local => "sync_local",
    }
}

impl fmt::Display for EngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message())
    }
}

impl std::error::Error for EngineError {}

impl From<DocError> for EngineError {
    fn from(error: DocError) -> Self {
        Self::Doc(error)
    }
}

impl From<Failure> for EngineError {
    fn from(failure: Failure) -> Self {
        Self::Storage(failure.kind)
    }
}

/// Reports whether the file holds the record exactly as a change left it.
fn holds(snapshot: &Snapshot, changed: &Changed) -> bool {
    match changed {
        Changed::Task(task) => snapshot.tasks.iter().any(|current| current == task),
        Changed::List(list) => snapshot.lists.iter().any(|current| current == list),
    }
}

struct Cached {
    etag: String,
    snapshot: Arc<Snapshot>,
}

pub struct Engine {
    remote: Box<dyn Remote>,
    cache: Mutex<Option<Cached>>,
    /// Serializes writes in this process, so they do not refuse each other.
    writing: Mutex<()>,
    max_file_bytes: usize,
}

impl Engine {
    pub fn new(remote: Box<dyn Remote>) -> Self {
        Self {
            remote,
            cache: Mutex::new(None),
            writing: Mutex::new(()),
            max_file_bytes: MAX_FILE_BYTES,
        }
    }

    /// Uses another size budget for uploads. Tests use a small one.
    #[cfg(test)]
    pub(crate) fn with_file_limit(mut self, bytes: usize) -> Self {
        self.max_file_bytes = bytes;
        self
    }

    /// Returns the current sync file.
    pub fn read(&self) -> Result<Arc<Snapshot>, EngineError> {
        self.fetch().map(|(_, snapshot)| snapshot)
    }

    /// Applies one change to the sync file and returns the affected record with the new file.
    ///
    /// Nothing is uploaded when the change leaves the file as it is.
    pub fn write(
        &self,
        access: &Access,
        op: &Op,
        stamp: &Stamp,
    ) -> Result<(Changed, Arc<Snapshot>), EngineError> {
        self.write_all(access, std::slice::from_ref(op), stamp)
    }

    /// Applies several changes to one list or task in one upload.
    ///
    /// The changes run in order on the same copy. The guard still allows only
    /// one record to differ, so they must all concern the same record. Returns
    /// the record after the last change.
    pub fn write_all(
        &self,
        access: &Access,
        ops: &[Op],
        stamp: &Stamp,
    ) -> Result<(Changed, Arc<Snapshot>), EngineError> {
        let _writing = self.writing.lock().unwrap_or_else(|lock| lock.into_inner());
        for _ in 0..UPLOAD_ATTEMPTS {
            let (etag, before) = self.fetch()?;
            let mut after = (*before).clone();
            let mut changed = None;
            for op in ops {
                changed = Some(apply(&mut after, access, op, stamp)?);
            }
            let Some(changed) = changed else {
                return Err(DocError::Guard("no change was requested").into());
            };
            if after == *before {
                return Ok((changed, before));
            }
            let body = after.to_json();
            if body.len() > self.max_file_bytes {
                return Err(DocError::FileTooLarge.into());
            }
            // The upload must pass the checks every device applies to a download.
            let after = Snapshot::parse(&body)
                .map_err(|_| DocError::Guard("the result is not a valid sync file"))?;
            guard(&before, &after, access)?;
            match self.remote.upload(&body, Some(&etag)) {
                Ok(Stored::Done { etag }) => {
                    let after = Arc::new(after);
                    self.remember(etag, Arc::clone(&after));
                    return Ok((changed, after));
                }
                // A device replaced the file after the download.
                Ok(Stored::Changed) => {}
                Err(failure) => {
                    // The upload may have reached the storage although its answer was lost.
                    // The record tells: a device may have changed other records since.
                    return match self.fetch() {
                        Ok((_, current)) if holds(&current, &changed) => Ok((changed, current)),
                        Ok(_) => Err(failure.into()),
                        Err(_) => Err(EngineError::Unknown),
                    };
                }
            }
        }
        Err(EngineError::Storage(FailureKind::Busy))
    }

    /// Downloads the file unless it still has the cached ETag.
    fn fetch(&self) -> Result<(String, Arc<Snapshot>), EngineError> {
        let cached = self
            .cache
            .lock()
            .unwrap_or_else(|lock| lock.into_inner())
            .as_ref()
            .map(|cached| (cached.etag.clone(), Arc::clone(&cached.snapshot)));
        let since = cached.as_ref().map(|(etag, _)| etag.as_str());
        match self.remote.download(since)? {
            Downloaded::Unchanged => cached.ok_or(EngineError::Storage(FailureKind::Rejected)),
            Downloaded::Missing => {
                *self.cache.lock().unwrap_or_else(|lock| lock.into_inner()) = None;
                Err(EngineError::Missing)
            }
            Downloaded::File { snapshot, etag } => {
                let snapshot = Arc::new(snapshot);
                self.remember(etag.clone(), Arc::clone(&snapshot));
                Ok((etag, snapshot))
            }
        }
    }

    fn remember(&self, etag: String, snapshot: Arc<Snapshot>) {
        *self.cache.lock().unwrap_or_else(|lock| lock.into_inner()) =
            Some(Cached { etag, snapshot });
    }
}
