use crate::error::ThothError;
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
