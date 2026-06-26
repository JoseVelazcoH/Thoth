use ratatui::{
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Paragraph, StatefulWidget},
    Frame,
};

use crate::search::CommandRow;
use crate::tui::app::App;
use crate::tui::time::format_relative;

const EXIT_OK_GLYPH: &str = "✓";
const EXIT_FAIL_GLYPH: &str = "✗";

fn exit_span(exit_code: i64) -> Span<'static> {
    if exit_code == 0 {
        Span::styled(EXIT_OK_GLYPH, Style::default().fg(Color::Green))
    } else {
        Span::styled(EXIT_FAIL_GLYPH, Style::default().fg(Color::Red))
    }
}

fn row_line<'a>(row: &'a CommandRow, now: i64, width: usize) -> Line<'a> {
    let rel_time = format_relative(row.timestamp, now);
    let glyph = exit_span(row.exit_code);

    let time_span = Span::raw(format!("{:>8} ", rel_time));
    let project_span = Span::raw(format!("{} ", row.project));

    let overhead = rel_time.len() + 1 + 1 + 1 + row.project.len() + 1;
    let cmd_width = if width > overhead {
        width - overhead
    } else {
        1
    };
    let command = if row.command.len() > cmd_width {
        format!("{}…", &row.command[..cmd_width.saturating_sub(1)])
    } else {
        row.command.clone()
    };

    Line::from(vec![
        time_span,
        glyph,
        Span::raw(" "),
        project_span,
        Span::raw(command),
    ])
}

fn filter_chips(app: &App) -> String {
    let mut parts = Vec::new();
    if let Some(ref p) = app.filters.project {
        parts.push(format!("[project:{p}]"));
    }
    for tag in &app.filters.tag {
        parts.push(format!("[tag:{tag}]"));
    }
    if let Some(ref e) = app.filters.exit {
        parts.push(format!("[exit:{e:?}]"));
    }
    if let Some(ref s) = app.filters.since {
        parts.push(format!("[since:{s}]"));
    }
    if let Some(ref u) = app.filters.until {
        parts.push(format!("[until:{u}]"));
    }
    parts.join(" ")
}

pub fn draw(frame: &mut Frame, app: &App, now: i64) {
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    let header_area = chunks[0];
    let list_area = chunks[1];
    let query_area = chunks[2];
    let status_area = chunks[3];

    let version = env!("CARGO_PKG_VERSION");
    let header_left = format!(" tth v{version}");
    let history_right = format!("History count: {} ", app.all_rows.len());
    let pad = (header_area.width as usize).saturating_sub(header_left.len() + history_right.len());
    let header_text = format!("{header_left}{:pad$}{history_right}", "", pad = pad);
    let header =
        Paragraph::new(header_text).style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_widget(header, header_area);

    let width = list_area.width as usize;
    let height = list_area.height as usize;

    let visible: Vec<usize> = if app.filtered.is_empty() {
        vec![]
    } else {
        let total = app.filtered.len();
        let scroll = app.scroll.min(total.saturating_sub(1));
        let end = (scroll + height).min(total);
        (scroll..end).collect()
    };

    let selected_in_visible = if visible.is_empty() {
        None
    } else {
        let scroll = app.scroll.min(app.filtered.len().saturating_sub(1));
        if app.selected >= scroll && app.selected < scroll + visible.len() {
            Some(app.selected - scroll)
        } else {
            None
        }
    };

    let items: Vec<ListItem> = visible
        .iter()
        .enumerate()
        .map(|(vi, &fi)| {
            let row = &app.all_rows[app.filtered[fi]];
            let line = row_line(row, now, width);
            let style = if Some(vi) == selected_in_visible {
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(line).style(style)
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(selected_in_visible);

    let list = List::new(items).block(Block::default());
    StatefulWidget::render(list, list_area, frame.buffer_mut(), &mut list_state);

    let query_text = format!("> {}", app.query);
    let query_widget = Paragraph::new(query_text);
    frame.render_widget(query_widget, query_area);

    let chips = filter_chips(app);
    let result_count = app.filtered.len();
    let status_text = if chips.is_empty() {
        format!("{result_count} results")
    } else {
        format!("{chips}  {result_count} results")
    };
    let status_widget = Paragraph::new(status_text).alignment(Alignment::Left);
    frame.render_widget(status_widget, status_area);
}

pub fn format_action_line(action: Option<&crate::tui::app::Action>) -> Option<String> {
    use crate::tui::app::Action;
    match action {
        Some(Action::Run(cmd)) => Some(format!("RUN:{cmd}")),
        Some(Action::Edit(cmd)) => Some(format!("EDIT:{cmd}")),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::CommandRow;
    use crate::tui::app::{Action, App};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    const TEST_NOW: i64 = 1_000_000_000;
    const TEST_WIDTH: u16 = 80;
    const TEST_HEIGHT: u16 = 24;

    fn make_row(cmd: &str, ts: i64, exit: i64, project: &str) -> CommandRow {
        CommandRow {
            command: cmd.to_string(),
            timestamp: ts,
            exit_code: exit,
            project: project.to_string(),
            directory: "/tmp".to_string(),
            tags: "[]".to_string(),
            session_id: "s1".to_string(),
            duration_ms: 100,
        }
    }

    fn render_app(app: &App) -> String {
        let backend = TestBackend::new(TEST_WIDTH, TEST_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, app, TEST_NOW)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let mut lines: Vec<String> = Vec::new();
        for row in 0..TEST_HEIGHT {
            let mut line = String::new();
            for col in 0..TEST_WIDTH {
                line.push(buf[(col, row)].symbol().chars().next().unwrap_or(' '));
            }
            lines.push(line.trim_end().to_string());
        }
        lines.join("\n")
    }

    fn app_with_rows() -> App {
        let mut app = App::new();
        app.all_rows = vec![
            make_row("git status", TEST_NOW - 60, 0, "proj-alpha"),
            make_row("docker run nginx", TEST_NOW - 3600, 1, "proj-beta"),
            make_row("cargo build", TEST_NOW - 86400, 0, "proj-alpha"),
        ];
        app.recompute();
        app
    }

    #[test]
    fn header_contains_version_and_history_count() {
        let app = app_with_rows();
        let text = render_app(&app);
        assert!(text.contains("tth v"), "header must contain version string");
        assert!(
            text.contains("History count: 3"),
            "header must contain history count: got {text}"
        );
    }

    #[test]
    fn results_list_contains_command_text() {
        let app = app_with_rows();
        let text = render_app(&app);
        assert!(
            text.contains("git status"),
            "list must contain 'git status'"
        );
        assert!(
            text.contains("docker run nginx"),
            "list must contain 'docker run'"
        );
        assert!(
            text.contains("cargo build"),
            "list must contain 'cargo build'"
        );
    }

    #[test]
    fn results_list_contains_relative_times() {
        let app = app_with_rows();
        let text = render_app(&app);
        assert!(
            text.contains("ago"),
            "rows must show relative times containing 'ago'"
        );
    }

    #[test]
    fn results_list_contains_project_names() {
        let app = app_with_rows();
        let text = render_app(&app);
        assert!(text.contains("proj-alpha"), "rows must show project name");
    }

    #[test]
    fn query_line_shows_prompt() {
        let mut app = App::new();
        app.all_rows = vec![make_row("ls", TEST_NOW - 10, 0, "p")];
        app.query = "my search".to_string();
        app.recompute();
        let text = render_app(&app);
        assert!(
            text.contains("> my search"),
            "query line must show '> my search'; got:\n{text}"
        );
    }

    #[test]
    fn query_line_shows_empty_prompt_when_no_query() {
        let app = app_with_rows();
        let text = render_app(&app);
        assert!(text.contains('>'), "query line must contain '>'");
    }

    #[test]
    fn selected_row_is_styled_reversed() {
        let mut app = App::new();
        app.all_rows = vec![
            make_row("first-cmd", TEST_NOW - 10, 0, "p"),
            make_row("second-cmd", TEST_NOW - 20, 0, "p"),
        ];
        app.recompute();
        app.selected = 0;

        let backend = TestBackend::new(TEST_WIDTH, TEST_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, &app, TEST_NOW)).unwrap();
        let buf = terminal.backend().buffer().clone();

        let row1_y = 1u16;
        let any_reversed = (0..TEST_WIDTH).any(|x| {
            buf[(x, row1_y)]
                .style()
                .add_modifier
                .contains(Modifier::REVERSED)
        });
        assert!(any_reversed, "selected row must have REVERSED modifier");
    }

    #[test]
    fn status_bar_shows_result_count() {
        let app = app_with_rows();
        let text = render_app(&app);
        assert!(
            text.contains("3 results"),
            "status bar must show '3 results'; got:\n{text}"
        );
    }

    #[test]
    fn status_bar_shows_filter_chips_when_active() {
        let mut app = App::new();
        app.all_rows = vec![make_row("ls", TEST_NOW - 10, 0, "myproject")];
        app.filters.project = Some("myproject".to_string());
        app.recompute();
        let text = render_app(&app);
        assert!(
            text.contains("[project:myproject]"),
            "status bar must show project filter chip; got:\n{text}"
        );
    }

    #[test]
    fn format_action_line_run_produces_correct_prefix() {
        let action = Action::Run("git pull".to_string());
        let result = format_action_line(Some(&action));
        assert_eq!(result, Some("RUN:git pull".to_string()));
    }

    #[test]
    fn format_action_line_edit_produces_correct_prefix() {
        let action = Action::Edit("vim main.rs".to_string());
        let result = format_action_line(Some(&action));
        assert_eq!(result, Some("EDIT:vim main.rs".to_string()));
    }

    #[test]
    fn format_action_line_none_produces_no_output() {
        let result = format_action_line(None);
        assert!(result.is_none(), "None action must produce no stdout line");
    }

    #[test]
    fn format_action_line_run_no_newline_in_result() {
        let action = Action::Run("echo hello".to_string());
        let line = format_action_line(Some(&action)).unwrap();
        assert!(!line.contains('\n'), "action line must not contain newline");
    }

    #[test]
    fn format_action_line_edit_no_newline_in_result() {
        let action = Action::Edit("nano config".to_string());
        let line = format_action_line(Some(&action)).unwrap();
        assert!(!line.contains('\n'), "action line must not contain newline");
    }
}
