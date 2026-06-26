use rusqlite::Connection;

use crate::error::ThothError;

pub enum Scope {
    Global,
    Terminal(String),
}

pub fn resolve_scope(terminal_id_flag: Option<String>, env_session_id: Option<String>) -> Scope {
    if let Some(tid) = terminal_id_flag {
        return Scope::Terminal(tid);
    }
    if let Some(sid) = env_session_id {
        return Scope::Terminal(sid);
    }
    Scope::Global
}

pub struct ForgetRow {
    pub id: i64,
    pub timestamp: i64,
    pub project: String,
    pub exit_code: i64,
    pub command: String,
}

const SELECT_COLS: &str = "SELECT id, timestamp, project, exit_code, command FROM commands";

pub fn select_targets(
    conn: &Connection,
    scope: &Scope,
    n: usize,
) -> Result<Vec<ForgetRow>, ThothError> {
    let rows = match scope {
        Scope::Terminal(tid) => {
            let sql = format!(
                "{SELECT_COLS} WHERE terminal_id = ?1 ORDER BY timestamp DESC, id DESC LIMIT ?2"
            );
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map(rusqlite::params![tid, n as i64], map_row)?;
            collect_rows(mapped)?
        }
        Scope::Global => {
            let sql = format!("{SELECT_COLS} ORDER BY timestamp DESC, id DESC LIMIT ?1");
            let mut stmt = conn.prepare(&sql)?;
            let mapped = stmt.query_map(rusqlite::params![n as i64], map_row)?;
            collect_rows(mapped)?
        }
    };
    Ok(rows)
}

fn map_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ForgetRow> {
    Ok(ForgetRow {
        id: row.get(0)?,
        timestamp: row.get(1)?,
        project: row.get(2)?,
        exit_code: row.get(3)?,
        command: row.get(4)?,
    })
}

fn collect_rows(
    mapped: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<ForgetRow>>,
) -> Result<Vec<ForgetRow>, ThothError> {
    let mut out = Vec::new();
    for r in mapped {
        out.push(r?);
    }
    Ok(out)
}

pub fn delete_targets(conn: &Connection, ids: &[i64]) -> Result<usize, ThothError> {
    if ids.is_empty() {
        return Ok(0);
    }
    let placeholders: String = ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("DELETE FROM commands WHERE id IN ({placeholders})");
    let params: Vec<&dyn rusqlite::ToSql> =
        ids.iter().map(|id| id as &dyn rusqlite::ToSql).collect();
    let deleted = conn.execute(&sql, params.as_slice())?;
    Ok(deleted)
}

pub fn render_preview(rows: &[ForgetRow]) -> String {
    use crate::search::fmt_timestamp;
    let mut out = String::new();
    for row in rows {
        let ts = fmt_timestamp(row.timestamp);
        let exit_label = if row.exit_code == 0 { "ok" } else { "fail" };
        out.push_str(&format!(
            "  {} | {} | {} | {}\n",
            ts, row.project, exit_label, row.command
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{apply_migrations, connect_memory};

    fn mem_conn() -> Connection {
        let mut c = connect_memory().unwrap();
        apply_migrations(&mut c).unwrap();
        c
    }

    fn insert_cmd(
        conn: &Connection,
        cmd: &str,
        project: &str,
        ts: i64,
        terminal_id: Option<&str>,
    ) -> i64 {
        conn.execute(
            "INSERT INTO commands(command, directory, project, session_id, timestamp, terminal_id) \
             VALUES(?1, '/tmp', ?2, 's1', ?3, ?4)",
            rusqlite::params![cmd, project, ts, terminal_id],
        )
        .unwrap();
        conn.last_insert_rowid()
    }

    fn count_all(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))
            .unwrap()
    }

    fn fts_match(conn: &Connection, term: &str) -> bool {
        let sql = format!(
            "SELECT rowid FROM commands_fts WHERE commands_fts MATCH '\"{}\"'",
            term
        );
        conn.query_row(&sql, [], |r| r.get::<_, i64>(0)).is_ok()
    }

    #[test]
    fn resolve_scope_flag_wins() {
        let scope = resolve_scope(Some("flag-tid".into()), Some("env-sid".into()));
        match scope {
            Scope::Terminal(tid) => assert_eq!(tid, "flag-tid"),
            Scope::Global => panic!("expected Terminal"),
        }
    }

    #[test]
    fn resolve_scope_env_used_when_no_flag() {
        let scope = resolve_scope(None, Some("env-sid".into()));
        match scope {
            Scope::Terminal(tid) => assert_eq!(tid, "env-sid"),
            Scope::Global => panic!("expected Terminal"),
        }
    }

    #[test]
    fn resolve_scope_global_when_neither() {
        let scope = resolve_scope(None, None);
        assert!(matches!(scope, Scope::Global));
    }

    #[test]
    fn select_targets_global_returns_n_newest() {
        let conn = mem_conn();
        insert_cmd(&conn, "cmd_a", "p", 1000, None);
        insert_cmd(&conn, "cmd_b", "p", 2000, None);
        insert_cmd(&conn, "cmd_c", "p", 3000, None);
        insert_cmd(&conn, "cmd_d", "p", 4000, None);

        let rows = select_targets(&conn, &Scope::Global, 2).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].command, "cmd_d");
        assert_eq!(rows[1].command, "cmd_c");
    }

    #[test]
    fn select_targets_terminal_filters_by_terminal_id() {
        let conn = mem_conn();
        insert_cmd(&conn, "term_a_1", "p", 1000, Some("tid-a"));
        insert_cmd(&conn, "term_b_1", "p", 2000, Some("tid-b"));
        insert_cmd(&conn, "term_a_2", "p", 3000, Some("tid-a"));
        insert_cmd(&conn, "global_1", "p", 4000, None);

        let rows = select_targets(&conn, &Scope::Terminal("tid-a".into()), 10).unwrap();
        assert_eq!(rows.len(), 2);
        let cmds: Vec<&str> = rows.iter().map(|r| r.command.as_str()).collect();
        assert!(cmds.contains(&"term_a_2"));
        assert!(cmds.contains(&"term_a_1"));
        assert!(!cmds.contains(&"term_b_1"));
        assert!(!cmds.contains(&"global_1"));
    }

    #[test]
    fn select_targets_same_timestamp_tiebreak_by_id_desc() {
        let conn = mem_conn();
        let id1 = insert_cmd(&conn, "first_inserted", "p", 5000, None);
        let id2 = insert_cmd(&conn, "second_inserted", "p", 5000, None);

        let rows = select_targets(&conn, &Scope::Global, 2).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0].id, id2,
            "higher id should come first when timestamps equal"
        );
        assert_eq!(rows[1].id, id1);
    }

    #[test]
    fn select_targets_n_larger_than_available_returns_all() {
        let conn = mem_conn();
        insert_cmd(&conn, "only_cmd", "p", 1000, None);

        let rows = select_targets(&conn, &Scope::Global, 100).unwrap();
        assert_eq!(rows.len(), 1);
    }

    #[test]
    fn delete_targets_removes_rows_and_updates_fts() {
        let conn = mem_conn();
        if !crate::database::fts5_available(&conn) {
            return;
        }
        let id1 = insert_cmd(&conn, "unique_forget_cmd_xyz", "p", 1000, None);
        let id2 = insert_cmd(&conn, "unique_forget_cmd_abc", "p", 2000, None);
        insert_cmd(&conn, "keep_me", "p", 3000, None);

        assert!(fts_match(&conn, "unique_forget_cmd_xyz"));
        assert!(fts_match(&conn, "unique_forget_cmd_abc"));

        let deleted = delete_targets(&conn, &[id1, id2]).unwrap();
        assert_eq!(deleted, 2);

        assert_eq!(count_all(&conn), 1);
        assert!(!fts_match(&conn, "unique_forget_cmd_xyz"));
        assert!(!fts_match(&conn, "unique_forget_cmd_abc"));
        assert!(fts_match(&conn, "keep_me"));
    }

    #[test]
    fn delete_targets_empty_ids_is_noop() {
        let conn = mem_conn();
        insert_cmd(&conn, "some_cmd", "p", 1000, None);

        let deleted = delete_targets(&conn, &[]).unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(count_all(&conn), 1);
    }

    #[test]
    fn render_preview_contains_timestamp_project_command() {
        let rows = vec![ForgetRow {
            id: 1,
            timestamp: 1_700_000_000,
            project: "myproject".into(),
            exit_code: 0,
            command: "cargo build".into(),
        }];
        let out = render_preview(&rows);
        assert!(out.contains("myproject"));
        assert!(out.contains("cargo build"));
        assert!(out.contains("2023-11-14"));
    }
}
