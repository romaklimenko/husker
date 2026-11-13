//! The background thread that decides when to sync and runs each sync.
//!
//! The worker owns its own database connection. It notices local changes
//! through SQLite's data version, which moves when any other connection
//! commits, so no caller has to report its writes.

use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, RecvTimeoutError, Sender},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use super::{Failure, FailureKind, remote::Remote, sync_once};
use crate::store::Store;

/// When a process syncs without being asked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Cadence {
    /// How long after the last local commit a sync starts.
    pub debounce: Duration,
    /// How often a sync starts without any other trigger.
    pub poll: Duration,
    /// Input after this long without input starts a sync. `None` disables the trigger.
    pub wake_after: Option<Duration>,
    /// How often the worker looks for local commits and due triggers.
    pub tick: Duration,
}

impl Cadence {
    /// The terminal waits a minute after the last change, so a burst of edits uploads once.
    pub const TERMINAL: Self = Self {
        debounce: Duration::from_secs(60),
        poll: Duration::from_secs(60),
        wake_after: Some(Duration::from_secs(60)),
        tick: Duration::from_millis(500),
    };

    /// An agent's change uploads within seconds, because its process can end at any time.
    pub const MCP: Self = Self {
        debounce: Duration::from_secs(2),
        poll: Duration::from_secs(60),
        wake_after: None,
        tick: Duration::from_millis(500),
    };
}

/// Decides whether a sync is due. Holds no I/O, so tests drive it with made-up instants.
#[derive(Clone, Debug)]
struct Schedule {
    cadence: Cadence,
    next_poll: Instant,
    push_at: Option<Instant>,
    last_input: Instant,
    requested: bool,
}

impl Schedule {
    fn new(cadence: Cadence, now: Instant, sync_at_start: bool) -> Self {
        Self {
            cadence,
            next_poll: if sync_at_start {
                now
            } else {
                now + cadence.poll
            },
            push_at: None,
            last_input: now,
            requested: false,
        }
    }

    /// Another connection committed. Every commit restarts the quiet period.
    fn local_change(&mut self, now: Instant) {
        self.push_at = Some(now + self.cadence.debounce);
    }

    /// The user pressed a key, pasted, or used the mouse.
    fn input(&mut self, now: Instant) {
        if self
            .cadence
            .wake_after
            .is_some_and(|idle| now.duration_since(self.last_input) >= idle)
        {
            self.requested = true;
        }
        self.last_input = now;
    }

    fn request(&mut self) {
        self.requested = true;
    }

    fn due(&self, now: Instant) -> bool {
        self.requested || self.push_at.is_some_and(|at| now >= at) || now >= self.next_poll
    }

    /// A sync ended. After a failure the next attempt waits for the timer or a request.
    fn finished(&mut self, now: Instant) {
        self.requested = false;
        self.push_at = None;
        self.next_poll = now + self.cadence.poll;
    }
}

enum Command {
    Input,
    SyncNow,
    Finish(Sender<Option<Failure>>),
}

/// A running sync thread. Dropping it stops the thread without a final sync.
pub struct SyncWorker {
    commands: Sender<Command>,
    failure: Arc<Mutex<Option<Failure>>>,
    thread: Option<JoinHandle<()>>,
}

impl SyncWorker {
    /// Starts the worker on the database at `database`.
    ///
    /// `sync_at_start` runs the first sync at once. `log_failures` writes the
    /// first failure of each streak to stderr, for processes without a screen.
    pub fn spawn(
        database: PathBuf,
        remote: Box<dyn Remote>,
        cadence: Cadence,
        sync_at_start: bool,
        log_failures: bool,
    ) -> Self {
        let (commands, receiver) = mpsc::channel();
        let failure = Arc::new(Mutex::new(None));
        let slot = Arc::clone(&failure);
        let thread = std::thread::Builder::new()
            .name("husker-sync".into())
            .spawn(move || {
                run(
                    database,
                    remote,
                    cadence,
                    sync_at_start,
                    log_failures,
                    receiver,
                    slot,
                )
            })
            .ok();
        Self {
            commands,
            failure,
            thread,
        }
    }

    /// Reports user input, which starts a sync after an idle period.
    pub fn input(&self) {
        let _ = self.commands.send(Command::Input);
    }

    /// Starts a sync as soon as the worker is free.
    pub fn sync_now(&self) {
        let _ = self.commands.send(Command::SyncNow);
    }

    /// Returns the first failure since the last successful sync, once.
    pub fn take_failure(&self) -> Option<Failure> {
        self.failure
            .lock()
            .unwrap_or_else(|lock| lock.into_inner())
            .take()
    }

    /// Uploads unsent changes and stops the worker.
    ///
    /// Waits at most `timeout`. Returns the failure when the changes could not
    /// be sent. They stay in the database for the next sync.
    pub fn finish(mut self, timeout: Duration) -> Option<Failure> {
        let (reply, result) = mpsc::channel();
        if self.commands.send(Command::Finish(reply)).is_err() {
            return self.take_failure();
        }
        match result.recv_timeout(timeout) {
            Ok(result) => {
                if let Some(thread) = self.thread.take() {
                    let _ = thread.join();
                }
                result
            }
            Err(RecvTimeoutError::Timeout) => Some(Failure::new(
                FailureKind::Offline,
                "The sync did not finish in time.",
            )),
            Err(RecvTimeoutError::Disconnected) => self.take_failure(),
        }
    }
}

fn run(
    database: PathBuf,
    remote: Box<dyn Remote>,
    cadence: Cadence,
    sync_at_start: bool,
    log_failures: bool,
    commands: Receiver<Command>,
    failure: Arc<Mutex<Option<Failure>>>,
) {
    let report = |problem: Failure| {
        if log_failures {
            eprintln!("Sync failed: {problem}");
        }
        *failure.lock().unwrap_or_else(|lock| lock.into_inner()) = Some(problem);
    };
    let mut store = match Store::open(&database) {
        Ok(store) => store,
        Err(error) => {
            report(Failure::local(error));
            return;
        }
    };
    let mut schedule = Schedule::new(cadence, Instant::now(), sync_at_start);
    let mut seen = store.data_version().ok();
    let mut failing = false;
    loop {
        match commands.recv_timeout(cadence.tick) {
            Ok(Command::Input) => schedule.input(Instant::now()),
            Ok(Command::SyncNow) => schedule.request(),
            Ok(Command::Finish(reply)) => {
                let result = match store.sync_pending() {
                    Ok(false) => None,
                    Ok(true) => sync_once(&mut store, remote.as_ref()).err(),
                    Err(error) => Some(Failure::local(error)),
                };
                let _ = reply.send(result);
                return;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
        let version = store.data_version().ok();
        if version != seen {
            seen = version;
            schedule.local_change(Instant::now());
        }
        if schedule.due(Instant::now()) {
            let result = sync_once(&mut store, remote.as_ref());
            schedule.finished(Instant::now());
            match result {
                Ok(_) => failing = false,
                Err(problem) => {
                    if !failing {
                        report(problem);
                    }
                    failing = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{model::TaskInput, sync::remote::testing::MemoryRemote};

    const SECOND: Duration = Duration::from_secs(1);

    #[test]
    fn schedule_syncs_at_start_then_on_the_timer() {
        let start = Instant::now();
        let mut schedule = Schedule::new(Cadence::TERMINAL, start, true);
        assert!(schedule.due(start));
        schedule.finished(start);
        assert!(!schedule.due(start + 59 * SECOND));
        assert!(schedule.due(start + 60 * SECOND));
        let waiting = Schedule::new(Cadence::MCP, start, false);
        assert!(!waiting.due(start));
        assert!(waiting.due(start + 60 * SECOND));
    }

    #[test]
    fn schedule_waits_for_a_quiet_period_after_the_last_change() {
        let start = Instant::now();
        let mut schedule = Schedule::new(Cadence::TERMINAL, start, false);
        schedule.finished(start + 50 * SECOND);
        schedule.local_change(start + 55 * SECOND);
        schedule.local_change(start + 70 * SECOND);
        assert!(!schedule.due(start + 109 * SECOND));
        // The timer at 110 seconds comes first and also sends the change.
        assert!(schedule.due(start + 110 * SECOND));
        schedule.finished(start + 110 * SECOND);
        assert!(!schedule.due(start + 131 * SECOND));
        let mut agent = Schedule::new(Cadence::MCP, start, false);
        agent.local_change(start + SECOND);
        assert!(!agent.due(start + 2 * SECOND));
        assert!(agent.due(start + 3 * SECOND));
    }

    #[test]
    fn schedule_syncs_on_input_after_idle_and_on_request() {
        let start = Instant::now();
        let mut schedule = Schedule::new(Cadence::TERMINAL, start, false);
        schedule.input(start + 30 * SECOND);
        assert!(!schedule.due(start + 30 * SECOND));
        schedule.finished(start + 31 * SECOND);
        schedule.input(start + 90 * SECOND);
        assert!(schedule.due(start + 90 * SECOND));
        schedule.finished(start + 90 * SECOND);
        schedule.input(start + 91 * SECOND);
        assert!(!schedule.due(start + 91 * SECOND));
        schedule.request();
        assert!(schedule.due(start + 91 * SECOND));
        let mut agent = Schedule::new(Cadence::MCP, start, false);
        agent.input(start + 30 * SECOND);
        assert!(!agent.due(start + 30 * SECOND));
    }

    fn fast() -> Cadence {
        Cadence {
            debounce: Duration::from_millis(150),
            poll: Duration::from_secs(3600),
            wake_after: None,
            tick: Duration::from_millis(20),
        }
    }

    fn wait_for(mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !condition() {
            assert!(Instant::now() < deadline, "condition not met in time");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn task(title: &str) -> TaskInput {
        TaskInput {
            list_id: 1,
            title: title.into(),
            ..TaskInput::default()
        }
    }

    #[test]
    fn worker_syncs_at_start_after_changes_and_on_request() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("tasks.db");
        let mut store = Store::open(&database).unwrap();
        let remote = MemoryRemote::shared();
        let worker =
            SyncWorker::spawn(database, Box::new(Arc::clone(&remote)), fast(), true, false);
        wait_for(|| remote.snapshot().is_some());
        assert_eq!(remote.snapshot().unwrap().tasks.len(), 0);

        store
            .create_task(&task("Sent after a quiet period"))
            .unwrap();
        wait_for(|| remote.snapshot().unwrap().tasks.len() == 1);

        let (downloads, _, _) = remote.counts();
        worker.sync_now();
        wait_for(|| remote.counts().0 > downloads);
        assert_eq!(worker.take_failure(), None);
        assert_eq!(worker.finish(Duration::from_secs(5)), None);
    }

    #[test]
    fn worker_reports_a_failure_once_and_sends_pending_changes_at_finish() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("tasks.db");
        let mut store = Store::open(&database).unwrap();
        let remote = MemoryRemote::shared();
        remote.fail(FailureKind::Offline);
        let worker =
            SyncWorker::spawn(database, Box::new(Arc::clone(&remote)), fast(), true, false);
        wait_for(|| {
            worker
                .take_failure()
                .is_some_and(|failure| failure.kind == FailureKind::Offline)
        });
        worker.sync_now();
        store.create_task(&task("Waits for the network")).unwrap();
        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(worker.take_failure(), None);

        remote.recover();
        assert_eq!(worker.finish(Duration::from_secs(5)), None);
        assert_eq!(remote.snapshot().unwrap().tasks.len(), 1);
    }

    #[test]
    fn worker_finish_skips_the_network_without_pending_changes_and_reports_failure() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("tasks.db");
        let mut store = Store::open(&database).unwrap();
        let remote = MemoryRemote::shared();
        let worker = SyncWorker::spawn(
            database.clone(),
            Box::new(Arc::clone(&remote)),
            fast(),
            true,
            false,
        );
        wait_for(|| remote.snapshot().is_some());
        let before = remote.counts();
        assert_eq!(worker.finish(Duration::from_secs(5)), None);
        assert_eq!(remote.counts(), before);

        let worker = SyncWorker::spawn(
            database,
            Box::new(Arc::clone(&remote)),
            fast(),
            false,
            false,
        );
        store.create_task(&task("Cannot be sent")).unwrap();
        remote.fail(FailureKind::Denied);
        let failure = worker.finish(Duration::from_secs(5)).unwrap();
        assert_eq!(failure.kind, FailureKind::Denied);
        assert_eq!(remote.snapshot().unwrap().tasks.len(), 0);
    }
}
