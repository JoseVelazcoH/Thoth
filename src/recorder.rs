use crate::cli::RecordArgs;
use crate::error::ThothError;
use crate::logging::log_error;
use crate::project::infer_project;
use crate::session::get_or_create;
use rusqlite::{Connection, TransactionBehavior};

pub fn normalize_tags(raw: &str) -> String {
    if raw.is_empty() {
        return String::from("[]");
    }
    match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(serde_json::Value::Array(arr)) => {
            if arr.iter().all(|v| v.is_string()) {
                raw.to_string()
            } else {
                String::from("[]")
            }
        }
        _ => String::from("[]"),
    }
}

pub fn record_inner(
    args: &RecordArgs,
    gap_minutes: i64,
    conn: &mut Connection,
) -> Result<(), ThothError> {
    let tags = normalize_tags(&args.tags);
    let directory = args.dir.clone().unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    });
    let timestamp = args.timestamp.unwrap_or_else(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64
    });

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let project = infer_project(&directory, &tx)?;
    let sid = get_or_create(&project, timestamp, gap_minutes, &tx)?;

    tx.execute(
        "INSERT INTO commands(command, directory, project, session_id, timestamp, exit_code, duration_ms, tags, terminal_id) \
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            args.cmd,
            directory,
            project,
            sid,
            timestamp,
            args.exit_code,
            args.duration,
            tags,
            args.terminal_id
        ],
    )?;

    tx.execute(
        "INSERT INTO projects(path, name, last_seen, command_count) VALUES(?1, ?2, ?3, 1) \
         ON CONFLICT(path) DO UPDATE SET name=excluded.name, last_seen=excluded.last_seen, command_count=command_count+1",
        rusqlite::params![directory, project, timestamp],
    )?;

    tx.execute(
        "UPDATE sessions SET ended_at=?1, command_count=command_count+1 WHERE session_id=?2",
        rusqlite::params![timestamp, sid],
    )?;

    tx.commit()?;
    Ok(())
}

pub fn record(
    args: &RecordArgs,
    gap_minutes: i64,
    history_filters: &[String],
    conn: &mut Connection,
) {
    let (regexes, invalid) = crate::search::compile_filters(history_filters);
    for pat in &invalid {
        crate::logging::log_error(&format!(
            "invalid history filter pattern (skipped): {}",
            pat
        ));
    }
    if crate::search::is_filtered(&args.cmd, &regexes) {
        return;
    }
    match record_inner(args, gap_minutes, conn) {
        Ok(()) => {}
        Err(ThothError::Sqlite(ref e))
            if matches!(
                e,
                rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error {
                        code: rusqlite::ErrorCode::DatabaseBusy,
                        ..
                    },
                    _
                ) | rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error {
                        code: rusqlite::ErrorCode::DatabaseLocked,
                        ..
                    },
                    _
                )
            ) =>
        {
            match record_inner(args, gap_minutes, conn) {
                Ok(()) => {}
                Err(e) => log_error(&e.to_string()),
            }
        }
        Err(e) => log_error(&e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::RecordArgs;
    use rusqlite::Connection;
    use tempfile::TempDir;

    const DEFAULT_GAP: i64 = 30;

    fn mem_conn() -> Connection {
        let mut c = crate::database::connect_memory().unwrap();
        crate::database::apply_migrations(&mut c).unwrap();
        c
    }

    fn disk_conn(dir: &TempDir) -> Connection {
        let path = dir.path().join("history.db");
        crate::database::get_connection(Some(&path)).unwrap()
    }

    fn base_args() -> RecordArgs {
        RecordArgs {
            cmd: String::from("echo hi"),
            dir: Some(String::from("/tmp")),
            exit_code: 0,
            duration: 5,
            timestamp: Some(1700000000),
            tags: String::from("[]"),
            terminal_id: None,
        }
    }

    #[test]
    fn terminal_id_persisted_when_provided() {
        let mut conn = mem_conn();
        let args = RecordArgs {
            terminal_id: Some(String::from("abc")),
            ..base_args()
        };
        record_inner(&args, DEFAULT_GAP, &mut conn).unwrap();
        let val: Option<String> = conn
            .query_row(
                "SELECT terminal_id FROM commands WHERE command='echo hi'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(val, Some(String::from("abc")));
    }

    #[test]
    fn terminal_id_null_when_omitted() {
        let mut conn = mem_conn();
        let args = base_args();
        record_inner(&args, DEFAULT_GAP, &mut conn).unwrap();
        let val: Option<String> = conn
            .query_row(
                "SELECT terminal_id FROM commands WHERE command='echo hi'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(val.is_none());
    }

    #[test]
    fn normalize_tags_valid() {
        assert_eq!(normalize_tags(r#"["a","b"]"#), r#"["a","b"]"#);
    }

    #[test]
    fn normalize_tags_invalid() {
        assert_eq!(normalize_tags("not json"), "[]");
    }

    #[test]
    fn normalize_tags_not_array() {
        assert_eq!(normalize_tags(r#"{"k":"v"}"#), "[]");
    }

    #[test]
    fn normalize_tags_array_not_strings() {
        assert_eq!(normalize_tags("[1,2]"), "[]");
    }

    #[test]
    fn successful_record_inserts_row() {
        let mut conn = mem_conn();
        let args = base_args();
        record_inner(&args, DEFAULT_GAP, &mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM commands WHERE command='echo hi'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn command_count_increments() {
        let mut conn = mem_conn();
        let dir = "/tmp/count-test";
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        conn.execute(
            "INSERT INTO projects(path, name, last_seen, command_count) VALUES(?1, 'myapp', ?2, 5)",
            rusqlite::params![dir, now],
        )
        .unwrap();
        let args = RecordArgs {
            cmd: String::from("ls"),
            dir: Some(dir.to_string()),
            exit_code: 0,
            duration: 1,
            timestamp: Some(1700000000),
            tags: String::from("[]"),
            terminal_id: None,
        };
        record_inner(&args, DEFAULT_GAP, &mut conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT command_count FROM projects WHERE path=?1",
                rusqlite::params![dir],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 6);
    }

    #[test]
    fn session_updated_after_record() {
        let mut conn = mem_conn();
        let args = base_args();
        record_inner(&args, DEFAULT_GAP, &mut conn).unwrap();
        let (ended_at, count): (i64, i64) = conn
            .query_row("SELECT ended_at, command_count FROM sessions", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(ended_at, 1700000000);
        assert!(count >= 1);
    }

    #[test]
    fn atomicity_no_orphan_session() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("history.db");
        let mut conn = crate::database::get_connection(Some(&db_path)).unwrap();

        let conn2 = Connection::open(&db_path).unwrap();
        conn2.execute_batch("PRAGMA busy_timeout=0;").unwrap();
        conn2.execute("BEGIN IMMEDIATE", []).unwrap();

        let args = base_args();
        let result = record_inner(&args, DEFAULT_GAP, &mut conn);

        drop(conn2);

        assert!(result.is_err());

        let commands: i64 = conn
            .query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))
            .unwrap();
        let sessions: i64 = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(commands, 0, "orphan command found");
        assert_eq!(sessions, 0, "orphan session found");
    }

    #[test]
    fn record_never_propagates_error() {
        let dir = TempDir::new().unwrap();
        let log_path = dir.path().join("error.log");
        crate::logging::setup(log_path.clone());

        let mut conn = disk_conn(&dir);
        conn.execute_batch("DROP TABLE commands;").unwrap();

        let args = base_args();
        record(&args, DEFAULT_GAP, &[], &mut conn);

        let log = std::fs::read_to_string(&log_path).unwrap_or_default();
        assert!(!log.is_empty(), "error was not logged");
    }

    #[test]
    fn double_failure_no_row_no_panic() {
        let dir = TempDir::new().unwrap();
        let db_path = dir.path().join("locked.db");
        let mut conn = crate::database::get_connection(Some(&db_path)).unwrap();

        let blocker = Connection::open(&db_path).unwrap();
        blocker.execute_batch("PRAGMA busy_timeout=0;").unwrap();
        blocker.execute("BEGIN IMMEDIATE", []).unwrap();

        let args = base_args();
        record(&args, DEFAULT_GAP, &[], &mut conn);

        drop(blocker);

        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            count, 0,
            "no row should have been inserted on double failure"
        );
    }

    #[test]
    fn history_filter_matching_command_not_inserted() {
        let mut conn = mem_conn();
        let args = RecordArgs {
            cmd: String::from("mysql --password=secret"),
            ..base_args()
        };
        let filters = vec!["--password".to_string()];
        record(&args, DEFAULT_GAP, &filters, &mut conn);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0, "matching command should not be inserted");
    }

    #[test]
    fn history_filter_non_matching_command_is_inserted() {
        let mut conn = mem_conn();
        let filters = vec!["--password".to_string()];
        record(&base_args(), DEFAULT_GAP, &filters, &mut conn);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1, "non-matching command should be inserted");
    }

    #[test]
    fn history_filter_invalid_pattern_does_not_break_recording() {
        let mut conn = mem_conn();
        let filters = vec!["[invalid".to_string()];
        record(&base_args(), DEFAULT_GAP, &filters, &mut conn);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            count, 1,
            "invalid pattern should be skipped; command should still be recorded"
        );
    }

    #[test]
    fn history_filter_empty_filter_records_all() {
        let mut conn = mem_conn();
        record(&base_args(), DEFAULT_GAP, &[], &mut conn);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}
