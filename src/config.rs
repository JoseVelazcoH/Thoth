use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::ThothError;
use crate::theme::{self, Theme};

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
# Regex patterns; matching commands are NEVER recorded.
# Thoth's own commands (tth, tth-sw, tth-tag, etc.) are ignored by default via this filter.
# To add more patterns (e.g. secrets), extend the list:
# filter = ["^\\s*tth\\b", "--password", "export .*TOKEN"]

# [theme]
# Built-in themes: default, ember, frost, latte, frappe, macchiato, mocha
# You can also drop a <name>.toml file in ~/.config/thoth/themes/ for a custom theme.
# name = "default"
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

fn default_history_filter() -> Vec<String> {
    vec!["^\\s*tth\\b".to_string()]
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct History {
    #[serde(default = "default_history_filter")]
    pub filter: Vec<String>,
}

impl Default for History {
    fn default() -> Self {
        Self {
            filter: default_history_filter(),
        }
    }
}

fn default_theme_name() -> String {
    "default".into()
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(default)]
pub struct ThemeSection {
    #[serde(default = "default_theme_name")]
    pub name: String,
}

impl Default for ThemeSection {
    fn default() -> Self {
        Self {
            name: default_theme_name(),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Config {
    pub session: Session,
    pub tui: Tui,
    pub search: Search,
    pub history: History,
    pub theme: ThemeSection,
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

pub fn themes_dir_from(
    thoth_config: Option<&str>,
    xdg_config: Option<&str>,
    home: &Path,
) -> PathBuf {
    let config_file = config_path_from(thoth_config, xdg_config, home);
    config_file.parent().unwrap_or(home).join("themes")
}

pub fn resolve_themes_dir() -> PathBuf {
    let thoth_config = std::env::var("THOTH_CONFIG").ok();
    let xdg_config = std::env::var("XDG_CONFIG_HOME").ok();
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
    themes_dir_from(
        thoth_config.as_deref(),
        xdg_config.as_deref(),
        Path::new(&home),
    )
}

#[derive(Deserialize, Debug, Default)]
struct ThemeFile {
    extends: Option<String>,
    selection_bg: Option<String>,
    selection_fg: Option<String>,
    accent: Option<String>,
    dim: Option<String>,
    border: Option<String>,
    ok: Option<String>,
    fail: Option<String>,
    project: Option<String>,
    command: Option<String>,
    header: Option<String>,
    controls: Option<String>,
    directory: Option<String>,
    tags: Option<String>,
}

pub fn apply_theme_file(base: Theme, toml_text: &str) -> Theme {
    let file: ThemeFile = toml::from_str(toml_text).unwrap_or_default();
    let mut t = base;

    macro_rules! patch {
        ($field:ident) => {
            if let Some(ref s) = file.$field {
                match theme::parse_color(s) {
                    Ok(c) => t.$field = c,
                    Err(_) => eprintln!(
                        "thoth: invalid theme color for '{}': {}",
                        stringify!($field),
                        s
                    ),
                }
            }
        };
    }

    patch!(selection_bg);
    patch!(selection_fg);
    patch!(accent);
    patch!(dim);
    patch!(border);
    patch!(ok);
    patch!(fail);
    patch!(project);
    patch!(command);
    patch!(header);
    patch!(controls);
    patch!(directory);
    patch!(tags);

    t
}

pub fn resolve_theme(name: &str, themes_dir: &Path) -> Theme {
    if let Some(t) = theme::builtin(name) {
        return t;
    }

    let file_path = themes_dir.join(format!("{name}.toml"));
    if file_path.exists() {
        match std::fs::read_to_string(&file_path) {
            Ok(text) => {
                let extends_name: String = {
                    let tf: ThemeFile = toml::from_str(&text).unwrap_or_default();
                    tf.extends.unwrap_or_else(|| "default".into())
                };
                let base = theme::builtin(&extends_name).unwrap_or_default();
                return apply_theme_file(base, &text);
            }
            Err(e) => {
                eprintln!(
                    "thoth: cannot read theme file {}: {}",
                    file_path.display(),
                    e
                );
            }
        }
    } else {
        eprintln!("thoth: unknown theme '{}', using default", name);
    }

    Theme::default()
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
    use crate::theme::Theme;
    use ratatui::style::Color;
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
    fn history_default_filter_is_tth_pattern() {
        let h = History::default();
        assert_eq!(h.filter, vec!["^\\s*tth\\b".to_string()]);
    }

    #[test]
    fn parse_empty_string_yields_tth_filter() {
        let cfg = parse("").unwrap();
        assert_eq!(cfg.history.filter, vec!["^\\s*tth\\b".to_string()]);
    }

    #[test]
    fn parse_history_filter_replaces_default() {
        let cfg = parse("[history]\nfilter = [\"x\"]").unwrap();
        assert_eq!(cfg.history.filter, vec!["x".to_string()]);
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
    fn theme_section_default_name_is_default() {
        assert_eq!(ThemeSection::default().name, "default");
    }

    #[test]
    fn parse_empty_string_theme_name_is_default() {
        let cfg = parse("").unwrap();
        assert_eq!(cfg.theme.name, "default");
    }

    #[test]
    fn parse_theme_name_mocha() {
        let cfg = parse("[theme]\nname = \"mocha\"").unwrap();
        assert_eq!(cfg.theme.name, "mocha");
    }

    #[test]
    fn parse_default_config_toml_equals_default_with_theme() {
        let cfg = parse(DEFAULT_CONFIG_TOML).unwrap();
        assert_eq!(cfg.theme.name, "default");
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn apply_theme_file_patches_two_slots() {
        let base = Theme::default();
        let toml = "selection_bg = \"#ff8800\"\naccent = \"blue\"\n";
        let result = apply_theme_file(base, toml);
        assert_eq!(result.selection_bg, Color::Rgb(255, 136, 0));
        assert_eq!(result.accent, Color::Blue);
        assert_eq!(result.ok, Theme::default().ok);
        assert_eq!(result.dim, Theme::default().dim);
    }

    #[test]
    fn apply_theme_file_invalid_color_keeps_base() {
        let base = Theme::default();
        let toml = "ok = \"nope\"\n";
        let result = apply_theme_file(base, toml);
        assert_eq!(result.ok, Theme::default().ok);
    }

    #[test]
    fn apply_theme_file_empty_toml_returns_base_unchanged() {
        let base = Theme::default();
        let result = apply_theme_file(base, "");
        assert_eq!(result, Theme::default());
    }

    #[test]
    fn resolve_theme_builtin_mocha() {
        let dir = TempDir::new().unwrap();
        let t = resolve_theme("mocha", dir.path());
        assert_eq!(t, crate::theme::builtin("mocha").unwrap());
    }

    #[test]
    fn resolve_theme_unknown_returns_default() {
        let dir = TempDir::new().unwrap();
        let t = resolve_theme("does-not-exist", dir.path());
        assert_eq!(t, Theme::default());
    }

    #[test]
    fn resolve_theme_file_with_extends_and_override() {
        let dir = TempDir::new().unwrap();
        let content = "extends = \"mocha\"\naccent = \"red\"\n";
        std::fs::write(dir.path().join("mine.toml"), content).unwrap();
        let t = resolve_theme("mine", dir.path());
        let mocha = crate::theme::builtin("mocha").unwrap();
        assert_eq!(t.accent, Color::Red);
        assert_eq!(t.ok, mocha.ok);
        assert_eq!(t.selection_bg, mocha.selection_bg);
    }

    #[test]
    fn themes_dir_from_uses_config_parent() {
        let home = Path::new("/home/user");
        let dir = themes_dir_from(None, None, home);
        assert_eq!(dir, PathBuf::from("/home/user/.config/thoth/themes"));
    }

    #[test]
    fn themes_dir_from_xdg_config() {
        let home = Path::new("/home/user");
        let dir = themes_dir_from(None, Some("/xdg/cfg"), home);
        assert_eq!(dir, PathBuf::from("/xdg/cfg/thoth/themes"));
    }

    #[test]
    fn themes_dir_from_thoth_config_override() {
        let home = Path::new("/home/user");
        let dir = themes_dir_from(Some("/custom/thoth/config.toml"), None, home);
        assert_eq!(dir, PathBuf::from("/custom/thoth/themes"));
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
