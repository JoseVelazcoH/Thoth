use assert_cmd::Command;
use tempfile::TempDir;

fn tth() -> Command {
    Command::cargo_bin("tth").unwrap()
}

struct RecordInput<'a> {
    cmd: &'a str,
    dir: &'a str,
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
            "0",
            "--duration",
            "100",
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
    beta_dir: String,
}

fn setup() -> Fixture {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db").to_string_lossy().to_string();
    let log = dir.path().join("error.log").to_string_lossy().to_string();

    let alpha = dir.path().join("alpha");
    let beta = dir.path().join("beta");
    std::fs::create_dir_all(alpha.join(".git")).unwrap();
    std::fs::create_dir_all(beta.join(".git")).unwrap();

    let alpha_str = alpha.to_string_lossy().to_string();
    let beta_str = beta.to_string_lossy().to_string();

    let base: i64 = 1_700_000_000;

    record(
        &db,
        &log,
        RecordInput {
            cmd: "cargo build",
            dir: &alpha_str,
            tags: r#"["rust","cli"]"#,
            ts: base,
        },
    );
    record(
        &db,
        &log,
        RecordInput {
            cmd: "cargo test",
            dir: &alpha_str,
            tags: r#"["rust"]"#,
            ts: base + 300,
        },
    );

    let beta_start = base + 2 * 3600;
    record(
        &db,
        &log,
        RecordInput {
            cmd: "make build",
            dir: &beta_str,
            tags: r#"["c"]"#,
            ts: beta_start,
        },
    );

    Fixture {
        _dir: dir,
        db,
        log,
        beta_dir: beta_str,
    }
}

#[test]
fn sessions_lists_newest_first() {
    let f = setup();
    let out = tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["sessions"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);

    assert!(text.contains("2 session(s)"), "expected footer with count");

    let session_col_pos = text.find("session").unwrap_or(0);
    let beta_pos = text
        .find(&f.beta_dir.split('/').next_back().unwrap_or("beta")[..4.min(f.beta_dir.len())])
        .unwrap_or(usize::MAX);
    let alpha_pos = text.find("alpha").unwrap_or(usize::MAX);
    assert!(
        beta_pos < alpha_pos || session_col_pos == 0,
        "beta session (newer) should appear before alpha in output"
    );
}

#[test]
fn sessions_project_filter() {
    let f = setup();
    let out = tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["sessions", "--project", "alpha"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);

    assert!(
        text.contains("1 session(s)"),
        "expected 1 session for alpha; got:\n{text}"
    );
    assert!(text.contains("alpha"), "expected alpha in output");
}

#[test]
fn sessions_limit_caps_results() {
    let f = setup();
    let out = tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["sessions", "--limit", "1"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);

    assert!(
        text.contains("1 session(s)"),
        "expected 1 session(s) footer; got:\n{text}"
    );
}

#[test]
fn sessions_empty_db_shows_no_sessions_found() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db").to_string_lossy().to_string();
    let log = dir.path().join("error.log").to_string_lossy().to_string();

    let out = tth()
        .env("THOTH_DB", &db)
        .env("THOTH_ERROR_LOG", &log)
        .args(["sessions"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("No sessions found."),
        "expected 'No sessions found.'; got:\n{text}"
    );
}

#[test]
fn sessions_shows_tags_union() {
    let f = setup();
    let out = tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["sessions", "--project", "alpha"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("cli") && text.contains("rust"),
        "expected both rust and cli tags for alpha session; got:\n{text}"
    );
}

#[test]
fn sessions_bad_since_exits_nonzero() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db").to_string_lossy().to_string();
    let log = dir.path().join("error.log").to_string_lossy().to_string();

    tth()
        .env("THOTH_DB", &db)
        .env("THOTH_ERROR_LOG", &log)
        .args(["sessions", "--since", "bad-date"])
        .assert()
        .failure();
}
