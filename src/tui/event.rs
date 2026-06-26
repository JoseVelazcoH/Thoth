use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::app::{Action, App};

pub enum Outcome {
    Continue,
    Exit,
}

pub fn handle_key(key: KeyEvent, app: &mut App) -> Outcome {
    match key.code {
        KeyCode::Esc => {
            app.action = None;
            Outcome::Exit
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.action = None;
            Outcome::Exit
        }
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

#[cfg(test)]
mod tests {
    use super::*;
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
    fn up_arrow_moves_selection_up() {
        let mut app = app_with_rows();
        app.selected = 1;
        handle_key(key(KeyCode::Up), &mut app);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn down_arrow_moves_selection_down() {
        let mut app = app_with_rows();
        app.selected = 0;
        handle_key(key(KeyCode::Down), &mut app);
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn ctrl_p_moves_selection_up() {
        let mut app = app_with_rows();
        app.selected = 1;
        handle_key(ctrl(KeyCode::Char('p')), &mut app);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn ctrl_n_moves_selection_down() {
        let mut app = app_with_rows();
        app.selected = 0;
        handle_key(ctrl(KeyCode::Char('n')), &mut app);
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn printable_char_appends_to_query_and_recomputes() {
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
    fn up_at_top_is_noop() {
        let mut app = app_with_rows();
        app.selected = 0;
        handle_key(key(KeyCode::Up), &mut app);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn down_at_bottom_is_noop() {
        let mut app = app_with_rows();
        app.selected = 1;
        handle_key(key(KeyCode::Down), &mut app);
        assert_eq!(app.selected, 1);
    }
}
