use crate::error::ThothError;
use crate::search::CommandRow;
use rusqlite::Connection;

pub fn start_line(name: &str) -> String {
    let escaped = name.replace('\'', "'\\''");
    format!("export TTH_ACTIVE_WORKSPACE='{escaped}'")
}

pub fn end_line() -> String {
    String::from("unset TTH_ACTIVE_WORKSPACE")
}

pub fn normalize_workspace(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub struct WorkspaceRow {
    pub name: String,
    pub command_count: i64,
    pub first_ts: i64,
    pub last_ts: i64,
}

pub fn list_workspace_commands(
    conn: &Connection,
    name: &str,
) -> Result<Vec<CommandRow>, ThothError> {
    let mut stmt = conn.prepare(
        "SELECT c.timestamp, c.project, c.tags, c.exit_code, c.duration_ms, \
         c.directory, c.command, c.session_id, c.workspace \
         FROM commands c \
         WHERE c.workspace = ? \
         ORDER BY c.timestamp ASC, c.id ASC",
    )?;
    let rows = stmt.query_map([name], |row| {
        Ok(CommandRow {
            timestamp: row.get(0)?,
            project: row.get(1)?,
            tags: row.get(2)?,
            exit_code: row.get(3)?,
            duration_ms: row.get(4)?,
            directory: row.get(5)?,
            command: row.get(6)?,
            session_id: row.get(7)?,
            workspace: row.get(8)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn list_workspaces(conn: &Connection) -> Result<Vec<WorkspaceRow>, ThothError> {
    let mut stmt = conn.prepare(
        "SELECT workspace, COUNT(*), MIN(timestamp), MAX(timestamp) \
         FROM commands \
         WHERE workspace IS NOT NULL AND workspace <> '' \
         GROUP BY workspace \
         ORDER BY MAX(timestamp) DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(WorkspaceRow {
            name: row.get(0)?,
            command_count: row.get(1)?,
            first_ts: row.get(2)?,
            last_ts: row.get(3)?,
        })
    })?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_conn() -> Connection {
        let mut c = crate::database::connect_memory().unwrap();
        crate::database::apply_migrations(&mut c).unwrap();
        c
    }

    #[test]
    fn start_line_basic() {
        assert_eq!(start_line("demo"), "export TTH_ACTIVE_WORKSPACE='demo'");
    }

    #[test]
    fn start_line_escapes_single_quote() {
        assert_eq!(start_line("it's"), "export TTH_ACTIVE_WORKSPACE='it'\\''s'");
    }

    #[test]
    fn end_line_is_unset() {
        assert_eq!(end_line(), "unset TTH_ACTIVE_WORKSPACE");
    }

    #[test]
    fn normalize_workspace_empty_string_is_none() {
        assert!(normalize_workspace("").is_none());
    }

    #[test]
    fn normalize_workspace_whitespace_only_is_none() {
        assert!(normalize_workspace("   ").is_none());
    }

    #[test]
    fn normalize_workspace_trims_and_returns_some() {
        assert_eq!(normalize_workspace(" a "), Some(String::from("a")));
    }

    #[test]
    fn normalize_workspace_no_trim_needed() {
        assert_eq!(normalize_workspace("ws1"), Some(String::from("ws1")));
    }

    fn seed_ws_command(conn: &Connection, ws: &str, cmd: &str, ts: i64, exit: i64) {
        conn.execute(
            "INSERT INTO commands(command, directory, project, session_id, timestamp, exit_code, duration_ms, tags, workspace) \
             VALUES(?1, '/tmp', 'p', 's1', ?2, ?3, 100, '[]', ?4)",
            rusqlite::params![cmd, ts, exit, ws],
        )
        .unwrap();
    }

    #[test]
    fn list_workspace_commands_empty_returns_empty() {
        let conn = mem_conn();
        let rows = list_workspace_commands(&conn, "ws-a").unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn list_workspace_commands_filters_by_workspace() {
        let conn = mem_conn();
        seed_ws_command(&conn, "ws-a", "cmd-a1", 1000, 0);
        seed_ws_command(&conn, "ws-b", "cmd-b1", 2000, 0);
        let rows = list_workspace_commands(&conn, "ws-a").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].command, "cmd-a1");
    }

    #[test]
    fn list_workspace_commands_returns_asc_order() {
        let conn = mem_conn();
        seed_ws_command(&conn, "ws-a", "first", 1000, 0);
        seed_ws_command(&conn, "ws-a", "second", 2000, 0);
        seed_ws_command(&conn, "ws-a", "third", 3000, 1);
        let rows = list_workspace_commands(&conn, "ws-a").unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].command, "first");
        assert_eq!(rows[1].command, "second");
        assert_eq!(rows[2].command, "third");
    }

    #[test]
    fn list_workspace_commands_populates_all_fields() {
        let conn = mem_conn();
        seed_ws_command(&conn, "ws-x", "ls -la", 5000, 0);
        let rows = list_workspace_commands(&conn, "ws-x").unwrap();
        assert_eq!(rows.len(), 1);
        let r = &rows[0];
        assert_eq!(r.command, "ls -la");
        assert_eq!(r.timestamp, 5000);
        assert_eq!(r.exit_code, 0);
        assert_eq!(r.workspace, Some("ws-x".into()));
    }

    #[test]
    fn list_workspaces_empty_db() {
        let conn = mem_conn();
        let rows = list_workspaces(&conn).unwrap();
        assert!(rows.is_empty());
    }

    #[test]
    fn list_workspaces_excludes_null_and_empty() {
        let mut conn = mem_conn();
        let base = crate::cli::RecordArgs {
            cmd: String::from("echo"),
            dir: Some(String::from("/tmp")),
            exit_code: 0,
            duration: 0,
            timestamp: Some(1700000000),
            tags: String::from("[]"),
            terminal_id: None,
            workspace: None,
        };
        crate::recorder::record_inner(&base, 30, &mut conn).unwrap();
        let rows = list_workspaces(&conn).unwrap();
        assert!(rows.is_empty(), "NULL workspace must be excluded");
    }

    #[test]
    fn list_workspaces_groups_counts_orders_by_last_desc() {
        let mut conn = mem_conn();

        let ws_a_early = crate::cli::RecordArgs {
            cmd: String::from("cmd1"),
            dir: Some(String::from("/tmp")),
            exit_code: 0,
            duration: 0,
            timestamp: Some(1700000001),
            tags: String::from("[]"),
            terminal_id: None,
            workspace: Some(String::from("ws-a")),
        };
        crate::recorder::record_inner(&ws_a_early, 30, &mut conn).unwrap();

        let ws_a_late = crate::cli::RecordArgs {
            cmd: String::from("cmd2"),
            dir: Some(String::from("/tmp")),
            exit_code: 0,
            duration: 0,
            timestamp: Some(1700000003),
            tags: String::from("[]"),
            terminal_id: None,
            workspace: Some(String::from("ws-a")),
        };
        crate::recorder::record_inner(&ws_a_late, 30, &mut conn).unwrap();

        let ws_b = crate::cli::RecordArgs {
            cmd: String::from("cmd3"),
            dir: Some(String::from("/tmp")),
            exit_code: 0,
            duration: 0,
            timestamp: Some(1700000002),
            tags: String::from("[]"),
            terminal_id: None,
            workspace: Some(String::from("ws-b")),
        };
        crate::recorder::record_inner(&ws_b, 30, &mut conn).unwrap();

        let rows = list_workspaces(&conn).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "ws-a");
        assert_eq!(rows[0].command_count, 2);
        assert_eq!(rows[0].last_ts, 1700000003);
        assert_eq!(rows[1].name, "ws-b");
        assert_eq!(rows[1].command_count, 1);
    }
}
