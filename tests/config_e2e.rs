use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn tth_config_env(dir: &TempDir) -> (std::path::PathBuf, std::path::PathBuf) {
    let db = dir.path().join("history.db");
    let cfg = dir.path().join("config.toml");
    (db, cfg)
}

fn tth_with(db: &std::path::Path, cfg: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("tth").unwrap();
    cmd.env("THOTH_DB", db.to_str().unwrap())
        .env("THOTH_CONFIG", cfg.to_str().unwrap())
        .env("THOTH_ERROR_LOG", "/dev/null")
        .env("NO_COLOR", "1");
    cmd
}

#[test]
fn config_set_then_get_orientation() {
    let dir = TempDir::new().unwrap();
    let (db, cfg) = tth_config_env(&dir);

    tth_with(&db, &cfg)
        .args(["config", "set", "tui.orientation", "top"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("set tui.orientation = top"));

    tth_with(&db, &cfg)
        .args(["config", "get", "tui.orientation"])
        .assert()
        .code(0)
        .stdout(predicate::str::is_match("^top\n?$").unwrap());
}

#[test]
fn config_print_shows_toml_after_set() {
    let dir = TempDir::new().unwrap();
    let (db, cfg) = tth_config_env(&dir);

    tth_with(&db, &cfg)
        .args(["config", "set", "tui.orientation", "top"])
        .assert()
        .code(0);

    tth_with(&db, &cfg)
        .args(["config", "print"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("orientation"));
}

#[test]
fn config_bare_prints_summary() {
    let dir = TempDir::new().unwrap();
    let (db, cfg) = tth_config_env(&dir);

    tth_with(&db, &cfg)
        .args(["config"])
        .assert()
        .code(0)
        .stdout(
            predicate::str::contains("gap_minutes")
                .and(predicate::str::contains("orientation"))
                .and(predicate::str::contains("default_limit")),
        );
}

#[test]
fn config_set_invalid_gap_minutes_exits_nonzero_and_file_unchanged() {
    let dir = TempDir::new().unwrap();
    let (db, cfg) = tth_config_env(&dir);

    std::fs::write(&cfg, "# existing content\n").unwrap();
    let original = std::fs::read_to_string(&cfg).unwrap();

    tth_with(&db, &cfg)
        .args(["config", "set", "session.gap_minutes", "abc"])
        .assert()
        .code(1);

    let after = std::fs::read_to_string(&cfg).unwrap();
    assert_eq!(original, after, "file must not change on error");
}

#[test]
fn config_get_unknown_key_exits_nonzero() {
    let dir = TempDir::new().unwrap();
    let (db, cfg) = tth_config_env(&dir);

    tth_with(&db, &cfg)
        .args(["config", "get", "foo.bar"])
        .assert()
        .code(1);
}

#[test]
fn install_writes_default_config_when_absent() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let cfg = dir.path().join("subdir").join("config.toml");
    let rc = dir.path().join(".zshrc");

    Command::cargo_bin("tth")
        .unwrap()
        .env("THOTH_DB", db.to_str().unwrap())
        .env("THOTH_CONFIG", cfg.to_str().unwrap())
        .env("THOTH_ERROR_LOG", "/dev/null")
        .env("NO_COLOR", "1")
        .env("SHELL", "/bin/zsh")
        .args(["install", "--rc-file", rc.to_str().unwrap()])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("Wrote default config to"));

    assert!(cfg.exists(), "config file must have been written");
    let content = std::fs::read_to_string(&cfg).unwrap();
    assert!(
        content.contains("gap_minutes"),
        "default config must have gap_minutes"
    );
}

#[test]
fn install_does_not_overwrite_existing_config() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let cfg = dir.path().join("config.toml");
    let rc = dir.path().join(".zshrc");

    std::fs::write(&cfg, "# my custom config\n").unwrap();

    Command::cargo_bin("tth")
        .unwrap()
        .env("THOTH_DB", db.to_str().unwrap())
        .env("THOTH_CONFIG", cfg.to_str().unwrap())
        .env("THOTH_ERROR_LOG", "/dev/null")
        .env("NO_COLOR", "1")
        .env("SHELL", "/bin/zsh")
        .args(["install", "--rc-file", rc.to_str().unwrap()])
        .assert()
        .code(0);

    let content = std::fs::read_to_string(&cfg).unwrap();
    assert_eq!(
        content, "# my custom config\n",
        "existing config must not be overwritten"
    );
}
