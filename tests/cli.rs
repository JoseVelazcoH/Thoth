use assert_cmd::Command;
use predicates::prelude::*;
use rusqlite::Connection;
use tempfile::TempDir;
use std::path::PathBuf;

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
fn cli_terminal_id_persisted() {
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
            "test-cmd",
            "--exit",
            "0",
            "--duration",
            "1",
            "--timestamp",
            "1700000000",
            "--terminal-id",
            "term-abc",
        ])
        .assert()
        .code(0);

    let conn = Connection::open(&db_path).unwrap();
    let terminal_id: Option<String> = conn
        .query_row(
            "SELECT terminal_id FROM commands WHERE command='test-cmd'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(terminal_id, Some(String::from("term-abc")));
}

#[test]
fn cli_terminal_id_null_when_omitted() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("history.db");
    tth()
        .env("THOTH_DB", db_path.to_str().unwrap())
        .env(
            "THOTH_ERROR_LOG",
            dir.path().join("error.log").to_str().unwrap(),
        )
        .args(["record", "--cmd", "no-tid-cmd", "--exit", "0"])
        .assert()
        .code(0);

    let conn = Connection::open(&db_path).unwrap();
    let terminal_id: Option<String> = conn
        .query_row(
            "SELECT terminal_id FROM commands WHERE command='no-tid-cmd'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(terminal_id.is_none());
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

#[test]
fn new_session_id_prints_uuid() {
    let output = tth().args(["new-session-id"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let trimmed = stdout.trim();
    assert_eq!(trimmed.len(), 36, "expected uuid v4 length");
    assert_eq!(
        trimmed.chars().filter(|&c| c == '-').count(),
        4,
        "expected 4 hyphens in uuid"
    );
}

#[test]
fn new_session_id_differs_across_runs() {
    let out1 = tth().args(["new-session-id"]).output().unwrap();
    let out2 = tth().args(["new-session-id"]).output().unwrap();
    let id1 = String::from_utf8(out1.stdout).unwrap();
    let id2 = String::from_utf8(out2.stdout).unwrap();
    assert_ne!(
        id1.trim(),
        id2.trim(),
        "two runs must produce distinct uuids"
    );
}

#[test]
fn install_writes_sentinel_to_rc_file() {
    let dir = TempDir::new().unwrap();
    let rc = dir.path().join(".bashrc");
    let db_path = dir.path().join("history.db");
    tth()
        .env("THOTH_DB", db_path.to_str().unwrap())
        .env(
            "THOTH_ERROR_LOG",
            dir.path().join("error.log").to_str().unwrap(),
        )
        .args([
            "install",
            "--shell",
            "bash",
            "--rc-file",
            rc.to_str().unwrap(),
        ])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("Installed"));
    let content = std::fs::read_to_string(&rc).unwrap();
    assert!(content.contains("# === THOTH HOOKS BEGIN ==="));
}

#[test]
fn install_idempotent_no_duplicate_sentinel() {
    let dir = TempDir::new().unwrap();
    let rc = dir.path().join(".zshrc");
    let db_path = dir.path().join("history.db");
    for _ in 0..2 {
        tth()
            .env("THOTH_DB", db_path.to_str().unwrap())
            .env(
                "THOTH_ERROR_LOG",
                dir.path().join("error.log").to_str().unwrap(),
            )
            .args([
                "install",
                "--shell",
                "zsh",
                "--rc-file",
                rc.to_str().unwrap(),
            ])
            .assert()
            .code(0);
    }
    let content = std::fs::read_to_string(&rc).unwrap();
    assert_eq!(content.matches("# === THOTH HOOKS BEGIN ===").count(), 1);
}

#[test]
fn uninstall_removes_sentinel_from_rc_file() {
    let dir = TempDir::new().unwrap();
    let rc = dir.path().join(".bashrc");
    let db_path = dir.path().join("history.db");
    tth()
        .env("THOTH_DB", db_path.to_str().unwrap())
        .env(
            "THOTH_ERROR_LOG",
            dir.path().join("error.log").to_str().unwrap(),
        )
        .args([
            "install",
            "--shell",
            "bash",
            "--rc-file",
            rc.to_str().unwrap(),
        ])
        .assert()
        .code(0);
    tth()
        .env("THOTH_DB", db_path.to_str().unwrap())
        .env(
            "THOTH_ERROR_LOG",
            dir.path().join("error.log").to_str().unwrap(),
        )
        .args(["uninstall", "--rc-file", rc.to_str().unwrap()])
        .assert()
        .code(0);
    let content = std::fs::read_to_string(&rc).unwrap();
    assert!(!content.contains("# === THOTH HOOKS BEGIN ==="));
}

#[test]
fn init_zsh_prints_hook_script() {
    let output = tth().args(["init", "zsh"]).output().unwrap();
    assert!(output.status.success(), "tth init zsh must exit 0");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("_tth_preexec"), "missing _tth_preexec");
    assert!(stdout.contains("_tth_precmd"), "missing _tth_precmd");
    assert!(stdout.contains("bindkey '^R'"), "missing bindkey '^R'");
    assert!(stdout.contains("tth|tth\\ *"), "missing capture exclusion");
    assert!(
        !stdout.contains("# === THOTH HOOKS BEGIN ==="),
        "raw script must not contain sentinel"
    );
}

#[test]
fn init_bash_prints_hook_script() {
    let output = tth().args(["init", "bash"]).output().unwrap();
    assert!(output.status.success(), "tth init bash must exit 0");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("_thoth_preexec"), "missing _thoth_preexec");
    assert!(stdout.contains("_thoth_precmd"), "missing _thoth_precmd");
    assert!(stdout.contains("bind -x"), "missing bind -x");
    assert!(stdout.contains("tth|tth\\ *"), "missing capture exclusion");
    assert!(
        !stdout.contains("# === THOTH HOOKS BEGIN ==="),
        "raw script must not contain sentinel"
    );
}

#[test]
fn init_unknown_shell_exits_nonzero() {
    tth()
        .args(["init", "fish"])
        .assert()
        .failure()
        .stderr(predicate::str::starts_with("tth: "));
}

#[test]
fn install_writes_eval_line_not_full_body() {
    let dir = TempDir::new().unwrap();
    let rc = dir.path().join(".zshrc");
    let db_path = dir.path().join("history.db");
    tth()
        .env("THOTH_DB", db_path.to_str().unwrap())
        .env(
            "THOTH_ERROR_LOG",
            dir.path().join("error.log").to_str().unwrap(),
        )
        .args([
            "install",
            "--shell",
            "zsh",
            "--rc-file",
            rc.to_str().unwrap(),
        ])
        .assert()
        .code(0);
    let content = std::fs::read_to_string(&rc).unwrap();
    assert!(
        content.contains("eval \"$(tth init zsh)\""),
        "eval line not found; got:\n{content}"
    );
    assert!(
        !content.contains("_tth_preexec"),
        "full hook body must not be written to rc"
    );
}

#[test]
fn install_migration_replaces_old_full_block() {
    let dir = TempDir::new().unwrap();
    let rc = dir.path().join(".zshrc");
    let db_path = dir.path().join("history.db");
    let old_block =
        "# === THOTH HOOKS BEGIN ===\n_tth_preexec() { : ; }\n# === THOTH HOOKS END ===\n";
    std::fs::write(&rc, old_block).unwrap();
    tth()
        .env("THOTH_DB", db_path.to_str().unwrap())
        .env(
            "THOTH_ERROR_LOG",
            dir.path().join("error.log").to_str().unwrap(),
        )
        .args([
            "install",
            "--shell",
            "zsh",
            "--rc-file",
            rc.to_str().unwrap(),
        ])
        .assert()
        .code(0);
    let content = std::fs::read_to_string(&rc).unwrap();
    assert_eq!(
        content.matches("# === THOTH HOOKS BEGIN ===").count(),
        1,
        "expected exactly one sentinel block"
    );
    assert!(
        content.contains("eval \"$(tth init zsh)\""),
        "eval line not found after migration"
    );
    assert!(
        !content.contains("_tth_preexec() { : ; }"),
        "old body still present after migration"
    );
}

#[test]
fn status_exits_0_and_prints_info() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("history.db");
    tth()
        .env("THOTH_DB", db_path.to_str().unwrap())
        .env(
            "THOTH_ERROR_LOG",
            dir.path().join("error.log").to_str().unwrap(),
        )
        .args(["status"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("Schema version"));
}

#[test]
fn lifecycle_error_exits_nonzero_and_prints_stderr() {
    let dir = TempDir::new().unwrap();
    let db_path = dir.path().join("history.db");
    let unwritable = dir.path().join("no").join("such").join("path.rc");
    tth()
        .env("THOTH_DB", db_path.to_str().unwrap())
        .env(
            "THOTH_ERROR_LOG",
            dir.path().join("error.log").to_str().unwrap(),
        )
        .args([
            "install",
            "--shell",
            "bash",
            "--rc-file",
            unwritable.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::starts_with("tth: "));
}

#[test]
fn record_never_fail_contract_preserved() {
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
        .args(["record", "--cmd", "whoami"])
        .assert()
        .code(0);
}

fn tth_with_env(db_path: &PathBuf, error_log: &PathBuf) -> Command {
    let mut cmd = tth();
    cmd.env("THOTH_DB", db_path.to_str().unwrap())
        .env("THOTH_ERROR_LOG", error_log.to_str().unwrap());
    cmd
}

#[test]
fn prompt_framework_starship_prints_env_var_module() {
    let dir = TempDir::new().unwrap();
    tth_with_env(&dir.path().join("h.db"), &dir.path().join("e.log"))
        .args(["prompt", "--framework", "starship"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("env_var.thoth_tags"))
        .stdout(predicate::str::contains("TTH_PROMPT_TAGS"));
}

#[test]
fn prompt_unknown_framework_exits_nonzero() {
    let dir = TempDir::new().unwrap();
    tth_with_env(&dir.path().join("h.db"), &dir.path().join("e.log"))
        .args(["prompt", "--framework", "fish"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown framework"));
}

#[test]
fn prompt_framework_generic_prints_generic_snippet() {
    let dir = TempDir::new().unwrap();
    tth_with_env(&dir.path().join("h.db"), &dir.path().join("e.log"))
        .args(["prompt", "--framework", "generic"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("PROMPT").or(predicate::str::contains("PS1")));
}

#[test]
fn install_output_includes_prompt_hint() {
    let dir = TempDir::new().unwrap();
    let rc = dir.path().join(".bashrc");
    tth_with_env(&dir.path().join("h.db"), &dir.path().join("e.log"))
        .args([
            "install",
            "--shell",
            "bash",
            "--rc-file",
            rc.to_str().unwrap(),
        ])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("To show active tags in your prompt:"));
}

#[test]
fn doctor_exits_0_and_prints_checklist() {
    let dir = TempDir::new().unwrap();
    tth_with_env(&dir.path().join("h.db"), &dir.path().join("e.log"))
        .args(["doctor"])
        .assert()
        .code(0)
        .stdout(
            predicate::str::contains("hooks installed")
                .and(predicate::str::contains("database"))
                .and(predicate::str::contains("tth on PATH"))
                .and(predicate::str::contains("prompt tags visibility")),
        );
}
