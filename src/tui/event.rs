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
        Tab::Sessions => handle_sessions_key(key, app),
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

fn handle_sessions_key(key: KeyEvent, app: &mut App) -> Outcome {
    match key.code {
        KeyCode::Enter => {
            app.open_session();
            Outcome::Continue
        }
        KeyCode::Up => {
            app.session_move_up();
            Outcome::Continue
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.session_move_up();
            Outcome::Continue
        }
        KeyCode::Down => {
            app.session_move_down();
            Outcome::Continue
        }
        KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.session_move_down();
            Outcome::Continue
        }
        _ => Outcome::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::SessionRow;
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
            },
        ];
        app.recompute();
        app
    }

    fn app_with_sessions() -> App {
        let mut app = App::new();
        app.tab = Tab::Sessions;
        app.sessions = vec![
            SessionRow {
                id: "sid-new".into(),
                project: "proj".into(),
                started_at: 2000,
                ended_at: 3000,
                command_count: 1,
                tags: vec![],
            },
            SessionRow {
                id: "sid-old".into(),
                project: "proj".into(),
                started_at: 1000,
                ended_at: 2000,
                command_count: 1,
                tags: vec![],
            },
        ];
        app.session_selected = 0;
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
        app.tab = Tab::Sessions;
        handle_key(key(KeyCode::Left), &mut app);
        assert_eq!(app.tab, Tab::History);
    }

    #[test]
    fn right_switches_to_sessions_tab() {
        let mut app = App::new();
        app.sessions = vec![SessionRow {
            id: "s1".into(),
            project: "p".into(),
            started_at: 1000,
            ended_at: 2000,
            command_count: 0,
            tags: vec![],
        }];
        handle_key(key(KeyCode::Right), &mut app);
        assert_eq!(app.tab, Tab::Sessions);
    }

    #[test]
    fn right_on_sessions_stays_sessions() {
        let mut app = App::new();
        app.tab = Tab::Sessions;
        app.sessions = vec![SessionRow {
            id: "s1".into(),
            project: "p".into(),
            started_at: 1000,
            ended_at: 2000,
            command_count: 0,
            tags: vec![],
        }];
        handle_key(key(KeyCode::Right), &mut app);
        assert_eq!(app.tab, Tab::Sessions);
    }

    #[test]
    fn left_on_history_stays_history() {
        let mut app = App::new();
        assert_eq!(app.tab, Tab::History);
        handle_key(key(KeyCode::Left), &mut app);
        assert_eq!(app.tab, Tab::History);
    }

    #[test]
    fn sessions_up_changes_session_selected_not_history_selected() {
        let mut app = app_with_sessions();
        let history_selected_before = app.selected;
        handle_key(key(KeyCode::Up), &mut app);
        assert_eq!(
            app.selected, history_selected_before,
            "history selected must not change"
        );
        assert_eq!(app.session_selected, 1);
    }

    #[test]
    fn sessions_down_decreases_session_selected() {
        let mut app = app_with_sessions();
        app.session_selected = 1;
        handle_key(key(KeyCode::Down), &mut app);
        assert_eq!(app.session_selected, 0);
    }

    #[test]
    fn sessions_enter_is_continue_not_exit() {
        let mut app = app_with_sessions();
        let outcome = handle_key(key(KeyCode::Enter), &mut app);
        assert!(
            matches!(outcome, Outcome::Continue),
            "Sessions Enter must be Continue"
        );
    }

    #[test]
    fn sessions_enter_sets_history_reload_and_switches_tab() {
        let mut app = app_with_sessions();
        handle_key(key(KeyCode::Enter), &mut app);
        assert!(app.needs_history_reload);
        assert_eq!(app.tab, Tab::History);
        assert_eq!(app.filters.session, Some("sid-new".into()));
    }

    #[test]
    fn sessions_char_does_not_modify_query() {
        let mut app = app_with_sessions();
        handle_key(key(KeyCode::Char('g')), &mut app);
        assert_eq!(app.query, "", "typing in Sessions must not modify query");
    }

    #[test]
    fn sessions_backspace_does_not_modify_query() {
        let mut app = app_with_sessions();
        app.query = "existing".into();
        handle_key(key(KeyCode::Backspace), &mut app);
        assert_eq!(
            app.query, "existing",
            "backspace in Sessions must not modify query"
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
