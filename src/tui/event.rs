use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::app::{Action, App, Tab};

pub enum Outcome {
    Continue,
    Exit,
}

pub fn handle_key(key: KeyEvent, app: &mut App) -> Outcome {
    match key.code {
        KeyCode::Esc => {
            app.action = None;
            return Outcome::Exit;
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.action = None;
            return Outcome::Exit;
        }
        KeyCode::Left => {
            app.prev_tab();
            return Outcome::Continue;
        }
        KeyCode::Right => {
            app.next_tab();
            return Outcome::Continue;
        }
        _ => {}
    }

    match app.tab {
        Tab::History => handle_history_key(key, app),
        Tab::Workspaces => handle_ws_key(key, app),
    }
}

fn handle_history_key(key: KeyEvent, app: &mut App) -> Outcome {
    match key.code {
        KeyCode::Enter => {
            if let Some(cmd) = app.selected_command() {
                app.action = Some(Action::Run(cmd.to_string()));
            }
            Outcome::Exit
        }
        KeyCode::Tab => {
            if let Some(cmd) = app.selected_command() {
                app.action = Some(Action::Edit(cmd.to_string()));
            }
            Outcome::Exit
        }
        KeyCode::Up => {
            app.move_up();
            Outcome::Continue
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.move_up();
            Outcome::Continue
        }
        KeyCode::Down => {
            app.move_down();
            Outcome::Continue
        }
        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.move_down();
            Outcome::Continue
        }
        KeyCode::Backspace => {
            app.query.pop();
            app.recompute();
            Outcome::Continue
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.query.push(c);
            app.recompute();
            Outcome::Continue
        }
        _ => Outcome::Continue,
    }
}

fn handle_ws_key(key: KeyEvent, app: &mut App) -> Outcome {
    match key.code {
        KeyCode::Enter => Outcome::Continue,
        KeyCode::Up => {
            app.ws_move_up();
            Outcome::Continue
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.ws_move_up();
            Outcome::Continue
        }
        KeyCode::Down => {
            app.ws_move_down();
            Outcome::Continue
        }
        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.ws_move_down();
            Outcome::Continue
        }
        _ => Outcome::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspaces::WorkspaceRow;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn app_with_rows() -> App {
        let mut app = App::new();
        app.all_rows = vec![
            crate::search::CommandRow {
                timestamp: 2000,
                project: String::from("p"),
                tags: String::from("[]"),
                exit_code: 0,
                duration_ms: 100,
                directory: String::from("/tmp"),
                command: String::from("git status"),
                session_id: String::from("s1"),
                workspace: None,
            },
            crate::search::CommandRow {
                timestamp: 1000,
                project: String::from("p"),
                tags: String::from("[]"),
                exit_code: 0,
                duration_ms: 100,
                directory: String::from("/tmp"),
                command: String::from("ls -la"),
                session_id: String::from("s1"),
                workspace: None,
            },
        ];
        app.recompute();
        app
    }

    fn app_with_workspaces() -> App {
        let mut app = App::new();
        app.tab = Tab::Workspaces;
        app.workspaces = vec![
            WorkspaceRow {
                name: "ws-new".into(),
                command_count: 3,
                first_ts: 1000,
                last_ts: 3000,
            },
            WorkspaceRow {
                name: "ws-old".into(),
                command_count: 1,
                first_ts: 500,
                last_ts: 1000,
            },
        ];
        app.ws_selected = 0;
        app
    }

    #[test]
    fn esc_sets_no_action_and_exits() {
        let mut app = app_with_rows();
        let outcome = handle_key(key(KeyCode::Esc), &mut app);
        assert!(app.action.is_none());
        assert!(matches!(outcome, Outcome::Exit));
    }

    #[test]
    fn ctrl_c_sets_no_action_and_exits() {
        let mut app = app_with_rows();
        let outcome = handle_key(ctrl(KeyCode::Char('c')), &mut app);
        assert!(app.action.is_none());
        assert!(matches!(outcome, Outcome::Exit));
    }

    #[test]
    fn enter_sets_run_action_and_exits() {
        let mut app = app_with_rows();
        let outcome = handle_key(key(KeyCode::Enter), &mut app);
        assert!(matches!(outcome, Outcome::Exit));
        assert!(matches!(&app.action, Some(Action::Run(cmd)) if cmd == "git status"));
    }

    #[test]
    fn tab_sets_edit_action_and_exits() {
        let mut app = app_with_rows();
        let outcome = handle_key(key(KeyCode::Tab), &mut app);
        assert!(matches!(outcome, Outcome::Exit));
        assert!(matches!(&app.action, Some(Action::Edit(cmd)) if cmd == "git status"));
    }

    #[test]
    fn up_arrow_moves_to_older_command() {
        let mut app = app_with_rows();
        app.selected = 0;
        let cmd_before = app.selected_command().map(str::to_string);
        handle_key(key(KeyCode::Up), &mut app);
        let cmd_after = app.selected_command().map(str::to_string);
        assert_ne!(cmd_before, cmd_after, "Up must change the selected command");
        assert_eq!(
            app.selected, 1,
            "Up increases selected index toward older row"
        );
    }

    #[test]
    fn down_arrow_moves_to_newer_command() {
        let mut app = app_with_rows();
        app.selected = 1;
        let cmd_before = app.selected_command().map(str::to_string);
        handle_key(key(KeyCode::Down), &mut app);
        let cmd_after = app.selected_command().map(str::to_string);
        assert_ne!(
            cmd_before, cmd_after,
            "Down must change the selected command"
        );
        assert_eq!(
            app.selected, 0,
            "Down decreases selected index toward newer row"
        );
    }

    #[test]
    fn ctrl_p_moves_to_older_command() {
        let mut app = app_with_rows();
        app.selected = 0;
        handle_key(ctrl(KeyCode::Char('p')), &mut app);
        assert_eq!(
            app.selected, 1,
            "Ctrl-P increases selected index toward older row"
        );
    }

    #[test]
    fn ctrl_n_moves_to_newer_command() {
        let mut app = app_with_rows();
        app.selected = 1;
        handle_key(ctrl(KeyCode::Char('n')), &mut app);
        assert_eq!(
            app.selected, 0,
            "Ctrl-N decreases selected index toward newer row"
        );
    }

    #[test]
    fn printable_char_appends_to_query() {
        let mut app = app_with_rows();
        handle_key(key(KeyCode::Char('g')), &mut app);
        assert_eq!(app.query, "g");
    }

    #[test]
    fn backspace_removes_last_char() {
        let mut app = app_with_rows();
        app.query = String::from("git");
        app.recompute();
        handle_key(key(KeyCode::Backspace), &mut app);
        assert_eq!(app.query, "gi");
    }

    #[test]
    fn backspace_on_empty_query_is_noop() {
        let mut app = app_with_rows();
        handle_key(key(KeyCode::Backspace), &mut app);
        assert_eq!(app.query, "");
    }

    #[test]
    fn typing_filters_results() {
        let mut app = app_with_rows();
        handle_key(key(KeyCode::Char('g')), &mut app);
        handle_key(key(KeyCode::Char('i')), &mut app);
        handle_key(key(KeyCode::Char('t')), &mut app);
        assert!(!app.filtered.is_empty());
        assert_eq!(app.all_rows[app.filtered[0]].command, "git status");
    }

    #[test]
    fn enter_on_empty_list_exits_without_action() {
        let mut app = App::new();
        let outcome = handle_key(key(KeyCode::Enter), &mut app);
        assert!(matches!(outcome, Outcome::Exit));
        assert!(app.action.is_none());
    }

    #[test]
    fn up_at_oldest_is_noop() {
        let mut app = app_with_rows();
        app.selected = 1;
        handle_key(key(KeyCode::Up), &mut app);
        assert_eq!(app.selected, 1, "Up at oldest row must not go further");
    }

    #[test]
    fn down_at_newest_is_noop() {
        let mut app = app_with_rows();
        app.selected = 0;
        handle_key(key(KeyCode::Down), &mut app);
        assert_eq!(app.selected, 0, "Down at newest row must not go further");
    }

    #[test]
    fn left_switches_to_history_tab() {
        let mut app = App::new();
        app.tab = Tab::Workspaces;
        handle_key(key(KeyCode::Left), &mut app);
        assert_eq!(app.tab, Tab::History);
    }

    #[test]
    fn right_switches_to_workspaces_tab() {
        let mut app = App::new();
        app.workspaces = vec![WorkspaceRow {
            name: "ws1".into(),
            command_count: 1,
            first_ts: 1000,
            last_ts: 2000,
        }];
        handle_key(key(KeyCode::Right), &mut app);
        assert_eq!(app.tab, Tab::Workspaces);
    }

    #[test]
    fn right_on_workspaces_stays_workspaces() {
        let mut app = App::new();
        app.tab = Tab::Workspaces;
        app.workspaces = vec![WorkspaceRow {
            name: "ws1".into(),
            command_count: 1,
            first_ts: 1000,
            last_ts: 2000,
        }];
        handle_key(key(KeyCode::Right), &mut app);
        assert_eq!(app.tab, Tab::Workspaces);
    }

    #[test]
    fn left_on_history_stays_history() {
        let mut app = App::new();
        assert_eq!(app.tab, Tab::History);
        handle_key(key(KeyCode::Left), &mut app);
        assert_eq!(app.tab, Tab::History);
    }

    #[test]
    fn ws_up_changes_ws_selected_not_history_selected() {
        let mut app = app_with_workspaces();
        app.ws_selected = 1;
        let history_selected_before = app.selected;
        handle_key(key(KeyCode::Up), &mut app);
        assert_eq!(
            app.selected, history_selected_before,
            "history selected must not change"
        );
        assert_eq!(app.ws_selected, 0);
    }

    #[test]
    fn ws_down_increases_ws_selected() {
        let mut app = app_with_workspaces();
        app.ws_selected = 0;
        handle_key(key(KeyCode::Down), &mut app);
        assert_eq!(app.ws_selected, 1);
    }

    #[test]
    fn ws_enter_is_continue_not_exit() {
        let mut app = app_with_workspaces();
        let outcome = handle_key(key(KeyCode::Enter), &mut app);
        assert!(
            matches!(outcome, Outcome::Continue),
            "Workspaces Enter must be Continue (no-op in PR2)"
        );
    }

    #[test]
    fn ws_enter_does_not_change_tab_or_filters() {
        let mut app = app_with_workspaces();
        handle_key(key(KeyCode::Enter), &mut app);
        assert_eq!(app.tab, Tab::Workspaces, "Enter must not change tab");
        assert!(
            app.filters.session.is_none(),
            "Enter must not set session filter"
        );
    }

    #[test]
    fn ws_char_does_not_modify_query() {
        let mut app = app_with_workspaces();
        handle_key(key(KeyCode::Char('g')), &mut app);
        assert_eq!(app.query, "", "typing in Workspaces must not modify query");
    }

    #[test]
    fn ws_backspace_does_not_modify_query() {
        let mut app = app_with_workspaces();
        app.query = "existing".into();
        handle_key(key(KeyCode::Backspace), &mut app);
        assert_eq!(
            app.query, "existing",
            "backspace in Workspaces must not modify query"
        );
    }

    #[test]
    fn history_enter_still_exits_with_run() {
        let mut app = app_with_rows();
        assert_eq!(app.tab, Tab::History);
        let outcome = handle_key(key(KeyCode::Enter), &mut app);
        assert!(matches!(outcome, Outcome::Exit));
        assert!(matches!(&app.action, Some(Action::Run(_))));
    }

    #[test]
    fn history_char_still_filters() {
        let mut app = app_with_rows();
        assert_eq!(app.tab, Tab::History);
        handle_key(key(KeyCode::Char('g')), &mut app);
        assert_eq!(app.query, "g");
    }
}
