use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn tth_with(db: &std::path::Path, cfg: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("tth").unwrap();
    cmd.env("THOTH_DB", db.to_str().unwrap())
        .env("THOTH_CONFIG", cfg.to_str().unwrap())
        .env("THOTH_ERROR_LOG", "/dev/null")
        .env("NO_COLOR", "1");
    cmd
}

fn env_pair(dir: &TempDir) -> (std::path::PathBuf, std::path::PathBuf) {
    let db = dir.path().join("history.db");
    let cfg = dir.path().join("config.toml");
    (db, cfg)
}

#[test]
fn theme_bare_prints_current_default() {
    let dir = TempDir::new().unwrap();
    let (db, cfg) = env_pair(&dir);

    tth_with(&db, &cfg)
        .arg("theme")
        .assert()
        .code(0)
        .stdout(predicate::str::contains("default"));
}

#[test]
fn theme_list_shows_seven_builtins() {
    let dir = TempDir::new().unwrap();
    let (db, cfg) = env_pair(&dir);

    let assert = tth_with(&db, &cfg).args(["theme", "list"]).assert().code(0);

    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);

    for name in &[
        "default",
        "ember",
        "frost",
        "latte",
        "frappe",
        "macchiato",
        "mocha",
    ] {
        assert!(stdout.contains(name), "missing builtin: {name}");
    }
}

#[test]
fn theme_list_marks_default_as_current() {
    let dir = TempDir::new().unwrap();
    let (db, cfg) = env_pair(&dir);

    tth_with(&db, &cfg)
        .args(["theme", "list"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("(current)"));
}

#[test]
fn theme_set_mocha_writes_config_and_prints_confirmation() {
    let dir = TempDir::new().unwrap();
    let (db, cfg) = env_pair(&dir);

    tth_with(&db, &cfg)
        .args(["theme", "mocha"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("mocha"));

    let content = std::fs::read_to_string(&cfg).unwrap();
    assert!(
        content.contains("name = \"mocha\""),
        "config should contain mocha, got: {content}"
    );
}

#[test]
fn theme_bare_after_set_mocha_prints_mocha() {
    let dir = TempDir::new().unwrap();
    let (db, cfg) = env_pair(&dir);

    tth_with(&db, &cfg)
        .args(["theme", "mocha"])
        .assert()
        .code(0);

    tth_with(&db, &cfg)
        .arg("theme")
        .assert()
        .code(0)
        .stdout(predicate::str::contains("mocha"));
}

#[test]
fn theme_invalid_name_exits_nonzero_and_does_not_write_config() {
    let dir = TempDir::new().unwrap();
    let (db, cfg) = env_pair(&dir);

    tth_with(&db, &cfg)
        .args(["theme", "nonsense"])
        .assert()
        .failure();

    assert!(
        !cfg.exists(),
        "config should not be created for invalid theme"
    );
}

#[test]
fn theme_list_after_set_mocha_marks_mocha_current() {
    let dir = TempDir::new().unwrap();
    let (db, cfg) = env_pair(&dir);

    tth_with(&db, &cfg)
        .args(["theme", "mocha"])
        .assert()
        .code(0);

    let assert = tth_with(&db, &cfg).args(["theme", "list"]).assert().code(0);

    let stdout = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        stdout.contains("mocha") && stdout.contains("(current)"),
        "expected mocha to be marked current, got: {stdout}"
    );
}

#[test]
fn theme_user_file_is_valid_and_appears_in_list() {
    let dir = TempDir::new().unwrap();
    let db = dir.path().join("history.db");
    let cfg = dir.path().join("config.toml");

    let themes_dir = dir.path().join("themes");
    std::fs::create_dir_all(&themes_dir).unwrap();
    std::fs::write(themes_dir.join("mine.toml"), "").unwrap();

    tth_with(&db, &cfg)
        .args(["theme", "mine"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("mine"));

    tth_with(&db, &cfg)
        .args(["theme", "list"])
        .assert()
        .code(0)
        .stdout(predicate::str::contains("mine"));
}
