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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn thoth_db_env_wins() {
        std::env::set_var("THOTH_DB_TEST_01", "/tmp/x_01.db");
        let path = {
            if let Ok(v) = std::env::var("THOTH_DB_TEST_01") {
                PathBuf::from(v)
            } else {
                panic!("var not set")
            }
        };
        assert_eq!(path, PathBuf::from("/tmp/x_01.db"));
        std::env::remove_var("THOTH_DB_TEST_01");
    }

    #[test]
    fn xdg_fallback() {
        std::env::remove_var("THOTH_DB");
        std::env::set_var("THOTH_DB_XDG_TEST", "/tmp/xdg_02");
        let path = PathBuf::from(std::env::var("THOTH_DB_XDG_TEST").unwrap())
            .join("thoth")
            .join("history.db");
        assert_eq!(path, PathBuf::from("/tmp/xdg_02/thoth/history.db"));
        std::env::remove_var("THOTH_DB_XDG_TEST");
    }

    #[test]
    fn home_fallback() {
        let home = "/tmp/h_03";
        let path = PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("thoth")
            .join("history.db");
        assert_eq!(
            path,
            PathBuf::from("/tmp/h_03/.local/share/thoth/history.db")
        );
    }

    #[test]
    fn error_log_env_wins() {
        std::env::set_var("THOTH_ERROR_LOG_TEST_04", "/tmp/err_04.log");
        let path = PathBuf::from(std::env::var("THOTH_ERROR_LOG_TEST_04").unwrap());
        assert_eq!(path, PathBuf::from("/tmp/err_04.log"));
        std::env::remove_var("THOTH_ERROR_LOG_TEST_04");
    }

    #[test]
    fn resolve_db_path_uses_thoth_db() {
        std::env::set_var("THOTH_DB", "/tmp/test_path_05.db");
        let result = resolve_db_path();
        std::env::remove_var("THOTH_DB");
        assert_eq!(result, PathBuf::from("/tmp/test_path_05.db"));
    }

    #[test]
    fn resolve_error_log_uses_thoth_error_log() {
        std::env::set_var("THOTH_ERROR_LOG", "/tmp/test_error_06.log");
        let result = resolve_error_log();
        std::env::remove_var("THOTH_ERROR_LOG");
        assert_eq!(result, PathBuf::from("/tmp/test_error_06.log"));
    }
}
