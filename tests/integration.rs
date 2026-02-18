use assert_cmd::Command;
use assert_fs::TempDir;
use predicates::prelude::*;
use std::fs;
use std::time::{Duration, SystemTime};

/// Sets up a temporary HOME with `.claude/projects/{project}/` and copies
/// fixtures into it with UUID filenames. Returns the temp dir (must be kept alive).
fn setup_test_home(fixtures: &[(&str, &str, SystemTime)]) -> TempDir {
    let tmp = TempDir::new().unwrap();
    let projects = tmp.path().join(".claude").join("projects");
    fs::create_dir_all(&projects).unwrap();

    for (project_name, fixture_name, mtime) in fixtures {
        let project_dir = projects.join(project_name);
        fs::create_dir_all(&project_dir).unwrap();

        let fixture_src = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(fixture_name);

        // Use a deterministic UUID based on the fixture name
        let uuid = fixture_to_uuid(fixture_name);
        let dest = project_dir.join(format!("{}.jsonl", uuid));

        fs::copy(&fixture_src, &dest).unwrap();

        // Set mtime
        let times = fs::FileTimes::new().set_modified(*mtime);
        fs::File::options()
            .write(true)
            .open(&dest)
            .unwrap()
            .set_times(times)
            .unwrap();
    }

    tmp
}

/// Map fixture name to a deterministic UUID for test predictability.
fn fixture_to_uuid(name: &str) -> &str {
    match name {
        "normal.jsonl" => "eb53d999-8692-42ce-a376-4f82206a086d",
        "meta_only.jsonl" => "ab53d999-8692-42ce-a376-4f82206a086d",
        "empty.jsonl" => "cb53d999-8692-42ce-a376-4f82206a086d",
        "xml_markup.jsonl" => "db53d999-8692-42ce-a376-4f82206a086d",
        "slash_command.jsonl" => "fb53d999-8692-42ce-a376-4f82206a086d",
        "array_content.jsonl" => "0b53d999-8692-42ce-a376-4f82206a086d",
        "compact_summary.jsonl" => "1b53d999-8692-42ce-a376-4f82206a086d",
        "truncated.jsonl" => "2b53d999-8692-42ce-a376-4f82206a086d",
        "summary_only.jsonl" => "3b53d999-8692-42ce-a376-4f82206a086d",
        "no_cwd.jsonl" => "4b53d999-8692-42ce-a376-4f82206a086d",
        "image_paste.jsonl" => "5b53d999-8692-42ce-a376-4f82206a086d",
        _ => panic!("Unknown fixture: {}", name),
    }
}

fn ccsesh_cmd(home: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("ccsesh").unwrap();
    cmd.env("HOME", home.path().to_str().unwrap());
    cmd.env("NO_COLOR", "1");
    cmd
}

// ---- Default format tests ----

#[test]
fn default_format_with_sessions() {
    let now = SystemTime::now();
    let tmp = setup_test_home(&[
        ("-project-a", "normal.jsonl", now),
        ("-project-b", "slash_command.jsonl", now - Duration::from_secs(3600)),
    ]);

    ccsesh_cmd(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("Recent Claude Code sessions:"))
        .stdout(predicate::str::contains("Resume: ccsesh <number>"))
        .stdout(predicate::str::contains("Design technical approach for ccsesh"));
}

#[test]
fn default_format_shows_indices() {
    let now = SystemTime::now();
    let tmp = setup_test_home(&[
        ("-project-a", "normal.jsonl", now),
        ("-project-b", "slash_command.jsonl", now - Duration::from_secs(60)),
    ]);

    let output = ccsesh_cmd(&tmp).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should show indices 0 and 1
    assert!(stdout.contains("0"));
    assert!(stdout.contains("1"));
}

// ---- JSON format tests ----

#[test]
fn json_output_valid() {
    let now = SystemTime::now();
    let tmp = setup_test_home(&[
        ("-project-a", "normal.jsonl", now),
    ]);

    let output = ccsesh_cmd(&tmp)
        .arg("--json")
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .expect("Output should be valid JSON");

    let arr = parsed.as_array().expect("Should be a JSON array");
    assert_eq!(arr.len(), 1);

    let session = &arr[0];
    assert_eq!(session["index"], 0);
    assert!(session["session_id"].as_str().unwrap().len() == 36);
    assert!(session["project_dir"].is_string());
    assert!(session["project_dir_display"].is_string());
    assert!(session["last_active"].is_string());
    assert!(session["last_active_relative"].is_string());
    assert!(session["resume_command"].as_str().unwrap().contains("claude --resume"));
}

#[test]
fn json_takes_precedence_over_format() {
    let now = SystemTime::now();
    let tmp = setup_test_home(&[
        ("-project-a", "normal.jsonl", now),
    ]);

    let output = ccsesh_cmd(&tmp)
        .args(["--json", "--format", "short"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should be JSON, not short format
    let _parsed: serde_json::Value = serde_json::from_str(&stdout)
        .expect("--json should produce JSON even with --format short");
}

// ---- Short format tests ----

#[test]
fn short_format_output() {
    let now = SystemTime::now();
    let tmp = setup_test_home(&[
        ("-project-a", "normal.jsonl", now),
        ("-project-b", "slash_command.jsonl", now - Duration::from_secs(120)),
    ]);

    ccsesh_cmd(&tmp)
        .args(["--format", "short", "--limit", "2"])
        .assert()
        .success()
        // Short format has no header/footer
        .stdout(predicate::str::contains("Recent Claude Code sessions:").not())
        .stdout(predicate::str::contains("Resume:").not());
}

// ---- Limit tests ----

#[test]
fn limit_zero_no_error() {
    let now = SystemTime::now();
    let tmp = setup_test_home(&[
        ("-project-a", "normal.jsonl", now),
    ]);

    ccsesh_cmd(&tmp)
        .args(["--limit", "0"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Recent Claude Code sessions:"))
        .stdout(predicate::str::contains("Resume: ccsesh <number>"));
}

#[test]
fn limit_restricts_output() {
    let now = SystemTime::now();
    let tmp = setup_test_home(&[
        ("-project-a", "normal.jsonl", now),
        ("-project-b", "slash_command.jsonl", now - Duration::from_secs(60)),
        ("-project-c", "array_content.jsonl", now - Duration::from_secs(120)),
    ]);

    let output = ccsesh_cmd(&tmp)
        .args(["--json", "--limit", "2"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 2);
}

#[test]
fn limit_zero_json_empty_array() {
    let now = SystemTime::now();
    let tmp = setup_test_home(&[
        ("-project-a", "normal.jsonl", now),
    ]);

    let output = ccsesh_cmd(&tmp)
        .args(["--json", "--limit", "0"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(parsed.as_array().unwrap().len(), 0);
}

#[test]
fn limit_zero_short_empty() {
    let now = SystemTime::now();
    let tmp = setup_test_home(&[
        ("-project-a", "normal.jsonl", now),
    ]);

    ccsesh_cmd(&tmp)
        .args(["--format", "short", "--limit", "0"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

// ---- Resume tests ----

#[test]
fn resume_without_shell_mode() {
    let now = SystemTime::now();
    let tmp = setup_test_home(&[
        ("-project-a", "normal.jsonl", now),
    ]);

    ccsesh_cmd(&tmp)
        .arg("0")
        .assert()
        .success()
        .stdout(predicate::str::contains("To resume this session, run:"))
        .stdout(predicate::str::contains("claude --resume"));
}

#[test]
fn resume_with_shell_mode() {
    let now = SystemTime::now();
    let tmp = setup_test_home(&[
        ("-project-a", "normal.jsonl", now),
    ]);

    ccsesh_cmd(&tmp)
        .args(["0", "--shell-mode", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("__CCSESH_EXEC__"))
        .stdout(predicate::str::contains("claude --resume"));
}

#[test]
fn resume_out_of_range() {
    let now = SystemTime::now();
    let tmp = setup_test_home(&[
        ("-project-a", "normal.jsonl", now),
        ("-project-b", "slash_command.jsonl", now - Duration::from_secs(60)),
    ]);

    ccsesh_cmd(&tmp)
        .arg("99")
        .assert()
        .failure()
        .stderr(predicate::str::contains("out of range"));
}

#[test]
fn limit_3_index_4_out_of_range() {
    let now = SystemTime::now();
    let tmp = setup_test_home(&[
        ("-proj-a", "normal.jsonl", now),
        ("-proj-b", "slash_command.jsonl", now - Duration::from_secs(60)),
        ("-proj-c", "array_content.jsonl", now - Duration::from_secs(120)),
        ("-proj-d", "truncated.jsonl", now - Duration::from_secs(180)),
        ("-proj-e", "compact_summary.jsonl", now - Duration::from_secs(240)),
    ]);

    // limit 3 means only indices 0-2 are valid; index 4 should fail
    ccsesh_cmd(&tmp)
        .args(["--limit", "3", "4"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("out of range"));
}

// ---- Shell mode without index ----

#[test]
fn shell_mode_without_index_errors() {
    let now = SystemTime::now();
    let tmp = setup_test_home(&[
        ("-project-a", "normal.jsonl", now),
    ]);

    ccsesh_cmd(&tmp)
        .args(["--shell-mode", "fish"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--shell-mode requires a session index"));
}

// ---- Unknown command ----

#[test]
fn unknown_command_errors() {
    let now = SystemTime::now();
    let tmp = setup_test_home(&[
        ("-project-a", "normal.jsonl", now),
    ]);

    ccsesh_cmd(&tmp)
        .arg("foobar")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown command 'foobar'"));
}

// ---- Init tests ----

#[test]
fn init_fish() {
    let tmp = TempDir::new().unwrap();

    ccsesh_cmd(&tmp)
        .args(["init", "fish"])
        .assert()
        .success()
        .stdout(predicate::str::contains("function ccsesh"));
}

#[test]
fn init_bash() {
    let tmp = TempDir::new().unwrap();

    ccsesh_cmd(&tmp)
        .args(["init", "bash"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ccsesh()"));
}

#[test]
fn init_zsh() {
    let tmp = TempDir::new().unwrap();

    ccsesh_cmd(&tmp)
        .args(["init", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ccsesh()"))
        .stdout(predicate::str::contains("print -r --"));
}

#[test]
fn init_without_shell_errors() {
    let tmp = TempDir::new().unwrap();

    ccsesh_cmd(&tmp)
        .arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("ccsesh init <fish|bash|zsh>"));
}

#[test]
fn init_unknown_shell_errors() {
    let tmp = TempDir::new().unwrap();

    ccsesh_cmd(&tmp)
        .args(["init", "nushell"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Unknown shell 'nushell'"));
}

// ---- Empty/missing directory tests ----

#[test]
fn empty_session_directory_errors() {
    let tmp = TempDir::new().unwrap();
    let projects = tmp.path().join(".claude").join("projects");
    fs::create_dir_all(&projects).unwrap();

    ccsesh_cmd(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("No Claude Code sessions found"));
}

#[test]
fn missing_projects_dir_errors() {
    let tmp = TempDir::new().unwrap();
    // Don't create .claude/projects/

    ccsesh_cmd(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("No Claude Code session directory found"));
}

// ---- Edge case: all sessions unparseable ----

#[test]
fn all_unparseable_sessions_treated_as_no_sessions() {
    let tmp = TempDir::new().unwrap();
    let projects = tmp.path().join(".claude").join("projects");
    let project = projects.join("-project-a");
    fs::create_dir_all(&project).unwrap();

    // Create a file with an invalid UUID filename
    let bad_file = project.join("not-a-uuid.jsonl");
    fs::write(&bad_file, r#"{"type":"user","message":{"content":"test"}}"#).unwrap();

    ccsesh_cmd(&tmp)
        .assert()
        .failure()
        .stderr(predicate::str::contains("No Claude Code sessions found"));
}

// ---- JSON schema completeness ----

#[test]
fn json_schema_fields_complete() {
    let now = SystemTime::now();
    let tmp = setup_test_home(&[
        ("-project-a", "normal.jsonl", now),
    ]);

    let output = ccsesh_cmd(&tmp)
        .arg("--json")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let session = &parsed[0];

    // All required fields present
    assert!(session.get("index").is_some(), "missing index");
    assert!(session.get("session_id").is_some(), "missing session_id");
    assert!(session.get("project_dir").is_some(), "missing project_dir");
    assert!(session.get("project_dir_display").is_some(), "missing project_dir_display");
    assert!(session.get("last_active").is_some(), "missing last_active");
    assert!(session.get("last_active_relative").is_some(), "missing last_active_relative");
    assert!(session.get("first_prompt").is_some(), "missing first_prompt");
    assert!(session.get("slug").is_some(), "missing slug");
    assert!(session.get("resume_command").is_some(), "missing resume_command");

    // last_active should end with Z (UTC)
    let last_active = session["last_active"].as_str().unwrap();
    assert!(last_active.ends_with('Z'), "last_active should end with Z");

    // resume_command should have cd and claude --resume
    let cmd = session["resume_command"].as_str().unwrap();
    assert!(cmd.contains("cd "));
    assert!(cmd.contains("claude --resume"));
}

// ---- Session with slug but no prompt ----

#[test]
fn session_with_slug_no_prompt_shown_in_default() {
    let now = SystemTime::now();
    let tmp = setup_test_home(&[
        ("-project-a", "meta_only.jsonl", now),
    ]);

    ccsesh_cmd(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("gentle-morning-breeze"));
}

// ---- Empty session fallback ----

#[test]
fn empty_session_shows_fallback() {
    let now = SystemTime::now();
    let tmp = setup_test_home(&[
        ("-project-a", "empty.jsonl", now),
    ]);

    ccsesh_cmd(&tmp)
        .assert()
        .success()
        .stdout(predicate::str::contains("(empty session)"));
}

// ---- JSON nullable fields ----

#[test]
fn json_null_first_prompt_for_meta_only() {
    let now = SystemTime::now();
    let tmp = setup_test_home(&[
        ("-project-a", "meta_only.jsonl", now),
    ]);

    let output = ccsesh_cmd(&tmp)
        .arg("--json")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let session = &parsed[0];

    assert!(session["first_prompt"].is_null(), "meta-only session should have null first_prompt");
    assert!(session["slug"].is_string(), "meta-only session should have slug");
}

#[test]
fn json_null_slug_for_empty_session() {
    let now = SystemTime::now();
    let tmp = setup_test_home(&[
        ("-project-a", "empty.jsonl", now),
    ]);

    let output = ccsesh_cmd(&tmp)
        .arg("--json")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let session = &parsed[0];

    assert!(session["first_prompt"].is_null());
    assert!(session["slug"].is_null());
}
