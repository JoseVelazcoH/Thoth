use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::ThothError;

const VALID_KEYS: &[(&str, &str)] = &[
    ("session.gap_minutes", "positive integer"),
    ("tui.orientation", r#""bottom" or "top""#),
    ("search.default_limit", "positive integer"),
];

pub const DEFAULT_CONFIG_TOML: &str = r#"# Thoth configuration. All settings are optional; values shown are the defaults.

[session]
# Minutes of inactivity that start a new work session.
gap_minutes = 30

[tui]
# Interactive search panel position: "bottom" or "top".
orientation = "bottom"
# Columns to show in the interactive panel, in order. Available: timestamp, project, tags, exit, duration, directory, command
# columns = ["timestamp", "duration", "exit", "project", "command"]

[search]
# Default maximum number of results for `tth search`.
default_limit = 50
# Columns to show, in order. Available: timestamp, project, tags, exit, duration, directory, command
# columns = ["timestamp", "project", "tags", "exit", "duration", "directory", "command"]
#
# Regex patterns; commands matching any pattern are hidden from search results.
# filter = ["^ls$", "^cd "]

[history]
# Regex patterns; matching commands are NEVER recorded (useful to avoid storing secrets).
# filter = ["--password", "export .*TOKEN", "AKIA[0-9A-Z]{16}"]
"#;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Orientation {
    #[default]
    Bottom,
    Top,
}

impl Orientation {
    pub fn is_bottom(&self) -> bool {
        matches!(self, Orientation::Bottom)
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct Session {
    pub gap_minutes: i64,
}

impl Default for Session {
    fn default() -> Self {
        Self { gap_minutes: 30 }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct Tui {
    pub orientation: Orientation,
    #[serde(default = "default_tui_columns")]
    pub columns: Vec<String>,
}

impl Default for Tui {
    fn default() -> Self {
        Self {
            orientation: Orientation::Bottom,
            columns: default_tui_columns(),
        }
    }
}

pub fn default_tui_columns() -> Vec<String> {
    vec![
        "timestamp".into(),
        "duration".into(),
        "exit".into(),
        "project".into(),
        "command".into(),
    ]
}

pub fn default_search_columns() -> Vec<String> {
    vec![
        "timestamp".into(),
        "project".into(),
        "tags".into(),
        "exit".into(),
        "duration".into(),
        "directory".into(),
        "command".into(),
    ]
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct Search {
    pub default_limit: usize,
    #[serde(default = "default_search_columns")]
    pub columns: Vec<String>,
    #[serde(default)]
    pub filter: Vec<String>,
}

impl Default for Search {
    fn default() -> Self {
        Self {
            default_limit: 50,
            columns: default_search_columns(),
            filter: vec![],
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct History {
    pub filter: Vec<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Config {
    pub session: Session,
    pub tui: Tui,
    pub search: Search,
    pub history: History,
}

fn config_path_from(thoth_config: Option<&str>, xdg_config: Option<&str>, home: &Path) -> PathBuf {
    if let Some(v) = thoth_config {
        return PathBuf::from(v);
    }
    if let Some(x) = xdg_config {
        return PathBuf::from(x).join("thoth").join("config.toml");
    }
    home.join(".config").join("thoth").join("config.toml")
}

pub fn resolve_config_path() -> PathBuf {
    let thoth_config = std::env::var("THOTH_CONFIG").ok();
    let xdg_config = std::env::var("XDG_CONFIG_HOME").ok();
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
    config_path_from(
        thoth_config.as_deref(),
        xdg_config.as_deref(),
        Path::new(&home),
    )
}

pub fn parse(text: &str) -> Result<Config, ThothError> {
    toml::from_str(text).map_err(|e| ThothError::Config(e.to_string()))
}

pub fn load() -> Config {
    let path = resolve_config_path();
    if !path.exists() {
        return Config::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(text) => match parse(&text) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!(
                    "thoth: ignoring invalid config at {}: {}; using defaults",
                    path.display(),
                    e
                );
                Config::default()
            }
        },
        Err(e) => {
            eprintln!(
                "thoth: ignoring invalid config at {}: {}; using defaults",
                path.display(),
                e
            );
            Config::default()
        }
    }
}

pub fn config_toml(cfg: &Config) -> Result<String, ThothError> {
    toml::to_string(cfg).map_err(|e| ThothError::Config(e.to_string()))
}

pub fn get_value(cfg: &Config, key: &str) -> Result<String, ThothError> {
    match key {
        "session.gap_minutes" => Ok(cfg.session.gap_minutes.to_string()),
        "tui.orientation" => {
            let v = if cfg.tui.orientation.is_bottom() {
                "bottom"
            } else {
                "top"
            };
            Ok(v.to_string())
        }
        "search.default_limit" => Ok(cfg.search.default_limit.to_string()),
        _ => Err(ThothError::Config(format!(
            "unknown key '{}'; valid keys: {}",
            key,
            VALID_KEYS
                .iter()
                .map(|(k, _)| *k)
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

pub fn apply_set(existing_toml: &str, key: &str, value: &str) -> Result<String, ThothError> {
    let parts: Vec<&str> = key.splitn(2, '.').collect();
    if parts.len() != 2 {
        return Err(ThothError::Config(format!(
            "unknown key '{}'; valid keys: {}",
            key,
            VALID_KEYS
                .iter()
                .map(|(k, _)| *k)
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    let section = parts[0];
    let field = parts[1];

    let typed_value: toml_edit::Value = match key {
        "session.gap_minutes" | "search.default_limit" => {
            let n: i64 = value
                .parse()
                .map_err(|_| ThothError::Config(format!("'{}' must be a positive integer", key)))?;
            if n <= 0 {
                return Err(ThothError::Config(format!(
                    "'{}' must be a positive integer (got {})",
                    key, n
                )));
            }
            toml_edit::value(n)
                .into_value()
                .map_err(|e| ThothError::Config(format!("toml_edit error: {}", e)))?
        }
        "tui.orientation" => {
            if value != "bottom" && value != "top" {
                return Err(ThothError::Config(format!(
                    "'tui.orientation' must be \"bottom\" or \"top\" (got '{}')",
                    value
                )));
            }
            toml_edit::value(value)
                .into_value()
                .map_err(|e| ThothError::Config(format!("toml_edit error: {}", e)))?
        }
        _ => {
            return Err(ThothError::Config(format!(
                "unknown key '{}'; valid keys: {}",
                key,
                VALID_KEYS
                    .iter()
                    .map(|(k, _)| *k)
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
    };

    let mut doc: toml_edit::DocumentMut = existing_toml
        .parse()
        .map_err(|e: toml_edit::TomlError| ThothError::Config(e.to_string()))?;

    if doc.get(section).is_none() {
        doc[section] = toml_edit::table();
    }
    doc[section][field] = toml_edit::Item::Value(typed_value);

    Ok(doc.to_string())
}

pub fn write_set(key: &str, value: &str) -> Result<(), ThothError> {
    let path = resolve_config_path();
    let existing = if path.exists() {
        std::fs::read_to_string(&path)?
    } else {
        DEFAULT_CONFIG_TOML.to_string()
    };
    let new_toml = apply_set(&existing, key, value)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, new_toml)?;
    Ok(())
}

pub fn ensure_default_config(path: &Path) -> Result<bool, ThothError> {
    if path.exists() {
        return Ok(false);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, DEFAULT_CONFIG_TOML)?;
    Ok(true)
}

pub fn use_color() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

pub fn render_config(cfg: &Config, path: &Path, exists: bool, color: bool) -> String {
    let orientation = if cfg.tui.orientation.is_bottom() {
        "bottom"
    } else {
        "top"
    };

    if color {
        use crossterm::style::Stylize;
        let exists_str = if exists {
            "true".green().to_string()
        } else {
            "false".red().to_string()
        };
        format!(
            "Config path: {}\nExists:      {}\n{} gap_minutes = {}\n{} orientation = {}\n{} default_limit = {}\n",
            path.display().to_string().yellow(),
            exists_str,
            "[session]".cyan(),
            cfg.session.gap_minutes.to_string().green(),
            "[tui]".cyan(),
            orientation.green(),
            "[search]".cyan(),
            cfg.search.default_limit.to_string().green(),
        )
    } else {
        let exists_str = if exists { "true" } else { "false" };
        format!(
            "Config path: {}\nExists:      {}\n[session] gap_minutes = {}\n[tui] orientation = {}\n[search] default_limit = {}\n",
            path.display(),
            exists_str,
            cfg.session.gap_minutes,
            orientation,
            cfg.search.default_limit,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn parse_empty_string_uses_defaults() {
        let cfg = parse("").unwrap();
        assert_eq!(cfg.session.gap_minutes, 30);
        assert_eq!(cfg.tui.orientation, Orientation::Bottom);
        assert_eq!(cfg.search.default_limit, 50);
    }

    #[test]
    fn parse_partial_session_gap() {
        let cfg = parse("[session]\ngap_minutes = 15").unwrap();
        assert_eq!(cfg.session.gap_minutes, 15);
        assert_eq!(cfg.tui.orientation, Orientation::Bottom);
        assert_eq!(cfg.search.default_limit, 50);
    }

    #[test]
    fn parse_full_config() {
        let toml = "[session]\ngap_minutes = 10\n[tui]\norientation = \"top\"\n[search]\ndefault_limit = 25";
        let cfg = parse(toml).unwrap();
        assert_eq!(cfg.session.gap_minutes, 10);
        assert_eq!(cfg.tui.orientation, Orientation::Top);
        assert_eq!(cfg.search.default_limit, 25);
    }

    #[test]
    fn parse_bad_toml_returns_err() {
        assert!(parse("not = {valid = toml").is_err());
    }

    #[test]
    fn parse_bad_orientation_returns_err() {
        assert!(parse("[tui]\norientation = \"sideways\"").is_err());
    }

    #[test]
    fn config_path_from_override_wins() {
        let result = config_path_from(
            Some("/custom/config.toml"),
            Some("/xdg"),
            Path::new("/home/user"),
        );
        assert_eq!(result, PathBuf::from("/custom/config.toml"));
    }

    #[test]
    fn config_path_from_xdg_fallback() {
        let result = config_path_from(None, Some("/xdg/config"), Path::new("/home/user"));
        assert_eq!(result, PathBuf::from("/xdg/config/thoth/config.toml"));
    }

    #[test]
    fn config_path_from_home_fallback() {
        let result = config_path_from(None, None, Path::new("/home/user"));
        assert_eq!(
            result,
            PathBuf::from("/home/user/.config/thoth/config.toml")
        );
    }

    #[test]
    fn render_config_defaults() {
        let cfg = Config::default();
        let path = PathBuf::from("/home/user/.config/thoth/config.toml");
        let out = render_config(&cfg, &path, false, false);
        assert!(out.contains("gap_minutes = 30"));
        assert!(out.contains("orientation = bottom"));
        assert!(out.contains("default_limit = 50"));
        assert!(out.contains("false"));
    }

    #[test]
    fn render_config_custom() {
        let cfg = parse("[session]\ngap_minutes = 10\n[tui]\norientation = \"top\"\n[search]\ndefault_limit = 3").unwrap();
        let path = PathBuf::from("/tmp/config.toml");
        let out = render_config(&cfg, &path, true, false);
        assert!(out.contains("gap_minutes = 10"));
        assert!(out.contains("orientation = top"));
        assert!(out.contains("default_limit = 3"));
        assert!(out.contains("true"));
    }

    #[test]
    fn orientation_is_bottom_helper() {
        assert!(Orientation::Bottom.is_bottom());
        assert!(!Orientation::Top.is_bottom());
    }

    #[test]
    fn tui_default_columns_are_five() {
        let tui = Tui::default();
        assert_eq!(
            tui.columns,
            vec!["timestamp", "duration", "exit", "project", "command"]
        );
    }

    #[test]
    fn parse_tui_columns_custom() {
        let cfg = parse("[tui]\ncolumns = [\"exit\", \"command\"]").unwrap();
        assert_eq!(cfg.tui.columns, vec!["exit", "command"]);
    }

    #[test]
    fn parse_default_config_toml_equals_default() {
        let cfg = parse(DEFAULT_CONFIG_TOML).unwrap();
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn config_toml_round_trips_to_default() {
        let cfg = Config::default();
        let toml_str = config_toml(&cfg).unwrap();
        assert!(!toml_str.is_empty());
        let cfg2 = parse(&toml_str).unwrap();
        assert_eq!(cfg, cfg2);
    }

    #[test]
    fn get_value_gap_minutes() {
        let cfg = Config::default();
        assert_eq!(get_value(&cfg, "session.gap_minutes").unwrap(), "30");
    }

    #[test]
    fn get_value_orientation() {
        let cfg = Config::default();
        assert_eq!(get_value(&cfg, "tui.orientation").unwrap(), "bottom");
    }

    #[test]
    fn get_value_default_limit() {
        let cfg = Config::default();
        assert_eq!(get_value(&cfg, "search.default_limit").unwrap(), "50");
    }

    #[test]
    fn get_value_unknown_key_returns_err() {
        let cfg = Config::default();
        assert!(get_value(&cfg, "foo.bar").is_err());
    }

    #[test]
    fn apply_set_gap_minutes_creates_section() {
        let result = apply_set("", "session.gap_minutes", "15").unwrap();
        assert!(result.contains("[session]"));
        assert!(result.contains("gap_minutes = 15"));
    }

    #[test]
    fn apply_set_preserves_comment_and_sibling_key() {
        let existing = "# my config\n[session]\ngap_minutes = 30\n# comment\n";
        let result = apply_set(existing, "session.gap_minutes", "60").unwrap();
        assert!(result.contains("# my config"));
        assert!(result.contains("# comment"));
        assert!(result.contains("gap_minutes = 60"));
    }

    #[test]
    fn apply_set_orientation_top() {
        let result = apply_set("", "tui.orientation", "top").unwrap();
        assert!(result.contains("orientation = \"top\""));
    }

    #[test]
    fn apply_set_invalid_gap_minutes_string_returns_err() {
        assert!(apply_set("", "session.gap_minutes", "abc").is_err());
    }

    #[test]
    fn apply_set_invalid_gap_minutes_zero_returns_err() {
        assert!(apply_set("", "session.gap_minutes", "0").is_err());
    }

    #[test]
    fn apply_set_invalid_orientation_returns_err() {
        assert!(apply_set("", "tui.orientation", "sideways").is_err());
    }

    #[test]
    fn apply_set_unknown_key_returns_err() {
        assert!(apply_set("", "foo.bar", "baz").is_err());
    }

    #[test]
    fn render_config_no_color_has_no_ansi() {
        let cfg = Config::default();
        let path = PathBuf::from("/tmp/config.toml");
        let out = render_config(&cfg, &path, false, false);
        assert!(!out.contains('\x1b'));
    }

    #[test]
    fn render_config_with_color_has_ansi() {
        let cfg = Config::default();
        let path = PathBuf::from("/tmp/config.toml");
        let out = render_config(&cfg, &path, false, true);
        assert!(out.contains('\x1b'));
    }

    #[test]
    fn ensure_default_config_writes_when_absent() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("thoth").join("config.toml");
        let wrote = ensure_default_config(&path).unwrap();
        assert!(wrote);
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, DEFAULT_CONFIG_TOML);
    }

    #[test]
    fn ensure_default_config_does_not_overwrite() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "custom content").unwrap();
        let wrote = ensure_default_config(&path).unwrap();
        assert!(!wrote);
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, "custom content");
    }
}
