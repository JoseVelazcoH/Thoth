use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

fn tth(db_dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("tth").unwrap();
    cmd.env("THOTH_DB", db_dir.path().join("history.db"));
    cmd
}

fn tth_with_tags(db_dir: &TempDir, active_tags: &str) -> Command {
    let mut cmd = tth(db_dir);
    cmd.env("TTH_ACTIVE_TAGS", active_tags);
    cmd
}

fn record_with_tags(db_dir: &TempDir, command: &str, tags: &str) {
    tth(db_dir)
        .args([
            "record",
            "--cmd",
            command,
            "--dir",
            "/tmp",
            "--exit",
            "0",
            "--duration",
            "0",
            "--timestamp",
            "1700000000",
            "--tags",
            tags,
        ])
        .assert()
        .success();
}

#[test]
fn tag_unset_env_adds_single_tag_to_stdout() {
    let dir = TempDir::new().unwrap();
    let out = tth(&dir).args(["tag", "x"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("export TTH_ACTIVE_TAGS='[\"x\"]'"),
        "stdout: {stdout}"
    );
    assert!(
        stdout.contains("export TTH_PROMPT_TAGS='[x]'"),
        "stdout: {stdout}"
    );
    assert!(out.status.success());
}

#[test]
fn tag_with_existing_adds_second() {
    let dir = TempDir::new().unwrap();
    let out = tth_with_tags(&dir, r#"["x"]"#)
        .args(["tag", "y"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains(r#"["x","y"]"#), "stdout: {stdout}");
    assert!(stdout.contains("[x][y]"), "stdout: {stdout}");
}

#[test]
fn tag_duplicate_is_noop() {
    let dir = TempDir::new().unwrap();
    let out = tth_with_tags(&dir, r#"["x"]"#)
        .args(["tag", "x"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains(r#"["x"]"#), "stdout: {stdout}");
    assert!(!stdout.contains(r#"["x","x"]"#), "dup added: {stdout}");
}

#[test]
fn tag_empty_name_errors_nonzero() {
    let dir = TempDir::new().unwrap();
    tth(&dir).args(["tag", ""]).assert().failure();
}

#[test]
fn untag_removes_one() {
    let dir = TempDir::new().unwrap();
    let out = tth_with_tags(&dir, r#"["x","y"]"#)
        .args(["untag", "x"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains(r#"["y"]"#), "stdout: {stdout}");
    assert!(!stdout.contains("\"x\""), "x still present: {stdout}");
}

#[test]
fn untag_all_clears() {
    let dir = TempDir::new().unwrap();
    let out = tth_with_tags(&dir, r#"["x","y"]"#)
        .args(["untag", "--all"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("TTH_ACTIVE_TAGS='[]'"), "stdout: {stdout}");
    assert!(stdout.contains("TTH_PROMPT_TAGS=''"), "stdout: {stdout}");
}

#[test]
fn untag_no_args_errors_nonzero() {
    let dir = TempDir::new().unwrap();
    tth(&dir).args(["untag"]).assert().failure();
}

#[test]
fn tags_no_flag_reads_env_active() {
    let dir = TempDir::new().unwrap();
    tth_with_tags(&dir, r#"["fix","perf"]"#)
        .args(["tags"])
        .assert()
        .success()
        .stdout(contains("fix"))
        .stdout(contains("perf"));
}

#[test]
fn tags_no_flag_no_env_shows_none() {
    let dir = TempDir::new().unwrap();
    let out = tth(&dir).args(["tags"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("(none)") || stdout.trim().is_empty() || stdout.contains("no active"),
        "expected none indicator, got: {stdout}"
    );
}

#[test]
fn tags_list_queries_db() {
    let dir = TempDir::new().unwrap();
    record_with_tags(&dir, "git commit", r#"["fix","perf"]"#);
    record_with_tags(&dir, "cargo test", r#"["fix"]"#);
    tth(&dir)
        .args(["tags", "--list"])
        .assert()
        .success()
        .stdout(contains("fix"))
        .stdout(contains("perf"));
}

#[test]
fn prompt_prints_snippet() {
    let dir = TempDir::new().unwrap();
    let out = tth(&dir).args(["prompt"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.trim().is_empty(),
        "expected prompt snippet, got empty stdout"
    );
    assert!(out.status.success());
}

#[test]
fn tag_confirmation_goes_to_stderr() {
    let dir = TempDir::new().unwrap();
    let out = tth(&dir).args(["tag", "fix"]).output().unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.is_empty(), "expected confirmation on stderr");
}
