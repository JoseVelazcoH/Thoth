use crate::error::ThothError;
use rusqlite::{Connection, TransactionBehavior};

type CommandFields = (
    String,
    String,
    String,
    String,
    i64,
    i64,
    i64,
    String,
    Option<String>,
    Option<String>,
);

pub fn edit_command(conn: &mut Connection, id: i64, new_command: &str) -> Result<(), ThothError> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    let existing: Option<CommandFields> = {
        let mut stmt = tx.prepare(
            "SELECT command, directory, project, session_id, timestamp, exit_code, duration_ms, tags, terminal_id, workspace \
             FROM commands WHERE id = ?1",
        )?;
        stmt.query_row(rusqlite::params![id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
            ))
        })
        .ok()
    };

    let (
        _,
        directory,
        project,
        session_id,
        timestamp,
        exit_code,
        duration_ms,
        tags,
        terminal_id,
        workspace,
    ) = match existing {
        Some(row) => row,
        None => {
            tx.rollback()?;
            return Ok(());
        }
    };

    tx.execute("DELETE FROM commands WHERE id = ?1", rusqlite::params![id])?;

    tx.execute(
        "INSERT INTO commands(id, command, directory, project, session_id, timestamp, exit_code, duration_ms, tags, terminal_id, workspace) \
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            id,
            new_command,
            directory,
            project,
            session_id,
            timestamp,
            exit_code,
            duration_ms,
            tags,
            terminal_id,
            workspace
        ],
    )?;

    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_conn() -> Connection {
        let mut c = crate::database::connect_memory().unwrap();
        crate::database::apply_migrations(&mut c).unwrap();
        c
    }

    fn seed(conn: &Connection, cmd: &str) -> i64 {
        conn.execute(
            "INSERT INTO commands(command, directory, project, session_id, timestamp, exit_code, duration_ms, tags, terminal_id, workspace) \
             VALUES(?1, '/home/user', 'myproject', 'ses-abc', 1700000001, 0, 250, '[\"rust\"]', 'term-1', 'ws-a')",
            rusqlite::params![cmd],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    #[test]
    fn edit_command_changes_command_text() {
        let mut conn = mem_conn();
        let id = seed(&conn, "git stattus");
        edit_command(&mut conn, id, "git status").unwrap();
        let cmd: String = conn
            .query_row(
                "SELECT command FROM commands WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cmd, "git status");
    }

    #[test]
    fn edit_command_preserves_all_other_columns() {
        let mut conn = mem_conn();
        let id = seed(&conn, "old cmd");
        edit_command(&mut conn, id, "new cmd").unwrap();

        let (directory, project, session_id, timestamp, exit_code, duration_ms, tags, terminal_id, workspace): (
            String, String, String, i64, i64, i64, String, Option<String>, Option<String>,
        ) = conn
            .query_row(
                "SELECT directory, project, session_id, timestamp, exit_code, duration_ms, tags, terminal_id, workspace \
                 FROM commands WHERE id = ?1",
                rusqlite::params![id],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                        r.get(7)?,
                        r.get(8)?,
                    ))
                },
            )
            .unwrap();

        assert_eq!(directory, "/home/user");
        assert_eq!(project, "myproject");
        assert_eq!(session_id, "ses-abc");
        assert_eq!(timestamp, 1700000001);
        assert_eq!(exit_code, 0);
        assert_eq!(duration_ms, 250);
        assert_eq!(tags, "[\"rust\"]");
        assert_eq!(terminal_id, Some("term-1".into()));
        assert_eq!(workspace, Some("ws-a".into()));
    }

    #[test]
    fn edit_command_preserves_same_id() {
        let mut conn = mem_conn();
        let id = seed(&conn, "old cmd");
        edit_command(&mut conn, id, "new cmd").unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM commands WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "row with same id must still exist");
    }

    #[test]
    fn edit_command_nonexistent_id_is_noop() {
        let mut conn = mem_conn();
        let result = edit_command(&mut conn, 99999, "anything");
        assert!(result.is_ok(), "nonexistent id must be a safe no-op");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn edit_command_keeps_fts_in_sync() {
        let mut conn = mem_conn();
        if !crate::database::fts5_available(&conn) {
            return;
        }

        let id = seed(&conn, "old_unique_text_xyzzy");
        edit_command(&mut conn, id, "new_unique_text_quux").unwrap();

        let found_new: Option<i64> = conn
            .query_row(
                "SELECT rowid FROM commands_fts WHERE commands_fts MATCH '\"new_unique_text_quux\"'",
                [],
                |r| r.get(0),
            )
            .ok();
        assert!(
            found_new.is_some(),
            "FTS must find the new command text after edit"
        );

        let found_old: Option<i64> = conn
            .query_row(
                "SELECT rowid FROM commands_fts WHERE commands_fts MATCH '\"old_unique_text_xyzzy\"'",
                [],
                |r| r.get(0),
            )
            .ok();
        assert!(
            found_old.is_none(),
            "FTS must NOT find the old command text after edit"
        );
    }
}
