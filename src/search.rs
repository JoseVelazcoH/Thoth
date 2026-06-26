use std::cmp::Ordering;

use comfy_table::{Cell, Color, ContentArrangement, Table};
use rusqlite::{Connection, ToSql};

use crate::error::ThothError;

pub use crate::cli::SearchArgs;

const SECS_PER_DAY: i64 = 86_400;
const MS_PER_SEC: i64 = 1_000;

const DAYS_FROM_EPOCH_TO_YEAR_ZERO: i64 = 719_468;

#[derive(clap::ValueEnum, Clone, Debug, PartialEq)]
pub enum ExitFilter {
    Ok,
    Fail,
    Any,
}

pub struct CommandRow {
    pub timestamp: i64,
    pub project: String,
    pub tags: String,
    pub exit_code: i64,
    pub duration_ms: i64,
    pub directory: String,
    pub command: String,
    pub session_id: String,
}

pub fn parse_duration(s: &str) -> Result<(Ordering, i64), ThothError> {
    let (ord, rest) = if let Some(n) = s.strip_prefix('>') {
        (Ordering::Greater, n)
    } else if let Some(n) = s.strip_prefix('<') {
        (Ordering::Less, n)
    } else {
        return Err(ThothError::Search(format!(
            "duration must start with '>' or '<', got: {s}"
        )));
    };
    let secs: i64 = rest
        .parse()
        .map_err(|_| ThothError::Search(format!("invalid duration seconds: {rest}")))?;
    Ok((ord, secs * MS_PER_SEC))
}

pub fn parse_date(s: &str, now: i64) -> Result<i64, ThothError> {
    let today_midnight = now - now.rem_euclid(SECS_PER_DAY);
    match s {
        "today" => Ok(today_midnight),
        "yesterday" => Ok(today_midnight - SECS_PER_DAY),
        "last week" => Ok(today_midnight - 7 * SECS_PER_DAY),
        other => parse_ymd(other),
    }
}

fn parse_ymd(s: &str) -> Result<i64, ThothError> {
    let parts: Vec<&str> = s.splitn(3, '-').collect();
    if parts.len() != 3 {
        return Err(ThothError::Search(format!("invalid date: {s}")));
    }
    let year: i64 = parts[0]
        .parse()
        .map_err(|_| ThothError::Search(format!("invalid date: {s}")))?;
    let month: i64 = parts[1]
        .parse()
        .map_err(|_| ThothError::Search(format!("invalid date: {s}")))?;
    let day: i64 = parts[2]
        .parse()
        .map_err(|_| ThothError::Search(format!("invalid date: {s}")))?;

    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(ThothError::Search(format!("invalid date: {s}")));
    }

    let max_day = month_max_day(year, month);
    if day > max_day {
        return Err(ThothError::Search(format!("invalid date: {s}")));
    }

    let days = days_since_epoch(year, month, day);
    Ok(days * SECS_PER_DAY)
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn month_max_day(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

fn days_since_epoch(year: i64, month: i64, day: i64) -> i64 {
    let (y, m) = if month <= 2 {
        (year - 1, month + 12)
    } else {
        (year, month)
    };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m - 3) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - DAYS_FROM_EPOCH_TO_YEAR_ZERO
}

pub fn build_query(
    args: &SearchArgs,
    now: i64,
) -> Result<(String, Vec<Box<dyn ToSql>>), ThothError> {
    let cols = "c.timestamp, c.project, c.tags, c.exit_code, c.duration_ms, c.directory, c.command, c.session_id";

    let tokens: Vec<String> = match &args.query {
        Some(q) => q
            .split_whitespace()
            .filter(|t| !t.is_empty())
            .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
            .collect(),
        None => vec![],
    };

    let use_fts = !tokens.is_empty();

    let mut fragments: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();

    if use_fts {
        let match_str = tokens.join(" ");
        fragments.push("f.commands_fts MATCH ?".to_string());
        params.push(Box::new(match_str));
    }

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

    if let Some(ref dur_str) = args.duration {
        let (ord, ms) = parse_duration(dur_str)?;
        match ord {
            Ordering::Greater => {
                fragments.push("c.duration_ms > ?".to_string());
                params.push(Box::new(ms));
            }
            Ordering::Less => {
                fragments.push("c.duration_ms < ?".to_string());
                params.push(Box::new(ms));
            }
            Ordering::Equal => {}
        }
    }

    if let Some(ref since_str) = args.since {
        let ts = parse_date(since_str, now)?;
        fragments.push("c.timestamp >= ?".to_string());
        params.push(Box::new(ts));
    }

    if let Some(ref until_str) = args.until {
        let ts = parse_date(until_str, now)?;
        fragments.push("c.timestamp <= ?".to_string());
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

    let limit = args.limit as i64;
    params.push(Box::new(limit));

    let sql = if use_fts {
        let where_clause = fragments.join(" AND ");
        format!(
            "SELECT {cols} FROM commands c INNER JOIN commands_fts f ON f.rowid = c.id WHERE {where_clause} ORDER BY c.timestamp DESC LIMIT ?"
        )
    } else if fragments.is_empty() {
        format!("SELECT {cols} FROM commands c ORDER BY c.timestamp DESC LIMIT ?")
    } else {
        let where_clause = fragments.join(" AND ");
        format!(
            "SELECT {cols} FROM commands c WHERE {where_clause} ORDER BY c.timestamp DESC LIMIT ?"
        )
    };

    Ok((sql, params))
}

pub fn execute(
    args: &SearchArgs,
    conn: &Connection,
    now: i64,
) -> Result<Vec<CommandRow>, ThothError> {
    let (sql, params) = build_query(args, now)?;
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
        |row| {
            Ok(CommandRow {
                timestamp: row.get(0)?,
                project: row.get(1)?,
                tags: row.get(2)?,
                exit_code: row.get(3)?,
                duration_ms: row.get(4)?,
                directory: row.get(5)?,
                command: row.get(6)?,
                session_id: row.get(7)?,
            })
        },
    )?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row?);
    }
    Ok(result)
}

pub fn render(rows: &[CommandRow], show_session: bool) -> String {
    if rows.is_empty() {
        return String::from("0 result(s)\n");
    }

    if show_session {
        render_by_session(rows)
    } else {
        render_table(rows)
    }
}

fn render_table(rows: &[CommandRow]) -> String {
    let mut table = Table::new();
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table.set_header(vec![
        "timestamp",
        "project",
        "tags",
        "exit",
        "duration",
        "directory",
        "command",
    ]);
    for row in rows {
        table.add_row(vec![
            Cell::new(fmt_timestamp(row.timestamp)),
            Cell::new(&row.project),
            Cell::new(&row.tags),
            exit_cell(row.exit_code),
            Cell::new(fmt_duration(row.duration_ms)),
            Cell::new(&row.directory),
            Cell::new(&row.command),
        ]);
    }
    format!("{table}\n{} result(s)\n", rows.len())
}

fn render_by_session(rows: &[CommandRow]) -> String {
    let mut output = String::new();
    let mut current_session: Option<&str> = None;

    let mut group_start = 0;
    let mut i = 0;
    while i <= rows.len() {
        let session_changed =
            i == rows.len() || current_session.is_none_or(|s| s != rows[i].session_id);

        if session_changed && i > 0 {
            let group = &rows[group_start..i];
            let first = &group[0];
            let short_id = &first.session_id[..first.session_id.len().min(8)];
            let date = &fmt_timestamp(first.timestamp)[..10];
            let header = format!(
                "--- session {} · {} · {} ---\n",
                short_id, date, first.project
            );
            output.push_str(&header);

            let mut sub = Table::new();
            sub.set_content_arrangement(ContentArrangement::Dynamic);
            sub.set_header(vec!["timestamp", "exit", "duration", "command"]);
            for r in group {
                sub.add_row(vec![
                    Cell::new(fmt_timestamp(r.timestamp)),
                    exit_cell(r.exit_code),
                    Cell::new(fmt_duration(r.duration_ms)),
                    Cell::new(&r.command),
                ]);
            }
            output.push_str(&format!("{sub}\n"));
            group_start = i;
        }

        if i < rows.len() {
            current_session = Some(&rows[i].session_id);
        }
        i += 1;
    }

    output.push_str(&format!("{} result(s)\n", rows.len()));
    output
}

fn exit_cell(code: i64) -> Cell {
    if code == 0 {
        Cell::new("ok").fg(Color::Green)
    } else {
        Cell::new("fail").fg(Color::Red)
    }
}

pub(crate) fn fmt_timestamp(epoch: i64) -> String {
    let secs = epoch;
    let days = secs.div_euclid(SECS_PER_DAY);
    let time_secs = secs.rem_euclid(SECS_PER_DAY);
    let hh = time_secs / 3600;
    let mm = (time_secs % 3600) / 60;

    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02} {hh:02}:{mm:02}")
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + DAYS_FROM_EPOCH_TO_YEAR_ZERO;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    (y, mo, d)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::SearchArgs;

    fn default_args() -> SearchArgs {
        SearchArgs {
            query: None,
            project: None,
            tag: vec![],
            exit: None,
            duration: None,
            since: None,
            until: None,
            session: None,
            limit: 50,
            show_session: false,
        }
    }

    #[test]
    fn parse_duration_greater() {
        let (ord, ms) = parse_duration(">30").unwrap();
        assert_eq!(ord, Ordering::Greater);
        assert_eq!(ms, 30_000);
    }

    #[test]
    fn parse_duration_less() {
        let (ord, ms) = parse_duration("<5").unwrap();
        assert_eq!(ord, Ordering::Less);
        assert_eq!(ms, 5_000);
    }

    #[test]
    fn parse_duration_no_prefix_is_err() {
        assert!(parse_duration("30").is_err());
    }

    #[test]
    fn parse_duration_non_numeric_is_err() {
        assert!(parse_duration(">abc").is_err());
    }

    const FIXED_NOW: i64 = 1_700_000_000;

    #[test]
    fn parse_date_today() {
        let midnight = FIXED_NOW - FIXED_NOW.rem_euclid(SECS_PER_DAY);
        assert_eq!(parse_date("today", FIXED_NOW).unwrap(), midnight);
    }

    #[test]
    fn parse_date_yesterday() {
        let midnight = FIXED_NOW - FIXED_NOW.rem_euclid(SECS_PER_DAY);
        assert_eq!(
            parse_date("yesterday", FIXED_NOW).unwrap(),
            midnight - SECS_PER_DAY
        );
    }

    #[test]
    fn parse_date_last_week() {
        let midnight = FIXED_NOW - FIXED_NOW.rem_euclid(SECS_PER_DAY);
        assert_eq!(
            parse_date("last week", FIXED_NOW).unwrap(),
            midnight - 7 * SECS_PER_DAY
        );
    }

    #[test]
    fn parse_date_ymd() {
        let epoch = parse_date("2024-01-15", FIXED_NOW).unwrap();
        assert_eq!(epoch, 19_737 * SECS_PER_DAY);
    }

    #[test]
    fn parse_date_bad_string_is_err() {
        assert!(parse_date("bad-date", FIXED_NOW).is_err());
    }

    #[test]
    fn parse_date_invalid_month_is_err() {
        assert!(parse_date("2024-13-01", FIXED_NOW).is_err());
    }

    #[test]
    fn parse_date_feb_31_is_err() {
        assert!(parse_date("2024-02-31", FIXED_NOW).is_err());
    }

    #[test]
    fn parse_date_feb_29_non_leap_is_err() {
        assert!(parse_date("2023-02-29", FIXED_NOW).is_err());
    }

    #[test]
    fn parse_date_feb_29_leap_is_ok() {
        assert!(parse_date("2024-02-29", FIXED_NOW).is_ok());
    }

    #[test]
    fn parse_date_april_31_is_err() {
        assert!(parse_date("2024-04-31", FIXED_NOW).is_err());
    }

    #[test]
    fn build_query_no_filters() {
        let args = default_args();
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(!sql.contains("WHERE"));
        assert!(sql.contains("LIMIT ?"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn build_query_project() {
        let args = SearchArgs {
            project: Some("foo".into()),
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("c.project = ?"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn build_query_exit_ok() {
        let args = SearchArgs {
            exit: Some(ExitFilter::Ok),
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("c.exit_code = 0"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn build_query_exit_fail() {
        let args = SearchArgs {
            exit: Some(ExitFilter::Fail),
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("c.exit_code != 0"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn build_query_exit_any() {
        let args = SearchArgs {
            exit: Some(ExitFilter::Any),
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(!sql.contains("exit_code = 0") && !sql.contains("exit_code != 0"));
        assert_eq!(params.len(), 1);
    }

    #[test]
    fn build_query_duration_greater() {
        let args = SearchArgs {
            duration: Some(">30".into()),
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("c.duration_ms > ?"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn build_query_duration_less() {
        let args = SearchArgs {
            duration: Some("<5".into()),
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("c.duration_ms < ?"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn build_query_since() {
        let args = SearchArgs {
            since: Some("today".into()),
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("c.timestamp >= ?"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn build_query_until() {
        let args = SearchArgs {
            until: Some("today".into()),
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("c.timestamp <= ?"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn build_query_session() {
        let args = SearchArgs {
            session: Some("abc123".into()),
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("c.session_id = ?"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn build_query_one_tag() {
        let args = SearchArgs {
            tag: vec!["rust".into()],
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("EXISTS(SELECT 1 FROM json_each(c.tags) WHERE value = ?)"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn build_query_two_tags() {
        let args = SearchArgs {
            tag: vec!["rust".into(), "cli".into()],
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        let count = sql
            .matches("EXISTS(SELECT 1 FROM json_each(c.tags) WHERE value = ?)")
            .count();
        assert_eq!(count, 2);
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn build_query_fts_single_token() {
        let args = SearchArgs {
            query: Some("foo".into()),
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("INNER JOIN commands_fts"));
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn build_query_fts_multi_token() {
        let conn = mem_conn();
        if !crate::database::fts5_available(&conn) {
            return;
        }
        seed(&conn, s("foo bar baz", "p", 1_000, 0, 100, "[]", "s1"));
        seed(&conn, s("foo only", "p", 2_000, 0, 100, "[]", "s1"));
        seed(&conn, s("bar only", "p", 3_000, 0, 100, "[]", "s1"));
        let args = SearchArgs {
            query: Some("foo bar".into()),
            ..default_args()
        };
        let rows = execute(&args, &conn, FIXED_NOW).unwrap();
        assert_eq!(
            rows.len(),
            1,
            "multi-token FTS must require all tokens present; got {} rows",
            rows.len()
        );
        assert_eq!(rows[0].command, "foo bar baz");
    }

    #[test]
    fn build_query_fts_hyphenated_token_integration() {
        let conn = mem_conn();
        if !crate::database::fts5_available(&conn) {
            return;
        }
        seed(
            &conn,
            s("docker-compose up", "p", 1_000, 0, 100, "[]", "s1"),
        );
        seed(&conn, s("ls -la", "p", 2_000, 0, 100, "[]", "s1"));
        let args = SearchArgs {
            query: Some("docker-compose".into()),
            ..default_args()
        };
        let rows = execute(&args, &conn, FIXED_NOW).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].command, "docker-compose up");
    }

    #[test]
    fn build_query_empty_query_no_fts() {
        let args = SearchArgs {
            query: Some("   ".into()),
            ..default_args()
        };
        let (sql, _) = build_query(&args, FIXED_NOW).unwrap();
        assert!(!sql.contains("INNER JOIN"));
    }

    #[test]
    fn build_query_combined_project_tag_fts_param_order() {
        let args = SearchArgs {
            query: Some("docker".into()),
            project: Some("myapp".into()),
            tag: vec!["infra".into()],
            ..default_args()
        };
        let (sql, params) = build_query(&args, FIXED_NOW).unwrap();
        assert!(sql.contains("INNER JOIN commands_fts"));
        assert!(sql.contains("c.project = ?"));
        assert!(sql.contains("EXISTS(SELECT 1 FROM json_each(c.tags) WHERE value = ?)"));
        assert_eq!(params.len(), 4);
    }

    fn mem_conn() -> rusqlite::Connection {
        let mut c = crate::database::connect_memory().unwrap();
        crate::database::apply_migrations(&mut c).unwrap();
        c
    }

    struct SeedArgs<'a> {
        cmd: &'a str,
        project: &'a str,
        ts: i64,
        exit: i64,
        dur: i64,
        tags: &'a str,
        session: &'a str,
    }

    fn seed(conn: &rusqlite::Connection, a: SeedArgs<'_>) {
        conn.execute(
            "INSERT INTO commands(command, directory, project, session_id, timestamp, exit_code, duration_ms, tags) \
             VALUES(?1, '/tmp', ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![a.cmd, a.project, a.session, a.ts, a.exit, a.dur, a.tags],
        ).unwrap();
    }

    fn s<'a>(
        cmd: &'a str,
        project: &'a str,
        ts: i64,
        exit: i64,
        dur: i64,
        tags: &'a str,
        session: &'a str,
    ) -> SeedArgs<'a> {
        SeedArgs {
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
    fn execute_no_filters_returns_all_ordered_desc() {
        let conn = mem_conn();
        seed(&conn, s("cmd_a", "proj", 1_000, 0, 100, "[]", "s1"));
        seed(&conn, s("cmd_b", "proj", 2_000, 0, 200, "[]", "s1"));
        seed(&conn, s("cmd_c", "proj", 3_000, 0, 300, "[]", "s1"));
        let args = default_args();
        let rows = execute(&args, &conn, FIXED_NOW).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].command, "cmd_c");
        assert_eq!(rows[2].command, "cmd_a");
    }

    #[test]
    fn execute_since_filter() {
        let conn = mem_conn();
        seed(&conn, s("old_cmd", "p", 100, 0, 100, "[]", "s1"));
        seed(&conn, s("new_cmd", "p", 1_700_000_000, 0, 100, "[]", "s1"));
        let args = SearchArgs {
            since: Some("2020-01-01".into()),
            ..default_args()
        };
        let rows = execute(&args, &conn, FIXED_NOW).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].command, "new_cmd");
    }

    #[test]
    fn execute_limit() {
        let conn = mem_conn();
        for i in 0i64..5 {
            seed(
                &conn,
                s(&format!("cmd_{i}"), "p", i * 1000, 0, 100, "[]", "s1"),
            );
        }
        let args = SearchArgs {
            limit: 2,
            ..default_args()
        };
        let rows = execute(&args, &conn, FIXED_NOW).unwrap();
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn fmt_timestamp_known_value() {
        let result = fmt_timestamp(1_700_000_000);
        assert_eq!(result, "2023-11-14 22:13");
    }

    #[test]
    fn fmt_duration_ms() {
        assert_eq!(fmt_duration(500), "500ms");
    }

    #[test]
    fn fmt_duration_exactly_1s() {
        assert_eq!(fmt_duration(1000), "1s");
    }

    #[test]
    fn fmt_duration_2s() {
        assert_eq!(fmt_duration(2000), "2s");
    }

    #[test]
    fn fmt_duration_decimal() {
        assert_eq!(fmt_duration(2300), "2.3s");
    }

    #[test]
    fn exit_zero_cell_is_ok() {
        let cell = exit_cell(0);
        assert_eq!(cell.content(), "ok");
    }

    #[test]
    fn exit_nonzero_cell_is_fail() {
        let cell = exit_cell(1);
        assert_eq!(cell.content(), "fail");
    }

    fn fixture_rows() -> Vec<CommandRow> {
        vec![
            CommandRow {
                timestamp: 1_700_000_000,
                project: "alpha".into(),
                tags: r#"["rust"]"#.into(),
                exit_code: 0,
                duration_ms: 500,
                directory: "/home/user/alpha".into(),
                command: "cargo build".into(),
                session_id: "session-aaa".into(),
            },
            CommandRow {
                timestamp: 1_700_001_000,
                project: "beta".into(),
                tags: "[]".into(),
                exit_code: 1,
                duration_ms: 2300,
                directory: "/home/user/beta".into(),
                command: "make test".into(),
                session_id: "session-bbb".into(),
            },
        ]
    }

    #[test]
    fn render_normal_contains_headers_and_commands() {
        let rows = fixture_rows();
        let out = render(&rows, false);
        assert!(out.contains("timestamp"));
        assert!(out.contains("command"));
        assert!(out.contains("project"));
        assert!(out.contains("cargo build"));
        assert!(out.contains("make test"));
        assert!(out.contains("2 result(s)"));
    }

    #[test]
    fn render_show_session_reduced_columns_two_sessions() {
        let rows = vec![
            CommandRow {
                timestamp: 1_700_000_000,
                project: "alpha".into(),
                tags: r#"["rust"]"#.into(),
                exit_code: 0,
                duration_ms: 100,
                directory: "/home/user/alpha".into(),
                command: "cmd_a".into(),
                session_id: "session-aaa".into(),
            },
            CommandRow {
                timestamp: 1_700_001_000,
                project: "alpha".into(),
                tags: "[]".into(),
                exit_code: 0,
                duration_ms: 200,
                directory: "/home/user/alpha".into(),
                command: "cmd_b".into(),
                session_id: "session-aaa".into(),
            },
            CommandRow {
                timestamp: 1_700_002_000,
                project: "beta".into(),
                tags: "[]".into(),
                exit_code: 1,
                duration_ms: 500,
                directory: "/home/user/beta".into(),
                command: "cmd_c".into(),
                session_id: "session-bbb".into(),
            },
        ];
        let out = render(&rows, true);
        let headers_count = out.matches("---").count();
        assert!(headers_count >= 2, "expected two session header lines");
        let header_lines: Vec<&str> = out.lines().filter(|l| l.starts_with("---")).collect();
        assert_eq!(
            header_lines.len(),
            2,
            "expected exactly two session headers"
        );
        assert!(
            header_lines[0].contains("alpha"),
            "first header should include project alpha"
        );
        assert!(
            header_lines[1].contains("beta"),
            "second header should include project beta"
        );
        assert!(out.contains("cmd_a"));
        assert!(out.contains("cmd_b"));
        assert!(out.contains("cmd_c"));
        assert!(out.contains("timestamp"));
        assert!(out.contains("exit"));
        assert!(out.contains("duration"));
        assert!(out.contains("command"));
        assert!(!out.contains("tags"));
        assert!(!out.contains("directory"));
        assert!(out.contains("3 result(s)"));
        let sub_table_lines: Vec<&str> = out
            .lines()
            .filter(|l| !l.starts_with("---") && l.contains("project"))
            .collect();
        assert!(
            sub_table_lines.is_empty(),
            "project must not appear as a sub-table column header; offending lines: {sub_table_lines:?}"
        );
    }

    #[test]
    fn render_empty_rows_does_not_panic() {
        let out = render(&[], false);
        assert!(
            out.contains("0 result(s)"),
            "empty render must show '0 result(s)'; got: {out}"
        );
    }
}
