use std::path::PathBuf;

pub fn resolve_db_path() -> PathBuf {
    if let Ok(v) = std::env::var("THOTH_DB") {
        return PathBuf::from(v);
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return PathBuf::from(xdg).join("thoth").join("history.db");
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
    PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("thoth")
        .join("history.db")
}

pub fn resolve_error_log() -> PathBuf {
    if let Ok(v) = std::env::var("THOTH_ERROR_LOG") {
        return PathBuf::from(v);
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        return PathBuf::from(xdg).join("thoth").join("error.log");
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
    PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("thoth")
        .join("error.log")
}
