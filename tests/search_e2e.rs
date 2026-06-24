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
    _alpha_dir: String,
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

    let alpha_str = alpha.to_string_lossy().to_string();
    let beta_str = beta.to_string_lossy().to_string();

    record(
        &db,
        &log,
        RecordInput {
            cmd: "cargo build",
            dir: &alpha_str,
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
            dir: &beta_str,
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
            dir: &alpha_str,
            exit: 0,
            dur: 3000,
            tags: r#"["docker","infra"]"#,
            ts: 1_700_002_000,
        },
    );
    record(
        &db,
        &log,
        RecordInput {
            cmd: "ls -la",
            dir: &beta_str,
            exit: 0,
            dur: 50,
            tags: "[]",
            ts: 1_700_003_000,
        },
    );

    Fixture {
        _dir: dir,
        db,
        log,
        _alpha_dir: alpha_str,
        _beta_dir: beta_str,
    }
}

#[test]
fn search_no_filters_exits_0_and_shows_commands() {
    let f = setup();
    tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["search"])
        .assert()
        .code(0)
        .stdout(predicates::str::contains("cargo build"));
}

#[test]
fn search_project_filter_includes_and_excludes() {
    let f = setup();
    let out = tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["search", "--project", "alpha"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("cargo build"),
        "expected cargo build in output"
    );
    assert!(
        !text.contains("make test"),
        "expected make test to be excluded"
    );
}

#[test]
fn search_exit_fail_filter() {
    let f = setup();
    let out = tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["search", "--exit", "fail"])
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
fn search_tag_filter() {
    let f = setup();
    let out = tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["search", "--tag", "rust"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("cargo build"));
    assert!(!text.contains("make test"));
}

#[test]
fn search_duration_valid_exits_0() {
    let f = setup();
    tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["search", "--duration", ">0"])
        .assert()
        .code(0);
}

#[test]
fn search_duration_bad_exits_nonzero_with_error() {
    let f = setup();
    let cmd_out = tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["search", "--duration", "bad"])
        .output()
        .unwrap();
    assert!(!cmd_out.status.success(), "expected non-zero exit");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&cmd_out.stdout),
        String::from_utf8_lossy(&cmd_out.stderr)
    );
    assert!(
        combined.contains("duration") || combined.contains("bad") || combined.contains("Error"),
        "expected error text, got: {combined}"
    );
}

#[test]
fn search_since_bad_date_exits_nonzero() {
    let f = setup();
    let cmd_out = tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["search", "--since", "bad-date"])
        .output()
        .unwrap();
    assert!(
        !cmd_out.status.success(),
        "expected non-zero exit for bad date"
    );
}

#[test]
fn search_show_session_exits_0_with_separator() {
    let f = setup();
    let out = tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["search", "--show-session"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(text.contains("---"), "expected session separator in output");
}
