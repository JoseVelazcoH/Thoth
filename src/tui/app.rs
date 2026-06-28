use rusqlite::Connection;

use crate::cli::SearchArgs;
use crate::error::ThothError;
use crate::search::{CommandRow, ExitFilter};
use crate::theme::Theme;
use crate::tui::fuzzy;
use crate::workspaces::WorkspaceRow;

const TUI_LIMIT: usize = 5000;
const LABEL_MAX: usize = 40;

pub fn parse_query(input: &str, now: i64) -> (FilterState, String) {
    let mut filters = FilterState::new();
    let mut free_words: Vec<&str> = Vec::new();

    for token in input.split_whitespace() {
        if let Some((key, value)) = token.split_once(':') {
            if value.is_empty() {
                free_words.push(token);
                continue;
            }
            match key {
                "project" | "p" => {
                    filters.project = Some(value.to_string());
                }
                "tag" | "t" => {
                    let v = value.to_string();
                    if !filters.tag.contains(&v) {
                        filters.tag.push(v);
                    }
                }
                "exit" | "e" => {
                    let parsed = match value.to_lowercase().as_str() {
                        "ok" => Some(ExitFilter::Ok),
                        "fail" => Some(ExitFilter::Fail),
                        "any" => Some(ExitFilter::Any),
                        _ => None,
                    };
                    match parsed {
                        Some(ef) => filters.exit = Some(ef),
                        None => free_words.push(token),
                    }
                }
                "since" => {
                    if crate::search::parse_date(value, now).is_ok() {
                        filters.since = Some(value.to_string());
                    } else {
                        free_words.push(token);
                    }
                }
                "until" => {
                    if crate::search::parse_date(value, now).is_ok() {
                        filters.until = Some(value.to_string());
                    } else {
                        free_words.push(token);
                    }
                }
                "dur" | "duration" => {
                    filters.duration = Some(value.to_string());
                }
                _ => {
                    free_words.push(token);
                }
            }
        } else {
            free_words.push(token);
        }
    }

    (filters, free_words.join(" "))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Tab {
    #[default]
    History,
    Workspaces,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Mode {
    #[default]
    Insert,
    Normal,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum WsPane {
    #[default]
    List,
    Commands,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeleteOrigin {
    History,
    Workspace,
}

pub struct ConfirmReplay {
    pub workspace: String,
    pub count: usize,
}

pub struct ConfirmDelete {
    pub id: i64,
    pub label: String,
    pub origin: DeleteOrigin,
}

pub enum Confirm {
    Replay(ConfirmReplay),
    Delete(ConfirmDelete),
}

pub struct EditState {
    pub id: i64,
    pub buffer: String,
}

#[derive(PartialEq, Debug)]
pub struct FilterState {
    pub project: Option<String>,
    pub tag: Vec<String>,
    pub exit: Option<ExitFilter>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub session: Option<String>,
    pub duration: Option<String>,
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
            duration: None,
        }
    }

    pub fn to_search_args(&self) -> SearchArgs {
        SearchArgs {
            query: None,
            project: self.project.clone(),
            tag: self.tag.clone(),
            exit: self.exit.clone(),
            duration: self.duration.clone(),
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

pub struct App {
    pub query: String,
    pub fuzzy_query: String,
    pub cmdline: Option<String>,
    pub all_rows: Vec<CommandRow>,
    pub filtered: Vec<usize>,
    pub selected: usize,
    pub filters: FilterState,
    pub action: Option<Action>,
    pub tab: Tab,
    pub mode: Mode,
    pub show_help: bool,
    pub workspaces: Vec<WorkspaceRow>,
    pub ws_selected: usize,
    pub ws_commands: Vec<CommandRow>,
    pub ws_pane: WsPane,
    pub ws_cmd_selected: usize,
    pub needs_ws_reload: bool,
    pub needs_ws_commands_reload: bool,
    pub needs_history_reload: bool,
    pub confirm: Option<Confirm>,
    pub replay_workspace: Option<String>,
    pub pending_delete: Option<(i64, DeleteOrigin)>,
    pub edit: Option<EditState>,
    pub pending_edit: Option<(i64, String)>,
    pub theme: Theme,
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
            fuzzy_query: String::new(),
            cmdline: None,
            all_rows: vec![],
            filtered: vec![],
            selected: 0,
            filters: FilterState::new(),
            action: None,
            tab: Tab::History,
            mode: Mode::Insert,
            show_help: false,
            workspaces: vec![],
            ws_selected: 0,
            ws_commands: vec![],
            ws_pane: WsPane::List,
            ws_cmd_selected: 0,
            needs_ws_reload: false,
            needs_ws_commands_reload: false,
            needs_history_reload: false,
            confirm: None,
            replay_workspace: None,
            pending_delete: None,
            edit: None,
            pending_edit: None,
            theme: Theme::default(),
        }
    }

    pub fn enter_normal_mode(&mut self) {
        self.mode = Mode::Normal;
    }

    pub fn enter_insert_mode(&mut self) {
        self.mode = Mode::Insert;
    }

    pub fn toggle_ws_pane(&mut self) {
        match self.ws_pane {
            WsPane::List => {
                self.ws_pane = WsPane::Commands;
                let max = self.ws_commands.len().saturating_sub(1);
                if self.ws_cmd_selected > max {
                    self.ws_cmd_selected = 0;
                }
            }
            WsPane::Commands => {
                self.ws_pane = WsPane::List;
            }
        }
    }

    pub fn ws_cmd_move_up(&mut self) {
        if self.ws_cmd_selected > 0 {
            self.ws_cmd_selected -= 1;
        }
    }

    pub fn ws_cmd_move_down(&mut self) {
        let max = self.ws_commands.len().saturating_sub(1);
        if self.ws_cmd_selected < max {
            self.ws_cmd_selected += 1;
        }
    }

    pub fn selected_history_id(&self) -> Option<i64> {
        let idx = self.filtered.get(self.selected)?;
        self.all_rows.get(*idx).map(|r| r.id)
    }

    pub fn selected_ws_command_id(&self) -> Option<i64> {
        self.ws_commands.get(self.ws_cmd_selected).map(|r| r.id)
    }

    pub fn begin_delete_confirm_history(&mut self) {
        if let Some(id) = self.selected_history_id() {
            let label = self
                .filtered
                .get(self.selected)
                .and_then(|idx| self.all_rows.get(*idx))
                .map(|r| truncate_label(&r.command))
                .unwrap_or_default();
            self.confirm = Some(Confirm::Delete(ConfirmDelete {
                id,
                label,
                origin: DeleteOrigin::History,
            }));
        }
    }

    pub fn begin_delete_confirm_ws(&mut self) {
        if let Some(id) = self.selected_ws_command_id() {
            let label = self
                .ws_commands
                .get(self.ws_cmd_selected)
                .map(|r| truncate_label(&r.command))
                .unwrap_or_default();
            self.confirm = Some(Confirm::Delete(ConfirmDelete {
                id,
                label,
                origin: DeleteOrigin::Workspace,
            }));
        }
    }

    pub fn begin_replay_confirm(&mut self) {
        if let Some(ws) = self.selected_workspace() {
            let workspace = ws.name.clone();
            let count = self.ws_commands.len();
            self.confirm = Some(Confirm::Replay(ConfirmReplay { workspace, count }));
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
        let max = self.ws_commands.len().saturating_sub(1);
        if self.ws_cmd_selected > max {
            self.ws_cmd_selected = 0;
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

    pub fn apply_query(&mut self, now: i64) {
        let (filters, free) = parse_query(&self.query, now);
        let changed = filters != self.filters;
        self.filters = filters;
        self.fuzzy_query = free.clone();
        self.query = free;
        if changed {
            self.needs_history_reload = true;
        }
        self.recompute();
    }

    pub fn open_cmdline(&mut self) {
        self.cmdline = Some(String::new());
    }

    pub fn cmdline_push(&mut self, c: char) {
        if let Some(ref mut buf) = self.cmdline {
            buf.push(c);
        }
    }

    pub fn cmdline_backspace(&mut self) {
        if let Some(ref mut buf) = self.cmdline {
            buf.pop();
        }
    }

    pub fn cmdline_cancel(&mut self) {
        self.cmdline = None;
    }

    pub fn cmdline_submit(&mut self, now: i64) {
        let buf = match self.cmdline.take() {
            Some(b) => b,
            None => return,
        };
        let (filters, free) = parse_query(&buf, now);
        let changed = filters != self.filters;
        self.filters = filters;
        self.query = free;
        if changed {
            self.needs_history_reload = true;
        }
        self.recompute();
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

    pub fn begin_edit_history(&mut self) {
        if let Some(id) = self.selected_history_id() {
            let buffer = self
                .filtered
                .get(self.selected)
                .and_then(|idx| self.all_rows.get(*idx))
                .map(|r| r.command.clone())
                .unwrap_or_default();
            self.edit = Some(EditState { id, buffer });
        }
    }

    pub fn begin_edit_ws(&mut self) {
        if let Some(id) = self.selected_ws_command_id() {
            let buffer = self
                .ws_commands
                .get(self.ws_cmd_selected)
                .map(|r| r.command.clone())
                .unwrap_or_default();
            self.edit = Some(EditState { id, buffer });
        }
    }

    pub fn edit_push(&mut self, c: char) {
        if let Some(ref mut es) = self.edit {
            es.buffer.push(c);
        }
    }

    pub fn edit_backspace(&mut self) {
        if let Some(ref mut es) = self.edit {
            es.buffer.pop();
        }
    }

    pub fn edit_cancel(&mut self) {
        self.edit = None;
    }

    pub fn edit_commit(&mut self) {
        if let Some(es) = self.edit.take() {
            self.pending_edit = Some((es.id, es.buffer));
        }
    }
}

fn truncate_label(s: &str) -> String {
    if s.chars().count() <= LABEL_MAX {
        s.to_string()
    } else {
        let end = s
            .char_indices()
            .nth(LABEL_MAX.saturating_sub(1))
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        format!("{}...", &s[..end])
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
                id: (i + 1) as i64,
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
        assert!(matches!(c, Confirm::Replay(r) if r.workspace == "demo" && r.count == 3));
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

    #[test]
    fn enter_normal_mode_sets_normal() {
        let mut app = App::new();
        assert_eq!(app.mode, Mode::Insert);
        app.enter_normal_mode();
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn enter_insert_mode_sets_insert() {
        let mut app = App::new();
        app.enter_normal_mode();
        assert_eq!(app.mode, Mode::Normal);
        app.enter_insert_mode();
        assert_eq!(app.mode, Mode::Insert);
    }

    #[test]
    fn toggle_ws_pane_list_to_commands() {
        let mut app = App::new();
        assert_eq!(app.ws_pane, WsPane::List);
        app.toggle_ws_pane();
        assert_eq!(app.ws_pane, WsPane::Commands);
    }

    #[test]
    fn toggle_ws_pane_commands_to_list() {
        let mut app = App::new();
        app.ws_pane = WsPane::Commands;
        app.toggle_ws_pane();
        assert_eq!(app.ws_pane, WsPane::List);
    }

    #[test]
    fn toggle_ws_pane_clamps_cmd_selected() {
        let mut app = app_with_ws("demo", 2);
        app.ws_cmd_selected = 5;
        app.toggle_ws_pane();
        assert!(
            app.ws_cmd_selected <= 1,
            "ws_cmd_selected must clamp to valid range after toggling to Commands"
        );
    }

    #[test]
    fn ws_cmd_move_up_decreases() {
        let mut app = app_with_ws("demo", 3);
        app.ws_pane = WsPane::Commands;
        app.ws_cmd_selected = 2;
        app.ws_cmd_move_up();
        assert_eq!(app.ws_cmd_selected, 1);
    }

    #[test]
    fn ws_cmd_move_up_clamps_at_zero() {
        let mut app = app_with_ws("demo", 3);
        app.ws_pane = WsPane::Commands;
        app.ws_cmd_selected = 0;
        app.ws_cmd_move_up();
        assert_eq!(app.ws_cmd_selected, 0);
    }

    #[test]
    fn ws_cmd_move_down_increases() {
        let mut app = app_with_ws("demo", 3);
        app.ws_pane = WsPane::Commands;
        app.ws_cmd_selected = 0;
        app.ws_cmd_move_down();
        assert_eq!(app.ws_cmd_selected, 1);
    }

    #[test]
    fn ws_cmd_move_down_clamps_at_max() {
        let mut app = app_with_ws("demo", 3);
        app.ws_pane = WsPane::Commands;
        app.ws_cmd_selected = 2;
        app.ws_cmd_move_down();
        assert_eq!(app.ws_cmd_selected, 2);
    }

    #[test]
    fn selected_history_id_returns_correct_id() {
        let conn = make_conn();
        seed(&conn, "git status", 2000);
        seed(&conn, "ls -la", 1000);
        let mut app = App::new();
        app.reload(&conn, 9999).unwrap();
        app.selected = 0;
        let id = app.selected_history_id().unwrap();
        assert!(id > 0);
        let row = &app.all_rows[app.filtered[0]];
        assert_eq!(id, row.id);
    }

    #[test]
    fn selected_history_id_none_when_empty() {
        let app = App::new();
        assert!(app.selected_history_id().is_none());
    }

    #[test]
    fn selected_ws_command_id_returns_correct_id() {
        let app = app_with_ws("demo", 3);
        let id = app.selected_ws_command_id().unwrap();
        assert_eq!(id, app.ws_commands[0].id);
    }

    #[test]
    fn selected_ws_command_id_uses_ws_cmd_selected() {
        let mut app = app_with_ws("demo", 3);
        app.ws_cmd_selected = 2;
        let id = app.selected_ws_command_id().unwrap();
        assert_eq!(id, app.ws_commands[2].id);
    }

    #[test]
    fn begin_delete_confirm_history_sets_delete_confirm() {
        let conn = make_conn();
        seed(&conn, "git status", 2000);
        let mut app = App::new();
        app.reload(&conn, 9999).unwrap();
        app.selected = 0;
        let expected_id = app.selected_history_id().unwrap();
        app.begin_delete_confirm_history();
        let c = app.confirm.as_ref().unwrap();
        assert!(
            matches!(c, Confirm::Delete(d) if d.id == expected_id && matches!(d.origin, DeleteOrigin::History))
        );
    }

    #[test]
    fn begin_delete_confirm_history_no_rows_does_nothing() {
        let mut app = App::new();
        app.begin_delete_confirm_history();
        assert!(app.confirm.is_none());
    }

    #[test]
    fn begin_delete_confirm_ws_sets_delete_confirm() {
        let mut app = app_with_ws("demo", 2);
        app.ws_cmd_selected = 0;
        let expected_id = app.ws_commands[0].id;
        app.begin_delete_confirm_ws();
        let c = app.confirm.as_ref().unwrap();
        assert!(
            matches!(c, Confirm::Delete(d) if d.id == expected_id && matches!(d.origin, DeleteOrigin::Workspace))
        );
    }

    #[test]
    fn begin_delete_confirm_ws_no_commands_does_nothing() {
        let mut app = App::new();
        app.begin_delete_confirm_ws();
        assert!(app.confirm.is_none());
    }

    #[test]
    fn cancel_confirm_clears_delete_confirm() {
        let mut app = app_with_ws("demo", 2);
        app.begin_delete_confirm_ws();
        assert!(app.confirm.is_some());
        app.cancel_confirm();
        assert!(app.confirm.is_none());
    }

    #[test]
    fn begin_edit_history_sets_edit_with_correct_id_and_buffer() {
        let conn = make_conn();
        seed(&conn, "git status", 2000);
        let mut app = App::new();
        app.reload(&conn, 9999).unwrap();
        app.selected = 0;
        let expected_id = app.selected_history_id().unwrap();
        app.begin_edit_history();
        let es = app.edit.as_ref().unwrap();
        assert_eq!(es.id, expected_id);
        assert_eq!(es.buffer, "git status");
    }

    #[test]
    fn begin_edit_history_no_selection_is_noop() {
        let mut app = App::new();
        app.begin_edit_history();
        assert!(app.edit.is_none());
    }

    #[test]
    fn begin_edit_ws_sets_edit_with_correct_id_and_buffer() {
        let mut app = app_with_ws("demo", 2);
        app.ws_cmd_selected = 1;
        let expected_id = app.ws_commands[1].id;
        let expected_buf = app.ws_commands[1].command.clone();
        app.begin_edit_ws();
        let es = app.edit.as_ref().unwrap();
        assert_eq!(es.id, expected_id);
        assert_eq!(es.buffer, expected_buf);
    }

    #[test]
    fn begin_edit_ws_no_selection_is_noop() {
        let mut app = App::new();
        app.begin_edit_ws();
        assert!(app.edit.is_none());
    }

    #[test]
    fn edit_push_appends_char_to_buffer() {
        let mut app = App::new();
        app.edit = Some(EditState {
            id: 1,
            buffer: "git".into(),
        });
        app.edit_push(' ');
        app.edit_push('s');
        assert_eq!(app.edit.as_ref().unwrap().buffer, "git s");
    }

    #[test]
    fn edit_backspace_removes_last_char() {
        let mut app = App::new();
        app.edit = Some(EditState {
            id: 1,
            buffer: "git status".into(),
        });
        app.edit_backspace();
        assert_eq!(app.edit.as_ref().unwrap().buffer, "git statu");
    }

    #[test]
    fn edit_cancel_clears_edit() {
        let mut app = App::new();
        app.edit = Some(EditState {
            id: 1,
            buffer: "something".into(),
        });
        app.edit_cancel();
        assert!(app.edit.is_none());
    }

    const PARSE_NOW: i64 = 1_700_000_000;

    #[test]
    fn parse_query_project_and_free_text() {
        let (fs, free) = parse_query("project:thoth cargo", PARSE_NOW);
        assert_eq!(fs.project, Some("thoth".into()));
        assert_eq!(free, "cargo");
    }

    #[test]
    fn parse_query_short_project_alias() {
        let (fs, free) = parse_query("p:web t:rust t:cli build", PARSE_NOW);
        assert_eq!(fs.project, Some("web".into()));
        assert!(fs.tag.contains(&"rust".to_string()));
        assert!(fs.tag.contains(&"cli".to_string()));
        assert_eq!(free, "build");
    }

    #[test]
    fn parse_query_exit_fail() {
        let (fs, free) = parse_query("exit:fail", PARSE_NOW);
        assert_eq!(fs.exit, Some(ExitFilter::Fail));
        assert!(free.is_empty());
    }

    #[test]
    fn parse_query_exit_ok() {
        let (fs, free) = parse_query("exit:ok", PARSE_NOW);
        assert_eq!(fs.exit, Some(ExitFilter::Ok));
        assert!(free.is_empty());
    }

    #[test]
    fn parse_query_exit_bogus_becomes_free_text() {
        let (fs, free) = parse_query("exit:bogus x", PARSE_NOW);
        assert!(fs.exit.is_none());
        assert!(free.contains("exit:bogus"));
        assert!(free.contains('x'));
    }

    #[test]
    fn parse_query_since_valid() {
        let (fs, free) = parse_query("since:today ls", PARSE_NOW);
        assert_eq!(fs.since, Some("today".into()));
        assert_eq!(free, "ls");
    }

    #[test]
    fn parse_query_since_invalid_becomes_free_text() {
        let (fs, free) = parse_query("since:notadate ls", PARSE_NOW);
        assert!(fs.since.is_none());
        assert!(free.contains("since:notadate"));
    }

    #[test]
    fn parse_query_duration() {
        let (fs, free) = parse_query("dur:>30 ls", PARSE_NOW);
        assert_eq!(fs.duration, Some(">30".into()));
        assert_eq!(free, "ls");
    }

    #[test]
    fn parse_query_empty_value_becomes_free_text() {
        let (fs, free) = parse_query("project: ls", PARSE_NOW);
        assert!(fs.project.is_none());
        assert!(free.contains("project:"));
        assert!(free.contains("ls"));
    }

    #[test]
    fn parse_query_only_free_text() {
        let input = "git status";
        let (fs, free) = parse_query(input, PARSE_NOW);
        assert!(fs.project.is_none());
        assert!(fs.exit.is_none());
        assert!(fs.tag.is_empty());
        assert!(fs.since.is_none());
        assert!(fs.until.is_none());
        assert!(fs.duration.is_none());
        assert_eq!(free, input);
    }

    #[test]
    fn parse_query_combined_project_exit_free() {
        let (fs, free) = parse_query("project:thoth exit:fail cargo", PARSE_NOW);
        assert_eq!(fs.project, Some("thoth".into()));
        assert_eq!(fs.exit, Some(ExitFilter::Fail));
        assert_eq!(free, "cargo");
    }

    #[test]
    fn apply_query_sets_filters_and_needs_reload_when_filters_change() {
        let mut app = App::new();
        app.query = "project:myapp".into();
        app.apply_query(PARSE_NOW);
        assert_eq!(app.filters.project, Some("myapp".into()));
        assert!(app.needs_history_reload);
        assert!(
            app.query.is_empty(),
            "query must become the free text after parsing"
        );
    }

    #[test]
    fn apply_query_does_not_set_needs_reload_when_only_free_text_changes() {
        let mut app = App::new();
        app.query = "cargo".into();
        app.apply_query(PARSE_NOW);
        assert!(!app.needs_history_reload);
        assert_eq!(app.query, "cargo");
    }

    #[test]
    fn apply_query_recompute_uses_free_text_as_query() {
        let conn = make_conn();
        seed(&conn, "docker run nginx", 1000);
        seed(&conn, "ls -la", 2000);
        let mut app = App::new();
        app.reload(&conn, 9999).unwrap();
        app.query = "docker".into();
        app.apply_query(PARSE_NOW);
        assert_eq!(app.filtered.len(), 1);
        assert_eq!(app.all_rows[app.filtered[0]].command, "docker run nginx");
    }

    #[test]
    fn to_search_args_passes_duration_through() {
        let mut fs = FilterState::new();
        fs.duration = Some(">30".into());
        let args = fs.to_search_args();
        assert_eq!(args.duration, Some(">30".into()));
    }

    const CMDLINE_NOW: i64 = 1_700_000_000;

    #[test]
    fn open_cmdline_sets_some_empty_string() {
        let mut app = App::new();
        assert!(app.cmdline.is_none());
        app.open_cmdline();
        assert_eq!(app.cmdline, Some(String::new()));
    }

    #[test]
    fn cmdline_push_appends_char() {
        let mut app = App::new();
        app.open_cmdline();
        app.cmdline_push('p');
        app.cmdline_push(':');
        assert_eq!(app.cmdline, Some("p:".to_string()));
    }

    #[test]
    fn cmdline_backspace_removes_last_char() {
        let mut app = App::new();
        app.open_cmdline();
        app.cmdline_push('a');
        app.cmdline_push('b');
        app.cmdline_backspace();
        assert_eq!(app.cmdline, Some("a".to_string()));
    }

    #[test]
    fn cmdline_cancel_closes_without_applying() {
        let mut app = App::new();
        app.filters.project = Some("old".into());
        app.open_cmdline();
        app.cmdline_push('p');
        app.cmdline_push(':');
        app.cmdline_push('x');
        app.cmdline_cancel();
        assert!(app.cmdline.is_none());
        assert_eq!(
            app.filters.project,
            Some("old".into()),
            "cancel must not change filters"
        );
    }

    #[test]
    fn cmdline_submit_project_and_free_text() {
        let conn = make_conn();
        seed(&conn, "docker run nginx", 1000);
        seed(&conn, "ls -la", 2000);
        let mut app = App::new();
        app.reload(&conn, 9999).unwrap();
        app.open_cmdline();
        for c in "project:thoth cargo".chars() {
            app.cmdline_push(c);
        }
        app.cmdline_submit(CMDLINE_NOW);
        assert_eq!(app.filters.project, Some("thoth".into()));
        assert_eq!(app.query, "cargo");
        assert!(app.needs_history_reload);
        assert!(app.cmdline.is_none());
    }

    #[test]
    fn cmdline_submit_only_free_text_does_not_set_reload() {
        let mut app = App::new();
        app.open_cmdline();
        for c in "cargo".chars() {
            app.cmdline_push(c);
        }
        app.cmdline_submit(CMDLINE_NOW);
        assert_eq!(app.query, "cargo");
        assert!(
            !app.needs_history_reload,
            "free-text only must not trigger reload"
        );
        assert!(app.cmdline.is_none());
    }

    #[test]
    fn cmdline_submit_empty_clears_filters() {
        let mut app = App::new();
        app.filters.project = Some("myapp".into());
        app.needs_history_reload = false;
        app.open_cmdline();
        app.cmdline_submit(CMDLINE_NOW);
        assert!(
            app.filters.project.is_none(),
            "empty submit must clear filters"
        );
        assert_eq!(app.query, "");
        assert!(
            app.needs_history_reload,
            "clearing filters must trigger reload"
        );
        assert!(app.cmdline.is_none());
    }

    #[test]
    fn recompute_ranks_on_raw_query() {
        let conn = make_conn();
        seed(&conn, "git status", 2000);
        seed(&conn, "ls -la", 1000);
        let mut app = App::new();
        app.reload(&conn, 9999).unwrap();
        app.query = "git".into();
        app.recompute();
        assert!(!app.filtered.is_empty());
        assert_eq!(app.all_rows[app.filtered[0]].command, "git status");
    }

    #[test]
    fn edit_commit_sets_pending_edit_and_clears_edit() {
        let mut app = App::new();
        app.edit = Some(EditState {
            id: 42,
            buffer: "corrected cmd".into(),
        });
        app.edit_commit();
        assert!(app.edit.is_none());
        assert_eq!(app.pending_edit, Some((42, "corrected cmd".to_string())));
    }
}
