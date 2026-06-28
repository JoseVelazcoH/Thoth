use assert_cmd::Command;
use tempfile::TempDir;

fn tth() -> Command {
    Command::cargo_bin("tth").unwrap()
}

struct RecordInput<'a> {
    cmd: &'a str,
    dir: &'a str,
    exit: i64,
    dur: i64,
    tags: &'a str,
    ts: i64,
}

fn record(db: &str, log: &str, r: RecordInput<'_>) {
    tth()
        .env("THOTH_DB", db)
        .env("THOTH_ERROR_LOG", log)
        .args([
            "record",
            "--cmd",
            r.cmd,
            "--dir",
            r.dir,
            "--exit",
            &r.exit.to_string(),
            "--duration",
            &r.dur.to_string(),
            "--tags",
            r.tags,
            "--timestamp",
            &r.ts.to_string(),
        ])
        .assert()
        .code(0);
}

struct Fixture {
    _dir: TempDir,
    db: String,
    log: String,
    alpha_dir: String,
    _beta_dir: String,
}

fn setup() -> Fixture {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db").to_string_lossy().to_string();
    let log = dir.path().join("error.log").to_string_lossy().to_string();

    let alpha = dir.path().join("alpha");
    let beta = dir.path().join("beta");
    std::fs::create_dir_all(alpha.join(".git")).unwrap();
    std::fs::create_dir_all(beta.join(".git")).unwrap();

    let alpha_dir = alpha.to_string_lossy().to_string();
    let beta_dir = beta.to_string_lossy().to_string();

    record(
        &db,
        &log,
        RecordInput {
            cmd: "cargo build",
            dir: &alpha_dir,
            exit: 0,
            dur: 1200,
            tags: r#"["rust"]"#,
            ts: 1_700_000_000,
        },
    );
    record(
        &db,
        &log,
        RecordInput {
            cmd: "make test",
            dir: &beta_dir,
            exit: 1,
            dur: 500,
            tags: "[]",
            ts: 1_700_001_000,
        },
    );
    record(
        &db,
        &log,
        RecordInput {
            cmd: "docker run nginx",
            dir: &alpha_dir,
            exit: 0,
            dur: 3000,
            tags: r#"["docker","infra"]"#,
            ts: 1_700_002_000,
        },
    );

    Fixture {
        _dir: dir,
        db,
        log,
        alpha_dir,
        _beta_dir: beta_dir.to_string(),
    }
}

#[test]
fn export_no_filters_exits_0_and_starts_with_shebang() {
    let f = setup();
    let out = tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["export"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.starts_with("#!/usr/bin/env bash\n"),
        "output must start with shebang; got:\n{text}"
    );
    assert!(text.contains("# Thoth export"));
    assert!(text.contains("cargo build"));
    assert!(text.contains("make test"));
    assert!(text.contains("docker run nginx"));
}

#[test]
fn export_chronological_order() {
    let f = setup();
    let out = tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["export"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    let pos_first = text.find("cargo build").unwrap();
    let pos_second = text.find("make test").unwrap();
    let pos_third = text.find("docker run nginx").unwrap();
    assert!(
        pos_first < pos_second && pos_second < pos_third,
        "commands must appear in chronological order"
    );
}

#[test]
fn export_row_comment_format() {
    let f = setup();
    let out = tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["export"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("[22:13:20]"),
        "HH:MM:SS comment expected; got:\n{text}"
    );
    assert!(text.contains("[exit: 0]"));
    assert!(text.contains("[duration: 1.2s]"));
    assert!(text.contains(&format!("[{}]", f.alpha_dir)));
}

#[test]
fn export_project_filter() {
    let f = setup();
    let out = tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["export", "--project", "alpha"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("cargo build"));
    assert!(text.contains("docker run nginx"));
    assert!(!text.contains("make test"));
    assert!(text.contains("# Project: alpha"));
}

#[test]
fn export_exit_fail_filter() {
    let f = setup();
    let out = tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["export", "--exit", "fail"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("make test"));
    assert!(!text.contains("cargo build"));
}

#[test]
fn export_tag_filter() {
    let f = setup();
    let out = tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["export", "--tag", "rust"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("cargo build"));
    assert!(!text.contains("make test"));
    assert!(!text.contains("docker run nginx"));
    assert!(text.contains("# Tags: rust"));
}

#[test]
fn export_session_filter() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db").to_string_lossy().to_string();
    let log = dir.path().join("error.log").to_string_lossy().to_string();

    let alpha = dir.path().join("alpha");
    let beta = dir.path().join("beta");
    std::fs::create_dir_all(alpha.join(".git")).unwrap();
    std::fs::create_dir_all(beta.join(".git")).unwrap();

    let alpha_dir = alpha.to_string_lossy().to_string();
    let beta_dir = beta.to_string_lossy().to_string();

    record(
        &db,
        &log,
        RecordInput {
            cmd: "alpha_cmd_1",
            dir: &alpha_dir,
            exit: 0,
            dur: 100,
            tags: "[]",
            ts: 1_700_000_000,
        },
    );
    record(
        &db,
        &log,
        RecordInput {
            cmd: "beta_cmd_1",
            dir: &beta_dir,
            exit: 0,
            dur: 100,
            tags: "[]",
            ts: 1_700_000_001,
        },
    );
    let conn = rusqlite::Connection::open(&db).unwrap();
    let alpha_session_id: String = conn
        .query_row(
            "SELECT session_id FROM commands WHERE command = 'alpha_cmd_1' LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();

    let out = tth()
        .env("THOTH_DB", &db)
        .env("THOTH_ERROR_LOG", &log)
        .args(["export", "--session", &alpha_session_id])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("alpha_cmd_1"),
        "alpha_cmd_1 must be in session export; got:\n{text}"
    );
    assert!(
        !text.contains("beta_cmd_1"),
        "beta_cmd_1 must be excluded; got:\n{text}"
    );
}

#[test]
fn export_empty_result_still_valid_bash() {
    let f = setup();
    let out = tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["export", "--project", "no-such-project"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.starts_with("#!/usr/bin/env bash\n"),
        "even empty export must start with shebang"
    );
    assert!(text.contains("# (no commands matched)"));
}

#[test]
fn export_output_parses_as_bash() {
    let f = setup();
    let out = tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["export"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();

    let dir = TempDir::new().unwrap();
    let script_path = dir.path().join("exported.sh");
    std::fs::write(&script_path, &out).unwrap();

    let status = std::process::Command::new("bash")
        .args(["-n", script_path.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(
        status.success(),
        "exported script must parse cleanly with bash -n"
    );
}
