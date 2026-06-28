use assert_cmd::Command;
use tempfile::TempDir;

fn tth(db_dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("tth").unwrap();
    cmd.env("THOTH_DB", db_dir.path().join("history.db"));
    cmd
}

fn record(db_dir: &TempDir, command: &str) {
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
            "--terminal-id",
            "default-tid",
        ])
        .assert()
        .success();
}

fn record_with_terminal(db_dir: &TempDir, command: &str, terminal_id: &str) {
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
            "--terminal-id",
            terminal_id,
        ])
        .assert()
        .success();
}

#[test]
fn dry_run_shows_preview_and_does_not_delete() {
    let dir = TempDir::new().unwrap();
    record(&dir, "echo alpha");
    record(&dir, "echo beta");
    record(&dir, "echo gamma");

    tth(&dir)
        .args(["forget", "--last", "2", "--dry-run"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Would forget 2 command(s)."));

    let out = tth(&dir)
        .args(["search", "--limit", "100"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("3 result(s)"),
        "dry-run must not delete; still expect 3 rows, got: {stdout}"
    );
}

#[test]
fn forget_last_2_leaves_1() {
    let dir = TempDir::new().unwrap();
    record(&dir, "echo one");
    record(&dir, "echo two");
    record(&dir, "echo three");

    tth(&dir)
        .args(["forget", "--last", "2"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Forgot 2 command(s)."));

    let out = tth(&dir)
        .args(["search", "--limit", "100"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("1 result(s)"),
        "expected 1 row after forgetting 2, got: {stdout}"
    );
}

#[test]
fn forget_on_empty_db_prints_no_commands() {
    let dir = TempDir::new().unwrap();

    tth(&dir)
        .arg("forget")
        .assert()
        .success()
        .stdout(predicates::str::contains("No commands to forget."));
}

#[test]
fn terminal_scoping_only_removes_target_terminal() {
    let dir = TempDir::new().unwrap();
    record_with_terminal(&dir, "cmd_x_1", "tid-x");
    record_with_terminal(&dir, "cmd_x_2", "tid-x");
    record_with_terminal(&dir, "cmd_y_1", "tid-y");

    tth(&dir)
        .args(["forget", "--last", "2", "--terminal-id", "tid-x"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Forgot 2 command(s)."));

    let out = tth(&dir)
        .args(["search", "--limit", "100"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("1 result(s)"),
        "expected 1 row (tid-y command) remaining, got: {stdout}"
    );
    assert!(
        stdout.contains("cmd_y_1"),
        "tid-y command must survive, got: {stdout}"
    );
}

#[test]
fn last_zero_returns_error() {
    let dir = TempDir::new().unwrap();

    tth(&dir)
        .args(["forget", "--last", "0"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("nothing to forget"));
}
