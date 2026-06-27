use crate::error::ThothError;
use std::path::{Path, PathBuf};

const SENTINEL_BEGIN: &str = "# === THOTH HOOKS BEGIN ===";
const SENTINEL_END: &str = "# === THOTH HOOKS END ===";

const ZSH_HOOK: &str = include_str!("../shells/thoth.zsh");
const BASH_HOOK: &str = include_str!("../shells/thoth.bash");

#[derive(Debug, Clone, PartialEq)]
pub enum Shell {
    Bash,
    Zsh,
}

pub fn detect_shell(
    shell_flag: Option<&str>,
    shell_env: Option<&str>,
) -> Result<Shell, ThothError> {
    if let Some(flag) = shell_flag {
        return match flag {
            "bash" => Ok(Shell::Bash),
            "zsh" => Ok(Shell::Zsh),
            other => Err(ThothError::Hook(format!(
                "unknown shell '{other}'; use --shell bash or --shell zsh"
            ))),
        };
    }
    let env = shell_env.unwrap_or("");
    let basename = Path::new(env)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    match basename {
        "bash" => Ok(Shell::Bash),
        "zsh" => Ok(Shell::Zsh),
        other => Err(ThothError::Hook(format!(
            "could not detect shell from '{other}'; pass --shell bash or --shell zsh"
        ))),
    }
}

pub fn default_rc_path(shell: &Shell, home: &Path) -> PathBuf {
    match shell {
        Shell::Bash => home.join(".bashrc"),
        Shell::Zsh => home.join(".zshrc"),
    }
}

fn hook_body(shell: &Shell) -> &'static str {
    match shell {
        Shell::Bash => BASH_HOOK,
        Shell::Zsh => ZSH_HOOK,
    }
}

pub fn render_init(shell: &Shell) -> &'static str {
    hook_body(shell)
}

fn eval_line(shell: &Shell) -> String {
    let name = match shell {
        Shell::Bash => "bash",
        Shell::Zsh => "zsh",
    };
    format!("eval \"$(tth init {name})\"")
}

pub fn has_block(content: &str) -> bool {
    content.contains(SENTINEL_BEGIN)
}

pub fn insert_or_replace_block(rc_contents: &str, hook_body_str: &str) -> String {
    let block = format!("{}\n{}\n{}", SENTINEL_BEGIN, hook_body_str, SENTINEL_END);
    if has_block(rc_contents) {
        let stripped = remove_block(rc_contents);
        let prefix = stripped.trim_end_matches('\n');
        if prefix.is_empty() {
            block
        } else {
            format!("{}\n{}", prefix, block)
        }
    } else if rc_contents.is_empty() {
        block
    } else {
        format!("{}\n{}", rc_contents.trim_end_matches('\n'), block)
            .trim_start_matches('\n')
            .to_string()
    }
}

pub fn remove_block(rc_contents: &str) -> String {
    if !has_block(rc_contents) {
        return rc_contents.to_string();
    }
    let mut result = String::new();
    let mut in_block = false;
    for line in rc_contents.lines() {
        if line == SENTINEL_BEGIN {
            in_block = true;
            continue;
        }
        if line == SENTINEL_END {
            in_block = false;
            continue;
        }
        if !in_block {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}

pub struct InstallReport {
    pub rc_path: PathBuf,
    pub shell: Shell,
    pub already_present: bool,
    pub reload_cmd: String,
}

pub struct UninstallReport {
    pub rc_path: PathBuf,
    pub block_was_present: bool,
}

pub struct StatusReport {
    pub hooks_installed: bool,
    pub schema_version: i64,
    pub total_commands: i64,
    pub last_timestamp: Option<i64>,
    pub session_id_set: bool,
    pub tth_on_path: bool,
}

pub fn install(shell: &Shell, rc_path: &Path) -> Result<InstallReport, ThothError> {
    let existing = if rc_path.exists() {
        std::fs::read_to_string(rc_path)?
    } else {
        String::new()
    };
    let already_present = has_block(&existing);
    let line = eval_line(shell);
    let new_content = insert_or_replace_block(&existing, &line);
    std::fs::write(rc_path, &new_content)?;
    let reload_cmd = match shell {
        Shell::Bash => "source ~/.bashrc".to_string(),
        Shell::Zsh => "exec zsh".to_string(),
    };
    Ok(InstallReport {
        rc_path: rc_path.to_path_buf(),
        shell: shell.clone(),
        already_present,
        reload_cmd,
    })
}

pub fn uninstall(rc_path: &Path) -> Result<UninstallReport, ThothError> {
    let existing = if rc_path.exists() {
        std::fs::read_to_string(rc_path)?
    } else {
        String::new()
    };
    let block_was_present = has_block(&existing);
    if block_was_present {
        let new_content = remove_block(&existing);
        std::fs::write(rc_path, &new_content)?;
    }
    Ok(UninstallReport {
        rc_path: rc_path.to_path_buf(),
        block_was_present,
    })
}

pub fn status(
    conn: &rusqlite::Connection,
    rc_path: &Path,
    session_id_set: bool,
    tth_on_path: bool,
) -> StatusReport {
    let hooks_installed = if rc_path.exists() {
        std::fs::read_to_string(rc_path)
            .map(|s| has_block(&s))
            .unwrap_or(false)
    } else {
        false
    };
    let schema_version = crate::database::current_version(conn);
    let total_commands: i64 = conn
        .query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))
        .unwrap_or(0);
    let last_timestamp: Option<i64> = conn
        .query_row("SELECT MAX(timestamp) FROM commands", [], |r| r.get(0))
        .unwrap_or(None);
    StatusReport {
        hooks_installed,
        schema_version,
        total_commands,
        last_timestamp,
        session_id_set,
        tth_on_path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tmp_rc(dir: &TempDir, name: &str) -> PathBuf {
        dir.path().join(name)
    }

    fn mem_conn() -> rusqlite::Connection {
        let mut c = crate::database::connect_memory().unwrap();
        crate::database::apply_migrations(&mut c).unwrap();
        c
    }

    #[test]
    fn detect_shell_bash_flag() {
        assert_eq!(detect_shell(Some("bash"), None).unwrap(), Shell::Bash);
    }

    #[test]
    fn detect_shell_zsh_flag() {
        assert_eq!(detect_shell(Some("zsh"), None).unwrap(), Shell::Zsh);
    }

    #[test]
    fn detect_shell_from_env_bash() {
        assert_eq!(detect_shell(None, Some("/bin/bash")).unwrap(), Shell::Bash);
    }

    #[test]
    fn detect_shell_from_env_zsh() {
        assert_eq!(
            detect_shell(None, Some("/usr/bin/zsh")).unwrap(),
            Shell::Zsh
        );
    }

    #[test]
    fn detect_shell_unknown_flag_errors() {
        let err = detect_shell(Some("fish"), None).unwrap_err();
        assert!(err.to_string().contains("unknown shell"));
    }

    #[test]
    fn detect_shell_unknown_env_errors() {
        let err = detect_shell(None, Some("/bin/fish")).unwrap_err();
        assert!(err.to_string().contains("could not detect shell"));
    }

    #[test]
    fn detect_shell_flag_overrides_env() {
        assert_eq!(
            detect_shell(Some("zsh"), Some("/bin/bash")).unwrap(),
            Shell::Zsh
        );
    }

    #[test]
    fn default_rc_path_bash() {
        let home = Path::new("/home/testuser");
        assert_eq!(
            default_rc_path(&Shell::Bash, home),
            PathBuf::from("/home/testuser/.bashrc")
        );
    }

    #[test]
    fn default_rc_path_zsh() {
        let home = Path::new("/home/testuser");
        assert_eq!(
            default_rc_path(&Shell::Zsh, home),
            PathBuf::from("/home/testuser/.zshrc")
        );
    }

    #[test]
    fn has_block_detects_sentinel() {
        let content = format!("{}\nsome hook\n{}", SENTINEL_BEGIN, SENTINEL_END);
        assert!(has_block(&content));
    }

    #[test]
    fn has_block_false_when_absent() {
        assert!(!has_block("just some rc content"));
    }

    #[test]
    fn insert_or_replace_appends_to_empty() {
        let result = insert_or_replace_block("", "hook body");
        assert!(result.contains(SENTINEL_BEGIN));
        assert!(result.contains("hook body"));
        assert!(result.contains(SENTINEL_END));
    }

    #[test]
    fn insert_or_replace_appends_to_existing() {
        let existing = "export PATH=$PATH:/usr/local/bin\n";
        let result = insert_or_replace_block(existing, "hook body");
        assert!(result.contains("export PATH"));
        assert!(result.contains(SENTINEL_BEGIN));
    }

    #[test]
    fn insert_or_replace_is_idempotent_no_duplicate() {
        let first = insert_or_replace_block("", "hook body");
        let second = insert_or_replace_block(&first, "hook body");
        let count = second.matches(SENTINEL_BEGIN).count();
        assert_eq!(count, 1, "duplicate sentinel blocks found");
    }

    #[test]
    fn insert_or_replace_replaces_existing_body() {
        let first = insert_or_replace_block("", "old hook body");
        let second = insert_or_replace_block(&first, "new hook body");
        assert!(!second.contains("old hook body"), "old body still present");
        assert!(second.contains("new hook body"));
    }

    #[test]
    fn insert_or_replace_no_double_blank_line_on_replace() {
        let first = insert_or_replace_block("", "hook body");
        let second = insert_or_replace_block(&first, "hook body v2");
        assert!(
            !second.contains("\n\n\n"),
            "double blank line found after replace: {:?}",
            second
        );
        assert!(
            !second.starts_with('\n'),
            "result starts with blank line after replace"
        );
    }

    #[test]
    fn remove_block_strips_sentinel_and_body() {
        let original = "before\n";
        let with_block = insert_or_replace_block(original, "hook body");
        let removed = remove_block(&with_block);
        assert!(!removed.contains(SENTINEL_BEGIN));
        assert!(!removed.contains("hook body"));
        assert!(removed.contains("before"));
    }

    #[test]
    fn remove_block_noop_when_absent() {
        let content = "no hook here\n";
        assert_eq!(remove_block(content), content);
    }

    #[test]
    fn install_writes_block_to_rc() {
        let dir = TempDir::new().unwrap();
        let rc = tmp_rc(&dir, ".zshrc");
        let report = install(&Shell::Zsh, &rc).unwrap();
        assert!(!report.already_present);
        let content = std::fs::read_to_string(&rc).unwrap();
        assert!(content.contains(SENTINEL_BEGIN));
    }

    #[test]
    fn install_on_existing_rc_does_not_duplicate() {
        let dir = TempDir::new().unwrap();
        let rc = tmp_rc(&dir, ".bashrc");
        std::fs::write(&rc, "export EDITOR=vim\n").unwrap();
        install(&Shell::Bash, &rc).unwrap();
        install(&Shell::Bash, &rc).unwrap();
        let content = std::fs::read_to_string(&rc).unwrap();
        let count = content.matches(SENTINEL_BEGIN).count();
        assert_eq!(count, 1);
        assert!(content.contains("export EDITOR=vim"));
    }

    #[test]
    fn install_already_present_flag() {
        let dir = TempDir::new().unwrap();
        let rc = tmp_rc(&dir, ".zshrc");
        let r1 = install(&Shell::Zsh, &rc).unwrap();
        assert!(!r1.already_present);
        let r2 = install(&Shell::Zsh, &rc).unwrap();
        assert!(r2.already_present);
    }

    #[test]
    fn uninstall_removes_block() {
        let dir = TempDir::new().unwrap();
        let rc = tmp_rc(&dir, ".zshrc");
        install(&Shell::Zsh, &rc).unwrap();
        let report = uninstall(&rc).unwrap();
        assert!(report.block_was_present);
        let content = std::fs::read_to_string(&rc).unwrap();
        assert!(!content.contains(SENTINEL_BEGIN));
    }

    #[test]
    fn uninstall_preserves_surrounding_lines() {
        let dir = TempDir::new().unwrap();
        let rc = tmp_rc(&dir, ".bashrc");
        std::fs::write(&rc, "# my custom config\nexport X=1\n").unwrap();
        install(&Shell::Bash, &rc).unwrap();
        uninstall(&rc).unwrap();
        let content = std::fs::read_to_string(&rc).unwrap();
        assert!(content.contains("export X=1"));
        assert!(!content.contains(SENTINEL_BEGIN));
    }

    #[test]
    fn uninstall_noop_when_block_absent() {
        let dir = TempDir::new().unwrap();
        let rc = tmp_rc(&dir, ".bashrc");
        std::fs::write(&rc, "no block\n").unwrap();
        let report = uninstall(&rc).unwrap();
        assert!(!report.block_was_present);
        assert_eq!(std::fs::read_to_string(&rc).unwrap(), "no block\n");
    }

    #[test]
    fn status_hooks_installed_true_after_install() {
        let dir = TempDir::new().unwrap();
        let rc = tmp_rc(&dir, ".zshrc");
        install(&Shell::Zsh, &rc).unwrap();
        let conn = mem_conn();
        let s = status(&conn, &rc, false, false);
        assert!(s.hooks_installed);
        assert_eq!(s.schema_version, 4);
        assert_eq!(s.total_commands, 0);
    }

    #[test]
    fn status_hooks_not_installed_before_install() {
        let dir = TempDir::new().unwrap();
        let rc = tmp_rc(&dir, ".zshrc");
        let conn = mem_conn();
        let s = status(&conn, &rc, true, true);
        assert!(!s.hooks_installed);
        assert!(s.session_id_set);
        assert!(s.tth_on_path);
    }

    #[test]
    fn zsh_hook_is_non_empty() {
        assert!(!ZSH_HOOK.is_empty());
    }

    #[test]
    fn bash_hook_is_non_empty() {
        assert!(!BASH_HOOK.is_empty());
    }

    #[test]
    fn render_init_zsh_returns_zsh_hook() {
        let script = render_init(&Shell::Zsh);
        assert!(script.contains("_tth_preexec"));
        assert!(script.contains("bindkey '^R'"));
        assert!(!script.contains(SENTINEL_BEGIN));
    }

    #[test]
    fn render_init_bash_returns_bash_hook() {
        let script = render_init(&Shell::Bash);
        assert!(script.contains("_thoth_preexec"));
        assert!(script.contains("bind -x"));
        assert!(!script.contains(SENTINEL_BEGIN));
    }

    #[test]
    fn eval_line_zsh_format() {
        let line = eval_line(&Shell::Zsh);
        assert_eq!(line, "eval \"$(tth init zsh)\"");
    }

    #[test]
    fn eval_line_bash_format() {
        let line = eval_line(&Shell::Bash);
        assert_eq!(line, "eval \"$(tth init bash)\"");
    }

    #[test]
    fn install_writes_eval_line_to_rc() {
        let dir = TempDir::new().unwrap();
        let rc = tmp_rc(&dir, ".zshrc");
        install(&Shell::Zsh, &rc).unwrap();
        let content = std::fs::read_to_string(&rc).unwrap();
        assert!(
            content.contains("eval \"$(tth init zsh)\""),
            "eval line not found"
        );
        assert!(
            !content.contains("_tth_preexec"),
            "full body must not appear in rc"
        );
    }

    #[test]
    fn install_migration_replaces_old_body_with_eval_line() {
        let dir = TempDir::new().unwrap();
        let rc = tmp_rc(&dir, ".zshrc");
        let old_block = format!(
            "{}\n_tth_preexec() {{ : ; }}\n{}",
            SENTINEL_BEGIN, SENTINEL_END
        );
        std::fs::write(&rc, &old_block).unwrap();
        install(&Shell::Zsh, &rc).unwrap();
        let content = std::fs::read_to_string(&rc).unwrap();
        assert_eq!(content.matches(SENTINEL_BEGIN).count(), 1);
        assert!(content.contains("eval \"$(tth init zsh)\""));
        assert!(!content.contains("_tth_preexec() { : ; }"));
    }
}
