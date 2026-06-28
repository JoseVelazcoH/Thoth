use crate::error::ThothError;
use rusqlite::Connection;

pub fn parse_active(json: &str) -> Vec<String> {
    match serde_json::from_str::<serde_json::Value>(json) {
        Ok(serde_json::Value::Array(arr)) => arr
            .into_iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect(),
        _ => vec![],
    }
}

pub fn add_tag(json: &str, name: &str) -> String {
    let mut tags = parse_active(json);
    if !tags.iter().any(|t| t == name) {
        tags.push(name.to_string());
    }
    serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string())
}

pub fn remove_tag(json: &str, name: &str) -> String {
    let tags: Vec<String> = parse_active(json)
        .into_iter()
        .filter(|t| t != name)
        .collect();
    serde_json::to_string(&tags).unwrap_or_else(|_| "[]".to_string())
}

pub fn clear_tags() -> String {
    "[]".to_string()
}

pub fn export_line(json: &str) -> String {
    let escaped = json.replace('\'', "'\\''");
    let prompt = format_prompt_segment(json);
    let escaped_prompt = prompt.replace('\'', "'\\''");
    format!(
        "export TTH_ACTIVE_TAGS='{}'\nexport TTH_PROMPT_TAGS='{}'",
        escaped, escaped_prompt
    )
}

pub fn format_prompt_segment(json: &str) -> String {
    let tags = parse_active(json);
    tags.iter().map(|t| format!("[{t}]")).collect()
}

pub fn list_db_tags(conn: &Connection) -> Result<Vec<(String, i64)>, ThothError> {
    let mut stmt = conn.prepare(
        "SELECT value, COUNT(*) c FROM commands, json_each(commands.tags) \
         GROUP BY value ORDER BY c DESC, value ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        let tag: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        Ok((tag, count))
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
    fn parse_active_empty_string() {
        assert_eq!(parse_active(""), Vec::<String>::new());
    }

    #[test]
    fn parse_active_invalid_json() {
        assert_eq!(parse_active("not json"), Vec::<String>::new());
    }

    #[test]
    fn parse_active_empty_array() {
        assert_eq!(parse_active("[]"), Vec::<String>::new());
    }

    #[test]
    fn parse_active_valid() {
        assert_eq!(parse_active(r#"["a","b"]"#), vec!["a", "b"]);
    }

    #[test]
    fn parse_active_mixed_types_filters_non_strings() {
        assert_eq!(parse_active(r#"["a",1,null]"#), vec!["a"]);
    }

    #[test]
    fn add_tag_to_empty() {
        assert_eq!(add_tag("[]", "x"), r#"["x"]"#);
    }

    #[test]
    fn add_tag_appends() {
        assert_eq!(add_tag(r#"["a"]"#, "b"), r#"["a","b"]"#);
    }

    #[test]
    fn add_tag_dedup_noop() {
        assert_eq!(add_tag(r#"["x"]"#, "x"), r#"["x"]"#);
    }

    #[test]
    fn add_tag_invalid_json_treated_as_empty() {
        assert_eq!(add_tag("bad", "x"), r#"["x"]"#);
    }

    #[test]
    fn remove_tag_removes() {
        assert_eq!(remove_tag(r#"["a","b"]"#, "a"), r#"["b"]"#);
    }

    #[test]
    fn remove_tag_missing_noop() {
        assert_eq!(remove_tag(r#"["a"]"#, "z"), r#"["a"]"#);
    }

    #[test]
    fn remove_tag_last_gives_empty() {
        assert_eq!(remove_tag(r#"["a"]"#, "a"), "[]");
    }

    #[test]
    fn clear_tags_gives_empty_array() {
        assert_eq!(clear_tags(), "[]");
    }

    #[test]
    fn export_line_basic() {
        let line = export_line(r#"["x"]"#);
        assert!(line.contains("export TTH_ACTIVE_TAGS='[\"x\"]'"));
        assert!(line.contains("export TTH_PROMPT_TAGS='[x]'"));
    }

    #[test]
    fn export_line_empty() {
        let line = export_line("[]");
        assert!(line.contains("export TTH_ACTIVE_TAGS='[]'"));
        assert!(line.contains("export TTH_PROMPT_TAGS=''"));
    }

    #[test]
    fn export_line_escapes_single_quote() {
        let json = r#"["it's"]"#;
        let line = export_line(json);
        assert!(line.contains("'\\''"));
    }

    #[test]
    fn format_prompt_segment_empty() {
        assert_eq!(format_prompt_segment("[]"), "");
    }

    #[test]
    fn format_prompt_segment_single() {
        assert_eq!(format_prompt_segment(r#"["a"]"#), "[a]");
    }

    #[test]
    fn format_prompt_segment_multiple() {
        assert_eq!(format_prompt_segment(r#"["a","b"]"#), "[a][b]");
    }

    #[test]
    fn list_db_tags_empty_db() {
        let conn = mem_conn();
        let tags = list_db_tags(&conn).unwrap();
        assert!(tags.is_empty());
    }

    #[test]
    fn list_db_tags_counts_and_order() {
        let mut conn = mem_conn();
        let args_base = crate::cli::RecordArgs {
            cmd: "test".into(),
            dir: Some("/tmp".into()),
            exit_code: 0,
            duration: 0,
            timestamp: Some(1000),
            tags: r#"["fix","perf"]"#.into(),
            terminal_id: None,
            workspace: None,
        };
        let mut a1 = args_base.clone();
        a1.timestamp = Some(1001);
        crate::recorder::record_inner(&a1, 30, &mut conn).unwrap();
        let mut a2 = args_base.clone();
        a2.timestamp = Some(1002);
        a2.tags = r#"["fix"]"#.into();
        crate::recorder::record_inner(&a2, 30, &mut conn).unwrap();

        let tags = list_db_tags(&conn).unwrap();
        assert_eq!(tags[0], ("fix".to_string(), 2));
        assert_eq!(tags[1], ("perf".to_string(), 1));
    }
}
