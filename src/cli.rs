use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Parser)]
#[command(
    name = "tth",
    arg_required_else_help = false,
    about = "Thoth: an intelligent shell history that captures commands with context",
    long_about = "Thoth records every command you run along with its working directory, \
project, exit code, duration, and any active tags. Run bare `tth` or press Ctrl-R \
(after `tth install`) to open an interactive fuzzy search. Use `tth search` for \
filtered, scriptable output. Manage your history with `tth sessions`, `tth forget`, \
`tth export`, and `tth tags`."
)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Option<Cmd>,
}

#[derive(Subcommand)]
pub enum Cmd {
    #[command(about = "Record a command into history (invoked by the shell hooks, not by hand)")]
    Record(RecordArgs),
    #[command(about = "Install the shell integration into your rc file")]
    Install(InstallArgs),
    #[command(about = "Remove the shell integration from your rc file")]
    Uninstall(UninstallArgs),
    #[command(about = "Show a quick status summary")]
    Status,
    #[command(about = "Search history with filters and full-text")]
    Search(SearchArgs),
    #[command(about = "List work sessions")]
    Sessions(SessionsArgs),
    #[command(about = "Delete recent commands from history")]
    Forget(ForgetArgs),
    #[command(about = "Export matching commands as a runnable bash script")]
    Export(ExportArgs),
    #[command(about = "Print the shell integration script for eval (eval \"$(tth init zsh)\")")]
    Init(InitArgs),
    #[command(about = "Print the export to activate a tag (used by the tth-tag shell function)")]
    Tag(TagArgs),
    #[command(
        about = "Print the export to deactivate a tag (used by the tth-untag shell function)"
    )]
    Untag(UntagArgs),
    #[command(about = "Show active tags, or all recorded tags with --list")]
    Tags(TagsArgs),
    #[command(about = "Print prompt-integration instructions for your prompt framework")]
    Prompt(PromptArgs),
    #[command(about = "Show history insights and statistics")]
    Stats(StatsArgs),
    #[command(about = "Run diagnostics and suggest fixes")]
    Doctor,
    #[command(
        hide = true,
        about = "Generate a new session ID (invoked by the shell hooks, not by hand)"
    )]
    NewSessionId,
    #[command(about = "Show and manage the effective configuration")]
    Config(ConfigArgs),
    #[command(about = "Start or end a workspace session (use tth-sw / tth-ew shell functions)")]
    Workspace(WorkspaceArgs),
    #[command(about = "List recorded workspaces with command counts")]
    Workspaces,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: Option<ConfigAction>,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum ConfigAction {
    #[command(
        about = "Print the canonical TOML of the loaded config (machine-readable, no color)"
    )]
    Print,
    #[command(about = "Print the value for a single dotted key (e.g. tui.orientation)")]
    Get(ConfigGetArgs),
    #[command(about = "Set a config key to a value, preserving comments in the file")]
    Set(ConfigSetArgs),
}

#[derive(clap::Args, Debug, Clone)]
pub struct ConfigGetArgs {
    #[arg(help = "Dotted key to read (e.g. session.gap_minutes)")]
    pub key: String,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ConfigSetArgs {
    #[arg(help = "Dotted key to set (e.g. tui.orientation)")]
    pub key: String,
    #[arg(help = "New value")]
    pub value: String,
}

#[derive(clap::Args, Debug, Clone)]
pub struct WorkspaceArgs {
    #[command(subcommand)]
    pub action: WorkspaceAction,
}

#[derive(clap::Subcommand, Debug, Clone)]
pub enum WorkspaceAction {
    #[command(about = "Activate a workspace by name (eval the output)")]
    Start(WorkspaceStartArgs),
    #[command(about = "Deactivate the current workspace (eval the output)")]
    End,
}

#[derive(clap::Args, Debug, Clone)]
pub struct WorkspaceStartArgs {
    #[arg(help = "Workspace name to activate")]
    pub name: String,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ForgetArgs {
    #[arg(
        long,
        default_value_t = 1,
        help = "Number of recent commands to forget"
    )]
    pub last: usize,
    #[arg(
        long,
        help = "Preview which commands would be deleted without deleting them"
    )]
    pub dry_run: bool,
    #[arg(
        long = "terminal-id",
        help = "Restrict to commands from this terminal session"
    )]
    pub terminal_id: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct RecordArgs {
    #[arg(long, help = "The command string to record")]
    pub cmd: String,
    #[arg(long, help = "Working directory where the command ran")]
    pub dir: Option<String>,
    #[arg(long = "exit", default_value_t = 0, help = "Exit code of the command")]
    pub exit_code: i64,
    #[arg(long, default_value_t = 0, help = "Command duration in milliseconds")]
    pub duration: i64,
    #[arg(long, help = "Unix timestamp when the command ran")]
    pub timestamp: Option<i64>,
    #[arg(long, default_value = "[]", help = "JSON array of active tag names")]
    pub tags: String,
    #[arg(long = "terminal-id", help = "Identifier for the terminal session")]
    pub terminal_id: Option<String>,
    #[arg(long, help = "Active workspace name (set by the shell hook)")]
    pub workspace: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct InstallArgs {
    #[arg(
        long,
        help = "Shell to install for (bash or zsh; auto-detected if omitted)"
    )]
    pub shell: Option<String>,
    #[arg(
        long = "rc-file",
        help = "Path to the rc file to modify (default: ~/.bashrc or ~/.zshrc)"
    )]
    pub rc_file: Option<PathBuf>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct SearchArgs {
    #[arg(help = "Optional full-text search query")]
    pub query: Option<String>,
    #[arg(short = 'p', long, help = "Filter by project (directory basename)")]
    pub project: Option<String>,
    #[arg(short = 't', long, action = clap::ArgAction::Append, help = "Filter by tag (repeatable)")]
    pub tag: Vec<String>,
    #[arg(long, help = "Filter by exit status: ok, fail, or any")]
    pub exit: Option<crate::search::ExitFilter>,
    #[arg(
        long,
        help = "Filter by duration in seconds, e.g. >30 (over 30s) or <5 (under 5s)"
    )]
    pub duration: Option<String>,
    #[arg(
        long,
        help = "Show commands after this time, e.g. '2h ago' or '2024-01-01'"
    )]
    pub since: Option<String>,
    #[arg(long, help = "Show commands before this time")]
    pub until: Option<String>,
    #[arg(long, help = "Filter by session ID")]
    pub session: Option<String>,
    #[arg(long, help = "Maximum number of results")]
    pub limit: Option<usize>,
    #[arg(
        long = "show-session",
        default_value_t = false,
        help = "Include session ID column in output"
    )]
    pub show_session: bool,
}

#[derive(clap::Args, Debug, Clone)]
pub struct SessionsArgs {
    #[arg(long, help = "Filter sessions by project name")]
    pub project: Option<String>,
    #[arg(long, help = "Show sessions after this time")]
    pub since: Option<String>,
    #[arg(long, help = "Show sessions before this time")]
    pub until: Option<String>,
    #[arg(
        long,
        default_value_t = 20,
        help = "Maximum number of sessions to show"
    )]
    pub limit: usize,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ExportArgs {
    #[arg(long, help = "Export commands from this session ID")]
    pub session: Option<String>,
    #[arg(short = 't', long, action = clap::ArgAction::Append, help = "Filter by tag (repeatable)")]
    pub tag: Vec<String>,
    #[arg(short = 'p', long, help = "Filter by project name")]
    pub project: Option<String>,
    #[arg(long, help = "Export commands after this time")]
    pub since: Option<String>,
    #[arg(long, help = "Filter by exit status: ok, fail, or any")]
    pub exit: Option<crate::search::ExitFilter>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct InitArgs {
    #[arg(help = "Shell to generate integration for (bash or zsh; auto-detected if omitted)")]
    pub shell: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct TagArgs {
    #[arg(help = "Tag name to activate")]
    pub name: String,
}

#[derive(clap::Args, Debug, Clone)]
pub struct UntagArgs {
    #[arg(help = "Tag name to deactivate")]
    pub name: Option<String>,
    #[arg(long, help = "Deactivate all tags at once")]
    pub all: bool,
}

#[derive(clap::Args, Debug, Clone)]
pub struct TagsArgs {
    #[arg(
        long,
        help = "Show all tags recorded in the database with command counts"
    )]
    pub list: bool,
}

#[derive(clap::Args, Debug, Clone)]
pub struct PromptArgs {
    #[arg(
        long,
        help = "Prompt framework to generate for: starship, p10k, oh-my-posh, or generic"
    )]
    pub framework: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct StatsArgs {
    #[arg(short = 'p', long, help = "Restrict stats to this project")]
    pub project: Option<String>,
    #[arg(long, help = "Restrict stats to commands after this time")]
    pub since: Option<String>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct UninstallArgs {
    #[arg(
        long = "keep-data",
        help = "Suppress the data-retention reminder message"
    )]
    pub keep_data: bool,
    #[arg(
        long = "rc-file",
        help = "Path to the rc file to modify (default: ~/.bashrc or ~/.zshrc)"
    )]
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
            let cfg = crate::config::load();
            let conn = crate::database::get_connection(None)?;
            let columns = crate::tui::render::resolve_tui_columns(&cfg.tui.columns);
            crate::tui::run(&conn, now, cfg.tui.orientation.is_bottom(), columns)?;
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
            let cfg = crate::config::load();
            match crate::database::get_connection(None) {
                Ok(mut conn) => crate::recorder::record(
                    &args,
                    cfg.session.gap_minutes,
                    &cfg.history.filter,
                    &mut conn,
                ),
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
            let framework = crate::prompt::detect_framework(&crate::prompt::probe_inputs());
            let snippet = crate::prompt::prompt_snippet(&framework);
            println!("\nTo show active tags in your prompt:\n{snippet}");
            let config_path = crate::config::resolve_config_path();
            if crate::config::ensure_default_config(&config_path)? {
                println!("Wrote default config to {}", config_path.display());
            }
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
        Some(Cmd::Search(mut args)) => {
            let cfg = crate::config::load();
            let resolved_limit = args.limit.unwrap_or(cfg.search.default_limit);
            let columns = crate::search::resolve_columns(&cfg.search.columns)?;
            let conn = crate::database::get_connection(None)?;

            let rows = if cfg.search.filter.is_empty() {
                args.limit = Some(resolved_limit);
                crate::search::execute(&args, &conn, now)?
            } else {
                let (regexes, invalid) = crate::search::compile_filters(&cfg.search.filter);
                for pat in &invalid {
                    eprintln!("thoth: invalid search filter pattern (skipped): {}", pat);
                }
                args.limit = None;
                let candidates = crate::search::execute(&args, &conn, now)?;
                candidates
                    .into_iter()
                    .filter(|r| !crate::search::is_filtered(&r.command, &regexes))
                    .take(resolved_limit)
                    .collect()
            };
            print!(
                "{}",
                crate::search::render(&rows, &columns, args.show_session)
            );
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
                workspace: None,
            };
            let rows = crate::export::collect(&conn, &export_args, now)?;
            let meta = crate::export::ExportMeta {
                project: args.project.as_deref(),
                tags: &args.tag,
            };
            print!("{}", crate::export::render_script(&rows, &meta, now));
        }
        Some(Cmd::Init(args)) => {
            let shell_env = std::env::var("SHELL").ok();
            let shell = crate::hooks::detect_shell(args.shell.as_deref(), shell_env.as_deref())?;
            print!("{}", crate::hooks::render_init(&shell));
        }
        Some(Cmd::Tag(args)) => {
            if args.name.is_empty() {
                return Err(crate::error::ThothError::Tag(
                    "tag name cannot be empty".into(),
                ));
            }
            let current = std::env::var("TTH_ACTIVE_TAGS").unwrap_or_else(|_| "[]".into());
            let new_json = crate::tags::add_tag(&current, &args.name);
            print!("{}", crate::tags::export_line(&new_json));
            let segment = crate::tags::format_prompt_segment(&new_json);
            let display = if segment.is_empty() {
                "(none)".to_string()
            } else {
                segment
            };
            eprintln!("Active tags: {display}");
        }
        Some(Cmd::Untag(args)) => {
            if args.name.is_none() && !args.all {
                return Err(crate::error::ThothError::Tag(
                    "specify a tag name or --all".into(),
                ));
            }
            let current = std::env::var("TTH_ACTIVE_TAGS").unwrap_or_else(|_| "[]".into());
            let new_json = if args.all {
                crate::tags::clear_tags()
            } else {
                crate::tags::remove_tag(&current, args.name.as_deref().unwrap_or(""))
            };
            print!("{}", crate::tags::export_line(&new_json));
            let segment = crate::tags::format_prompt_segment(&new_json);
            let display = if segment.is_empty() {
                "(none)".to_string()
            } else {
                segment
            };
            eprintln!("Active tags: {display}");
        }
        Some(Cmd::Tags(args)) => {
            if args.list {
                let conn = crate::database::get_connection(None)?;
                let tags = crate::tags::list_db_tags(&conn)?;
                if tags.is_empty() {
                    println!("(no tags recorded)");
                } else {
                    for (tag, count) in tags {
                        let noun = if count == 1 { "command" } else { "commands" };
                        println!("{tag} ({count} {noun})");
                    }
                }
            } else {
                let current = std::env::var("TTH_ACTIVE_TAGS").unwrap_or_else(|_| "[]".into());
                let tags = crate::tags::parse_active(&current);
                if tags.is_empty() {
                    println!("(none)");
                } else {
                    for tag in tags {
                        println!("{tag}");
                    }
                }
            }
        }
        Some(Cmd::Prompt(args)) => {
            let framework = if let Some(ref fw) = args.framework {
                crate::prompt::parse_framework(fw)?
            } else {
                crate::prompt::detect_framework(&crate::prompt::probe_inputs())
            };
            print!("{}", crate::prompt::prompt_snippet(&framework));
        }
        Some(Cmd::Stats(args)) => {
            let conn = crate::database::get_connection(None)?;
            let stats_args = crate::stats::StatsArgs {
                project: args.project,
                since: args.since,
            };
            let stats = crate::stats::compute(&conn, &stats_args, now)?;
            print!("{}", crate::stats::render(&stats));
        }
        Some(Cmd::Doctor) => {
            let shell_env = std::env::var("SHELL").ok();
            let shell = crate::hooks::detect_shell(None, shell_env.as_deref())
                .unwrap_or(crate::hooks::Shell::Bash);
            let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
            let rc_path = crate::hooks::default_rc_path(&shell, std::path::Path::new(&home));
            let hooks_installed = if rc_path.exists() {
                std::fs::read_to_string(&rc_path)
                    .map(|s| crate::hooks::has_block(&s))
                    .unwrap_or(false)
            } else {
                false
            };
            let db_result = crate::database::get_connection(None)
                .map(|conn| {
                    let schema_version = crate::database::current_version(&conn);
                    let total_commands: i64 = conn
                        .query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))
                        .unwrap_or(0);
                    let last_timestamp: Option<i64> = conn
                        .query_row("SELECT MAX(timestamp) FROM commands", [], |r| r.get(0))
                        .unwrap_or(None);
                    crate::doctor::DbInfo {
                        schema_version,
                        total_commands,
                        last_timestamp,
                    }
                })
                .map_err(|e| e.to_string());
            let tth_on_path = which_tth();
            let framework = crate::prompt::detect_framework(&crate::prompt::probe_inputs());
            let framework_config_text = read_framework_config(&framework, &home);
            let config_path = crate::config::resolve_config_path();
            let config_present = config_path.exists();
            let inputs = crate::doctor::DoctorInputs {
                hooks_installed,
                db_result,
                tth_on_path,
                framework,
                framework_config_text,
                config_path,
                config_present,
            };
            let report = crate::doctor::run_doctor(&inputs);
            print!("{}", crate::doctor::render_report(&report));
        }
        Some(Cmd::NewSessionId) => {
            println!("{}", uuid::Uuid::new_v4());
        }
        Some(Cmd::Config(args)) => {
            let cfg = crate::config::load();
            let path = crate::config::resolve_config_path();
            match args.action {
                None => {
                    let exists = path.exists();
                    let color = crate::config::use_color();
                    print!(
                        "{}",
                        crate::config::render_config(&cfg, &path, exists, color)
                    );
                }
                Some(ConfigAction::Print) => {
                    let toml = crate::config::config_toml(&cfg)?;
                    print!("{}", toml);
                }
                Some(ConfigAction::Get(a)) => {
                    let val = crate::config::get_value(&cfg, &a.key)?;
                    println!("{}", val);
                }
                Some(ConfigAction::Set(a)) => {
                    crate::config::write_set(&a.key, &a.value)?;
                    println!("set {} = {}", a.key, a.value);
                }
            }
        }
        Some(Cmd::Workspace(args)) => match args.action {
            WorkspaceAction::Start(a) => {
                if a.name.is_empty() {
                    return Err(crate::error::ThothError::Hook(
                        "workspace name cannot be empty".into(),
                    ));
                }
                print!("{}", crate::workspaces::start_line(&a.name));
                eprintln!("Active workspace: {}", a.name);
            }
            WorkspaceAction::End => {
                print!("{}", crate::workspaces::end_line());
                eprintln!("Workspace deactivated.");
            }
        },
        Some(Cmd::Workspaces) => {
            let conn = crate::database::get_connection(None)?;
            let rows = crate::workspaces::list_workspaces(&conn)?;
            if rows.is_empty() {
                println!("(no workspaces recorded)");
            } else {
                use comfy_table::{presets::UTF8_BORDERS_ONLY, ContentArrangement, Table};
                let mut table = Table::new();
                table.load_preset(UTF8_BORDERS_ONLY);
                table.set_content_arrangement(ContentArrangement::Dynamic);
                table.set_header(vec!["workspace", "commands", "last used"]);
                for row in &rows {
                    let last = crate::search::fmt_timestamp_pub(row.last_ts);
                    table.add_row(vec![row.name.clone(), row.command_count.to_string(), last]);
                }
                println!("{table}");
            }
        }
    }
    Ok(())
}

fn which_tth() -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join("tth").exists()))
        .unwrap_or(false)
}

fn read_framework_config(framework: &crate::prompt::PromptFramework, home: &str) -> Option<String> {
    use crate::prompt::PromptFramework;
    let home_path = std::path::Path::new(home);
    let config_path = match framework {
        PromptFramework::Starship => home_path.join(".config/starship.toml"),
        PromptFramework::Powerlevel10k => home_path.join(".p10k.zsh"),
        PromptFramework::OhMyPosh => {
            let dir = home_path.join(".config/oh-my-posh");
            if dir.exists() {
                if let Ok(mut entries) = std::fs::read_dir(&dir) {
                    if let Some(Ok(entry)) = entries.next() {
                        return std::fs::read_to_string(entry.path()).ok();
                    }
                }
            }
            return None;
        }
        PromptFramework::Generic => return None,
    };
    std::fs::read_to_string(config_path).ok()
}
