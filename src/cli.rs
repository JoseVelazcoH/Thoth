use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Parser)]
#[command(name = "tth", arg_required_else_help = false)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Option<Cmd>,
}

#[derive(Subcommand)]
pub enum Cmd {
    Record(RecordArgs),
    Install(InstallArgs),
    Uninstall(UninstallArgs),
    Status,
    Search(SearchArgs),
    Sessions(SessionsArgs),
    Forget(ForgetArgs),
    Export(ExportArgs),
    #[command(hide = true)]
    NewSessionId,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ForgetArgs {
    #[arg(long, default_value_t = 1)]
    pub last: usize,
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long = "terminal-id")]
    pub terminal_id: Option<String>,
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
pub struct SearchArgs {
    pub query: Option<String>,
    #[arg(short = 'p', long)]
    pub project: Option<String>,
    #[arg(short = 't', long, action = clap::ArgAction::Append)]
    pub tag: Vec<String>,
    #[arg(long)]
    pub exit: Option<crate::search::ExitFilter>,
    #[arg(long)]
    pub duration: Option<String>,
    #[arg(long)]
    pub since: Option<String>,
    #[arg(long)]
    pub until: Option<String>,
    #[arg(long)]
    pub session: Option<String>,
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
    #[arg(long = "show-session", default_value_t = false)]
    pub show_session: bool,
}

#[derive(clap::Args, Debug, Clone)]
pub struct SessionsArgs {
    #[arg(long)]
    pub project: Option<String>,
    #[arg(long)]
    pub since: Option<String>,
    #[arg(long)]
    pub until: Option<String>,
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ExportArgs {
    #[arg(long)]
    pub session: Option<String>,
    #[arg(short = 't', long, action = clap::ArgAction::Append)]
    pub tag: Vec<String>,
    #[arg(short = 'p', long)]
    pub project: Option<String>,
    #[arg(long)]
    pub since: Option<String>,
    #[arg(long)]
    pub exit: Option<crate::search::ExitFilter>,
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

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    match cli.cmd {
        None => {
            let conn = crate::database::get_connection(None)?;
            crate::tui::run(&conn, now)?;
        }
        Some(Cmd::Record(mut args)) => {
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
        Some(Cmd::Install(args)) => {
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
        Some(Cmd::Uninstall(args)) => {
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
        Some(Cmd::Status) => {
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
        Some(Cmd::Search(args)) => {
            let conn = crate::database::get_connection(None)?;
            let rows = crate::search::execute(&args, &conn, now)?;
            print!("{}", crate::search::render(&rows, args.show_session));
        }
        Some(Cmd::Sessions(args)) => {
            let conn = crate::database::get_connection(None)?;
            let sargs = crate::sessions::SessionsArgs {
                project: args.project,
                since: args.since,
                until: args.until,
                limit: args.limit,
            };
            let rows = crate::sessions::list_sessions(&conn, &sargs, now)?;
            print!("{}", crate::sessions::render(&rows));
        }
        Some(Cmd::Forget(args)) => {
            if args.last == 0 {
                return Err(crate::error::ThothError::Forget(
                    "nothing to forget: --last must be >= 1".into(),
                ));
            }
            let conn = crate::database::get_connection(None)?;
            let scope = crate::forget::resolve_scope(
                args.terminal_id,
                std::env::var("TTH_SESSION_ID").ok(),
            );
            let rows = crate::forget::select_targets(&conn, &scope, args.last)?;
            if rows.is_empty() {
                println!("No commands to forget.");
                return Ok(());
            }
            let ids: Vec<i64> = rows.iter().map(|r| r.id).collect();
            if args.dry_run {
                print!("{}", crate::forget::render_preview(&rows));
                println!("Would forget {} command(s).", ids.len());
            } else {
                crate::forget::delete_targets(&conn, &ids)?;
                println!("Forgot {} command(s).", ids.len());
            }
        }
        Some(Cmd::Export(args)) => {
            let conn = crate::database::get_connection(None)?;
            let export_args = crate::export::ExportArgs {
                session: args.session,
                tag: args.tag.clone(),
                project: args.project.clone(),
                since: args.since,
                exit: args.exit,
            };
            let rows = crate::export::collect(&conn, &export_args, now)?;
            let meta = crate::export::ExportMeta {
                project: args.project.as_deref(),
                tags: &args.tag,
            };
            print!("{}", crate::export::render_script(&rows, &meta, now));
        }
        Some(Cmd::NewSessionId) => {
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
