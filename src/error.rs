#[derive(thiserror::Error, Debug)]
pub enum ThothError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Hook(String),
    #[error("{0}")]
    Search(String),
}

#[cfg(test)]
mod tests {
    use super::ThothError;

    #[test]
    fn display_sqlite_variant() {
        let err = ThothError::Sqlite(rusqlite::Error::QueryReturnedNoRows);
        assert!(err.to_string().starts_with("sqlite:"));
    }

    #[test]
    fn display_io_variant() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err = ThothError::Io(io_err);
        assert!(err.to_string().starts_with("io:"));
    }

    #[test]
    fn hook_variant_formats_message() {
        let err = ThothError::Hook("could not write rc file".into());
        assert_eq!(err.to_string(), "could not write rc file");
    }

    #[test]
    fn search_variant_delegates_to_inner_string() {
        let err = ThothError::Search("bad input".into());
        assert_eq!(err.to_string(), "bad input");
    }
}
