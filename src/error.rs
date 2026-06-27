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
    #[error("{0}")]
    Forget(String),
    #[error("tui: {0}")]
    Tui(String),
    #[error("{0}")]
    Tag(String),
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
    fn string_variants_delegate_to_inner_string() {
        assert_eq!(
            ThothError::Hook("could not write rc file".into()).to_string(),
            "could not write rc file"
        );
        assert_eq!(
            ThothError::Search("bad input".into()).to_string(),
            "bad input"
        );
        assert_eq!(
            ThothError::Tui("render failed".into()).to_string(),
            "tui: render failed"
        );
        assert_eq!(
            ThothError::Tag("empty tag name".into()).to_string(),
            "empty tag name"
        );
    }
}
