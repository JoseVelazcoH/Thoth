use assert_cmd::Command;
use rusqlite::Connection;
use tempfile::TempDir;

fn tth() -> Command {
    Command::cargo_bin("tth").unwrap()
}

#[test]
fn cli_exits_0_on_success() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("history.db");
    tth()
        .env("THOTH_DB", db_path.to_str().unwrap())
        .env(
            "THOTH_ERROR_LOG",
            dir.path().join("error.log").to_str().unwrap(),
        )
        .args(["record", "--cmd", "ls"])
        .assert()
        .code(0);
}

#[test]
fn cli_exits_0_on_unwritable_db() {
    let dir = TempDir::new().unwrap();
    let ro_dir = dir.path().join("readonly");
    std::fs::create_dir_all(&ro_dir).unwrap();
    let mut perms = std::fs::metadata(&ro_dir).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o444);
    std::fs::set_permissions(&ro_dir, perms).unwrap();
    tth()
        .env("THOTH_DB", ro_dir.join("history.db").to_str().unwrap())
        .env(
            "THOTH_ERROR_LOG",
            dir.path().join("error.log").to_str().unwrap(),
        )
        .args(["record", "--cmd", "ls"])
        .assert()
        .code(0);
}

#[test]
fn cli_all_six_flags_persisted() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("history.db");
    tth()
        .env("THOTH_DB", db_path.to_str().unwrap())
        .env(
            "THOTH_ERROR_LOG",
            dir.path().join("error.log").to_str().unwrap(),
        )
        .args([
            "record",
            "--cmd",
            "echo hi",
            "--dir",
            "/tmp",
            "--exit",
            "42",
            "--duration",
            "100",
            "--timestamp",
            "1700000000",
            "--tags",
            r#"["test"]"#,
        ])
        .assert()
        .code(0);

    let conn = Connection::open(&db_path).unwrap();
    let (cmd, dir_val, exit_code, duration, ts, tags): (String, String, i64, i64, i64, String) =
        conn.query_row(
            "SELECT command, directory, exit_code, duration_ms, timestamp, tags FROM commands",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(cmd, "echo hi");
    assert_eq!(dir_val, "/tmp");
    assert_eq!(exit_code, 42);
    assert_eq!(duration, 100);
    assert_eq!(ts, 1700000000);
    assert_eq!(tags, r#"["test"]"#);
}

#[test]
fn cli_default_dir_is_cwd() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("history.db");
    let cwd = std::env::current_dir().unwrap();
    tth()
        .env("THOTH_DB", db_path.to_str().unwrap())
        .env(
            "THOTH_ERROR_LOG",
            dir.path().join("error.log").to_str().unwrap(),
        )
        .args(["record", "--cmd", "pwd"])
        .assert()
        .code(0);

    let conn = Connection::open(&db_path).unwrap();
    let directory: String = conn
        .query_row("SELECT directory FROM commands", [], |r| r.get(0))
        .unwrap();
    assert_eq!(directory, cwd.to_string_lossy().as_ref());
}
