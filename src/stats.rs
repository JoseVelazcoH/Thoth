use rusqlite::{Connection, ToSql};

use crate::error::ThothError;
use crate::search::parse_date;

const TOP_PROJECTS_LIMIT: usize = 5;
const TOP_COMMANDS_LIMIT: usize = 10;
const TOP_ERROR_TOOLS_LIMIT: usize = 5;
const MIN_TOOL_COUNT: i64 = 5;
const HISTOGRAM_BAR_WIDTH: usize = 30;
const SECS_PER_DAY: i64 = 86_400;
const SECS_PER_HOUR: i64 = 3_600;

pub struct StatsArgs {
    pub project: Option<String>,
    pub since: Option<String>,
}

pub struct Stats {
    pub total: i64,
    pub distinct_projects: i64,
    pub ok_count: i64,
    pub top_projects: Vec<(String, i64)>,
    pub top_commands: Vec<(String, i64)>,
    pub hour_counts: [i64; 24],
    pub error_tools: Vec<(String, i64, i64)>,
}

pub fn first_token(cmd: &str) -> &str {
    let trimmed = cmd.trim_start();
    match trimmed.find(|c: char| c.is_whitespace()) {
        Some(pos) => &trimmed[..pos],
        None => trimmed,
    }
}

struct FilterClause {
    where_sql: String,
    params: Vec<Box<dyn ToSql>>,
}

fn build_filter(args: &StatsArgs, now: i64) -> Result<FilterClause, ThothError> {
    let mut fragments: Vec<String> = Vec::new();
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();

    if let Some(ref p) = args.project {
        fragments.push("project = ?".to_string());
        params.push(Box::new(p.clone()));
    }

    if let Some(ref since_str) = args.since {
        let ts = parse_date(since_str, now)?;
        fragments.push("timestamp >= ?".to_string());
        params.push(Box::new(ts));
    }

    let where_sql = if fragments.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", fragments.join(" AND "))
    };

    Ok(FilterClause { where_sql, params })
}

pub fn compute(conn: &Connection, args: &StatsArgs, now: i64) -> Result<Stats, ThothError> {
    let filter = build_filter(args, now)?;

    let total: i64 = conn.query_row(
        &format!("SELECT COUNT(*) FROM commands {}", filter.where_sql),
        rusqlite::params_from_iter(filter.params.iter().map(|p| p.as_ref())),
        |r| r.get(0),
    )?;

    let distinct_projects: i64 = conn.query_row(
        &format!(
            "SELECT COUNT(DISTINCT project) FROM commands {}",
            filter.where_sql
        ),
        rusqlite::params_from_iter(filter.params.iter().map(|p| p.as_ref())),
        |r| r.get(0),
    )?;

    let ok_count: i64 = {
        let ok_filter = rebuild_params(args, now)?;
        let mut extra_fragments = Vec::new();
        if !ok_filter.where_sql.is_empty() {
            extra_fragments.push(ok_filter.where_sql.trim_start_matches("WHERE ").to_string());
        }
        extra_fragments.push("exit_code = 0".to_string());
        let where_sql = format!("WHERE {}", extra_fragments.join(" AND "));
        conn.query_row(
            &format!("SELECT COUNT(*) FROM commands {where_sql}"),
            rusqlite::params_from_iter(ok_filter.params.iter().map(|p| p.as_ref())),
            |r| r.get(0),
        )?
    };

    let top_projects: Vec<(String, i64)> = {
        let filter2 = rebuild_params(args, now)?;
        let sql = format!(
            "SELECT project, COUNT(*) as cnt FROM commands {} GROUP BY project ORDER BY cnt DESC LIMIT {}",
            filter2.where_sql,
            TOP_PROJECTS_LIMIT
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(filter2.params.iter().map(|p| p.as_ref())),
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        )?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        result
    };

    let raw_rows: Vec<(String, i64, i64)> = {
        let filter3 = rebuild_params(args, now)?;
        let sql = format!(
            "SELECT command, exit_code, timestamp FROM commands {}",
            filter3.where_sql
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(
            rusqlite::params_from_iter(filter3.params.iter().map(|p| p.as_ref())),
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        result
    };

    let mut cmd_counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    let mut hour_counts = [0i64; 24];
    let mut tool_fails: std::collections::HashMap<String, (i64, i64)> =
        std::collections::HashMap::new();

    for (cmd, exit_code, ts) in &raw_rows {
        *cmd_counts.entry(cmd.clone()).or_insert(0) += 1;

        let hour = (ts.rem_euclid(SECS_PER_DAY) / SECS_PER_HOUR) as usize;
        if hour < 24 {
            hour_counts[hour] += 1;
        }

        let tool = first_token(cmd).to_string();
        let entry = tool_fails.entry(tool).or_insert((0, 0));
        entry.1 += 1;
        if *exit_code != 0 {
            entry.0 += 1;
        }
    }

    let mut top_commands: Vec<(String, i64)> = cmd_counts.into_iter().collect();
    top_commands.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    top_commands.truncate(TOP_COMMANDS_LIMIT);

    let mut error_tools: Vec<(String, i64, i64)> = tool_fails
        .into_iter()
        .filter(|(_, (_, total))| *total >= MIN_TOOL_COUNT)
        .map(|(tool, (fails, total))| (tool, fails, total))
        .collect();
    error_tools.sort_by(|a, b| {
        let rate_a = a.1 as f64 / a.2 as f64;
        let rate_b = b.1 as f64 / b.2 as f64;
        rate_b
            .partial_cmp(&rate_a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    error_tools.truncate(TOP_ERROR_TOOLS_LIMIT);

    Ok(Stats {
        total,
        distinct_projects,
        ok_count,
        top_projects,
        top_commands,
        hour_counts,
        error_tools,
    })
}

fn rebuild_params(args: &StatsArgs, now: i64) -> Result<FilterClause, ThothError> {
    build_filter(args, now)
}

pub fn render(stats: &Stats) -> String {
    let mut out = String::new();

    out.push_str("=== Overview ===\n");
    if stats.total == 0 {
        out.push_str("No data recorded yet.\n");
        out.push('\n');
        out.push_str("=== Top Projects ===\n");
        out.push_str("(no data)\n");
        out.push('\n');
        out.push_str("=== Top Commands ===\n");
        out.push_str("(no data)\n");
        out.push('\n');
        out.push_str("=== Busiest Hour of Day (UTC) ===\n");
        for h in 0..24usize {
            out.push_str(&format!("{h:02} | \n"));
        }
        out.push('\n');
        out.push_str("=== Tools with Highest Error Rate ===\n");
        out.push_str("(no data)\n");
        return out;
    }

    let success_rate = (stats.ok_count as f64 / stats.total as f64) * 100.0;
    out.push_str(&format!("Total commands:    {}\n", stats.total));
    out.push_str(&format!("Active projects:   {}\n", stats.distinct_projects));
    out.push_str(&format!("Global success:    {:.1}%\n", success_rate));
    out.push('\n');

    out.push_str("=== Top Projects ===\n");
    for (project, count) in &stats.top_projects {
        out.push_str(&format!("  {project:<30} {count}\n"));
    }
    out.push('\n');

    out.push_str("=== Top Commands ===\n");
    for (cmd, count) in &stats.top_commands {
        out.push_str(&format!("  {cmd:<40} {count}\n"));
    }
    out.push('\n');

    out.push_str("=== Busiest Hour of Day (UTC) ===\n");
    let max_hour = *stats.hour_counts.iter().max().unwrap_or(&0);
    for h in 0..24usize {
        let count = stats.hour_counts[h];
        let bar_len = if max_hour > 0 {
            (count as usize * HISTOGRAM_BAR_WIDTH) / max_hour as usize
        } else {
            0
        };
        let bar: String = "#".repeat(bar_len);
        out.push_str(&format!(
            "{h:02} | {bar:<width$}  {count}\n",
            width = HISTOGRAM_BAR_WIDTH
        ));
    }
    out.push('\n');

    out.push_str("=== Tools with Highest Error Rate ===\n");
    if stats.error_tools.is_empty() {
        out.push_str("(no tools with enough data)\n");
    } else {
        for (tool, fails, total) in &stats.error_tools {
            let rate = (*fails as f64 / *total as f64) * 100.0;
            out.push_str(&format!("  {tool:<20} {fails}/{total} ({rate:.1}%)\n"));
        }
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

    fn insert(conn: &Connection, cmd: &str, project: &str, ts: i64, exit_code: i64) {
        conn.execute(
            "INSERT INTO commands(command, directory, project, session_id, timestamp, exit_code, duration_ms, tags) \
             VALUES(?1, '/tmp', ?2, 's1', ?3, ?4, 100, '[]')",
            rusqlite::params![cmd, project, ts, exit_code],
        )
        .unwrap();
    }

    const NOW: i64 = 1_700_000_000;

    #[test]
    fn first_token_normal() {
        assert_eq!(first_token("git status"), "git");
    }

    #[test]
    fn first_token_leading_spaces() {
        assert_eq!(first_token("  cargo build"), "cargo");
    }

    #[test]
    fn first_token_empty() {
        assert_eq!(first_token(""), "");
    }

    #[test]
    fn first_token_single_word() {
        assert_eq!(first_token("ls"), "ls");
    }

    #[test]
    fn hour_bucket_maps_correctly() {
        let ts_hour_14: i64 = 14 * SECS_PER_HOUR;
        let hour = (ts_hour_14.rem_euclid(SECS_PER_DAY) / SECS_PER_HOUR) as usize;
        assert_eq!(hour, 14);
    }

    #[test]
    fn hour_bucket_midnight() {
        let ts: i64 = 0;
        let hour = (ts.rem_euclid(SECS_PER_DAY) / SECS_PER_HOUR) as usize;
        assert_eq!(hour, 0);
    }

    #[test]
    fn compute_total_and_distinct_projects() {
        let conn = mem_conn();
        insert(&conn, "git status", "alpha", NOW - 100, 0);
        insert(&conn, "cargo build", "alpha", NOW - 200, 0);
        insert(&conn, "docker ps", "beta", NOW - 300, 1);

        let args = StatsArgs {
            project: None,
            since: None,
        };
        let stats = compute(&conn, &args, NOW).unwrap();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.distinct_projects, 2);
    }

    #[test]
    fn compute_success_rate() {
        let conn = mem_conn();
        insert(&conn, "cmd_ok", "p", NOW - 100, 0);
        insert(&conn, "cmd_ok2", "p", NOW - 200, 0);
        insert(&conn, "cmd_fail", "p", NOW - 300, 1);

        let args = StatsArgs {
            project: None,
            since: None,
        };
        let stats = compute(&conn, &args, NOW).unwrap();
        assert_eq!(stats.ok_count, 2);
        assert_eq!(stats.total, 3);
    }

    #[test]
    fn compute_top_projects_order() {
        let conn = mem_conn();
        insert(&conn, "cmd", "alpha", NOW - 100, 0);
        insert(&conn, "cmd", "alpha", NOW - 200, 0);
        insert(&conn, "cmd", "alpha", NOW - 300, 0);
        insert(&conn, "cmd", "beta", NOW - 400, 0);
        insert(&conn, "cmd", "beta", NOW - 500, 0);
        insert(&conn, "cmd", "gamma", NOW - 600, 0);

        let args = StatsArgs {
            project: None,
            since: None,
        };
        let stats = compute(&conn, &args, NOW).unwrap();
        assert_eq!(stats.top_projects[0].0, "alpha");
        assert_eq!(stats.top_projects[0].1, 3);
        assert_eq!(stats.top_projects[1].0, "beta");
    }

    #[test]
    fn compute_top_commands_order() {
        let conn = mem_conn();
        for _ in 0..5 {
            insert(&conn, "git status", "p", NOW - 100, 0);
        }
        for _ in 0..3 {
            insert(&conn, "cargo build", "p", NOW - 200, 0);
        }
        insert(&conn, "ls", "p", NOW - 300, 0);

        let args = StatsArgs {
            project: None,
            since: None,
        };
        let stats = compute(&conn, &args, NOW).unwrap();
        assert_eq!(stats.top_commands[0].0, "git status");
        assert_eq!(stats.top_commands[0].1, 5);
        assert_eq!(stats.top_commands[1].0, "cargo build");
    }

    #[test]
    fn compute_hour_counts() {
        let conn = mem_conn();
        let ts_h10 = 10 * SECS_PER_HOUR;
        let ts_h14 = 14 * SECS_PER_HOUR;
        insert(&conn, "cmd_a", "p", ts_h10, 0);
        insert(&conn, "cmd_b", "p", ts_h10 + 60, 0);
        insert(&conn, "cmd_c", "p", ts_h14, 0);

        let args = StatsArgs {
            project: None,
            since: None,
        };
        let stats = compute(&conn, &args, NOW).unwrap();
        assert_eq!(stats.hour_counts[10], 2);
        assert_eq!(stats.hour_counts[14], 1);
        assert_eq!(stats.hour_counts[0], 0);
    }

    #[test]
    fn compute_error_tools_excludes_below_min_count() {
        let conn = mem_conn();
        for _ in 0..4 {
            insert(&conn, "rare_tool cmd", "p", NOW - 100, 1);
        }
        for _ in 0..6 {
            insert(&conn, "cargo build", "p", NOW - 200, 1);
        }

        let args = StatsArgs {
            project: None,
            since: None,
        };
        let stats = compute(&conn, &args, NOW).unwrap();
        let tools: Vec<&str> = stats
            .error_tools
            .iter()
            .map(|(t, _, _)| t.as_str())
            .collect();
        assert!(
            !tools.contains(&"rare_tool"),
            "rare_tool has only 4 occurrences, below MIN_TOOL_COUNT=5"
        );
        assert!(tools.contains(&"cargo"));
    }

    #[test]
    fn compute_error_tools_rate() {
        let conn = mem_conn();
        for _ in 0..5 {
            insert(&conn, "docker ps", "p", NOW - 100, 0);
        }
        for _ in 0..5 {
            insert(&conn, "docker run", "p", NOW - 200, 1);
        }

        let args = StatsArgs {
            project: None,
            since: None,
        };
        let stats = compute(&conn, &args, NOW).unwrap();
        let docker = stats.error_tools.iter().find(|(t, _, _)| t == "docker");
        assert!(docker.is_some());
        let (_, fails, total) = docker.unwrap();
        assert_eq!(*fails, 5);
        assert_eq!(*total, 10);
    }

    #[test]
    fn compute_project_filter() {
        let conn = mem_conn();
        insert(&conn, "cmd", "alpha", NOW - 100, 0);
        insert(&conn, "cmd", "alpha", NOW - 200, 0);
        insert(&conn, "cmd", "beta", NOW - 300, 1);

        let args = StatsArgs {
            project: Some("alpha".into()),
            since: None,
        };
        let stats = compute(&conn, &args, NOW).unwrap();
        assert_eq!(stats.total, 2);
        assert_eq!(stats.ok_count, 2);
    }

    #[test]
    fn compute_since_filter() {
        let conn = mem_conn();
        insert(&conn, "old_cmd", "p", 1000, 0);
        insert(&conn, "new_cmd", "p", NOW - 100, 0);

        let args = StatsArgs {
            project: None,
            since: Some("2020-01-01".into()),
        };
        let stats = compute(&conn, &args, NOW).unwrap();
        assert_eq!(stats.total, 1);
        assert_eq!(stats.top_commands[0].0, "new_cmd");
    }

    #[test]
    fn render_contains_section_headers() {
        let stats = Stats {
            total: 10,
            distinct_projects: 2,
            ok_count: 8,
            top_projects: vec![("alpha".into(), 7), ("beta".into(), 3)],
            top_commands: vec![("git status".into(), 5)],
            hour_counts: {
                let mut h = [0i64; 24];
                h[14] = 3;
                h
            },
            error_tools: vec![("cargo".into(), 2, 6)],
        };
        let out = render(&stats);
        assert!(out.contains("=== Overview ==="));
        assert!(out.contains("=== Top Projects ==="));
        assert!(out.contains("=== Top Commands ==="));
        assert!(out.contains("=== Busiest Hour of Day (UTC) ==="));
        assert!(out.contains("=== Tools with Highest Error Rate ==="));
    }

    #[test]
    fn render_success_rate_format() {
        let stats = Stats {
            total: 3,
            distinct_projects: 1,
            ok_count: 2,
            top_projects: vec![],
            top_commands: vec![],
            hour_counts: [0i64; 24],
            error_tools: vec![],
        };
        let out = render(&stats);
        assert!(out.contains("66.7%"));
    }

    #[test]
    fn render_histogram_bar_line() {
        let mut hour_counts = [0i64; 24];
        hour_counts[14] = 10;
        let stats = Stats {
            total: 10,
            distinct_projects: 1,
            ok_count: 10,
            top_projects: vec![],
            top_commands: vec![],
            hour_counts,
            error_tools: vec![],
        };
        let out = render(&stats);
        let line_14 = out.lines().find(|l| l.starts_with("14 |")).unwrap();
        assert!(line_14.contains('#'), "hour 14 should have bar chars");
        assert!(line_14.contains("10"), "hour 14 should show count 10");
    }

    #[test]
    fn render_tool_error_line() {
        let stats = Stats {
            total: 10,
            distinct_projects: 1,
            ok_count: 5,
            top_projects: vec![],
            top_commands: vec![],
            hour_counts: [0i64; 24],
            error_tools: vec![("docker".into(), 3, 6)],
        };
        let out = render(&stats);
        assert!(out.contains("docker"));
        assert!(out.contains("3/6"));
        assert!(out.contains("50.0%"));
    }

    #[test]
    fn render_empty_db_no_panic() {
        let stats = Stats {
            total: 0,
            distinct_projects: 0,
            ok_count: 0,
            top_projects: vec![],
            top_commands: vec![],
            hour_counts: [0i64; 24],
            error_tools: vec![],
        };
        let out = render(&stats);
        assert!(out.contains("No data recorded yet.") || out.contains("(no data)"));
    }
}
