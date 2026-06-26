use comfy_table::{ContentArrangement, Table};
use rusqlite::{Connection, ToSql};

use crate::error::ThothError;
use crate::search::fmt_timestamp;
use crate::search::parse_date;

pub struct SessionsArgs {
    pub project: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub limit: usize,
}

pub struct SessionRow {
    pub id: String,
    pub project: String,
    pub started_at: i64,
    pub ended_at: i64,
    pub command_count: i64,
    pub tags: Vec<String>,
}

pub fn build_query(
    args: &SessionsArgs,
    now: i64,
) -> Result<(String, Vec<Box<dyn ToSql>>), ThothError> {
    let mut fragments: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();

    if let Some(ref p) = args.project {
        fragments.push("s.project = ?".to_string());
        params.push(Box::new(p.clone()));
    }

    if let Some(ref since_str) = args.since {
        let ts = parse_date(since_str, now)?;
        fragments.push("s.started_at >= ?".to_string());
        params.push(Box::new(ts));
    }

    if let Some(ref until_str) = args.until {
        let ts = parse_date(until_str, now)?;
        fragments.push("s.started_at <= ?".to_string());
        params.push(Box::new(ts));
    }

    params.push(Box::new(args.limit as i64));

    let sql = if fragments.is_empty() {
        "SELECT s.session_id, s.project, s.started_at, s.ended_at, s.command_count \
         FROM sessions s ORDER BY s.started_at DESC LIMIT ?"
            .to_string()
    } else {
        let where_clause = fragments.join(" AND ");
        format!(
            "SELECT s.session_id, s.project, s.started_at, s.ended_at, s.command_count \
             FROM sessions s WHERE {where_clause} ORDER BY s.started_at DESC LIMIT ?"
        )
    };

    Ok((sql, params))
}

pub fn list_sessions(
    conn: &Connection,
    args: &SessionsArgs,
    now: i64,
) -> Result<Vec<SessionRow>, ThothError> {
    let (sql, params) = build_query(args, now)?;
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
            ))
        },
    )?;

    let mut result = Vec::new();
    for row in rows {
        let (id, project, started_at, ended_at, command_count) = row?;

        let tags = collect_session_tags(conn, &id)?;

        result.push(SessionRow {
            id,
            project,
            started_at,
            ended_at,
            command_count,
            tags,
        });
    }
    Ok(result)
}

fn collect_session_tags(conn: &Connection, session_id: &str) -> Result<Vec<String>, ThothError> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT value FROM commands, json_each(commands.tags) \
         WHERE session_id = ? AND value != '' ORDER BY value",
    )?;
    let tags: rusqlite::Result<Vec<String>> = stmt
        .query_map(rusqlite::params![session_id], |row| row.get(0))?
        .collect();
    Ok(tags?)
}

pub fn render(rows: &[SessionRow]) -> String {
    if rows.is_empty() {
        return String::from("No sessions found.\n");
    }

    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec![
        "session", "project", "started", "ended", "commands", "tags",
    ]);

    for row in rows {
        let short_id = &row.id[..row.id.len().min(8)];
        let tags_str = if row.tags.is_empty() {
            String::from("-")
        } else {
            row.tags.join(",")
        };
        table.add_row(vec![
            short_id.to_string(),
            row.project.clone(),
            fmt_timestamp(row.started_at),
            fmt_timestamp(row.ended_at),
            row.command_count.to_string(),
            tags_str,
        ]);
    }

    format!("{table}\n{} session(s)\n", rows.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{apply_migrations, connect_memory};

    const FIXED_NOW: i64 = 1_700_000_000;

    fn mem_conn() -> Connection {
        let mut c = connect_memory().unwrap();
        apply_migrations(&mut c).unwrap();
        c
    }

    fn default_args() -> SessionsArgs {
        SessionsArgs {
            project: None,
            since: None,
            until: None,
            limit: 20,
        }
    }

    fn insert_session(conn: &Connection, id: &str, project: &str, started: i64, ended: i64) {
        conn.execute(
            "INSERT INTO sessions(session_id, project, started_at, ended_at, command_count) \
             VALUES(?1, ?2, ?3, ?4, 0)",
            rusqlite::params![id, project, started, ended],
        )
        .unwrap();
    }

    fn insert_command(conn: &Connection, session_id: &str, project: &str, ts: i64, tags: &str) {
        conn.execute(
            "INSERT INTO commands(command, directory, project, session_id, timestamp, exit_code, duration_ms, tags) \
             VALUES('cmd', '/tmp', ?1, ?2, ?3, 0, 100, ?4)",
            rusqlite::params![project, session_id, ts, tags],
        )
        .unwrap();
        conn.execute(
            "UPDATE sessions SET command_count = command_count + 1, ended_at = MAX(ended_at, ?1) \
             WHERE session_id = ?2",
            rusqlite::params![ts, session_id],
        )
        .unwrap();
    }

    #[test]
    fn build_query_no_filters_has_no_where() {
        let args = default_args();
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(!sql.contains("WHERE"));
        assert!(sql.contains("ORDER BY s.started_at DESC"));
        assert!(sql.contains("LIMIT ?"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn build_query_project_filter() {
        let args = SessionsArgs {
            project: Some("myapp".into()),
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("s.project = ?"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn build_query_since_filter() {
        let args = SessionsArgs {
            since: Some("today".into()),
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("s.started_at >= ?"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn build_query_until_filter() {
        let args = SessionsArgs {
            until: Some("today".into()),
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("s.started_at <= ?"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn build_query_combined_filters() {
        let args = SessionsArgs {
            project: Some("app".into()),
            since: Some("today".into()),
            until: Some("today".into()),
            limit: 5,
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("s.project = ?"));
        assert!(sql.contains("s.started_at >= ?"));
        assert!(sql.contains("s.started_at <= ?"));
        assert_eq!(params.len(), 4);
    }

    #[test]
    fn list_sessions_ordered_newest_first() {
        let conn = mem_conn();
        insert_session(&conn, "sid-old", "app", 1_000, 2_000);
        insert_session(&conn, "sid-new", "app", 3_000, 4_000);
        let args = default_args();
        let rows = list_sessions(&conn, &args, FIXED_NOW).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].id, "sid-new");
        assert_eq!(rows[1].id, "sid-old");
    }

    #[test]
    fn list_sessions_command_count_correct() {
        let conn = mem_conn();
        insert_session(&conn, "s1", "app", 1_000, 5_000);
        insert_command(&conn, "s1", "app", 1_000, "[]");
        insert_command(&conn, "s1", "app", 2_000, "[]");
        insert_command(&conn, "s1", "app", 3_000, "[]");
        let args = default_args();
        let rows = list_sessions(&conn, &args, FIXED_NOW).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].command_count, 3);
    }

    #[test]
    fn list_sessions_tags_union_deduped() {
        let conn = mem_conn();
        insert_session(&conn, "s1", "app", 1_000, 5_000);
        insert_command(&conn, "s1", "app", 1_000, r#"["a"]"#);
        insert_command(&conn, "s1", "app", 2_000, r#"["a","b"]"#);
        let args = default_args();
        let rows = list_sessions(&conn, &args, FIXED_NOW).unwrap();
        assert_eq!(rows[0].tags, vec!["a", "b"]);
    }

    #[test]
    fn list_sessions_empty_tags_gives_empty_vec() {
        let conn = mem_conn();
        insert_session(&conn, "s1", "app", 1_000, 2_000);
        insert_command(&conn, "s1", "app", 1_000, "[]");
        let args = default_args();
        let rows = list_sessions(&conn, &args, FIXED_NOW).unwrap();
        assert!(rows[0].tags.is_empty());
    }

    #[test]
    fn list_sessions_project_filter() {
        let conn = mem_conn();
        insert_session(&conn, "s-alpha", "alpha", 1_000, 2_000);
        insert_session(&conn, "s-beta", "beta", 3_000, 4_000);
        let args = SessionsArgs {
            project: Some("alpha".into()),
            ..default_args()
        };
        let rows = list_sessions(&conn, &args, FIXED_NOW).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].project, "alpha");
    }

    #[test]
    fn list_sessions_limit_respected() {
        let conn = mem_conn();
        for i in 0i64..5 {
            insert_session(&conn, &format!("sid-{i}"), "app", i * 1000, i * 1000 + 100);
        }
        let args = SessionsArgs {
            limit: 2,
            ..default_args()
        };
        let rows = list_sessions(&conn, &args, FIXED_NOW).unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn render_empty_shows_no_sessions_found() {
        let out = render(&[]);
        assert_eq!(out, "No sessions found.\n");
    }

    #[test]
    fn render_shows_headers_and_footer() {
        let rows = vec![SessionRow {
            id: "abcdefgh-1234-5678-90ab-cdef01234567".into(),
            project: "myapp".into(),
            started_at: 1_700_000_000,
            ended_at: 1_700_003_600,
            command_count: 5,
            tags: vec!["rust".into(), "cli".into()],
        }];
        let out = render(&rows);
        assert!(out.contains("session"), "missing session header");
        assert!(out.contains("project"), "missing project header");
        assert!(out.contains("started"), "missing started header");
        assert!(out.contains("ended"), "missing ended header");
        assert!(out.contains("commands"), "missing commands header");
        assert!(out.contains("tags"), "missing tags header");
        assert!(out.contains("1 session(s)"), "missing footer");
    }

    #[test]
    fn render_short_id_is_first_8_chars() {
        let rows = vec![SessionRow {
            id: "abcdefgh-1234".into(),
            project: "p".into(),
            started_at: 1_700_000_000,
            ended_at: 1_700_000_000,
            command_count: 1,
            tags: vec![],
        }];
        let out = render(&rows);
        assert!(out.contains("abcdefgh"), "expected 8-char prefix");
        assert!(
            !out.contains("abcdefgh-"),
            "must not show more than 8 chars from id"
        );
    }

    #[test]
    fn render_empty_tags_shows_dash() {
        let rows = vec![SessionRow {
            id: "aaaaaaaa".into(),
            project: "p".into(),
            started_at: 1_700_000_000,
            ended_at: 1_700_000_000,
            command_count: 0,
            tags: vec![],
        }];
        let out = render(&rows);
        assert!(out.contains('-'), "empty tags must render as dash");
    }

    #[test]
    fn render_tags_joined_by_comma() {
        let rows = vec![SessionRow {
            id: "aaaaaaaa".into(),
            project: "p".into(),
            started_at: 1_700_000_000,
            ended_at: 1_700_000_000,
            command_count: 2,
            tags: vec!["a".into(), "b".into()],
        }];
        let out = render(&rows);
        assert!(out.contains("a,b"), "tags must be joined by comma");
    }
}
