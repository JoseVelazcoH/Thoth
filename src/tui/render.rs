use ratatui::{
    layout::{Alignment, Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::tui::app::App;
use crate::tui::time::format_relative;

pub const BOTTOM_ANCHORED: bool = true;

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

const ACCENT: Color = Color::Cyan;
const DIM_COLOR: Color = Color::DarkGray;
const BORDER_COLOR: Color = Color::DarkGray;

pub fn draw(frame: &mut Frame, app: &App, now: i64, table_state: &mut TableState) {
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
    let accent_style = Style::default().fg(ACCENT).add_modifier(Modifier::BOLD);
    let dim_style = Style::default().fg(DIM_COLOR).add_modifier(Modifier::DIM);

    let accent_bar = Span::styled("▌ ", accent_style);
    let name_span = Span::styled("Thoth", accent_style);
    let sep_span = Span::styled(" · ", dim_style);
    let version_span = Span::styled(format!("v{version}"), dim_style);
    let count_sep = Span::styled("  ", dim_style);
    let count_span = Span::styled(format!("{} commands", app.all_rows.len()), dim_style);
    let history_right = format!("History count: {}", app.all_rows.len());
    let right_len = history_right.len();
    let left_len =
        2 + 5 + 3 + 1 + version.len() + 2 + format!("{} commands", app.all_rows.len()).len();
    let pad = (header_area.width as usize).saturating_sub(left_len + right_len);
    let padding_span = Span::raw(format!("{:pad$}", "", pad = pad));
    let history_span = Span::styled(history_right, dim_style);

    let header_line = Line::from(vec![
        accent_bar,
        name_span,
        sep_span,
        version_span,
        count_sep,
        count_span,
        padding_span,
        history_span,
    ]);
    let header = Paragraph::new(header_line);
    frame.render_widget(header, header_area);

    let controls_line = Line::from(vec![
        Span::styled(" ↑↓", accent_style),
        Span::styled(" navigate", dim_style),
        Span::styled(" · ", dim_style),
        Span::styled("↵", accent_style),
        Span::styled(" run", dim_style),
        Span::styled(" · ", dim_style),
        Span::styled("⇥", accent_style),
        Span::styled(" edit", dim_style),
        Span::styled(" · ", dim_style),
        Span::styled("esc", accent_style),
        Span::styled(" quit", dim_style),
    ]);
    let controls = Paragraph::new(controls_line);
    frame.render_widget(controls, controls_area);

    let border_style = Style::default().fg(BORDER_COLOR);
    let list_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(Span::styled(" history ", dim_style));

    let inner_list_area = list_block.inner(list_area);
    frame.render_widget(list_block, list_area);

    let time_w: u16 = 9;
    let dur_w: u16 = 7;
    let exit_w: u16 = 4;
    let proj_w: u16 = 14;
    let gaps: u16 = 4;
    let fixed: u16 = time_w + dur_w + exit_w + proj_w + gaps;
    let cmd_w: u16 = inner_list_area.width.saturating_sub(fixed);

    let dim = Style::default().fg(DIM_COLOR);
    let cyan = Style::default().fg(ACCENT);
    let blue = Style::default().fg(Color::Blue);

    let rows: Vec<Row> = app
        .filtered
        .iter()
        .rev()
        .map(|&fi| {
            let row = &app.all_rows[fi];
            let rel = format_relative(row.timestamp, now);
            let dur = format_duration(row.duration_ms);
            let (exit_label, exit_color) = exit_text(row.exit_code);
            let proj = truncate(&row.project, proj_w as usize);
            let cmd = display_command(&row.command);
            let cmd = truncate(&cmd, cmd_w as usize);

            let time_cell = Cell::from(Line::from(vec![Span::styled(rel, dim)]));
            let dur_cell = Cell::from(Line::from(vec![Span::styled(dur, cyan)]));
            let exit_cell = Cell::from(Line::from(vec![Span::styled(
                exit_label,
                Style::default().fg(exit_color),
            )]));
            let proj_cell = Cell::from(Line::from(vec![Span::styled(proj, blue)]));
            let cmd_cell = Cell::from(Line::from(vec![Span::raw(cmd)]));

            Row::new([time_cell, dur_cell, exit_cell, proj_cell, cmd_cell])
        })
        .collect();

    let widths = [
        Constraint::Length(time_w),
        Constraint::Length(dur_w),
        Constraint::Length(exit_w),
        Constraint::Length(proj_w),
        Constraint::Min(1),
    ];

    let highlight_style = Style::default()
        .add_modifier(Modifier::REVERSED)
        .add_modifier(Modifier::BOLD);

    let table = Table::new(rows, widths)
        .block(Block::default())
        .row_highlight_style(highlight_style);

    if app.filtered.is_empty() {
        table_state.select(None);
    } else {
        let display_idx = app.filtered.len() - 1 - app.selected;
        table_state.select(Some(display_idx));
    }
    frame.render_stateful_widget(table, inner_list_area, table_state);

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
        let mut ts = TableState::default();
        terminal.draw(|f| draw(f, app, TEST_NOW, &mut ts)).unwrap();
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
        let mut ts = TableState::default();
        terminal.draw(|f| draw(f, &app, TEST_NOW, &mut ts)).unwrap();
        let buf = terminal.backend().buffer().clone();

        let any_reversed = (0..TEST_HEIGHT).any(|row_y| {
            (0..TEST_WIDTH).any(|x| {
                buf[(x, row_y)]
                    .style()
                    .add_modifier
                    .contains(Modifier::REVERSED)
            })
        });
        assert!(
            any_reversed,
            "selected row must have REVERSED modifier somewhere in the frame"
        );
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
            text.contains("run"),
            "controls hint must contain 'run'; got:\n{text}"
        );
        assert!(
            text.contains("edit"),
            "controls hint must contain 'edit'; got:\n{text}"
        );
        assert!(
            text.contains("quit"),
            "controls hint must contain 'quit'; got:\n{text}"
        );
    }

    #[test]
    fn header_has_no_full_width_reversed_bar() {
        let app = app_with_rows();
        let backend = TestBackend::new(TEST_WIDTH, TEST_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut ts = TableState::default();
        terminal.draw(|f| draw(f, &app, TEST_NOW, &mut ts)).unwrap();
        let buf = terminal.backend().buffer().clone();

        let reversed_count = (0..TEST_WIDTH)
            .filter(|&x| {
                buf[(x, 0)]
                    .style()
                    .add_modifier
                    .contains(Modifier::REVERSED)
            })
            .count();
        assert_eq!(
            reversed_count, 0,
            "header row must have zero REVERSED cells; found {reversed_count}"
        );
    }

    #[test]
    fn border_title_history_appears() {
        let app = app_with_rows();
        let text = render_app(&app);
        assert!(
            text.contains("history"),
            "list border must show title 'history'; got:\n{text}"
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
    fn scroll_brings_selected_row_into_view() {
        const SMALL_HEIGHT: u16 = 8;
        const SMALL_WIDTH: u16 = 80;
        let mut app = App::new();
        app.all_rows = (0..20)
            .map(|i| make_row(&format!("cmd-row-{i:02}"), TEST_NOW - i * 60, 0, "p"))
            .collect();
        app.recompute();
        app.selected = 15;

        let backend = TestBackend::new(SMALL_WIDTH, SMALL_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut ts = TableState::default();
        terminal.draw(|f| draw(f, &app, TEST_NOW, &mut ts)).unwrap();
        let buf = terminal.backend().buffer().clone();

        let mut full_text = String::new();
        for row in 0..SMALL_HEIGHT {
            for col in 0..SMALL_WIDTH {
                full_text.push(buf[(col, row)].symbol().chars().next().unwrap_or(' '));
            }
            full_text.push('\n');
        }

        assert!(
            full_text.contains("cmd-row-15"),
            "selected row 15 must be visible after scroll; buffer:\n{full_text}"
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

    #[test]
    fn dump_bottom_anchored_buffers() {
        use crate::tui::event::handle_key;
        use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

        let mut app = App::new();
        app.all_rows = vec![
            make_row("cmd-newest", TEST_NOW - 10, 0, "proj"),
            make_row("cmd-newer", TEST_NOW - 60, 0, "proj"),
            make_row("cmd-middle", TEST_NOW - 300, 0, "proj"),
            make_row("cmd-older", TEST_NOW - 1200, 0, "proj"),
            make_row("cmd-oldest", TEST_NOW - 3600, 0, "proj"),
        ];
        app.recompute();

        let (default_buf, _) = render_app_small(&app, 80, 8);
        println!("=== DEFAULT (newest at bottom, cursor on newest) ===");
        println!("{default_buf}");

        let up_key = KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        for _ in 0..3 {
            handle_key(up_key, &mut app);
        }

        let (after_up3, _) = render_app_small(&app, 80, 8);
        println!("\n=== AFTER Up x3 (cursor on cmd-older) ===");
        println!("{after_up3}");

        assert!(default_buf.contains("cmd-newest"));
        assert!(after_up3.contains("cmd-older"));
    }

    fn app_bottom_anchored() -> App {
        let mut app = App::new();
        app.all_rows = vec![
            make_row("newest-cmd", TEST_NOW - 60, 0, "p"),
            make_row("middle-cmd", TEST_NOW - 3600, 0, "p"),
            make_row("oldest-cmd", TEST_NOW - 7200, 0, "p"),
        ];
        app.recompute();
        app
    }

    fn render_app_small(app: &App, width: u16, height: u16) -> (String, ratatui::buffer::Buffer) {
        let mut ts = TableState::default();
        render_app_small_with_state(app, width, height, &mut ts)
    }

    fn render_app_small_with_state(
        app: &App,
        width: u16,
        height: u16,
        ts: &mut TableState,
    ) -> (String, ratatui::buffer::Buffer) {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| draw(f, app, TEST_NOW, ts)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let mut lines: Vec<String> = Vec::new();
        for row in 0..height {
            let mut line = String::new();
            for col in 0..width {
                line.push(buf[(col, row)].symbol().chars().next().unwrap_or(' '));
            }
            lines.push(line.trim_end().to_string());
        }
        (lines.join("\n"), buf)
    }

    fn row_y_of(text: &str, buf: &ratatui::buffer::Buffer, width: u16, height: u16) -> Option<u16> {
        for row in 0..height {
            let line: String = (0..width)
                .map(|col| buf[(col, row)].symbol().chars().next().unwrap_or(' '))
                .collect();
            if line.contains(text) {
                return Some(row);
            }
        }
        None
    }

    #[test]
    fn bottom_anchored_flag_is_true() {
        const { assert!(BOTTOM_ANCHORED, "orientation must be bottom-anchored") };
    }

    #[test]
    fn newest_command_renders_below_oldest() {
        let app = app_bottom_anchored();
        let (_, buf) = render_app_small(&app, TEST_WIDTH, TEST_HEIGHT);
        let newest_y = row_y_of("newest-cmd", &buf, TEST_WIDTH, TEST_HEIGHT)
            .expect("newest-cmd must appear in buffer");
        let oldest_y = row_y_of("oldest-cmd", &buf, TEST_WIDTH, TEST_HEIGHT)
            .expect("oldest-cmd must appear in buffer");
        assert!(
            newest_y > oldest_y,
            "newest command must appear on a lower (higher y) row than oldest; newest_y={newest_y} oldest_y={oldest_y}"
        );
    }

    #[test]
    fn default_selection_is_newest_command() {
        let app = app_bottom_anchored();
        let cmd = app.selected_command().expect("must have a selection");
        assert_eq!(
            cmd, "newest-cmd",
            "default selection must be the most-recent command"
        );
    }

    #[test]
    fn up_key_moves_to_older_command() {
        use crate::tui::event::handle_key;
        use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

        let mut app = app_bottom_anchored();
        assert_eq!(app.selected_command(), Some("newest-cmd"));

        handle_key(
            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            },
            &mut app,
        );
        assert_eq!(
            app.selected_command(),
            Some("middle-cmd"),
            "Up from newest must select middle (older) command"
        );
    }

    #[test]
    fn down_key_moves_to_newer_command() {
        use crate::tui::event::handle_key;
        use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

        let mut app = app_bottom_anchored();
        handle_key(
            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            },
            &mut app,
        );
        assert_eq!(app.selected_command(), Some("middle-cmd"));

        handle_key(
            KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            },
            &mut app,
        );
        assert_eq!(
            app.selected_command(),
            Some("newest-cmd"),
            "Down from middle must go back to newest"
        );
    }

    #[test]
    fn up_cannot_go_past_oldest() {
        use crate::tui::event::handle_key;
        use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

        let mut app = app_bottom_anchored();
        let up_key = KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        for _ in 0..10 {
            handle_key(up_key, &mut app);
        }
        assert_eq!(
            app.selected_command(),
            Some("oldest-cmd"),
            "Up cannot go past the oldest command"
        );
    }

    #[test]
    fn down_cannot_go_past_newest() {
        use crate::tui::event::handle_key;
        use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

        let mut app = app_bottom_anchored();
        let down_key = KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        for _ in 0..10 {
            handle_key(down_key, &mut app);
        }
        assert_eq!(
            app.selected_command(),
            Some("newest-cmd"),
            "Down cannot go past the newest command"
        );
    }

    #[test]
    fn bottom_anchored_scroll_keeps_older_row_visible() {
        use crate::tui::event::handle_key;
        use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

        const H: u16 = 8;
        const W: u16 = 80;
        let mut app = App::new();
        app.all_rows = (0..20)
            .map(|i| make_row(&format!("scmd-{i:02}"), TEST_NOW - i as i64 * 60, 0, "p"))
            .collect();
        app.recompute();

        let up_key = KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };
        for _ in 0..15 {
            handle_key(up_key, &mut app);
        }

        let (_, buf) = render_app_small(&app, W, H);
        let mut full_text = String::new();
        for row in 0..H {
            for col in 0..W {
                full_text.push(buf[(col, row)].symbol().chars().next().unwrap_or(' '));
            }
            full_text.push('\n');
        }
        assert!(
            full_text.contains("scmd-15"),
            "after pressing Up 15 times, selected older row must be visible; buffer:\n{full_text}"
        );
    }

    #[test]
    fn dump_persistent_scroll_sequence() {
        use crate::tui::event::handle_key;
        use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

        const W: u16 = 80;
        const H: u16 = 9;

        let mut app = App::new();
        app.all_rows = (0..12)
            .map(|i| make_row(&format!("nav-{i:02}"), TEST_NOW - i as i64 * 60, 0, "p"))
            .collect();
        app.recompute();

        let up_key = KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };

        let mut ts = TableState::default();

        let (s0, _) = render_app_small_with_state(&app, W, H, &mut ts);
        println!("=== FRAME 0 (default, newest=nav-00 at bottom, highlighted) ===\n{s0}");

        handle_key(up_key, &mut app);
        let (s1, _) = render_app_small_with_state(&app, W, H, &mut ts);
        println!("\n=== FRAME 1 (Up x1 - cursor moves UP within viewport, no scroll) ===\n{s1}");

        handle_key(up_key, &mut app);
        let (s2, _) = render_app_small_with_state(&app, W, H, &mut ts);
        println!("\n=== FRAME 2 (Up x2 - cursor moves UP within viewport, no scroll) ===\n{s2}");

        handle_key(up_key, &mut app);
        handle_key(up_key, &mut app);
        handle_key(up_key, &mut app);
        handle_key(up_key, &mut app);
        handle_key(up_key, &mut app);
        handle_key(up_key, &mut app);
        let (s8, _) = render_app_small_with_state(&app, W, H, &mut ts);
        println!("\n=== FRAME 8 (Up x8 - content has scrolled, older commands visible) ===\n{s8}");
    }

    fn visible_commands(buf: &ratatui::buffer::Buffer, width: u16, height: u16) -> Vec<String> {
        let mut seen = Vec::new();
        for row in 0..height {
            let line: String = (0..width)
                .map(|col| buf[(col, row)].symbol().chars().next().unwrap_or(' '))
                .collect();
            for i in 0..12u32 {
                let name = format!("nav-{i:02}");
                if line.contains(&name) && !seen.contains(&name) {
                    seen.push(name);
                }
            }
        }
        seen
    }

    fn highlighted_command(
        buf: &ratatui::buffer::Buffer,
        width: u16,
        height: u16,
    ) -> Option<String> {
        for row in 0..height {
            let any_reversed = (0..width).any(|col| {
                buf[(col, row)]
                    .style()
                    .add_modifier
                    .contains(Modifier::REVERSED)
            });
            if any_reversed {
                let line: String = (0..width)
                    .map(|col| buf[(col, row)].symbol().chars().next().unwrap_or(' '))
                    .collect();
                for i in 0..12u32 {
                    let name = format!("nav-{i:02}");
                    if line.contains(&name) {
                        return Some(name);
                    }
                }
            }
        }
        None
    }

    #[test]
    fn persistent_state_cursor_moves_before_scroll() {
        use crate::tui::event::handle_key;
        use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

        const W: u16 = 80;
        const H: u16 = 9;

        let mut app = App::new();
        app.all_rows = (0..12)
            .map(|i| make_row(&format!("nav-{i:02}"), TEST_NOW - i as i64 * 60, 0, "p"))
            .collect();
        app.recompute();

        let up_key = KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };

        let mut ts = TableState::default();

        let (_, buf0) = render_app_small_with_state(&app, W, H, &mut ts);
        let visible0 = visible_commands(&buf0, W, H);
        let highlighted0 = highlighted_command(&buf0, W, H);

        handle_key(up_key, &mut app);
        let (_, buf1) = render_app_small_with_state(&app, W, H, &mut ts);
        let visible1 = visible_commands(&buf1, W, H);
        let highlighted1 = highlighted_command(&buf1, W, H);

        handle_key(up_key, &mut app);
        let (_, buf2) = render_app_small_with_state(&app, W, H, &mut ts);
        let visible2 = visible_commands(&buf2, W, H);
        let highlighted2 = highlighted_command(&buf2, W, H);

        assert!(
            highlighted0.is_some(),
            "must have a highlighted row at the start"
        );
        assert!(
            highlighted1.is_some(),
            "must have a highlighted row after Up x1"
        );
        assert!(
            highlighted2.is_some(),
            "must have a highlighted row after Up x2"
        );

        assert_ne!(
            highlighted0, highlighted1,
            "highlight must change after Up x1: before={highlighted0:?} after={highlighted1:?}"
        );
        assert_ne!(
            highlighted1, highlighted2,
            "highlight must change after Up x2: before={highlighted1:?} after={highlighted2:?}"
        );

        assert_eq!(
            visible0, visible1,
            "visible set must NOT change on the first Up press (cursor moves within viewport, no scroll yet)"
        );

        assert_eq!(
            visible1, visible2,
            "visible set must NOT change on the second Up press (still within viewport)"
        );
    }

    #[test]
    fn persistent_state_scrolls_only_at_viewport_edge() {
        use crate::tui::event::handle_key;
        use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

        const W: u16 = 80;
        const H: u16 = 9;

        let mut app = App::new();
        app.all_rows = (0..12)
            .map(|i| make_row(&format!("nav-{i:02}"), TEST_NOW - i as i64 * 60, 0, "p"))
            .collect();
        app.recompute();

        let up_key = KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        };

        let mut ts = TableState::default();
        let (_, buf_start) = render_app_small_with_state(&app, W, H, &mut ts);
        let visible_start = visible_commands(&buf_start, W, H);

        for _ in 0..8 {
            handle_key(up_key, &mut app);
            render_app_small_with_state(&app, W, H, &mut ts);
        }

        let (_, buf_end) = render_app_small_with_state(&app, W, H, &mut ts);
        let visible_end = visible_commands(&buf_end, W, H);

        assert_ne!(
            visible_start, visible_end,
            "after pressing Up enough times to leave the viewport, the visible set must have changed (scroll occurred)"
        );

        let highlighted_end = highlighted_command(&buf_end, W, H);
        assert!(
            highlighted_end.is_some(),
            "selected row must be highlighted and visible after scrolling"
        );
    }
}
