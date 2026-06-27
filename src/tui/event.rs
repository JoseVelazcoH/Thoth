use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::app::{Action, App, Confirm, Mode, Tab, WsPane};

fn handle_edit_key(key: KeyEvent, app: &mut App) -> Outcome {
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.action = None;
        return Outcome::Exit;
    }
    match key.code {
        KeyCode::Esc => {
            app.edit_cancel();
            Outcome::Continue
        }
        KeyCode::Enter => {
            app.edit_commit();
            Outcome::Continue
        }
        KeyCode::Backspace => {
            app.edit_backspace();
            Outcome::Continue
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.edit_push(c);
            Outcome::Continue
        }
        _ => Outcome::Continue,
    }
}

fn handle_confirm_key(key: crossterm::event::KeyEvent, app: &mut App) -> Outcome {
    use crossterm::event::KeyCode;
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.action = None;
        return Outcome::Exit;
    }
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') => match app.confirm.take() {
            Some(Confirm::Replay(r)) => {
                app.replay_workspace = Some(r.workspace);
                Outcome::Exit
            }
            Some(Confirm::Delete(d)) => {
                app.pending_delete = Some((d.id, d.origin));
                Outcome::Continue
            }
            None => Outcome::Continue,
        },
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            app.cancel_confirm();
            Outcome::Continue
        }
        _ => Outcome::Continue,
    }
}

pub enum Outcome {
    Continue,
    Exit,
}

pub fn handle_key(key: KeyEvent, app: &mut App) -> Outcome {
    if app.confirm.is_some() {
        return handle_confirm_key(key, app);
    }
    if app.edit.is_some() {
        return handle_edit_key(key, app);
    }

    match key.code {
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
    match app.mode {
        Mode::Insert => handle_history_insert_key(key, app),
        Mode::Normal => handle_history_normal_key(key, app),
    }
}

fn handle_history_insert_key(key: KeyEvent, app: &mut App) -> Outcome {
    match key.code {
        KeyCode::Esc => {
            app.enter_normal_mode();
            Outcome::Continue
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

fn handle_history_normal_key(key: KeyEvent, app: &mut App) -> Outcome {
    match key.code {
        KeyCode::Char('i') | KeyCode::Char('/') => {
            app.enter_insert_mode();
            Outcome::Continue
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.move_down();
            Outcome::Continue
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.move_up();
            Outcome::Continue
        }
        KeyCode::Char('d') => {
            app.begin_delete_confirm_history();
            Outcome::Continue
        }
        KeyCode::Char('e') => {
            app.begin_edit_history();
            Outcome::Continue
        }
        KeyCode::Enter => {
            if let Some(cmd) = app.selected_command() {
                app.action = Some(Action::Run(cmd.to_string()));
            }
            Outcome::Exit
        }
        KeyCode::Char('q') | KeyCode::Esc => {
            app.action = None;
            Outcome::Exit
        }
        _ => Outcome::Continue,
    }
}

fn handle_ws_key(key: KeyEvent, app: &mut App) -> Outcome {
    match key.code {
        KeyCode::Tab => {
            app.toggle_ws_pane();
            return Outcome::Continue;
        }
        KeyCode::Esc => {
            return Outcome::Exit;
        }
        _ => {}
    }

    match app.ws_pane {
        WsPane::List => handle_ws_list_key(key, app),
        WsPane::Commands => handle_ws_commands_key(key, app),
    }
}

fn handle_ws_list_key(key: KeyEvent, app: &mut App) -> Outcome {
    match key.code {
        KeyCode::Enter => {
            app.begin_replay_confirm();
            Outcome::Continue
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.ws_move_up();
            Outcome::Continue
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.ws_move_up();
            Outcome::Continue
        }
        KeyCode::Down | KeyCode::Char('j') => {
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

fn handle_ws_commands_key(key: KeyEvent, app: &mut App) -> Outcome {
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            app.ws_cmd_move_up();
            Outcome::Continue
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.ws_cmd_move_down();
            Outcome::Continue
        }
        KeyCode::Char('d') => {
            app.begin_delete_confirm_ws();
            Outcome::Continue
        }
        KeyCode::Char('e') => {
            app.begin_edit_ws();
            Outcome::Continue
        }
        _ => Outcome::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::{DeleteOrigin, Mode, WsPane};
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
                id: 1,
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
                id: 2,
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

    fn app_with_workspaces_and_commands() -> App {
        let mut app = app_with_workspaces();
        app.ws_commands = vec![crate::search::CommandRow {
            id: 10,
            command: "git status".into(),
            directory: "/tmp".into(),
            project: "p".into(),
            session_id: "s1".into(),
            timestamp: 1000,
            exit_code: 0,
            duration_ms: 100,
            tags: "[]".into(),
            workspace: Some("ws-new".into()),
        }];
        app
    }

    #[test]
    fn ctrl_c_sets_no_action_and_exits() {
        let mut app = app_with_rows();
        let outcome = handle_key(ctrl(KeyCode::Char('c')), &mut app);
        assert!(app.action.is_none());
        assert!(matches!(outcome, Outcome::Exit));
    }

    #[test]
    fn history_insert_enter_sets_run_action_and_exits() {
        let mut app = app_with_rows();
        let outcome = handle_key(key(KeyCode::Enter), &mut app);
        assert!(matches!(outcome, Outcome::Exit));
        assert!(matches!(&app.action, Some(Action::Run(cmd)) if cmd == "git status"));
    }

    #[test]
    fn history_insert_tab_sets_edit_action_and_exits() {
        let mut app = app_with_rows();
        let outcome = handle_key(key(KeyCode::Tab), &mut app);
        assert!(matches!(outcome, Outcome::Exit));
        assert!(matches!(&app.action, Some(Action::Edit(cmd)) if cmd == "git status"));
    }

    #[test]
    fn history_insert_esc_enters_normal_mode_no_exit() {
        let mut app = app_with_rows();
        assert_eq!(app.mode, Mode::Insert);
        let outcome = handle_key(key(KeyCode::Esc), &mut app);
        assert!(matches!(outcome, Outcome::Continue));
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn history_normal_i_enters_insert_mode() {
        let mut app = app_with_rows();
        app.enter_normal_mode();
        let outcome = handle_key(key(KeyCode::Char('i')), &mut app);
        assert!(matches!(outcome, Outcome::Continue));
        assert_eq!(app.mode, Mode::Insert);
    }

    #[test]
    fn history_normal_slash_enters_insert_mode() {
        let mut app = app_with_rows();
        app.enter_normal_mode();
        let outcome = handle_key(key(KeyCode::Char('/')), &mut app);
        assert!(matches!(outcome, Outcome::Continue));
        assert_eq!(app.mode, Mode::Insert);
    }

    #[test]
    fn history_normal_j_moves_down() {
        let mut app = app_with_rows();
        app.enter_normal_mode();
        app.selected = 1;
        handle_key(key(KeyCode::Char('j')), &mut app);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn history_normal_k_moves_up() {
        let mut app = app_with_rows();
        app.enter_normal_mode();
        app.selected = 0;
        handle_key(key(KeyCode::Char('k')), &mut app);
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn history_normal_d_opens_delete_confirm() {
        let mut app = app_with_rows();
        app.enter_normal_mode();
        let outcome = handle_key(key(KeyCode::Char('d')), &mut app);
        assert!(matches!(outcome, Outcome::Continue));
        assert!(app.confirm.is_some());
        assert!(matches!(
            app.confirm.as_ref().unwrap(),
            Confirm::Delete(d) if matches!(d.origin, DeleteOrigin::History)
        ));
    }

    #[test]
    fn history_normal_enter_runs_and_exits() {
        let mut app = app_with_rows();
        app.enter_normal_mode();
        let outcome = handle_key(key(KeyCode::Enter), &mut app);
        assert!(matches!(outcome, Outcome::Exit));
        assert!(matches!(&app.action, Some(Action::Run(_))));
    }

    #[test]
    fn history_normal_q_exits() {
        let mut app = app_with_rows();
        app.enter_normal_mode();
        let outcome = handle_key(key(KeyCode::Char('q')), &mut app);
        assert!(matches!(outcome, Outcome::Exit));
    }

    #[test]
    fn history_normal_esc_exits() {
        let mut app = app_with_rows();
        app.enter_normal_mode();
        let outcome = handle_key(key(KeyCode::Esc), &mut app);
        assert!(matches!(outcome, Outcome::Exit));
    }

    #[test]
    fn history_insert_typing_still_filters_regression() {
        let mut app = app_with_rows();
        assert_eq!(app.mode, Mode::Insert);
        handle_key(key(KeyCode::Char('g')), &mut app);
        assert_eq!(app.query, "g");
    }

    #[test]
    fn history_insert_up_arrow_moves_to_older_command() {
        let mut app = app_with_rows();
        app.selected = 0;
        handle_key(key(KeyCode::Up), &mut app);
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn history_insert_down_arrow_moves_to_newer_command() {
        let mut app = app_with_rows();
        app.selected = 1;
        handle_key(key(KeyCode::Down), &mut app);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn history_ctrl_p_moves_up() {
        let mut app = app_with_rows();
        app.selected = 0;
        handle_key(ctrl(KeyCode::Char('p')), &mut app);
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn history_ctrl_n_moves_down() {
        let mut app = app_with_rows();
        app.selected = 1;
        handle_key(ctrl(KeyCode::Char('n')), &mut app);
        assert_eq!(app.selected, 0);
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
    fn ws_enter_opens_confirm_modal() {
        let mut app = app_with_workspaces_and_commands();
        let outcome = handle_key(key(KeyCode::Enter), &mut app);
        assert!(matches!(outcome, Outcome::Continue));
        assert!(app.confirm.is_some(), "Enter must open confirm modal");
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

    #[test]
    fn confirm_replay_y_sets_replay_workspace_clears_confirm_and_exits() {
        let mut app = app_with_workspaces_and_commands();
        handle_key(key(KeyCode::Enter), &mut app);
        assert!(app.confirm.is_some());
        let outcome = handle_key(key(KeyCode::Char('y')), &mut app);
        assert!(matches!(outcome, Outcome::Exit));
        assert!(app.confirm.is_none(), "confirm must be cleared after y");
        assert_eq!(app.replay_workspace, Some("ws-new".into()));
    }

    #[test]
    fn confirm_replay_n_cancels_and_continues() {
        let mut app = app_with_workspaces_and_commands();
        handle_key(key(KeyCode::Enter), &mut app);
        let outcome = handle_key(key(KeyCode::Char('n')), &mut app);
        assert!(matches!(outcome, Outcome::Continue));
        assert!(app.confirm.is_none());
        assert!(app.replay_workspace.is_none());
    }

    #[test]
    fn confirm_replay_esc_cancels_and_continues() {
        let mut app = app_with_workspaces_and_commands();
        handle_key(key(KeyCode::Enter), &mut app);
        let outcome = handle_key(key(KeyCode::Esc), &mut app);
        assert!(matches!(outcome, Outcome::Continue));
        assert!(app.confirm.is_none());
    }

    #[test]
    fn ctrl_c_exits_even_with_confirm_open() {
        let mut app = app_with_workspaces_and_commands();
        handle_key(key(KeyCode::Enter), &mut app);
        assert!(app.confirm.is_some());
        let outcome = handle_key(ctrl(KeyCode::Char('c')), &mut app);
        assert!(
            matches!(outcome, Outcome::Exit),
            "Ctrl-C must exit from any state"
        );
        assert!(app.action.is_none());
    }

    #[test]
    fn delete_confirm_y_sets_pending_delete_continue_not_exit() {
        let mut app = app_with_rows();
        app.enter_normal_mode();
        handle_key(key(KeyCode::Char('d')), &mut app);
        assert!(app.confirm.is_some());
        let expected_id = if let Some(Confirm::Delete(d)) = app.confirm.as_ref() {
            d.id
        } else {
            panic!("expected Delete confirm");
        };
        let outcome = handle_key(key(KeyCode::Char('y')), &mut app);
        assert!(
            matches!(outcome, Outcome::Continue),
            "delete y must Continue, not Exit"
        );
        assert!(app.confirm.is_none());
        assert!(
            matches!(app.pending_delete, Some((id, DeleteOrigin::History)) if id == expected_id)
        );
    }

    #[test]
    fn delete_confirm_n_cancels_and_continues() {
        let mut app = app_with_rows();
        app.enter_normal_mode();
        handle_key(key(KeyCode::Char('d')), &mut app);
        let outcome = handle_key(key(KeyCode::Char('n')), &mut app);
        assert!(matches!(outcome, Outcome::Continue));
        assert!(app.confirm.is_none());
        assert!(app.pending_delete.is_none());
    }

    #[test]
    fn ws_tab_toggles_pane() {
        let mut app = app_with_workspaces();
        assert_eq!(app.ws_pane, WsPane::List);
        handle_key(key(KeyCode::Tab), &mut app);
        assert_eq!(app.ws_pane, WsPane::Commands);
        handle_key(key(KeyCode::Tab), &mut app);
        assert_eq!(app.ws_pane, WsPane::List);
    }

    #[test]
    fn ws_commands_pane_d_opens_delete_confirm_with_ws_command_id() {
        let mut app = app_with_workspaces_and_commands();
        app.ws_pane = WsPane::Commands;
        app.ws_cmd_selected = 0;
        let expected_id = app.ws_commands[0].id;
        let outcome = handle_key(key(KeyCode::Char('d')), &mut app);
        assert!(matches!(outcome, Outcome::Continue));
        assert!(
            matches!(app.confirm.as_ref(), Some(Confirm::Delete(d)) if d.id == expected_id && matches!(d.origin, DeleteOrigin::Workspace))
        );
    }

    #[test]
    fn ws_esc_exits() {
        let mut app = app_with_workspaces();
        let outcome = handle_key(key(KeyCode::Esc), &mut app);
        assert!(matches!(outcome, Outcome::Exit));
    }

    #[test]
    fn confirm_other_key_does_not_leak_through() {
        let mut app = app_with_workspaces_and_commands();
        handle_key(key(KeyCode::Enter), &mut app);
        let query_before = app.query.clone();
        let outcome = handle_key(key(KeyCode::Char('g')), &mut app);
        assert!(matches!(outcome, Outcome::Continue));
        assert_eq!(
            app.query, query_before,
            "typing must not leak to query while confirm is open"
        );
        assert!(
            app.confirm.is_some(),
            "confirm must stay open on unknown key"
        );
    }

    #[test]
    fn confirm_left_does_not_change_tab() {
        let mut app = app_with_workspaces_and_commands();
        handle_key(key(KeyCode::Enter), &mut app);
        let tab_before = app.tab;
        handle_key(key(KeyCode::Left), &mut app);
        assert_eq!(
            app.tab, tab_before,
            "Left must not change tab while confirm is open"
        );
    }

    #[test]
    fn history_enter_still_exits_with_run_regression() {
        let mut app = app_with_rows();
        assert_eq!(app.tab, Tab::History);
        let outcome = handle_key(key(KeyCode::Enter), &mut app);
        assert!(matches!(outcome, Outcome::Exit));
        assert!(matches!(&app.action, Some(Action::Run(_))));
    }

    #[test]
    fn ws_enter_is_continue_not_exit() {
        let mut app = app_with_workspaces();
        let outcome = handle_key(key(KeyCode::Enter), &mut app);
        assert!(
            matches!(outcome, Outcome::Continue),
            "Workspaces Enter must be Continue"
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
    fn replay_confirm_is_still_replay_type() {
        let mut app = app_with_workspaces_and_commands();
        handle_key(key(KeyCode::Enter), &mut app);
        assert!(
            matches!(app.confirm.as_ref(), Some(Confirm::Replay(_))),
            "Enter in Workspaces List must open Replay confirm, not Delete"
        );
    }

    #[test]
    fn history_normal_e_opens_edit() {
        let mut app = app_with_rows();
        app.enter_normal_mode();
        let outcome = handle_key(key(KeyCode::Char('e')), &mut app);
        assert!(matches!(outcome, Outcome::Continue));
        assert!(
            app.edit.is_some(),
            "e must open edit modal in History Normal"
        );
    }

    #[test]
    fn ws_commands_e_opens_edit() {
        let mut app = app_with_workspaces_and_commands();
        app.ws_pane = WsPane::Commands;
        let outcome = handle_key(key(KeyCode::Char('e')), &mut app);
        assert!(matches!(outcome, Outcome::Continue));
        assert!(app.edit.is_some(), "e must open edit modal in Ws Commands");
    }

    #[test]
    fn edit_char_appends_to_buffer_not_query() {
        let mut app = app_with_rows();
        app.enter_normal_mode();
        handle_key(key(KeyCode::Char('e')), &mut app);
        assert!(app.edit.is_some());
        let query_before = app.query.clone();
        handle_key(key(KeyCode::Char('x')), &mut app);
        assert_eq!(
            app.query, query_before,
            "typing while editing must not change query"
        );
        assert!(
            app.edit.as_ref().unwrap().buffer.ends_with('x'),
            "x must append to edit buffer"
        );
    }

    #[test]
    fn edit_backspace_removes_from_buffer() {
        let mut app = app_with_rows();
        app.enter_normal_mode();
        handle_key(key(KeyCode::Char('e')), &mut app);
        let original_len = app.edit.as_ref().unwrap().buffer.len();
        handle_key(key(KeyCode::Backspace), &mut app);
        assert_eq!(
            app.edit.as_ref().unwrap().buffer.len(),
            original_len.saturating_sub(1)
        );
    }

    #[test]
    fn edit_enter_commits_and_sets_pending_edit() {
        let mut app = app_with_rows();
        app.enter_normal_mode();
        handle_key(key(KeyCode::Char('e')), &mut app);
        assert!(app.edit.is_some());
        let outcome = handle_key(key(KeyCode::Enter), &mut app);
        assert!(matches!(outcome, Outcome::Continue));
        assert!(app.edit.is_none(), "edit must be cleared after Enter");
        assert!(
            app.pending_edit.is_some(),
            "pending_edit must be set after Enter"
        );
    }

    #[test]
    fn edit_esc_cancels() {
        let mut app = app_with_rows();
        app.enter_normal_mode();
        handle_key(key(KeyCode::Char('e')), &mut app);
        assert!(app.edit.is_some());
        let outcome = handle_key(key(KeyCode::Esc), &mut app);
        assert!(matches!(outcome, Outcome::Continue));
        assert!(app.edit.is_none(), "Esc must cancel edit");
        assert!(app.pending_edit.is_none());
    }

    #[test]
    fn ctrl_c_exits_even_while_editing() {
        let mut app = app_with_rows();
        app.enter_normal_mode();
        handle_key(key(KeyCode::Char('e')), &mut app);
        assert!(app.edit.is_some());
        let outcome = handle_key(ctrl(KeyCode::Char('c')), &mut app);
        assert!(
            matches!(outcome, Outcome::Exit),
            "Ctrl-C must exit from edit mode"
        );
    }

    #[test]
    fn left_right_do_not_switch_tabs_while_editing() {
        let mut app = app_with_rows();
        app.enter_normal_mode();
        handle_key(key(KeyCode::Char('e')), &mut app);
        assert!(app.edit.is_some());
        let tab_before = app.tab;
        handle_key(key(KeyCode::Left), &mut app);
        handle_key(key(KeyCode::Right), &mut app);
        assert_eq!(
            app.tab, tab_before,
            "Left/Right must not switch tab while editing"
        );
    }
}
