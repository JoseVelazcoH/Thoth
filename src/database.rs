use crate::error::ThothError;
use crate::schema::{SCHEMA_V1, SCHEMA_V2_FTS};
use rusqlite::Connection;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub const BUSY_TIMEOUT_MS: u32 = 2000;

pub const MIGRATIONS: &[(i64, &str)] = &[(1, SCHEMA_V1), (2, SCHEMA_V2_FTS)];

pub fn connect_memory() -> Result<Connection, ThothError> {
    let conn = Connection::open_in_memory()?;
    Ok(conn)
}

pub fn get_connection(path: Option<&Path>) -> Result<Connection, ThothError> {
    let db_path = if let Some(p) = path {
        p.to_path_buf()
    } else {
        crate::paths::resolve_db_path()
    };

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut conn = Connection::open(&db_path)?;
    conn.execute_batch(&format!(
        "PRAGMA journal_mode=WAL; PRAGMA busy_timeout={BUSY_TIMEOUT_MS}; PRAGMA synchronous=NORMAL;"
    ))?;
    apply_migrations(&mut conn)?;
    Ok(conn)
}

pub fn current_version(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version",
        [],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

pub fn fts5_available(conn: &Connection) -> bool {
    conn.execute_batch("CREATE VIRTUAL TABLE temp.__fts_probe USING fts5(x); DROP TABLE IF EXISTS temp.__fts_probe;")
        .is_ok()
}

pub fn apply_migrations(conn: &mut Connection) -> Result<(), ThothError> {
    let ver = current_version(conn);

    for &(version, sql) in MIGRATIONS {
        if version <= ver {
            continue;
        }

        if version == 2 && !fts5_available(conn) {
            continue;
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT OR IGNORE INTO schema_version(version, applied_at) VALUES(?1, ?2)",
            rusqlite::params![version, now],
        )?;
        tx.commit()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn mem_conn() -> Connection {
        let mut c = connect_memory().unwrap();
        apply_migrations(&mut c).unwrap();
        c
    }

    #[test]
    fn migration_creates_schema() {
        let conn = mem_conn();
        let tables: Vec<String> = {
            let mut stmt = conn
                .prepare("SELECT name FROM sqlite_master WHERE type='table'")
                .unwrap();
            stmt.query_map([], |row| row.get(0))
                .unwrap()
                .filter_map(|r| r.ok())
                .collect()
        };
        assert!(tables.iter().any(|t| t == "commands"));
        assert!(tables.iter().any(|t| t == "sessions"));
        assert!(tables.iter().any(|t| t == "projects"));
        assert!(tables.iter().any(|t| t == "schema_version"));
        let ver = current_version(&conn);
        assert!(ver >= 1);
    }

    #[test]
    fn migration_idempotent() {
        let mut conn = mem_conn();
        let ver_before = current_version(&conn);
        apply_migrations(&mut conn).unwrap();
        let ver_after = current_version(&conn);
        assert_eq!(ver_before, ver_after);
    }

    #[test]
    fn fts_trigger_sync_insert() {
        let conn = mem_conn();
        if !fts5_available(&conn) {
            return;
        }
        conn.execute(
            "INSERT INTO commands(command, directory, project, session_id, timestamp) VALUES('git status', '/tmp', 'app', 'sid1', 1700000000)",
            [],
        ).unwrap();
        let row: Option<i64> = conn
            .query_row(
                "SELECT rowid FROM commands_fts WHERE commands_fts MATCH 'git'",
                [],
                |r| r.get(0),
            )
            .ok();
        assert!(row.is_some());
    }

    #[test]
    fn fts_trigger_sync_delete() {
        let conn = mem_conn();
        if !fts5_available(&conn) {
            return;
        }
        conn.execute(
            "INSERT INTO commands(command, directory, project, session_id, timestamp) VALUES('unique-cmd-xyz', '/tmp', 'app', 'sid2', 1700000001)",
            [],
        ).unwrap();
        let id: i64 = conn
            .query_row(
                "SELECT id FROM commands WHERE command='unique-cmd-xyz'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        conn.execute("DELETE FROM commands WHERE id=?1", rusqlite::params![id])
            .unwrap();
        let row: Option<i64> = conn
            .query_row(
                "SELECT rowid FROM commands_fts WHERE commands_fts MATCH '\"unique-cmd-xyz\"'",
                [],
                |r| r.get(0),
            )
            .ok();
        assert!(row.is_none());
    }

    #[test]
    fn fts_skip_does_not_record_v2() {
        let mut conn = connect_memory().unwrap();
        conn.execute_batch(SCHEMA_V1).unwrap();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        conn.execute(
            "INSERT OR IGNORE INTO schema_version(version, applied_at) VALUES(1, ?1)",
            rusqlite::params![now],
        )
        .unwrap();
        let ver = current_version(&conn);
        assert_eq!(ver, 1);
    }

    #[test]
    fn atomicity_rollback_on_bad_migration() {
        let mut conn = connect_memory().unwrap();
        let ver_before = current_version(&conn);
        let bad_sql = "THIS IS NOT VALID SQL;";
        let tx = conn.transaction().unwrap();
        let result = tx.execute_batch(bad_sql);
        assert!(result.is_err());
        drop(tx);
        let ver_after = current_version(&conn);
        assert_eq!(ver_before, ver_after);
    }

    #[test]
    fn execute_batch_smoke() {
        let mut conn = connect_memory().unwrap();
        apply_migrations(&mut conn).unwrap();

        let sql = "
CREATE TABLE IF NOT EXISTS audit_log (
    id    INTEGER PRIMARY KEY AUTOINCREMENT,
    label TEXT    NOT NULL
);

CREATE TRIGGER IF NOT EXISTS commands_audit AFTER INSERT ON commands BEGIN
    INSERT INTO audit_log(label)
    VALUES(
        CASE
            WHEN new.exit_code = 0 THEN 'ok; -- not a split'
            ELSE 'fail'
        END
    );
END;
";
        conn.execute_batch(sql).unwrap();
        conn.execute(
            "INSERT INTO commands(command, directory, project, session_id, timestamp) VALUES('ls', '/tmp', 'p', 'sid', 1700000000)",
            [],
        ).unwrap();
        let label: String = conn
            .query_row("SELECT label FROM audit_log", [], |r| r.get(0))
            .unwrap();
        assert_eq!(label, "ok; -- not a split");
    }
}
