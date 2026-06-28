use nucleo_matcher::{
    pattern::{CaseMatching, Normalization, Pattern},
    Config, Matcher, Utf32Str,
};

use crate::search::CommandRow;

pub fn rank(query: &str, items: &[CommandRow]) -> Vec<usize> {
    if query.trim().is_empty() {
        return (0..items.len()).collect();
    }

    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

    let mut scored: Vec<(usize, u32)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, row)| {
            let tag_text = crate::tags::parse_active(&row.tags).join(" ");
            let mut parts = vec![row.command.as_str()];
            if !tag_text.is_empty() {
                parts.push(tag_text.as_str());
            }
            if !row.project.is_empty() {
                parts.push(row.project.as_str());
            }
            if !row.directory.is_empty() {
                parts.push(row.directory.as_str());
            }
            let hay = parts.join(" ");
            let mut haystack_buf = Vec::new();
            let haystack = Utf32Str::new(&hay, &mut haystack_buf);
            pattern.score(haystack, &mut matcher).map(|s| (i, s))
        })
        .collect();

    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored.into_iter().map(|(i, _)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(command: &str) -> CommandRow {
        CommandRow {
            id: 0,
            timestamp: 0,
            project: String::new(),
            tags: String::from("[]"),
            exit_code: 0,
            duration_ms: 0,
            directory: String::new(),
            command: command.to_string(),
            session_id: String::new(),
            workspace: None,
        }
    }

    fn row_with_tags(command: &str, tags: &str) -> CommandRow {
        let mut r = row(command);
        r.tags = tags.to_string();
        r
    }

    fn row_with_project(command: &str, project: &str) -> CommandRow {
        let mut r = row(command);
        r.project = project.to_string();
        r
    }

    fn row_with_directory(command: &str, directory: &str) -> CommandRow {
        let mut r = row(command);
        r.directory = directory.to_string();
        r
    }

    #[test]
    fn query_matches_a_tag_not_in_the_command() {
        let items = vec![
            row_with_tags("cargo build", r#"["release","perf"]"#),
            row_with_tags("ls -la", r#"["files"]"#),
        ];
        let result = rank("release", &items);
        assert_eq!(
            result.first(),
            Some(&0),
            "command tagged 'release' must match"
        );
        assert!(
            !result.contains(&1),
            "untagged-for-release row must not match"
        );
    }

    #[test]
    fn empty_query_returns_all_in_original_order() {
        let items = vec![row("git status"), row("ls -la"), row("cargo build")];
        let result = rank("", &items);
        assert_eq!(result, vec![0, 1, 2]);
    }

    #[test]
    fn whitespace_only_query_returns_all() {
        let items = vec![row("git status"), row("ls -la")];
        let result = rank("   ", &items);
        assert_eq!(result, vec![0, 1]);
    }

    #[test]
    fn fuzzy_dkr_matches_docker() {
        let items = vec![row("docker run nginx"), row("ls -la"), row("cargo test")];
        let result = rank("dkr", &items);
        assert!(!result.is_empty());
        assert_eq!(result[0], 0);
    }

    #[test]
    fn no_match_returns_empty() {
        let items = vec![row("git status"), row("ls -la"), row("cargo build")];
        let result = rank("zzzzzzzzzzz", &items);
        assert!(result.is_empty());
    }

    #[test]
    fn query_matches_project_not_in_command_or_tags() {
        let items = vec![
            row_with_project("ls -la", "myproject"),
            row("git status"),
        ];
        let result = rank("myproject", &items);
        assert!(
            result.contains(&0),
            "row with matching project must appear in results"
        );
        assert!(
            !result.contains(&1),
            "row with non-matching project must not appear"
        );
    }

    #[test]
    fn query_matches_directory_not_in_command_or_tags() {
        let items = vec![
            row_with_directory("ls", "/home/jose/special-dir"),
            row("git status"),
        ];
        let result = rank("special-dir", &items);
        assert!(
            result.contains(&0),
            "row with matching directory must appear in results"
        );
        assert!(
            !result.contains(&1),
            "row with non-matching directory must not appear"
        );
    }

    #[test]
    fn command_tag_match_still_works_after_haystack_extension() {
        let items = vec![
            row_with_tags("cargo build", r#"["release"]"#),
            row("unrelated"),
        ];
        let result = rank("release", &items);
        assert!(result.contains(&0), "tag match must still work");
        assert!(!result.contains(&1), "non-matching row must not appear");
    }

    #[test]
    fn ordering_better_match_comes_first() {
        let items = vec![row("cargo test"), row("cargo build --tests")];
        let result = rank("cargo test", &items);
        assert!(!result.is_empty());
        assert_eq!(result[0], 0);
    }
}
