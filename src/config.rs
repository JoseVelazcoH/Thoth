use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::ThothError;

#[derive(Deserialize, Debug, Clone, PartialEq, Default)]
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

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Session {
    pub gap_minutes: i64,
}

impl Default for Session {
    fn default() -> Self {
        Self { gap_minutes: 30 }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Tui {
    pub orientation: Orientation,
}

impl Default for Tui {
    fn default() -> Self {
        Self {
            orientation: Orientation::Bottom,
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Search {
    pub default_limit: usize,
}

impl Default for Search {
    fn default() -> Self {
        Self { default_limit: 50 }
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct Config {
    pub session: Session,
    pub tui: Tui,
    pub search: Search,
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

pub fn render_config(cfg: &Config, path: &Path, exists: bool) -> String {
    let exists_str = if exists { "true" } else { "false" };
    let orientation = if cfg.tui.orientation.is_bottom() {
        "bottom"
    } else {
        "top"
    };
    format!(
        "Config path: {}\nExists:      {}\n[session] gap_minutes = {}\n[tui] orientation = {}\n[search] default_limit = {}\n",
        path.display(),
        exists_str,
        cfg.session.gap_minutes,
        orientation,
        cfg.search.default_limit,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
        let out = render_config(&cfg, &path, false);
        assert!(out.contains("gap_minutes = 30"));
        assert!(out.contains("orientation = bottom"));
        assert!(out.contains("default_limit = 50"));
        assert!(out.contains("false"));
    }

    #[test]
    fn render_config_custom() {
        let cfg = parse("[session]\ngap_minutes = 10\n[tui]\norientation = \"top\"\n[search]\ndefault_limit = 3").unwrap();
        let path = PathBuf::from("/tmp/config.toml");
        let out = render_config(&cfg, &path, true);
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
}
