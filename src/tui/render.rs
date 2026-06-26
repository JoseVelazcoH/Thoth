use ratatui::{
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::tui::app::App;
use crate::tui::time::format_relative;

pub fn display_command(raw: &str) -> String {
    let collapsed: String = raw
        .chars()
        .map(|c| {
            if c == '\n' || c == '\r' || c == '\t' {
                ' '
            } else {
                c
            }
        })
        .collect();
    let mut result = String::with_capacity(collapsed.len());
    let mut prev_space = false;
    for c in collapsed.chars() {
        if c == ' ' {
            if !prev_space {
                result.push(c);
            }
            prev_space = true;
        } else {
            result.push(c);
            prev_space = false;
        }
    }
    result.trim().to_string()
}

fn format_duration(ms: i64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{}s", ms / 1000)
    }
}

fn exit_text(exit_code: i64) -> (&'static str, Color) {
    if exit_code == 0 {
        ("ok", Color::Green)
    } else {
        ("fail", Color::Red)
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let end = s
            .char_indices()
            .nth(max.saturating_sub(1))
            .map(|(i, _)| i)
            .unwrap_or(s.len());
        format!("{}...", &s[..end])
    }
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
        Constraint::Length(1),
    ])
    .split(area);

    let header_area = chunks[0];
    let list_area = chunks[1];
    let query_area = chunks[2];
    let status_area = chunks[3];
    let controls_area = chunks[4];

    let version = env!("CARGO_PKG_VERSION");
    let name_span = Span::styled(" Thoth", Style::default().add_modifier(Modifier::BOLD));
    let version_span = Span::styled(
        format!(" v{version}"),
        Style::default().add_modifier(Modifier::DIM),
    );
    let history_right = format!("History count: {} ", app.all_rows.len());
    let left_len = " Thoth".len() + format!(" v{version}").len();
    let pad = (header_area.width as usize).saturating_sub(left_len + history_right.len());
    let padding_span = Span::raw(format!("{:pad$}", "", pad = pad));
    let history_span = Span::raw(history_right);
    let header_line = Line::from(vec![name_span, version_span, padding_span, history_span]);
    let header =
        Paragraph::new(header_line).style(Style::default().add_modifier(Modifier::REVERSED));
    frame.render_widget(header, header_area);

    let controls = Paragraph::new(" up/down navigate  enter run  tab edit  esc exit")
        .style(Style::default().add_modifier(Modifier::DIM));
    frame.render_widget(controls, controls_area);

    let width = list_area.width as usize;

    let time_w: u16 = 9;
    let dur_w: u16 = 7;
    let exit_w: u16 = 4;
    let proj_w: u16 = 14;
    let gaps: u16 = 4;
    let fixed: u16 = time_w + dur_w + exit_w + proj_w + gaps;
    let cmd_w: u16 = (list_area.width).saturating_sub(fixed);

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

    let dim = Style::default().fg(Color::DarkGray);
    let cyan = Style::default().fg(Color::Cyan);
    let blue = Style::default().fg(Color::Blue);

    let rows: Vec<Row> = visible
        .iter()
        .enumerate()
        .map(|(vi, &fi)| {
            let row = &app.all_rows[app.filtered[fi]];
            let rel = format_relative(row.timestamp, now);
            let dur = format_duration(row.duration_ms);
            let (exit_label, exit_color) = exit_text(row.exit_code);
            let proj = truncate(&row.project, proj_w as usize);
            let cmd = display_command(&row.command);
            let cmd = truncate(&cmd, cmd_w as usize);

            let row_style = if Some(vi) == selected_in_visible {
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let time_cell = Cell::from(Line::from(vec![Span::styled(rel, dim)]));
            let dur_cell = Cell::from(Line::from(vec![Span::styled(dur, cyan)]));
            let exit_cell = Cell::from(Line::from(vec![Span::styled(
                exit_label,
                Style::default().fg(exit_color),
            )]));
            let proj_cell = Cell::from(Line::from(vec![Span::styled(proj, blue)]));
            let cmd_cell = Cell::from(Line::from(vec![Span::raw(cmd)]));

            Row::new([time_cell, dur_cell, exit_cell, proj_cell, cmd_cell]).style(row_style)
        })
        .collect();

    let widths = [
        Constraint::Length(time_w),
        Constraint::Length(dur_w),
        Constraint::Length(exit_w),
        Constraint::Length(proj_w),
        Constraint::Min(1),
    ];

    let _ = width;

    let table = Table::new(rows, widths).block(Block::default());
    frame.render_widget(table, list_area);

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
        assert!(
            text.contains("Thoth"),
            "header must contain product name 'Thoth'"
        );
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
    fn exit_ok_renders_as_text_not_glyph() {
        let mut app = App::new();
        app.all_rows = vec![make_row("ls", TEST_NOW - 10, 0, "p")];
        app.recompute();
        let text = render_app(&app);
        assert!(
            text.contains("ok"),
            "exit 0 must render as 'ok'; got:\n{text}"
        );
        assert!(!text.contains('✓'), "exit 0 must NOT render as glyph ✓");
    }

    #[test]
    fn exit_fail_renders_as_text_not_glyph() {
        let mut app = App::new();
        app.all_rows = vec![make_row("bad-cmd", TEST_NOW - 10, 1, "p")];
        app.recompute();
        let text = render_app(&app);
        assert!(
            text.contains("fail"),
            "exit nonzero must render as 'fail'; got:\n{text}"
        );
        assert!(
            !text.contains('✗'),
            "exit nonzero must NOT render as glyph ✗"
        );
    }

    #[test]
    fn controls_hint_is_visible() {
        let app = app_with_rows();
        let text = render_app(&app);
        assert!(
            text.contains("navigate"),
            "controls hint must contain 'navigate'; got:\n{text}"
        );
        assert!(
            text.contains("enter"),
            "controls hint must contain 'enter'; got:\n{text}"
        );
        assert!(
            text.contains("esc"),
            "controls hint must contain 'esc'; got:\n{text}"
        );
    }

    #[test]
    fn multiline_command_is_collapsed_in_render() {
        let mut app = App::new();
        app.all_rows = vec![make_row("git\nstatus\n  --short", TEST_NOW - 10, 0, "p")];
        app.recompute();
        let text = render_app(&app);
        assert!(
            !text.contains('\n') || !text.lines().any(|l| l.trim().starts_with("status")),
            "multi-line command must be collapsed to one line"
        );
        assert!(
            text.contains("git status --short"),
            "collapsed command must appear as single line; got:\n{text}"
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

    #[test]
    fn display_command_collapses_newlines() {
        assert_eq!(display_command("git\nstatus"), "git status");
    }

    #[test]
    fn display_command_collapses_carriage_returns() {
        assert_eq!(display_command("git\r\nstatus"), "git status");
    }

    #[test]
    fn display_command_collapses_tabs() {
        assert_eq!(display_command("git\tstatus"), "git status");
    }

    #[test]
    fn display_command_collapses_multiple_spaces() {
        assert_eq!(display_command("git   status"), "git status");
    }

    #[test]
    fn display_command_trims_leading_trailing() {
        assert_eq!(display_command("  git status  "), "git status");
    }

    #[test]
    fn display_command_multiline_with_indentation() {
        assert_eq!(
            display_command("git\nstatus\n  --short"),
            "git status --short"
        );
    }

    #[test]
    fn display_command_empty_string() {
        assert_eq!(display_command(""), "");
    }

    #[test]
    fn display_command_plain_command_unchanged() {
        assert_eq!(display_command("ls -la"), "ls -la");
    }
}
