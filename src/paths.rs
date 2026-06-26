use std::path::{Path, PathBuf};

fn db_path_from(thoth_db: Option<&str>, xdg: Option<&str>, home: &Path) -> PathBuf {
    if let Some(v) = thoth_db {
        return PathBuf::from(v);
    }
    if let Some(x) = xdg {
        return PathBuf::from(x).join("thoth").join("history.db");
    }
    home.join(".local")
        .join("share")
        .join("thoth")
        .join("history.db")
}

fn error_log_from(thoth_log: Option<&str>, xdg: Option<&str>, home: &Path) -> PathBuf {
    if let Some(v) = thoth_log {
        return PathBuf::from(v);
    }
    if let Some(x) = xdg {
        return PathBuf::from(x).join("thoth").join("error.log");
    }
    home.join(".local")
        .join("share")
        .join("thoth")
        .join("error.log")
}

pub fn resolve_db_path() -> PathBuf {
    let thoth_db = std::env::var("THOTH_DB").ok();
    let xdg = std::env::var("XDG_DATA_HOME").ok();
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
    db_path_from(thoth_db.as_deref(), xdg.as_deref(), Path::new(&home))
}

pub fn resolve_error_log() -> PathBuf {
    let thoth_log = std::env::var("THOTH_ERROR_LOG").ok();
    let xdg = std::env::var("XDG_DATA_HOME").ok();
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
    error_log_from(thoth_log.as_deref(), xdg.as_deref(), Path::new(&home))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn db_override_wins() {
        let result = db_path_from(
            Some("/custom/history.db"),
            Some("/xdg"),
            Path::new("/home/user"),
        );
        assert_eq!(result, PathBuf::from("/custom/history.db"));
    }

    #[test]
    fn db_xdg_fallback() {
        let result = db_path_from(None, Some("/xdg/data"), Path::new("/home/user"));
        assert_eq!(result, PathBuf::from("/xdg/data/thoth/history.db"));
    }

    #[test]
    fn db_home_fallback() {
        let result = db_path_from(None, None, Path::new("/home/user"));
        assert_eq!(
            result,
            PathBuf::from("/home/user/.local/share/thoth/history.db")
        );
    }

    #[test]
    fn error_log_override_wins() {
        let result = error_log_from(
            Some("/custom/error.log"),
            Some("/xdg"),
            Path::new("/home/user"),
        );
        assert_eq!(result, PathBuf::from("/custom/error.log"));
    }

    #[test]
    fn error_log_xdg_fallback() {
        let result = error_log_from(None, Some("/xdg/data"), Path::new("/home/user"));
        assert_eq!(result, PathBuf::from("/xdg/data/thoth/error.log"));
    }

    #[test]
    fn error_log_home_fallback() {
        let result = error_log_from(None, None, Path::new("/home/user"));
        assert_eq!(
            result,
            PathBuf::from("/home/user/.local/share/thoth/error.log")
        );
    }

    #[test]
    fn resolve_db_path_wires_thoth_db() {
        std::env::set_var("THOTH_DB", "/tmp/wiring_test.db");
        let result = resolve_db_path();
        std::env::remove_var("THOTH_DB");
        assert_eq!(result, PathBuf::from("/tmp/wiring_test.db"));
    }
}
