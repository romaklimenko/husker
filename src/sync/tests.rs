use super::*;

#[test]
fn settings_accept_https_and_local_http_and_never_echo_the_url() {
    let sas =
        Settings::parse("url = \"https://a.blob.core.windows.net/c/t.json?sig=S3CRET\"").unwrap();
    assert_eq!(sas.auth, Auth::Sas);
    assert!(sas.url.ends_with("sig=S3CRET"));
    for local in [
        "http://127.0.0.1:8080/t.json",
        "http://localhost/t.json",
        "http://[::1]:9/t.json",
    ] {
        assert!(Settings::parse(&format!("url = \"{local}\"")).is_ok());
    }
    let identity = Settings::parse(
        "url = \"https://a.blob.core.windows.net/c/t.json\"\nauth = \"managed-identity\"\nclient_id = \"abc\"",
    )
    .unwrap();
    assert_eq!(
        identity.auth,
        Auth::ManagedIdentity {
            client_id: Some("abc".into())
        }
    );
    for invalid in [
        "url = \"http://example.com/t.json?sig=S3CRET\"",
        "url = \"http://user@example.com/t.json?sig=S3CRET\"",
        "url = \"ftp://example.com/S3CRET\"",
        "url = \"example.com/S3CRET\"",
        "url = \"https:///S3CRET\"",
        "url = \"https://example.com/S3CRET\"\nauth = \"key\"",
        "url = \"https://example.com/S3CRET\"\nclient_id = \"abc\"",
        "url = \"https://example.com/S3CRET\"\nextra = 1",
        "auth = \"sas\"",
        "url = \"https://example.com/S3CRET",
    ] {
        let error = format!("{:#}", Settings::parse(invalid).unwrap_err());
        assert!(!error.contains("S3CRET"), "{error}");
        assert!(!error.contains("example.com"), "{error}");
    }
}

#[test]
fn a_missing_settings_file_leaves_sync_off() {
    let directory = tempfile::tempdir().unwrap();
    assert_eq!(Settings::load(directory.path()).unwrap(), None);
    std::fs::write(directory.path().join(SETTINGS_FILE), "url = 5").unwrap();
    let error = format!("{:#}", Settings::load(directory.path()).unwrap_err());
    assert!(error.contains("sync.toml"), "{error}");
}

// Engine tests: several devices, each with its own database, share one in-memory sync file.

use std::sync::Arc;

use chrono::NaiveDate;
use rusqlite::Connection;
use tempfile::TempDir;

use crate::{
    model::{Status, Task, TaskInput},
    store::{ListDeletionOutcome, TaskPurgeOutcome},
    sync::remote::testing::MemoryRemote,
};

struct Device {
    directory: TempDir,
    store: Store,
}

impl Device {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::open(&directory.path().join("tasks.db")).unwrap();
        Self { directory, store }
    }

    fn sync(&mut self, remote: &Arc<MemoryRemote>) -> Report {
        sync_once(&mut self.store, remote).unwrap()
    }

    fn add(&mut self, title: &str) -> Task {
        self.store
            .create_task(&TaskInput {
                list_id: 1,
                title: title.into(),
                ..TaskInput::default()
            })
            .unwrap()
    }

    fn task(&self, title: &str) -> Task {
        self.store
            .tasks()
            .unwrap()
            .into_iter()
            .find(|task| task.title == title)
            .unwrap_or_else(|| panic!("no task titled {title}"))
    }

    fn titles(&self) -> Vec<String> {
        let mut titles: Vec<String> = self
            .store
            .tasks()
            .unwrap()
            .into_iter()
            .map(|task| task.title)
            .collect();
        titles.sort();
        titles
    }

    fn raw(&self) -> Connection {
        Connection::open(self.directory.path().join("tasks.db")).unwrap()
    }

    /// Everything a user can see, without versions, which are local.
    fn visible(&self) -> Vec<Task> {
        self.store
            .tasks()
            .unwrap()
            .into_iter()
            .map(|task| Task { version: 0, ..task })
            .collect()
    }
}

fn day(day: u32) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(2026, 11, day)
}

/// Two devices that share the default list and have synced once.
fn pair() -> (Device, Device, Arc<MemoryRemote>) {
    let remote = MemoryRemote::shared();
    let mut first = Device::new();
    let mut second = Device::new();
    first.sync(&remote);
    second.sync(&remote);
    (first, second, remote)
}

#[test]
fn a_new_profile_joins_with_the_same_lists_tasks_and_ids() {
    let remote = MemoryRemote::shared();
    let mut laptop = Device::new();
    laptop.store.rename_list(1, "Private").unwrap();
    let work = laptop.store.create_list("Work").unwrap();
    laptop.add("Buy milk");
    let mut report = laptop.add("Write report");
    report = laptop
        .store
        .update_task(
            report.id,
            report.version,
            &TaskInput {
                list_id: work.id,
                title: report.title.clone(),
                description: "Draft\nsecond line".into(),
                due: day(20),
            },
        )
        .unwrap();
    laptop
        .store
        .set_status(report.id, report.version, Status::Done)
        .unwrap();
    let trashed = laptop.add("Old idea");
    laptop
        .store
        .set_deleted(trashed.id, trashed.version, true)
        .unwrap();
    assert_eq!(
        laptop.sync(&remote),
        Report {
            received: 0,
            sent: 5,
            conflicts: 0
        }
    );

    let mut desktop = Device::new();
    let report = desktop.sync(&remote);
    assert_eq!(report.sent, 0);
    assert_eq!(report.conflicts, 0);
    assert_eq!(desktop.visible(), laptop.visible());
    assert_eq!(
        desktop.store.lists().unwrap(),
        laptop.store.lists().unwrap()
    );
    assert_eq!(desktop.store.default_list_id().unwrap(), 1);
    assert_eq!(remote.snapshot().unwrap().lists.len(), 2);
    let done = desktop.task("Write report");
    assert_eq!(
        done.status_changed_on,
        laptop.task("Write report").status_changed_on
    );
    assert_eq!(
        done.status_changed_at,
        laptop.task("Write report").status_changed_at
    );
    assert_eq!(done.version, 1);

    // A task created on the second device reaches the first with the same ID.
    let created = desktop.add("From the desktop");
    desktop.sync(&remote);
    laptop.sync(&remote);
    assert_eq!(laptop.task("From the desktop").id, created.id);
    assert_eq!(desktop.visible(), laptop.visible());
}

#[test]
fn an_unchanged_sync_downloads_nothing_and_writes_nothing() {
    let (mut first, _second, remote) = pair();
    first.add("Milk");
    first.sync(&remote);
    let observer = Store::open(&first.directory.path().join("tasks.db")).unwrap();
    let version = observer.data_version().unwrap();
    let counters = first.store.sync_counters().unwrap();
    let before = remote.counts();
    assert_eq!(first.sync(&remote), Report::default());
    let after = remote.counts();
    assert_eq!(after.0, before.0 + 1, "one conditional download");
    assert_eq!(after.1, before.1, "no file transferred");
    assert_eq!(after.2, before.2, "no upload");
    assert_eq!(observer.data_version().unwrap(), version);
    assert_eq!(first.store.sync_counters().unwrap(), counters);
    assert!(!first.store.sync_pending().unwrap());
    assert_eq!(observer.data_version().unwrap(), version);
}

#[test]
fn edits_to_different_fields_of_one_task_combine() {
    let (mut first, mut second, remote) = pair();
    let milk = first.add("Milk");
    first.sync(&remote);
    second.sync(&remote);
    first
        .store
        .set_status(milk.id, milk.version, Status::Done)
        .unwrap();
    let theirs = second.task("Milk");
    second
        .store
        .set_due(theirs.id, theirs.version, day(20))
        .unwrap();
    first.sync(&remote);
    assert_eq!(
        second.sync(&remote),
        Report {
            received: 1,
            sent: 1,
            conflicts: 0
        }
    );
    first.sync(&remote);
    for device in [&first, &second] {
        let milk = device.task("Milk");
        assert_eq!(milk.status, Status::Done);
        assert_eq!(milk.due, day(20));
    }
    // The task changed by the sync has a new version, so a stale editor cannot overwrite it.
    assert_eq!(second.task("Milk").version, theirs.version + 2);
}

#[test]
fn a_refused_upload_downloads_again_and_keeps_both_changes() {
    let (mut first, mut second, remote) = pair();
    first.add("From the first device");
    second.add("From the second device");
    let racing = Arc::clone(&remote);
    *remote.before_upload.lock().unwrap() = Some(Box::new(move || {
        second.sync(&racing);
    }));
    let report = first.sync(&remote);
    assert!((1..=2).contains(&report.received), "{report:?}");
    // The new task, and the other device's task when its equal position moved.
    assert!((1..=2).contains(&report.sent), "{report:?}");
    let titles: Vec<String> = remote
        .snapshot()
        .unwrap()
        .tasks
        .into_iter()
        .map(|task| task.title)
        .collect();
    assert_eq!(titles.len(), 2);
    assert_eq!(
        first.titles(),
        ["From the first device", "From the second device"]
    );
}

#[test]
fn a_text_conflict_keeps_both_versions_on_every_device() {
    let (mut first, mut second, remote) = pair();
    let milk = first.add("Buy milk");
    first.sync(&remote);
    second.sync(&remote);
    let mut edit = TaskInput::from(&milk);
    edit.title = "Buy oat milk".into();
    first
        .store
        .update_task(milk.id, milk.version, &edit)
        .unwrap();
    let theirs = second.task("Buy milk");
    let mut edit = TaskInput::from(&theirs);
    edit.title = "Buy whole milk".into();
    edit.due = day(14);
    second
        .store
        .update_task(theirs.id, theirs.version, &edit)
        .unwrap();
    first.sync(&remote);
    assert_eq!(
        second.sync(&remote),
        Report {
            received: 2,
            sent: 2,
            conflicts: 1
        }
    );
    first.sync(&remote);
    for device in [&first, &second] {
        assert_eq!(
            device.titles(),
            ["Buy oat milk", "Buy whole milk #conflict"]
        );
        assert_eq!(device.task("Buy oat milk").id, milk.id);
        assert_eq!(device.task("Buy oat milk").due, day(14));
        let copy = device.task("Buy whole milk #conflict");
        assert_eq!(copy.due, day(14));
        assert_eq!(
            (copy.position - device.task("Buy oat milk").position).abs(),
            1
        );
    }
    assert_eq!(first.visible(), second.visible());
    assert_eq!(second.store.sync_counters().unwrap().conflicts, 1);
    assert_eq!(second.sync(&remote), Report::default());
    assert_eq!(first.sync(&remote), Report::default());
}

#[test]
fn a_permanent_deletion_applies_unless_the_other_device_edited_the_task() {
    let (mut first, mut second, remote) = pair();
    first.add("Untouched");
    first.add("Edited elsewhere");
    first.sync(&remote);
    second.sync(&remote);
    for title in ["Untouched", "Edited elsewhere"] {
        let task = first.task(title);
        let trashed = first
            .store
            .set_deleted(task.id, task.version, true)
            .unwrap();
        assert_eq!(
            first.store.purge_task(trashed.id, trashed.version).unwrap(),
            TaskPurgeOutcome::Deleted
        );
    }
    let edited = second.task("Edited elsewhere");
    second
        .store
        .set_due(edited.id, edited.version, day(20))
        .unwrap();
    first.sync(&remote);
    second.sync(&remote);
    first.sync(&remote);
    for device in [&first, &second] {
        assert_eq!(device.titles(), ["Edited elsewhere"]);
        assert_eq!(device.task("Edited elsewhere").due, day(20));
    }
}

#[test]
fn tasks_created_with_one_id_keep_their_local_ids() {
    let (mut first, mut second, remote) = pair();
    let ours = first.add("Created on the first device");
    let theirs = second.add("Created on the second device");
    assert_eq!(ours.id, theirs.id);
    first.sync(&remote);
    second.sync(&remote);
    first.sync(&remote);
    assert_eq!(first.task("Created on the first device").id, ours.id);
    assert_eq!(second.task("Created on the second device").id, theirs.id);
    assert_eq!(first.task("Created on the second device").id, ours.id + 1);
    assert_eq!(second.task("Created on the first device").id, theirs.id + 1);
    // Equal positions became unique in the same order on both devices.
    let order = |device: &Device| -> Vec<String> {
        let mut tasks = device.store.tasks().unwrap();
        tasks.sort_by_key(|task| task.position);
        tasks.into_iter().map(|task| task.title).collect()
    };
    assert_eq!(order(&first), order(&second));
    let positions: Vec<i64> = first
        .store
        .tasks()
        .unwrap()
        .iter()
        .map(|task| task.position)
        .collect();
    assert_ne!(positions[0], positions[1]);
    // IDs line up again for tasks created after the sync.
    let next = first.add("Created later");
    first.sync(&remote);
    second.sync(&remote);
    assert_eq!(second.task("Created later").id, next.id);
}

#[test]
fn lists_merge_by_name_and_a_deleted_list_moves_its_new_tasks_to_the_default_list() {
    let (mut first, mut second, remote) = pair();
    let ours = first.store.create_list("Work").unwrap();
    let theirs = second.store.create_list(" work").unwrap();
    for (device, list, title) in [
        (&mut first, ours.id, "First work task"),
        (&mut second, theirs.id, "Second work task"),
    ] {
        device
            .store
            .create_task(&TaskInput {
                list_id: list,
                title: title.into(),
                ..TaskInput::default()
            })
            .unwrap();
    }
    first.sync(&remote);
    second.sync(&remote);
    first.sync(&remote);
    for device in [&first, &second] {
        let lists = device.store.lists().unwrap();
        assert_eq!(
            lists
                .iter()
                .map(|list| list.name.as_str())
                .collect::<Vec<_>>(),
            ["Inbox", "Work"]
        );
        assert_eq!(lists[1].count, 2);
    }

    let errands = first.store.create_list("Errands").unwrap();
    first.sync(&remote);
    second.sync(&remote);
    let preview = first.store.preview_list_deletion(errands.id).unwrap();
    assert_eq!(
        first.store.delete_list(&preview).unwrap(),
        ListDeletionOutcome::Deleted
    );
    let errands_there = second
        .store
        .lists()
        .unwrap()
        .into_iter()
        .find(|list| list.name == "Errands")
        .unwrap();
    second
        .store
        .create_task(&TaskInput {
            list_id: errands_there.id,
            title: "Post letter".into(),
            ..TaskInput::default()
        })
        .unwrap();
    first.sync(&remote);
    second.sync(&remote);
    first.sync(&remote);
    for device in [&first, &second] {
        assert!(
            device
                .store
                .lists()
                .unwrap()
                .iter()
                .all(|list| list.name != "Errands")
        );
        assert_eq!(device.task("Post letter").list_id, 1);
    }
}

#[test]
fn a_missing_or_foreign_file_deletes_nothing() {
    let (mut first, mut second, remote) = pair();
    first.add("Kept through a deleted file");
    first.sync(&remote);
    second.sync(&remote);
    let collection = remote.snapshot().unwrap().collection;

    remote.replace(None);
    let report = second.sync(&remote);
    assert_eq!(report.received, 0);
    assert_eq!(second.titles(), ["Kept through a deleted file"]);
    let recreated = remote.snapshot().unwrap();
    assert_ne!(recreated.collection, collection);
    assert_eq!(recreated.tasks.len(), 1);

    // The first device still holds a base from the old collection. It joins the new file.
    first.add("Added before rejoining");
    first.sync(&remote);
    second.sync(&remote);
    for device in [&first, &second] {
        assert_eq!(
            device.titles(),
            ["Added before rejoining", "Kept through a deleted file"]
        );
    }

    // A file from an unrelated profile adds its tasks and removes none.
    let mut stranger = Device::new();
    stranger.add("From another profile");
    let foreign = MemoryRemote::shared();
    stranger.sync(&foreign);
    remote.replace(Some(foreign.snapshot().unwrap().to_json()));
    first.sync(&remote);
    assert_eq!(
        first.titles(),
        [
            "Added before rejoining",
            "From another profile",
            "Kept through a deleted file"
        ]
    );
}

#[test]
fn a_rejected_or_failed_download_changes_nothing() {
    let (mut first, _second, remote) = pair();
    first.add("Local task");
    first.sync(&remote);
    let valid = remote.snapshot().unwrap();
    first.add("Unsent task");
    let before = first.visible();
    let counters = first.store.sync_counters().unwrap();

    let mut newer: serde_json::Value = serde_json::from_slice(&valid.to_json()).unwrap();
    newer["format"] = 2.into();
    newer["extra"] = "future field".into();
    remote.replace(Some(serde_json::to_vec(&newer).unwrap()));
    let failure = sync_once(&mut first.store, &remote).unwrap_err();
    assert_eq!(failure.kind, FailureKind::Incompatible);
    assert!(failure.detail.contains("Update Husker"), "{failure}");

    let broken = |change: &dyn Fn(&mut serde_json::Value)| -> Vec<u8> {
        let mut file: serde_json::Value = serde_json::from_slice(&valid.to_json()).unwrap();
        change(&mut file);
        serde_json::to_vec(&file).unwrap()
    };
    for broken in [
        b"not json".to_vec(),
        broken(&|file| file["extra"] = 1.into()),
        broken(&|file| file["collection"] = "".into()),
        broken(&|file| file["lists"][0]["name"] = " ".into()),
        broken(&|file| {
            let list = file["lists"][0].clone();
            file["lists"].as_array_mut().unwrap().push(list);
        }),
        broken(&|file| file["tasks"][0]["title"] = "".into()),
        broken(&|file| file["tasks"][0]["uid"] = "".into()),
        broken(&|file| file["default_list"] = "missing".into()),
        broken(&|file| file["tasks"][0]["id"] = i64::MAX.into()),
        broken(&|file| file["tasks"][0]["id"] = (-1).into()),
        broken(&|file| file["tasks"][0]["position"] = i64::MIN.into()),
        broken(&|file| file["lists"][0]["position"] = i64::MAX.into()),
        broken(&|file| {
            file.as_object_mut().unwrap().remove("format");
        }),
    ] {
        remote.replace(Some(broken));
        let failure = sync_once(&mut first.store, &remote).unwrap_err();
        assert_eq!(failure.kind, FailureKind::Invalid, "{failure}");
    }
    remote.fail(FailureKind::Offline);
    assert_eq!(
        sync_once(&mut first.store, &remote).unwrap_err().kind,
        FailureKind::Offline
    );
    assert_eq!(first.visible(), before);
    assert_eq!(first.store.sync_counters().unwrap(), counters);
    assert!(first.store.sync_pending().unwrap());

    // The next successful sync sends the task that waited.
    remote.recover();
    remote.replace(Some(valid.to_json()));
    first.sync(&remote);
    assert_eq!(remote.snapshot().unwrap().tasks.len(), 2);
    assert!(!first.store.sync_pending().unwrap());
}

#[test]
fn rows_without_a_uid_from_an_older_process_get_one_and_sync() {
    let (mut first, mut second, remote) = pair();
    first.add("Inserted by an old process");
    first
        .raw()
        .execute("UPDATE tasks SET uid = NULL", [])
        .unwrap();
    assert!(first.store.sync_pending().unwrap());
    first.sync(&remote);
    second.sync(&remote);
    assert_eq!(second.titles(), ["Inserted by an old process"]);
    let uid: String = first
        .raw()
        .query_row("SELECT uid FROM tasks", [], |row| row.get(0))
        .unwrap();
    assert_eq!(uid.len(), 32);
    assert!(uid.chars().all(|character| character.is_ascii_hexdigit()));
}

#[test]
fn sync_operations_reject_restricted_stores_and_stale_downloads() {
    let (mut first, _second, remote) = pair();
    first.add("Task");
    let path = first.directory.path().join("tasks.db");
    for mut restricted in [
        Store::open_for_mcp(&path, Some("Inbox"), false).unwrap(),
        Store::open_for_mcp(&path, None, true).unwrap(),
    ] {
        assert!(restricted.sync_etag().is_err());
        assert_eq!(restricted.sync_counters().unwrap().conflicts, 0);
        assert!(restricted.sync_pending().is_err());
        assert!(restricted.sync_merge(None, Downloaded::Missing).is_err());
        let snapshot = remote.snapshot().unwrap();
        assert!(restricted.sync_uploaded(None, &snapshot, "1").is_err());
    }
    let counters = first.store.sync_counters().unwrap();
    // A download made before another process synced is not merged.
    assert_eq!(
        first
            .store
            .sync_merge(Some("stale"), Downloaded::Missing)
            .unwrap(),
        None
    );
    assert_eq!(
        first.store.sync_merge(None, Downloaded::Missing).unwrap(),
        None
    );
    // An upload recorded after another process moved the ETag changes nothing.
    let etag = first.store.sync_etag().unwrap();
    let snapshot = remote.snapshot().unwrap();
    first
        .store
        .sync_uploaded(Some("stale"), &snapshot, "99")
        .unwrap();
    assert_eq!(first.store.sync_etag().unwrap(), etag);
    assert_eq!(first.store.sync_counters().unwrap(), counters);
    assert_eq!(first.titles(), ["Task"]);
}

#[test]
fn counters_tell_sync_commits_that_changed_tasks_from_those_that_did_not() {
    let (mut first, mut second, remote) = pair();
    let start = second.store.sync_counters().unwrap();
    first.add("From the first device");
    first.sync(&remote);
    second.sync(&remote);
    let received = second.store.sync_counters().unwrap();
    assert_eq!(received.commits, start.commits + 1);
    assert_eq!(received.applied, start.applied + 1);
    second.add("From the second device");
    second.sync(&remote);
    let sent = second.store.sync_counters().unwrap();
    // One commit remembers what is being uploaded, and one records the answer.
    assert_eq!(sent.commits, received.commits + 2);
    assert_eq!(sent.applied, received.applied);
    assert_eq!(sent.conflicts, 0);
}

#[test]
fn a_restored_backup_catches_up_without_duplicates() {
    let (mut first, mut second, remote) = pair();
    first.add("Before the backup");
    first.sync(&remote);
    let backup = tempfile::tempdir().unwrap();
    drop(
        first
            .raw()
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);"),
    );
    std::fs::copy(
        first.directory.path().join("tasks.db"),
        backup.path().join("tasks.db"),
    )
    .unwrap();
    first.add("After the backup");
    first.sync(&remote);
    second.sync(&remote);

    let mut restored = Store::open(&backup.path().join("tasks.db")).unwrap();
    let report = sync_once(&mut restored, &remote).unwrap();
    assert_eq!(report.received, 1);
    assert_eq!(report.sent, 0);
    let mut titles: Vec<String> = restored
        .tasks()
        .unwrap()
        .into_iter()
        .map(|task| task.title)
        .collect();
    titles.sort();
    assert_eq!(titles, ["After the backup", "Before the backup"]);
    assert_eq!(second.titles(), titles);
}

#[test]
fn an_upload_with_a_lost_answer_does_not_undo_later_edits() {
    let (mut first, mut second, remote) = pair();
    let rent = first.add("Pay rent");
    let bank = first.add("Call bank");
    first.sync(&remote);

    // The storage keeps this upload, but the device never gets the answer.
    let rent = first.store.set_due(rent.id, rent.version, day(16)).unwrap();
    let mut edit = TaskInput::from(&bank);
    edit.title = "Call bank about loan".into();
    let bank = first
        .store
        .update_task(bank.id, bank.version, &edit)
        .unwrap();
    *remote.lose_next_answer.lock().unwrap() = true;
    assert_eq!(
        sync_once(&mut first.store, &remote).unwrap_err().kind,
        FailureKind::Offline
    );
    assert_eq!(remote.snapshot().unwrap().uploads.len(), 3);

    // The user edits both tasks again before the next sync.
    first.store.set_due(rent.id, rent.version, day(20)).unwrap();
    edit.title = "Call bank about mortgage".into();
    first
        .store
        .update_task(bank.id, bank.version, &edit)
        .unwrap();
    assert_eq!(
        first.sync(&remote),
        Report {
            received: 0,
            sent: 2,
            conflicts: 0
        }
    );
    second.sync(&remote);
    for device in [&first, &second] {
        assert_eq!(device.titles(), ["Call bank about mortgage", "Pay rent"]);
        assert_eq!(device.task("Pay rent").due, day(20));
    }
}

#[test]
fn a_lost_answer_is_recognized_after_another_device_uploaded_on_top() {
    let (mut first, mut second, remote) = pair();
    let bank = first.add("Call bank");
    first.sync(&remote);
    let mut edit = TaskInput::from(&bank);
    edit.title = "Call bank about loan".into();
    let bank = first
        .store
        .update_task(bank.id, bank.version, &edit)
        .unwrap();
    *remote.lose_next_answer.lock().unwrap() = true;
    assert!(sync_once(&mut first.store, &remote).is_err());

    // Another device receives the upload and replaces the file before the retry.
    second.sync(&remote);
    assert_eq!(second.titles(), ["Call bank about loan"]);
    second.add("From the second device");
    second.sync(&remote);

    edit.title = "Call bank about mortgage".into();
    first
        .store
        .update_task(bank.id, bank.version, &edit)
        .unwrap();
    let report = first.sync(&remote);
    assert_eq!(report.conflicts, 0);
    second.sync(&remote);
    for device in [&first, &second] {
        assert_eq!(
            device.titles(),
            ["Call bank about mortgage", "From the second device"]
        );
    }
}

#[test]
fn a_lost_answer_on_the_first_upload_keeps_later_edits_and_deletions() {
    let remote = MemoryRemote::shared();
    let mut device = Device::new();
    let kept = device.add("Kept");
    let purged = device.add("Deleted forever after the upload");
    *remote.lose_next_answer.lock().unwrap() = true;
    assert!(sync_once(&mut device.store, &remote).is_err());
    assert!(device.store.sync_pending().unwrap());

    device
        .store
        .set_due(kept.id, kept.version, day(20))
        .unwrap();
    let trashed = device
        .store
        .set_deleted(purged.id, purged.version, true)
        .unwrap();
    device
        .store
        .purge_task(trashed.id, trashed.version)
        .unwrap();
    let report = device.sync(&remote);
    assert_eq!(report.received, 0);
    assert_eq!(report.conflicts, 0);
    assert_eq!(device.titles(), ["Kept"]);
    assert_eq!(device.task("Kept").due, day(20));
    let file = remote.snapshot().unwrap();
    assert_eq!(file.tasks.len(), 1);
    assert_eq!(file.tasks[0].due, day(20));
}

#[test]
fn a_file_written_again_keeps_unsent_changes_and_deletes_nothing() {
    let (mut first, mut second, remote) = pair();
    first.add("Mark me done");
    first.sync(&remote);
    second.sync(&remote);
    first.add("Sent before the file was deleted");
    first.sync(&remote);
    let task = first.task("Mark me done");
    first
        .store
        .set_status(task.id, task.version, Status::Done)
        .unwrap();

    // The file is deleted. The second device, which is behind, writes it again.
    remote.replace(None);
    second.sync(&remote);
    assert_eq!(remote.snapshot().unwrap().tasks.len(), 1);

    let report = first.sync(&remote);
    assert_eq!(report.conflicts, 0);
    second.sync(&remote);
    for device in [&first, &second] {
        assert_eq!(
            device.titles(),
            ["Mark me done", "Sent before the file was deleted"]
        );
        assert_eq!(device.task("Mark me done").status, Status::Done);
    }
}

#[test]
fn a_file_written_again_from_an_old_state_keeps_newer_text_as_one_copy() {
    let (mut first, mut second, remote) = pair();
    let mut stale = Device::new();
    let flights = first.add("Book flights");
    first.sync(&remote);
    second.sync(&remote);
    stale.sync(&remote);
    let mut edit = TaskInput::from(&flights);
    edit.title = "Book flights to Oslo".into();
    first
        .store
        .update_task(flights.id, flights.version, &edit)
        .unwrap();
    first.sync(&remote);
    second.sync(&remote);

    // The file is deleted and the device that missed the rename writes it again.
    remote.replace(None);
    stale.sync(&remote);
    for _ in 0..2 {
        first.sync(&remote);
        second.sync(&remote);
        stale.sync(&remote);
    }
    for device in [&first, &second, &stale] {
        assert_eq!(
            device.titles(),
            ["Book flights", "Book flights to Oslo #conflict"]
        );
    }
    assert_eq!(first.sync(&remote), Report::default());
}

#[test]
fn a_permanent_deletion_wins_over_a_reorder_on_another_device() {
    let (mut first, mut second, remote) = pair();
    first.add("One");
    first.add("Two");
    first.sync(&remote);
    second.sync(&remote);
    let one = first.task("One");
    let trashed = first.store.set_deleted(one.id, one.version, true).unwrap();
    first.store.purge_task(trashed.id, trashed.version).unwrap();
    let (one, two) = (second.task("One"), second.task("Two"));
    second.store.swap_positions(&one, &two).unwrap();
    first.sync(&remote);
    second.sync(&remote);
    first.sync(&remote);
    for device in [&first, &second] {
        assert_eq!(device.titles(), ["Two"]);
    }
}

#[test]
fn an_id_freed_by_a_permanent_deletion_is_not_reused_for_a_synced_task() {
    let (mut first, mut second, remote) = pair();
    let local = second.add("Deleted forever here");
    let trashed = second
        .store
        .set_deleted(local.id, local.version, true)
        .unwrap();
    second
        .store
        .purge_task(trashed.id, trashed.version)
        .unwrap();
    let theirs = first.add("Created on the first device");
    assert_eq!(theirs.id, local.id);
    first.sync(&remote);
    second.sync(&remote);
    assert_eq!(
        second.task("Created on the first device").id,
        local.id + 1,
        "the ID of the deleted task must not point at another task"
    );
}

#[test]
fn a_sync_gives_up_after_five_refused_uploads_and_keeps_the_change() {
    let (mut first, _second, remote) = pair();
    first.add("Waits for a quiet file");
    *remote.always_refuse.lock().unwrap() = true;
    let downloads = remote.counts().0;
    let failure = sync_once(&mut first.store, &remote).unwrap_err();
    assert_eq!(failure.kind, FailureKind::Busy);
    assert_eq!(remote.counts().0, downloads + 5);
    assert!(first.store.sync_pending().unwrap());
    *remote.always_refuse.lock().unwrap() = false;
    assert_eq!(first.sync(&remote).sent, 1);
    assert_eq!(remote.snapshot().unwrap().tasks.len(), 1);
}

#[test]
fn the_file_remembers_twenty_uploads_and_names_the_default_list() {
    let (mut first, _second, remote) = pair();
    for number in 0..25 {
        first.add(&format!("Task {number}"));
        first.sync(&remote);
    }
    let file = remote.snapshot().unwrap();
    assert_eq!(file.uploads.len(), 20);
    assert_eq!(
        file.lists
            .iter()
            .find(|list| list.uid == file.default_list)
            .map(|list| list.name.as_str()),
        Some("Inbox")
    );
}
