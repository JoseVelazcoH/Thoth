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

fn setup_duration_boundary() -> Fixture {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db").to_string_lossy().to_string();
    let log = dir.path().join("error.log").to_string_lossy().to_string();
    let alpha = dir.path().join("alpha");
    std::fs::create_dir_all(alpha.join(".git")).unwrap();
    let alpha_str = alpha.to_string_lossy().to_string();
    let beta = dir.path().join("beta");
    std::fs::create_dir_all(beta.join(".git")).unwrap();
    let beta_str = beta.to_string_lossy().to_string();

    record(
        &db,
        &log,
        RecordInput {
            cmd: "fast_cmd",
            dir: &alpha_str,
            exit: 0,
            dur: 999,
            tags: "[]",
            ts: 1_700_000_000,
        },
    );
    record(
        &db,
        &log,
        RecordInput {
            cmd: "slow_cmd",
            dir: &alpha_str,
            exit: 0,
            dur: 3200,
            tags: "[]",
            ts: 1_700_001_000,
        },
    );
    record(
        &db,
        &log,
        RecordInput {
            cmd: "both_tags_cmd",
            dir: &alpha_str,
            exit: 0,
            dur: 500,
            tags: r#"["tagA","tagB"]"#,
            ts: 1_700_002_000,
        },
    );
    record(
        &db,
        &log,
        RecordInput {
            cmd: "only_tagA_cmd",
            dir: &beta_str,
            exit: 0,
            dur: 500,
            tags: r#"["tagA"]"#,
            ts: 1_700_003_000,
        },
    );
    record(
        &db,
        &log,
        RecordInput {
            cmd: "session2_cmd",
            dir: &beta_str,
            exit: 0,
            dur: 200,
            tags: "[]",
            ts: 1_700_010_000,
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
fn search_duration_boundary_include_exclude() {
    let f = setup_duration_boundary();
    let out = tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["search", "--duration", ">1"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("slow_cmd"),
        "slow_cmd (3200ms) should be included by >1s filter"
    );
    assert!(
        !text.contains("fast_cmd"),
        "fast_cmd (999ms) should be excluded by >1s filter"
    );
}

#[test]
fn search_tag_and_filter_requires_both() {
    let f = setup_duration_boundary();
    let out = tth()
        .env("THOTH_DB", &f.db)
        .env("THOTH_ERROR_LOG", &f.log)
        .args(["search", "--tag", "tagA", "--tag", "tagB"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    assert!(
        text.contains("both_tags_cmd"),
        "row with both tagA and tagB should be included"
    );
    assert!(
        !text.contains("only_tagA_cmd"),
        "row with only tagA should be excluded by AND filter"
    );
}

#[test]
fn search_show_session_two_sessions_shows_two_headers() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db").to_string_lossy().to_string();
    let log = dir.path().join("error.log").to_string_lossy().to_string();
    let alpha = dir.path().join("alpha");
    std::fs::create_dir_all(alpha.join(".git")).unwrap();
    let alpha_str = alpha.to_string_lossy().to_string();
    let beta = dir.path().join("beta");
    std::fs::create_dir_all(beta.join(".git")).unwrap();
    let beta_str = beta.to_string_lossy().to_string();

    record(
        &db,
        &log,
        RecordInput {
            cmd: "session1_cmd",
            dir: &alpha_str,
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
            cmd: "session2_cmd",
            dir: &beta_str,
            exit: 0,
            dur: 200,
            tags: "[]",
            ts: 1_700_100_000,
        },
    );

    let out = tth()
        .env("THOTH_DB", &db)
        .env("THOTH_ERROR_LOG", &log)
        .args(["search", "--show-session"])
        .assert()
        .code(0)
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8_lossy(&out);
    let header_count = text.lines().filter(|l| l.starts_with("---")).count();
    assert_eq!(
        header_count, 2,
        "expected two session headers; got: {header_count}\noutput:\n{text}"
    );
    assert!(
        text.contains("session1_cmd"),
        "session1_cmd must appear in output"
    );
    assert!(
        text.contains("session2_cmd"),
        "session2_cmd must appear in output"
    );
    assert!(text.contains("2 result(s)"), "count line must be present");
}
