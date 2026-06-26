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
            let mut haystack_buf = Vec::new();
            let haystack = Utf32Str::new(&row.command, &mut haystack_buf);
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
            timestamp: 0,
            project: String::new(),
            tags: String::from("[]"),
            exit_code: 0,
            duration_ms: 0,
            directory: String::new(),
            command: command.to_string(),
            session_id: String::new(),
        }
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
    fn exact_match_ranks_high() {
        let items = vec![row("git status"), row("git pull"), row("cargo build")];
        let result = rank("git status", &items);
        assert!(!result.is_empty());
        assert_eq!(result[0], 0);
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
    fn ordering_better_match_comes_first() {
        let items = vec![row("cargo test"), row("cargo build --tests")];
        let result = rank("cargo test", &items);
        assert!(!result.is_empty());
        assert_eq!(result[0], 0);
    }

    #[test]
    fn empty_items_returns_empty() {
        let result = rank("anything", &[]);
        assert!(result.is_empty());
    }
}
