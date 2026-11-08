use std::{
    path::Path,
    process::{Command, Output},
};

use serde_json::Value;

fn run(profile: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_husker"))
        .arg("--profile")
        .arg(profile)
        .args(arguments)
        .output()
        .unwrap()
}

fn json(profile: &Path, arguments: &[&str]) -> Value {
    let mut args = vec!["--json"];
    args.extend_from_slice(arguments);
    let output = run(profile, &args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn scripts_can_capture_filter_complete_trash_and_restore() {
    let directory = tempfile::tempdir().unwrap();
    let profile = directory.path();
    let list = json(profile, &["list-add", "Private"]);
    let task = json(
        profile,
        &[
            "add",
            "Køb mælk",
            "--list",
            "private",
            "--description",
            "Oat milk\nTwo cartons",
            "--tag",
            "Shopping",
            "--due",
            "2026-11-08",
        ],
    );
    assert_eq!(task["list_id"], list["id"]);
    let id = task["id"].to_string();
    let results = json(profile, &["tasks", "l:PRIVATE MÆLK #shopping"]);
    assert_eq!(results.as_array().unwrap().len(), 1);
    assert_eq!(
        json(profile, &["show", &id])["description"],
        "Oat milk\nTwo cartons"
    );
    assert_eq!(json(profile, &["done", &id])["status"], "done");
    assert!(json(profile, &["tasks"]).as_array().unwrap().is_empty());
    assert_eq!(json(profile, &["tasks", "s:done"])[0]["id"], task["id"]);
    json(profile, &["trash", &id]);
    assert!(
        json(profile, &["tasks", "s:all"])
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(json(profile, &["tasks", "--trash"])[0]["status"], "done");
    let restored = json(profile, &["restore", &id]);
    assert_eq!(restored["list_id"], list["id"]);
    assert_eq!(restored["status"], "done");
    json(profile, &["reopen", &id]);
    let lists = json(profile, &["lists"]);
    assert_eq!(lists[1]["count"], 1);
    assert_eq!(lists.as_array().unwrap().last().unwrap()["name"], "Trash");
}

#[test]
fn edits_preserve_unspecified_fields_and_reject_stale_versions() {
    let directory = tempfile::tempdir().unwrap();
    let profile = directory.path();
    let task = json(
        profile,
        &["add", "Task", "--due", "2026-11-08", "--tag", "first"],
    );
    let id = task["id"].to_string();
    let edited = json(profile, &["edit", &id, "--title", "Changed", "--clear-due"]);
    assert_eq!(edited["title"], "Changed");
    assert_eq!(edited["tags"], serde_json::json!(["first"]));
    assert!(edited["due"].is_null());
    assert_eq!(edited["ready"], true);
    let stale = run(
        profile,
        &[
            "--json",
            "edit",
            &id,
            "--title",
            "Overwrite",
            "--if-version",
            "1",
        ],
    );
    assert!(!stale.status.success());
    assert!(stale.stdout.is_empty());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("changed"));
    assert_eq!(json(profile, &["show", &id])["title"], "Changed");
    let edited = json(
        profile,
        &["edit", &id, "--clear-tags", "--readiness", "maybe"],
    );
    assert_eq!(edited["tags"], serde_json::json!([]));
    assert_eq!(edited["ready"], false);
    json(profile, &["edit", &id, "--due", "2026-11-08"]);
    let maybe = json(
        profile,
        &["edit", &id, "--clear-due", "--readiness", "maybe"],
    );
    assert!(maybe["due"].is_null());
    assert_eq!(maybe["ready"], false);
}

#[test]
fn invalid_input_has_no_success_output_or_task_write() {
    let directory = tempfile::tempdir().unwrap();
    let profile = directory.path();
    for args in [
        vec!["--json", "add", "Task", "--due", "2026-02-30"],
        vec!["--json", "add", "Task", "--due", "2026-2-03"],
        vec!["--json", "tasks", "due:2026-02-30"],
        vec!["--json", "show", "999"],
        vec!["--json", "add", "   "],
    ] {
        let output = run(profile, &args);
        assert!(!output.status.success(), "Unexpected success for {args:?}");
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
    assert!(
        json(profile, &["tasks", "s:all"])
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn profile_and_configuration_paths_are_independent() {
    let directory = tempfile::tempdir().unwrap();
    let first = directory.path().join("first");
    let second = directory.path().join("second");
    let settings = directory.path().join("danish.toml");
    std::fs::write(&settings, "language = 'da'\n").unwrap();
    json(
        &first,
        &[
            "--config",
            settings.to_str().unwrap(),
            "add",
            "Private task",
        ],
    );
    assert!(json(&second, &["tasks"]).as_array().unwrap().is_empty());
    assert_eq!(json(&first, &["tasks"])[0]["title"], "Private task");
    let missing = directory.path().join("missing.toml");
    let untouched = directory.path().join("untouched");
    assert!(
        !run(
            &untouched,
            &["--config", missing.to_str().unwrap(), "tasks"]
        )
        .status
        .success()
    );
    assert!(!untouched.exists());
}

#[test]
fn config_example_is_loadable_and_has_no_profile_side_effects() {
    let directory = tempfile::tempdir().unwrap();
    let profile = directory.path().join("new-profile");
    let output = run(&profile, &["config", "--example"]);
    assert!(output.status.success());
    assert!(!profile.exists());
    let path = directory.path().join("config.toml");
    std::fs::write(&path, output.stdout).unwrap();
    husker::config::Config::load(&path, true).unwrap();
}

#[test]
fn noninteractive_default_launch_fails_without_creating_a_profile() {
    let directory = tempfile::tempdir().unwrap();
    let profile = directory.path().join("unused");
    let output = run(&profile, &[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("interactive terminal"));
    assert!(!profile.exists());
}
