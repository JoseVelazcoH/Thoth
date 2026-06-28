use std::path::PathBuf;
use std::sync::Mutex;

pub const SIZE_CAP_BYTES: u64 = 1_048_576;

static LOG_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn setup(path: PathBuf) {
    let mut guard = LOG_PATH.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some(path);
}

pub fn log_error(msg: &str) {
    let guard = LOG_PATH.lock().unwrap_or_else(|e| e.into_inner());
    let path = match &*guard {
        Some(p) => p.clone(),
        None => crate::paths::resolve_error_log(),
    };
    drop(guard);
    let _ = write_with_rotation(&path, msg);
}

fn write_with_rotation(path: &PathBuf, msg: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if let Ok(meta) = std::fs::metadata(path) {
        if meta.len() > SIZE_CAP_BYTES {
            let backup = path.with_extension("log.1");
            let _ = std::fs::rename(path, &backup);
        }
    }

    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{msg}")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn writes_to_file_not_stderr() {
        let dir = TempDir::new().unwrap();
        let log_path = dir.path().join("error.log");
        setup(log_path.clone());
        log_error("boom");
        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("boom"));
    }

    #[test]
    fn rotation_on_size_cap() {
        let dir = TempDir::new().unwrap();
        let log_path = dir.path().join("error.log");
        let big = vec![b'x'; SIZE_CAP_BYTES as usize + 1];
        std::fs::write(&log_path, &big).unwrap();
        write_with_rotation(&log_path, "after rotation").unwrap();
        let backup = log_path.with_extension("log.1");
        assert!(backup.exists(), "backup file should exist after rotation");
        let new_content = std::fs::read_to_string(&log_path).unwrap();
        assert!(new_content.len() < SIZE_CAP_BYTES as usize);
    }

    #[test]
    fn rotation_overwrites_old_backup() {
        let dir = TempDir::new().unwrap();
        let log_path = dir.path().join("error.log");
        let backup = log_path.with_extension("log.1");
        std::fs::write(&backup, "old backup").unwrap();
        let big = vec![b'x'; SIZE_CAP_BYTES as usize + 1];
        std::fs::write(&log_path, &big).unwrap();
        write_with_rotation(&log_path, "trigger rotation").unwrap();
        let backup_content = std::fs::read_to_string(&backup).unwrap();
        assert_ne!(backup_content, "old backup", "backup should be overwritten");
    }
}
