use rusqlite::Connection;

use crate::cli::SearchArgs;
use crate::error::ThothError;
use crate::search::{CommandRow, ExitFilter};
use crate::sessions::{list_sessions, SessionRow, SessionsArgs};
use crate::tui::fuzzy;

const TUI_LIMIT: usize = 5000;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tab {
    #[default]
    History,
    Sessions,
}

pub struct FilterState {
    pub project: Option<String>,
    pub tag: Vec<String>,
    pub exit: Option<ExitFilter>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub session: Option<String>,
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
            session: None,
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
            session: self.session.clone(),
            limit: Some(TUI_LIMIT),
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
    pub tab: Tab,
    pub sessions: Vec<SessionRow>,
    pub session_selected: usize,
    pub session_commands: Vec<CommandRow>,
    pub needs_session_reload: bool,
    pub needs_session_commands_reload: bool,
    pub needs_history_reload: bool,
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
            tab: Tab::History,
            sessions: vec![],
            session_selected: 0,
            session_commands: vec![],
            needs_session_reload: false,
            needs_session_commands_reload: false,
            needs_history_reload: false,
        }
    }

    pub fn prev_tab(&mut self) {
        self.tab = Tab::History;
    }

    pub fn next_tab(&mut self) {
        if self.tab == Tab::History {
            self.tab = Tab::Sessions;
            if self.sessions.is_empty() {
                self.needs_session_reload = true;
                self.needs_session_commands_reload = true;
            }
        }
    }

    pub fn session_move_up(&mut self) {
        if self.session_selected > 0 {
            self.session_selected -= 1;
            self.needs_session_commands_reload = true;
        }
    }

    pub fn session_move_down(&mut self) {
        let max = self.sessions.len().saturating_sub(1);
        if self.session_selected < max {
            self.session_selected += 1;
            self.needs_session_commands_reload = true;
        }
    }

    pub fn selected_session(&self) -> Option<&SessionRow> {
        self.sessions.get(self.session_selected)
    }

    pub fn open_session(&mut self) {
        if let Some(s) = self.selected_session() {
            let id = s.id.clone();
            self.filters.session = Some(id);
            self.tab = Tab::History;
            self.query.clear();
            self.selected = 0;
            self.needs_history_reload = true;
        }
    }

    pub fn reload(&mut self, conn: &Connection, now: i64) -> Result<(), ThothError> {
        let args = self.filters.to_search_args();
        self.all_rows = crate::search::execute(&args, conn, now)?;
        self.recompute();
        Ok(())
    }

    pub fn reload_sessions(&mut self, conn: &Connection, now: i64) -> Result<(), ThothError> {
        let args = SessionsArgs {
            project: None,
            since: None,
            until: None,
            limit: TUI_LIMIT,
        };
        self.sessions = list_sessions(conn, &args, now)?;
        let max = self.sessions.len().saturating_sub(1);
        if self.session_selected > max {
            self.session_selected = 0;
        }
        self.needs_session_reload = false;
        self.needs_session_commands_reload = true;
        Ok(())
    }

    pub fn reload_session_commands(
        &mut self,
        conn: &Connection,
        now: i64,
    ) -> Result<(), ThothError> {
        if let Some(s) = self.selected_session() {
            let args = SearchArgs {
                query: None,
                project: None,
                tag: vec![],
                exit: None,
                duration: None,
                since: None,
                until: None,
                session: Some(s.id.clone()),
                limit: Some(TUI_LIMIT),
                show_session: false,
            };
            self.session_commands = crate::search::execute(&args, conn, now)?;
        } else {
            self.session_commands.clear();
        }
        self.needs_session_commands_reload = false;
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
        let max = self.filtered.len().saturating_sub(1);
        if self.selected < max {
            self.selected += 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
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

    fn seed_session(conn: &Connection, id: &str, project: &str, started: i64) {
        conn.execute(
            "INSERT INTO sessions(session_id, project, started_at, ended_at, command_count) \
             VALUES(?1, ?2, ?3, ?3, 0)",
            rusqlite::params![id, project, started],
        )
        .unwrap();
    }

    fn seed_command_for_session(conn: &Connection, session_id: &str, cmd: &str, ts: i64) {
        conn.execute(
            "INSERT INTO commands(command, directory, project, session_id, timestamp, exit_code, duration_ms, tags) \
             VALUES(?1, '/tmp', 'p', ?2, ?3, 0, 100, '[]')",
            rusqlite::params![cmd, session_id, ts],
        )
        .unwrap();
        conn.execute(
            "UPDATE sessions SET command_count = command_count + 1 WHERE session_id = ?1",
            rusqlite::params![session_id],
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
        assert!(args.session.is_none());
        assert_eq!(args.limit, Some(TUI_LIMIT));
    }

    #[test]
    fn to_search_args_all_filter_fields_map_through() {
        let mut fs = FilterState::new();
        fs.project = Some("myapp".into());
        fs.exit = Some(ExitFilter::Fail);
        fs.tag = vec!["rust".into(), "cli".into()];
        fs.since = Some("2024-01-01".into());
        fs.until = Some("2024-12-31".into());
        fs.session = Some("ses-abc123".into());
        let args = fs.to_search_args();
        assert!(args.query.is_none());
        assert_eq!(args.project, Some("myapp".into()));
        assert_eq!(args.exit, Some(ExitFilter::Fail));
        assert_eq!(args.tag, vec!["rust".to_string(), "cli".to_string()]);
        assert_eq!(args.since, Some("2024-01-01".into()));
        assert_eq!(args.until, Some("2024-12-31".into()));
        assert_eq!(args.session, Some("ses-abc123".into()));
        assert_eq!(args.limit, Some(TUI_LIMIT));
    }

    #[test]
    fn filter_state_session_defaults_to_none() {
        let fs = FilterState::new();
        assert!(fs.session.is_none());
    }

    #[test]
    fn prev_tab_from_history_stays_history() {
        let mut app = App::new();
        assert_eq!(app.tab, Tab::History);
        app.prev_tab();
        assert_eq!(app.tab, Tab::History);
    }

    #[test]
    fn next_tab_from_history_goes_to_sessions() {
        let mut app = App::new();
        app.sessions = vec![SessionRow {
            id: "s1".into(),
            project: "p".into(),
            started_at: 1000,
            ended_at: 2000,
            command_count: 1,
            tags: vec![],
        }];
        app.next_tab();
        assert_eq!(app.tab, Tab::Sessions);
    }

    #[test]
    fn next_tab_from_sessions_stays_sessions() {
        let mut app = App::new();
        app.tab = Tab::Sessions;
        app.sessions = vec![SessionRow {
            id: "s1".into(),
            project: "p".into(),
            started_at: 1000,
            ended_at: 2000,
            command_count: 1,
            tags: vec![],
        }];
        app.next_tab();
        assert_eq!(app.tab, Tab::Sessions);
    }

    #[test]
    fn prev_tab_from_sessions_goes_to_history() {
        let mut app = App::new();
        app.tab = Tab::Sessions;
        app.prev_tab();
        assert_eq!(app.tab, Tab::History);
    }

    #[test]
    fn next_tab_with_empty_sessions_sets_reload_flag() {
        let mut app = App::new();
        assert!(app.sessions.is_empty());
        app.next_tab();
        assert!(
            app.needs_session_reload,
            "must set needs_session_reload when sessions is empty"
        );
        assert!(app.needs_session_commands_reload);
    }

    #[test]
    fn next_tab_with_existing_sessions_does_not_set_reload_flag() {
        let mut app = App::new();
        app.sessions = vec![SessionRow {
            id: "s1".into(),
            project: "p".into(),
            started_at: 1000,
            ended_at: 2000,
            command_count: 1,
            tags: vec![],
        }];
        app.next_tab();
        assert!(!app.needs_session_reload);
    }

    #[test]
    fn session_move_up_decreases_index() {
        let mut app = App::new();
        app.sessions = vec![
            SessionRow {
                id: "s1".into(),
                project: "p".into(),
                started_at: 2000,
                ended_at: 3000,
                command_count: 0,
                tags: vec![],
            },
            SessionRow {
                id: "s2".into(),
                project: "p".into(),
                started_at: 1000,
                ended_at: 2000,
                command_count: 0,
                tags: vec![],
            },
        ];
        app.session_selected = 1;
        app.session_move_up();
        assert_eq!(app.session_selected, 0);
        assert!(app.needs_session_commands_reload);
    }

    #[test]
    fn session_move_up_clamps_at_top() {
        let mut app = App::new();
        app.sessions = vec![SessionRow {
            id: "s1".into(),
            project: "p".into(),
            started_at: 2000,
            ended_at: 3000,
            command_count: 0,
            tags: vec![],
        }];
        app.session_selected = 0;
        app.session_move_up();
        assert_eq!(app.session_selected, 0);
    }

    #[test]
    fn session_move_down_increases_index() {
        let mut app = App::new();
        app.sessions = vec![
            SessionRow {
                id: "s1".into(),
                project: "p".into(),
                started_at: 2000,
                ended_at: 3000,
                command_count: 0,
                tags: vec![],
            },
            SessionRow {
                id: "s2".into(),
                project: "p".into(),
                started_at: 1000,
                ended_at: 2000,
                command_count: 0,
                tags: vec![],
            },
        ];
        app.session_selected = 0;
        app.session_move_down();
        assert_eq!(app.session_selected, 1);
        assert!(app.needs_session_commands_reload);
    }

    #[test]
    fn session_move_down_clamps_at_bottom() {
        let mut app = App::new();
        app.sessions = vec![SessionRow {
            id: "s1".into(),
            project: "p".into(),
            started_at: 2000,
            ended_at: 3000,
            command_count: 0,
            tags: vec![],
        }];
        app.session_selected = 0;
        app.session_move_down();
        assert_eq!(app.session_selected, 0);
    }

    #[test]
    fn open_session_sets_filter_and_tab() {
        let mut app = App::new();
        app.sessions = vec![SessionRow {
            id: "session-abc".into(),
            project: "proj".into(),
            started_at: 1000,
            ended_at: 2000,
            command_count: 1,
            tags: vec![],
        }];
        app.tab = Tab::Sessions;
        app.query = "something".into();
        app.selected = 5;
        app.open_session();
        assert_eq!(app.filters.session, Some("session-abc".into()));
        assert_eq!(app.tab, Tab::History);
        assert_eq!(app.query, "");
        assert_eq!(app.selected, 0);
        assert!(app.needs_history_reload);
    }

    #[test]
    fn open_session_noop_when_no_sessions() {
        let mut app = App::new();
        app.open_session();
        assert!(app.filters.session.is_none());
        assert_eq!(app.tab, Tab::History);
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
    fn reload_sessions_populates_sessions() {
        let conn = make_conn();
        seed_session(&conn, "sid-a", "proj-a", 1000);
        seed_session(&conn, "sid-b", "proj-b", 2000);
        let mut app = App::new();
        app.reload_sessions(&conn, 9999).unwrap();
        assert_eq!(app.sessions.len(), 2);
        assert!(!app.needs_session_reload);
        assert!(app.needs_session_commands_reload);
    }

    #[test]
    fn reload_session_commands_filters_by_selected_session() {
        let conn = make_conn();
        seed_session(&conn, "sid-a", "proj", 1000);
        seed_session(&conn, "sid-b", "proj", 2000);
        seed_command_for_session(&conn, "sid-a", "cmd-for-a", 1100);
        seed_command_for_session(&conn, "sid-b", "cmd-for-b", 2100);

        let mut app = App::new();
        app.reload_sessions(&conn, 9999).unwrap();

        app.session_selected = 1;
        app.reload_session_commands(&conn, 9999).unwrap();
        assert_eq!(app.session_commands.len(), 1);
        assert_eq!(app.session_commands[0].command, "cmd-for-a");

        app.session_selected = 0;
        app.reload_session_commands(&conn, 9999).unwrap();
        assert_eq!(app.session_commands.len(), 1);
        assert_eq!(app.session_commands[0].command, "cmd-for-b");
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
    fn move_up_clamps_at_oldest() {
        let conn = make_conn();
        seed(&conn, "a", 1000);
        seed(&conn, "b", 2000);
        let mut app = App::new();
        app.reload(&conn, 9999).unwrap();
        app.move_up();
        app.move_up();
        app.move_up();
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn move_down_clamps_at_newest() {
        let conn = make_conn();
        seed(&conn, "a", 1000);
        seed(&conn, "b", 2000);
        let mut app = App::new();
        app.reload(&conn, 9999).unwrap();
        app.move_down();
        app.move_down();
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn selected_command_none_when_empty() {
        let app = App::new();
        assert!(app.selected_command().is_none());
    }
}
