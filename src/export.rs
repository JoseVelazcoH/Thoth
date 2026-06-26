use rusqlite::{Connection, ToSql};

use crate::error::ThothError;
use crate::search::{parse_date, ExitFilter};

const SECS_PER_DAY: i64 = 86_400;

pub struct ExportArgs {
    pub session: Option<String>,
    pub tag: Vec<String>,
    pub project: Option<String>,
    pub since: Option<String>,
    pub exit: Option<ExitFilter>,
}

pub struct ExportRow {
    pub command: String,
    pub directory: String,
    pub timestamp: i64,
    pub exit_code: i64,
    pub duration_ms: i64,
}

pub struct ExportMeta<'a> {
    pub project: Option<&'a str>,
    pub tags: &'a [String],
}

pub fn build_query(
    args: &ExportArgs,
    now: i64,
) -> Result<(String, Vec<Box<dyn ToSql>>), ThothError> {
    let cols = "c.command, c.directory, c.timestamp, c.exit_code, c.duration_ms";

    let mut fragments: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();

    if let Some(ref p) = args.project {
        fragments.push("c.project = ?".to_string());
        params.push(Box::new(p.clone()));
    }

    match &args.exit {
        Some(ExitFilter::Ok) => {
            fragments.push("c.exit_code = 0".to_string());
        }
        Some(ExitFilter::Fail) => {
            fragments.push("c.exit_code != 0".to_string());
        }
        Some(ExitFilter::Any) | None => {}
    }

    if let Some(ref since_str) = args.since {
        let ts = parse_date(since_str, now)?;
        fragments.push("c.timestamp >= ?".to_string());
        params.push(Box::new(ts));
    }

    if let Some(ref sid) = args.session {
        fragments.push("c.session_id = ?".to_string());
        params.push(Box::new(sid.clone()));
    }

    for tag in &args.tag {
        fragments.push("EXISTS(SELECT 1 FROM json_each(c.tags) WHERE value = ?)".to_string());
        params.push(Box::new(tag.clone()));
    }

    let sql = if fragments.is_empty() {
        format!("SELECT {cols} FROM commands c ORDER BY c.timestamp ASC, c.id ASC")
    } else {
        let where_clause = fragments.join(" AND ");
        format!(
            "SELECT {cols} FROM commands c WHERE {where_clause} ORDER BY c.timestamp ASC, c.id ASC"
        )
    };

    Ok((sql, params))
}

pub fn collect(
    conn: &Connection,
    args: &ExportArgs,
    now: i64,
) -> Result<Vec<ExportRow>, ThothError> {
    let (sql, params) = build_query(args, now)?;
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
        |row| {
            Ok(ExportRow {
                command: row.get(0)?,
                directory: row.get(1)?,
                timestamp: row.get(2)?,
                exit_code: row.get(3)?,
                duration_ms: row.get(4)?,
            })
        },
    )?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn fmt_hms(epoch: i64) -> String {
    let time_secs = epoch.rem_euclid(SECS_PER_DAY);
    let hh = time_secs / 3600;
    let mm = (time_secs % 3600) / 60;
    let ss = time_secs % 60;
    format!("{hh:02}:{mm:02}:{ss:02}")
}

fn fmt_duration(ms: i64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        let s = ms as f64 / 1000.0;
        let tenths = (s * 10.0).round() as i64;
        if tenths % 10 == 0 {
            format!("{}s", tenths / 10)
        } else {
            format!("{:.1}s", s)
        }
    }
}

pub fn render_script(rows: &[ExportRow], meta: &ExportMeta<'_>, now: i64) -> String {
    let date_str = {
        let days = now.div_euclid(SECS_PER_DAY);
        let (y, mo, d) = crate::search::civil_from_days(days);
        format!("{y:04}-{mo:02}-{d:02}")
    };

    let project_label = meta.project.unwrap_or("all");
    let tags_label = if meta.tags.is_empty() {
        "none".to_string()
    } else {
        meta.tags.join(", ")
    };

    let mut out = String::new();
    out.push_str("#!/usr/bin/env bash\n");
    out.push_str("# Thoth export\n");
    out.push_str(&format!("# Project: {project_label}\n"));
    out.push_str(&format!("# Tags: {tags_label}\n"));
    out.push_str(&format!("# Exported: {date_str}\n"));

    if rows.is_empty() {
        out.push_str("# (no commands matched)\n");
        return out;
    }

    for row in rows {
        let hms = fmt_hms(row.timestamp);
        let dur = fmt_duration(row.duration_ms);
        out.push('\n');
        out.push_str(&format!(
            "# [{hms}] [{}] [exit: {}] [duration: {dur}]\n",
            row.directory, row.exit_code
        ));
        out.push_str(&row.command);
        out.push('\n');
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXED_NOW: i64 = 1_700_000_000;

    fn default_args() -> ExportArgs {
        ExportArgs {
            session: None,
            tag: vec![],
            project: None,
            since: None,
            exit: None,
        }
    }

    fn mem_conn() -> rusqlite::Connection {
        let mut c = crate::database::connect_memory().unwrap();
        crate::database::apply_migrations(&mut c).unwrap();
        c
    }

    struct SeedRow<'a> {
        cmd: &'a str,
        project: &'a str,
        ts: i64,
        exit: i64,
        dur: i64,
        tags: &'a str,
        session: &'a str,
    }

    fn seed(conn: &rusqlite::Connection, r: SeedRow<'_>) {
        conn.execute(
            "INSERT INTO commands(command, directory, project, session_id, timestamp, exit_code, duration_ms, tags) \
             VALUES(?1, '/tmp', ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![r.cmd, r.project, r.session, r.ts, r.exit, r.dur, r.tags],
        )
        .unwrap();
    }

    fn s<'a>(
        cmd: &'a str,
        project: &'a str,
        ts: i64,
        exit: i64,
        dur: i64,
        tags: &'a str,
        session: &'a str,
    ) -> SeedRow<'a> {
        SeedRow {
            cmd,
            project,
            ts,
            exit,
            dur,
            tags,
            session,
        }
    }

    #[test]
    fn build_query_no_filters_has_no_where_asc_order_no_limit() {
        let args = default_args();
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(!sql.contains("WHERE"), "no WHERE clause expected");
        assert!(
            sql.contains("ORDER BY c.timestamp ASC"),
            "must order ASC; got: {sql}"
        );
        assert!(!sql.contains("LIMIT"), "no LIMIT expected");
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn build_query_project_filter() {
        let args = ExportArgs {
            project: Some("foo".into()),
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("c.project = ?"));
        assert!(sql.contains("ORDER BY c.timestamp ASC"));
        assert!(!sql.contains("LIMIT"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn build_query_one_tag() {
        let args = ExportArgs {
            tag: vec!["rust".into()],
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("EXISTS(SELECT 1 FROM json_each(c.tags) WHERE value = ?)"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn build_query_two_tags_both_present() {
        let args = ExportArgs {
            tag: vec!["rust".into(), "cli".into()],
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        let count = sql
            .matches("EXISTS(SELECT 1 FROM json_each(c.tags) WHERE value = ?)")
            .count();
        assert_eq!(count, 2, "two tag clauses expected for AND semantics");
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn build_query_since() {
        let args = ExportArgs {
            since: Some("today".into()),
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("c.timestamp >= ?"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn build_query_exit_ok() {
        let args = ExportArgs {
            exit: Some(ExitFilter::Ok),
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("c.exit_code = 0"));
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn build_query_exit_fail() {
        let args = ExportArgs {
            exit: Some(ExitFilter::Fail),
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("c.exit_code != 0"));
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn build_query_exit_any_no_exit_clause() {
        let args = ExportArgs {
            exit: Some(ExitFilter::Any),
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(!sql.contains("exit_code = 0") && !sql.contains("exit_code != 0"));
        assert_eq!(params.len(), 0);
    }

    #[test]
    fn build_query_session_filter() {
        let args = ExportArgs {
            session: Some("sess-abc".into()),
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("c.session_id = ?"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn build_query_combined_no_fts_no_limit() {
        let args = ExportArgs {
            project: Some("myapp".into()),
            tag: vec!["infra".into()],
            since: Some("today".into()),
            exit: Some(ExitFilter::Ok),
            session: Some("sess-x".into()),
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("c.project = ?"));
        assert!(sql.contains("c.exit_code = 0"));
        assert!(sql.contains("c.timestamp >= ?"));
        assert!(sql.contains("c.session_id = ?"));
        assert!(sql.contains("EXISTS(SELECT 1 FROM json_each(c.tags) WHERE value = ?)"));
        assert!(!sql.contains("LIMIT"));
        assert!(!sql.contains("JOIN"));
        assert!(sql.contains("ORDER BY c.timestamp ASC"));
        assert_eq!(params.len(), 4);
    }

    #[test]
    fn fmt_hms_known_epoch() {
        let epoch = 1_700_000_000_i64;
        let result = fmt_hms(epoch);
        assert_eq!(result, "22:13:20");
    }

    #[test]
    fn fmt_hms_midnight() {
        let result = fmt_hms(0);
        assert_eq!(result, "00:00:00");
    }

    #[test]
    fn fmt_hms_end_of_day() {
        let result = fmt_hms(86399);
        assert_eq!(result, "23:59:59");
    }

    #[test]
    fn render_script_starts_with_shebang() {
        let rows: Vec<ExportRow> = vec![];
        let meta = ExportMeta {
            project: None,
            tags: &[],
        };
        let out = render_script(&rows, &meta, FIXED_NOW);
        assert!(
            out.starts_with("#!/usr/bin/env bash\n"),
            "shebang must be first line"
        );
    }

    #[test]
    fn render_script_metadata_lines_present() {
        let rows: Vec<ExportRow> = vec![];
        let meta = ExportMeta {
            project: Some("myproj"),
            tags: &["rust".to_string()],
        };
        let out = render_script(&rows, &meta, FIXED_NOW);
        assert!(out.contains("# Thoth export\n"));
        assert!(out.contains("# Project: myproj\n"));
        assert!(out.contains("# Tags: rust\n"));
        assert!(out.contains("# Exported: 2023-11-14\n"));
    }

    #[test]
    fn render_script_no_project_shows_all() {
        let rows: Vec<ExportRow> = vec![];
        let meta = ExportMeta {
            project: None,
            tags: &[],
        };
        let out = render_script(&rows, &meta, FIXED_NOW);
        assert!(out.contains("# Project: all\n"));
        assert!(out.contains("# Tags: none\n"));
    }

    #[test]
    fn render_script_empty_rows_has_no_commands_comment() {
        let rows: Vec<ExportRow> = vec![];
        let meta = ExportMeta {
            project: None,
            tags: &[],
        };
        let out = render_script(&rows, &meta, FIXED_NOW);
        assert!(out.contains("# (no commands matched)"));
    }

    #[test]
    fn render_script_row_comment_and_command() {
        let rows = vec![ExportRow {
            command: "cargo build".into(),
            directory: "/home/user/proj".into(),
            timestamp: 1_700_000_000,
            exit_code: 0,
            duration_ms: 500,
        }];
        let meta = ExportMeta {
            project: None,
            tags: &[],
        };
        let out = render_script(&rows, &meta, FIXED_NOW);
        assert!(
            out.contains("# [22:13:20] [/home/user/proj] [exit: 0] [duration: 500ms]\n"),
            "row comment must be present; got:\n{out}"
        );
        assert!(out.contains("cargo build\n"));
    }

    #[test]
    fn render_script_chronological_order() {
        let rows = vec![
            ExportRow {
                command: "first_cmd".into(),
                directory: "/tmp".into(),
                timestamp: 1_700_000_000,
                exit_code: 0,
                duration_ms: 100,
            },
            ExportRow {
                command: "second_cmd".into(),
                directory: "/tmp".into(),
                timestamp: 1_700_001_000,
                exit_code: 0,
                duration_ms: 200,
            },
        ];
        let meta = ExportMeta {
            project: None,
            tags: &[],
        };
        let out = render_script(&rows, &meta, FIXED_NOW);
        let first_pos = out.find("first_cmd").unwrap();
        let second_pos = out.find("second_cmd").unwrap();
        assert!(
            first_pos < second_pos,
            "first_cmd must appear before second_cmd"
        );
    }

    #[test]
    fn collect_returns_oldest_first() {
        let conn = mem_conn();
        seed(&conn, s("cmd_a", "p", 1_000, 0, 100, "[]", "s1"));
        seed(&conn, s("cmd_b", "p", 3_000, 0, 100, "[]", "s1"));
        seed(&conn, s("cmd_c", "p", 2_000, 0, 100, "[]", "s1"));
        let args = default_args();
        let rows = collect(&conn, &args, FIXED_NOW).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].command, "cmd_a");
        assert_eq!(rows[1].command, "cmd_c");
        assert_eq!(rows[2].command, "cmd_b");
    }

    #[test]
    fn collect_project_filter() {
        let conn = mem_conn();
        seed(&conn, s("alpha_cmd", "alpha", 1_000, 0, 100, "[]", "s1"));
        seed(&conn, s("beta_cmd", "beta", 2_000, 0, 100, "[]", "s1"));
        let args = ExportArgs {
            project: Some("alpha".into()),
            ..default_args()
        };
        let rows = collect(&conn, &args, FIXED_NOW).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].command, "alpha_cmd");
    }

    #[test]
    fn collect_tag_filter() {
        let conn = mem_conn();
        seed(&conn, s("tagged", "p", 1_000, 0, 100, r#"["rust"]"#, "s1"));
        seed(&conn, s("untagged", "p", 2_000, 0, 100, "[]", "s1"));
        let args = ExportArgs {
            tag: vec!["rust".into()],
            ..default_args()
        };
        let rows = collect(&conn, &args, FIXED_NOW).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].command, "tagged");
    }

    #[test]
    fn collect_exit_filter_ok() {
        let conn = mem_conn();
        seed(&conn, s("ok_cmd", "p", 1_000, 0, 100, "[]", "s1"));
        seed(&conn, s("fail_cmd", "p", 2_000, 1, 100, "[]", "s1"));
        let args = ExportArgs {
            exit: Some(ExitFilter::Ok),
            ..default_args()
        };
        let rows = collect(&conn, &args, FIXED_NOW).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].command, "ok_cmd");
    }

    #[test]
    fn collect_session_filter() {
        let conn = mem_conn();
        seed(&conn, s("sess_a", "p", 1_000, 0, 100, "[]", "session-a"));
        seed(&conn, s("sess_b", "p", 2_000, 0, 100, "[]", "session-b"));
        let args = ExportArgs {
            session: Some("session-a".into()),
            ..default_args()
        };
        let rows = collect(&conn, &args, FIXED_NOW).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].command, "sess_a");
    }

    #[test]
    fn collect_since_filter() {
        let conn = mem_conn();
        seed(&conn, s("old_cmd", "p", 100, 0, 100, "[]", "s1"));
        seed(&conn, s("new_cmd", "p", 1_700_000_000, 0, 100, "[]", "s1"));
        let args = ExportArgs {
            since: Some("2020-01-01".into()),
            ..default_args()
        };
        let rows = collect(&conn, &args, FIXED_NOW).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].command, "new_cmd");
    }
}
