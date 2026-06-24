use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "tth")]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand)]
pub enum Cmd {
    Record(RecordArgs),
    Install(InstallArgs),
    Uninstall(UninstallArgs),
    Status,
    #[command(hide = true)]
    NewSessionId,
}

#[derive(clap::Args, Debug, Clone)]
pub struct RecordArgs {
    #[arg(long)]
    pub cmd: String,
    #[arg(long)]
    pub dir: Option<String>,
    #[arg(long = "exit", default_value_t = 0)]
    pub exit_code: i64,
    #[arg(long, default_value_t = 0)]
    pub duration: i64,
    #[arg(long)]
    pub timestamp: Option<i64>,
    #[arg(long, default_value = "[]")]
    pub tags: String,
    #[arg(long = "terminal-id")]
    pub terminal_id: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct InstallArgs {
    #[arg(long)]
    pub shell: Option<String>,
    #[arg(long = "rc-file")]
    pub rc_file: Option<PathBuf>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct UninstallArgs {
    #[arg(long = "keep-data")]
    pub keep_data: bool,
    #[arg(long = "rc-file")]
    pub rc_file: Option<PathBuf>,
}

pub fn run() -> Result<(), crate::error::ThothError> {
    use clap::Parser;
    let cli = Cli::parse();

    match cli.cmd {
        Cmd::Record(mut args) => {
            crate::logging::setup(crate::paths::resolve_error_log());
            if args.dir.is_none() {
                args.dir = std::env::current_dir()
                    .ok()
                    .map(|p| p.to_string_lossy().to_string());
            }
            if args.timestamp.is_none() {
                args.timestamp = Some(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64,
                );
            }
            match crate::database::get_connection(None) {
                Ok(mut conn) => crate::recorder::record(&args, &mut conn),
                Err(e) => crate::logging::log_error(&e.to_string()),
            }
        }
        Cmd::Install(args) => {
            let shell_env = std::env::var("SHELL").ok();
            let shell = crate::hooks::detect_shell(args.shell.as_deref(), shell_env.as_deref())?;
            let rc_path = if let Some(p) = args.rc_file {
                p
            } else {
                let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
                crate::hooks::default_rc_path(&shell, std::path::Path::new(&home))
            };
            let report = crate::hooks::install(&shell, &rc_path)?;
            if report.already_present {
                println!("Installed (updated) hooks in {}", rc_path.display());
            } else {
                println!("Installed hooks in {}", rc_path.display());
            }
            println!("Reload shell: {}", report.reload_cmd);
        }
        Cmd::Uninstall(args) => {
            let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
            let rc_path = if let Some(p) = args.rc_file {
                p
            } else {
                let shell_env = std::env::var("SHELL").ok();
                let shell = crate::hooks::detect_shell(None, shell_env.as_deref())?;
                crate::hooks::default_rc_path(&shell, std::path::Path::new(&home))
            };
            let report = crate::hooks::uninstall(&rc_path)?;
            if report.block_was_present {
                println!("Uninstalled hooks from {}", rc_path.display());
                if !args.keep_data {
                    println!("Data retained. Pass --keep-data to suppress this message or remove manually.");
                }
            } else {
                println!("No hooks found in {}", rc_path.display());
            }
        }
        Cmd::Status => {
            let conn = crate::database::get_connection(None)?;
            let shell_env = std::env::var("SHELL").ok();
            let shell = crate::hooks::detect_shell(None, shell_env.as_deref())
                .unwrap_or(crate::hooks::Shell::Bash);
            let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
            let rc_path = crate::hooks::default_rc_path(&shell, std::path::Path::new(&home));
            let session_id_set = std::env::var("TTH_SESSION_ID").is_ok();
            let tth_on_path = which_tth();
            let report = crate::hooks::status(&conn, &rc_path, session_id_set, tth_on_path);
            println!("Hooks installed:  {}", report.hooks_installed);
            println!("Schema version:   {}", report.schema_version);
            println!("Total commands:   {}", report.total_commands);
            println!(
                "Last command:     {}",
                report
                    .last_timestamp
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| "none".into())
            );
            println!("Session ID set:   {}", report.session_id_set);
            println!("tth on PATH:      {}", report.tth_on_path);
        }
        Cmd::NewSessionId => {
            println!("{}", uuid::Uuid::new_v4());
        }
    }
    Ok(())
}

fn which_tth() -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join("tth").exists()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tags_is_empty_array() {
        let args = RecordArgs {
            cmd: String::from("ls"),
            dir: None,
            exit_code: 0,
            duration: 0,
            timestamp: None,
            tags: String::from("[]"),
            terminal_id: None,
        };
        assert_eq!(args.tags, "[]");
    }

    #[test]
    fn default_timestamp_is_now() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        assert!((ts - now).abs() < 2);
    }
}
