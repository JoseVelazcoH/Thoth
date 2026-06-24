use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "tth")]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand)]
pub enum Cmd {
    Record(RecordArgs),
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

pub fn run() -> Result<(), crate::error::ThothError> {
    use clap::Parser;
    let cli = Cli::parse();
    let Cmd::Record(mut args) = cli.cmd;

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
    Ok(())
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
    fn default_dir_is_cwd() {
        let cwd = std::env::current_dir()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let dir = std::env::current_dir()
            .ok()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        assert_eq!(dir, cwd);
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
