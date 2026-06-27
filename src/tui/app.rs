use rusqlite::Connection;

use crate::cli::SearchArgs;
use crate::error::ThothError;
use crate::search::{CommandRow, ExitFilter};
use crate::tui::fuzzy;

const TUI_LIMIT: usize = 5000;

pub struct FilterState {
    pub project: Option<String>,
    pub tag: Vec<String>,
    pub exit: Option<ExitFilter>,
    pub since: Option<String>,
    pub until: Option<String>,
}

impl Default for FilterState {
    fn default() -> Self {
        Self::new()
    }
}

impl FilterState {
    pub fn new() -> Self {
        Self {
            project: None,
            tag: vec![],
            exit: None,
            since: None,
            until: None,
        }
    }

    pub fn to_search_args(&self) -> SearchArgs {
        SearchArgs {
            query: None,
            project: self.project.clone(),
            tag: self.tag.clone(),
            exit: self.exit.clone(),
            duration: None,
            since: self.since.clone(),
            until: self.until.clone(),
            session: None,
            limit: TUI_LIMIT,
            show_session: false,
        }
    }
}

pub enum Action {
    Run(String),
    Edit(String),
}

pub struct App {
    pub query: String,
    pub all_rows: Vec<CommandRow>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub filters: FilterState,
    pub action: Option<Action>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            all_rows: vec![],
            filtered: vec![],
            selected: 0,
            filters: FilterState::new(),
            action: None,
        }
    }

    pub fn reload(&mut self, conn: &Connection, now: i64) -> Result<(), ThothError> {
        let args = self.filters.to_search_args();
        self.all_rows = crate::search::execute(&args, conn, now)?;
        self.recompute();
        Ok(())
    }

    pub fn recompute(&mut self) {
        self.filtered = fuzzy::rank(&self.query, &self.all_rows);
        let max = self.filtered.len().saturating_sub(1);
        if self.selected > max {
            self.selected = max;
        }
    }

    pub fn selected_command(&self) -> Option<&str> {
        let idx = self.filtered.get(self.selected)?;
        self.all_rows.get(*idx).map(|r| r.command.as_str())
    }

    pub fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    pub fn move_down(&mut self) {
        let max = self.filtered.len().saturating_sub(1);
        if self.selected < max {
            self.selected += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_conn() -> Connection {
        let mut conn = crate::database::connect_memory().unwrap();
        crate::database::apply_migrations(&mut conn).unwrap();
        conn
    }

    fn seed(conn: &Connection, cmd: &str, ts: i64) {
        conn.execute(
            "INSERT INTO commands(command, directory, project, session_id, timestamp, exit_code, duration_ms, tags) \
             VALUES(?1, '/tmp', 'p', 's1', ?2, 0, 100, '[]')",
            rusqlite::params![cmd, ts],
        )
        .unwrap();
    }

    #[test]
    fn to_search_args_empty_filters() {
        let fs = FilterState::new();
        let args = fs.to_search_args();
        assert!(args.query.is_none());
        assert!(args.project.is_none());
        assert!(args.tag.is_empty());
        assert!(args.exit.is_none());
        assert!(args.since.is_none());
        assert!(args.until.is_none());
        assert_eq!(args.limit, TUI_LIMIT);
    }

    #[test]
    fn to_search_args_all_filter_fields_map_through() {
        let mut fs = FilterState::new();
        fs.project = Some("myapp".into());
        fs.exit = Some(ExitFilter::Fail);
        fs.tag = vec!["rust".into(), "cli".into()];
        fs.since = Some("2024-01-01".into());
        fs.until = Some("2024-12-31".into());
        let args = fs.to_search_args();
        assert!(args.query.is_none());
        assert_eq!(args.project, Some("myapp".into()));
        assert_eq!(args.exit, Some(ExitFilter::Fail));
        assert_eq!(args.tag, vec!["rust".to_string(), "cli".to_string()]);
        assert_eq!(args.since, Some("2024-01-01".into()));
        assert_eq!(args.until, Some("2024-12-31".into()));
        assert_eq!(args.limit, TUI_LIMIT);
    }

    #[test]
    fn reload_populates_all_rows() {
        let conn = make_conn();
        seed(&conn, "git status", 1000);
        seed(&conn, "ls -la", 2000);
        let mut app = App::new();
        app.reload(&conn, 9999).unwrap();
        assert_eq!(app.all_rows.len(), 2);
    }

    #[test]
    fn recompute_filters_on_query() {
        let conn = make_conn();
        seed(&conn, "docker run nginx", 1000);
        seed(&conn, "ls -la", 2000);
        let mut app = App::new();
        app.reload(&conn, 9999).unwrap();
        app.query = "docker".into();
        app.recompute();
        assert_eq!(app.filtered.len(), 1);
        assert_eq!(app.all_rows[app.filtered[0]].command, "docker run nginx");
    }

    #[test]
    fn selected_command_returns_correct_entry() {
        let conn = make_conn();
        seed(&conn, "git status", 2000);
        seed(&conn, "ls -la", 1000);
        let mut app = App::new();
        app.reload(&conn, 9999).unwrap();
        app.selected = 0;
        let cmd = app.selected_command().unwrap();
        assert_eq!(cmd, "git status");
    }

    #[test]
    fn move_down_clamps_at_last() {
        let conn = make_conn();
        seed(&conn, "a", 1000);
        seed(&conn, "b", 2000);
        let mut app = App::new();
        app.reload(&conn, 9999).unwrap();
        app.move_down();
        app.move_down();
        app.move_down();
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn move_up_clamps_at_zero() {
        let conn = make_conn();
        seed(&conn, "a", 1000);
        seed(&conn, "b", 2000);
        let mut app = App::new();
        app.reload(&conn, 9999).unwrap();
        app.move_up();
        app.move_up();
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn selected_command_none_when_empty() {
        let app = App::new();
        assert!(app.selected_command().is_none());
    }
}
