use rusqlite::Connection;

use crate::cli::SearchArgs;
use crate::error::ThothError;
use crate::search::{CommandRow, ExitFilter};
use crate::tui::fuzzy;
use crate::workspaces::WorkspaceRow;

const TUI_LIMIT: usize = 5000;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tab {
    #[default]
    History,
    Workspaces,
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
    Replay(String),
}

pub struct ConfirmReplay {
    pub workspace: String,
    pub count: usize,
}

pub struct App {
    pub query: String,
    pub all_rows: Vec<CommandRow>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub filters: FilterState,
    pub action: Option<Action>,
    pub tab: Tab,
    pub workspaces: Vec<WorkspaceRow>,
    pub ws_selected: usize,
    pub ws_commands: Vec<CommandRow>,
    pub needs_ws_reload: bool,
    pub needs_ws_commands_reload: bool,
    pub needs_history_reload: bool,
    pub confirm: Option<ConfirmReplay>,
    pub replay_workspace: Option<String>,
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
            workspaces: vec![],
            ws_selected: 0,
            ws_commands: vec![],
            needs_ws_reload: false,
            needs_ws_commands_reload: false,
            needs_history_reload: false,
            confirm: None,
            replay_workspace: None,
        }
    }

    pub fn begin_replay_confirm(&mut self) {
        if let Some(ws) = self.selected_workspace() {
            let workspace = ws.name.clone();
            let count = self.ws_commands.len();
            self.confirm = Some(ConfirmReplay { workspace, count });
        }
    }

    pub fn cancel_confirm(&mut self) {
        self.confirm = None;
    }

    pub fn prev_tab(&mut self) {
        self.tab = Tab::History;
    }

    pub fn next_tab(&mut self) {
        if self.tab == Tab::History {
            self.tab = Tab::Workspaces;
            if self.workspaces.is_empty() {
                self.needs_ws_reload = true;
                self.needs_ws_commands_reload = true;
            }
        }
    }

    pub fn ws_move_up(&mut self) {
        if self.ws_selected > 0 {
            self.ws_selected -= 1;
            self.needs_ws_commands_reload = true;
        }
    }

    pub fn ws_move_down(&mut self) {
        let max = self.workspaces.len().saturating_sub(1);
        if self.ws_selected < max {
            self.ws_selected += 1;
            self.needs_ws_commands_reload = true;
        }
    }

    pub fn selected_workspace(&self) -> Option<&WorkspaceRow> {
        self.workspaces.get(self.ws_selected)
    }

    pub fn reload(&mut self, conn: &Connection, now: i64) -> Result<(), ThothError> {
        let args = self.filters.to_search_args();
        self.all_rows = crate::search::execute(&args, conn, now)?;
        self.recompute();
        Ok(())
    }

    pub fn reload_workspaces(&mut self, conn: &Connection) -> Result<(), ThothError> {
        self.workspaces = crate::workspaces::list_workspaces(conn)?;
        let max = self.workspaces.len().saturating_sub(1);
        if self.ws_selected > max {
            self.ws_selected = 0;
        }
        self.needs_ws_reload = false;
        self.needs_ws_commands_reload = true;
        Ok(())
    }

    pub fn reload_ws_commands(&mut self, conn: &Connection) -> Result<(), ThothError> {
        if let Some(ws) = self.selected_workspace() {
            let name = ws.name.clone();
            self.ws_commands = crate::workspaces::list_workspace_commands(conn, &name)?;
        } else {
            self.ws_commands.clear();
        }
        self.needs_ws_commands_reload = false;
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

    fn seed_ws_command(conn: &Connection, ws: &str, cmd: &str, ts: i64) {
        conn.execute(
            "INSERT INTO commands(command, directory, project, session_id, timestamp, exit_code, duration_ms, tags, workspace) \
             VALUES(?1, '/tmp', 'p', 's1', ?2, 0, 100, '[]', ?3)",
            rusqlite::params![cmd, ts, ws],
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
    fn next_tab_from_history_goes_to_workspaces() {
        let mut app = App::new();
        app.workspaces = vec![WorkspaceRow {
            name: "ws1".into(),
            command_count: 1,
            first_ts: 1000,
            last_ts: 2000,
        }];
        app.next_tab();
        assert_eq!(app.tab, Tab::Workspaces);
    }

    #[test]
    fn next_tab_from_workspaces_stays_workspaces() {
        let mut app = App::new();
        app.tab = Tab::Workspaces;
        app.workspaces = vec![WorkspaceRow {
            name: "ws1".into(),
            command_count: 1,
            first_ts: 1000,
            last_ts: 2000,
        }];
        app.next_tab();
        assert_eq!(app.tab, Tab::Workspaces);
    }

    #[test]
    fn prev_tab_from_workspaces_goes_to_history() {
        let mut app = App::new();
        app.tab = Tab::Workspaces;
        app.prev_tab();
        assert_eq!(app.tab, Tab::History);
    }

    #[test]
    fn next_tab_with_empty_workspaces_sets_reload_flag() {
        let mut app = App::new();
        assert!(app.workspaces.is_empty());
        app.next_tab();
        assert!(
            app.needs_ws_reload,
            "must set needs_ws_reload when workspaces is empty"
        );
        assert!(app.needs_ws_commands_reload);
    }

    #[test]
    fn next_tab_with_existing_workspaces_does_not_set_reload_flag() {
        let mut app = App::new();
        app.workspaces = vec![WorkspaceRow {
            name: "ws1".into(),
            command_count: 1,
            first_ts: 1000,
            last_ts: 2000,
        }];
        app.next_tab();
        assert!(!app.needs_ws_reload);
    }

    #[test]
    fn ws_move_up_decreases_index() {
        let mut app = App::new();
        app.workspaces = vec![
            WorkspaceRow {
                name: "ws-a".into(),
                command_count: 2,
                first_ts: 1000,
                last_ts: 3000,
            },
            WorkspaceRow {
                name: "ws-b".into(),
                command_count: 1,
                first_ts: 2000,
                last_ts: 2000,
            },
        ];
        app.ws_selected = 1;
        app.ws_move_up();
        assert_eq!(app.ws_selected, 0);
        assert!(app.needs_ws_commands_reload);
    }

    #[test]
    fn ws_move_up_clamps_at_top() {
        let mut app = App::new();
        app.workspaces = vec![WorkspaceRow {
            name: "ws1".into(),
            command_count: 1,
            first_ts: 1000,
            last_ts: 2000,
        }];
        app.ws_selected = 0;
        app.ws_move_up();
        assert_eq!(app.ws_selected, 0);
    }

    #[test]
    fn ws_move_down_increases_index() {
        let mut app = App::new();
        app.workspaces = vec![
            WorkspaceRow {
                name: "ws-a".into(),
                command_count: 2,
                first_ts: 1000,
                last_ts: 3000,
            },
            WorkspaceRow {
                name: "ws-b".into(),
                command_count: 1,
                first_ts: 2000,
                last_ts: 2000,
            },
        ];
        app.ws_selected = 0;
        app.ws_move_down();
        assert_eq!(app.ws_selected, 1);
        assert!(app.needs_ws_commands_reload);
    }

    #[test]
    fn ws_move_down_clamps_at_bottom() {
        let mut app = App::new();
        app.workspaces = vec![WorkspaceRow {
            name: "ws1".into(),
            command_count: 1,
            first_ts: 1000,
            last_ts: 2000,
        }];
        app.ws_selected = 0;
        app.ws_move_down();
        assert_eq!(app.ws_selected, 0);
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
    fn reload_workspaces_populates_workspaces() {
        let conn = make_conn();
        seed_ws_command(&conn, "ws-a", "cmd1", 1000);
        seed_ws_command(&conn, "ws-b", "cmd2", 2000);
        let mut app = App::new();
        app.reload_workspaces(&conn).unwrap();
        assert_eq!(app.workspaces.len(), 2);
        assert!(!app.needs_ws_reload);
        assert!(app.needs_ws_commands_reload);
    }

    #[test]
    fn reload_ws_commands_filters_by_selected_workspace() {
        let conn = make_conn();
        seed_ws_command(&conn, "ws-a", "cmd-for-a", 1100);
        seed_ws_command(&conn, "ws-b", "cmd-for-b", 2100);

        let mut app = App::new();
        app.reload_workspaces(&conn).unwrap();

        app.ws_selected = 1;
        app.reload_ws_commands(&conn).unwrap();
        assert_eq!(app.ws_commands.len(), 1);
        assert_eq!(app.ws_commands[0].command, "cmd-for-a");

        app.ws_selected = 0;
        app.reload_ws_commands(&conn).unwrap();
        assert_eq!(app.ws_commands.len(), 1);
        assert_eq!(app.ws_commands[0].command, "cmd-for-b");
    }

    #[test]
    fn reload_ws_commands_returns_asc_order() {
        let conn = make_conn();
        seed_ws_command(&conn, "ws-x", "first", 1000);
        seed_ws_command(&conn, "ws-x", "second", 2000);
        seed_ws_command(&conn, "ws-x", "third", 3000);

        let mut app = App::new();
        app.reload_workspaces(&conn).unwrap();
        app.ws_selected = 0;
        app.reload_ws_commands(&conn).unwrap();
        assert_eq!(app.ws_commands.len(), 3);
        assert_eq!(app.ws_commands[0].command, "first");
        assert_eq!(app.ws_commands[1].command, "second");
        assert_eq!(app.ws_commands[2].command, "third");
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

    fn app_with_ws(name: &str, cmd_count: usize) -> App {
        let mut app = App::new();
        app.tab = crate::tui::app::Tab::Workspaces;
        app.workspaces = vec![WorkspaceRow {
            name: name.into(),
            command_count: cmd_count as i64,
            first_ts: 1000,
            last_ts: 2000,
        }];
        app.ws_selected = 0;
        app.ws_commands = (0..cmd_count)
            .map(|i| crate::search::CommandRow {
                command: format!("cmd-{i}"),
                directory: "/tmp".into(),
                project: "p".into(),
                session_id: "s1".into(),
                timestamp: 1000 + i as i64,
                exit_code: 0,
                duration_ms: 100,
                tags: "[]".into(),
                workspace: Some(name.into()),
            })
            .collect();
        app
    }

    #[test]
    fn begin_replay_confirm_sets_confirm_with_workspace_and_count() {
        let mut app = app_with_ws("demo", 3);
        app.begin_replay_confirm();
        let c = app.confirm.as_ref().unwrap();
        assert_eq!(c.workspace, "demo");
        assert_eq!(c.count, 3);
    }

    #[test]
    fn begin_replay_confirm_no_workspace_does_nothing() {
        let mut app = App::new();
        app.begin_replay_confirm();
        assert!(app.confirm.is_none());
    }

    #[test]
    fn cancel_confirm_clears_confirm() {
        let mut app = app_with_ws("ws-x", 2);
        app.begin_replay_confirm();
        assert!(app.confirm.is_some());
        app.cancel_confirm();
        assert!(app.confirm.is_none());
    }
}
