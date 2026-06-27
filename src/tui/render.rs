use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
    Frame,
};

use crate::search::{Column, CommandRow};
use crate::tui::app::{App, Confirm, Mode, Tab, WsPane};
use crate::tui::time::format_relative;
use crate::workspaces::WorkspaceRow;

const ACCENT: Color = Color::Cyan;
const DIM_COLOR: Color = Color::DarkGray;
const EDIT_MODAL_W: u16 = 60;
const EDIT_MODAL_H: u16 = 5;
const BORDER_COLOR: Color = Color::DarkGray;

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

fn short_id(id: &str) -> &str {
    let end = id.char_indices().nth(8).map(|(i, _)| i).unwrap_or(id.len());
    &id[..end]
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
    if let Some(ref s) = app.filters.session {
        parts.push(format!("[session:{}]", short_id(s)));
    }
    parts.join(" ")
}

pub fn tui_header(col: &Column) -> &'static str {
    match col {
        Column::Timestamp => "time",
        Column::Duration => "dur",
        Column::Exit => "exit",
        Column::Project => "project",
        Column::Command => "command",
        Column::Tags => "tags",
        Column::Directory => "dir",
    }
}

fn col_cap(col: &Column) -> u16 {
    match col {
        Column::Timestamp => 9,
        Column::Duration => 7,
        Column::Exit => 4,
        Column::Project => 20,
        Column::Tags => 16,
        Column::Directory => 30,
        Column::Command => u16::MAX,
    }
}

fn col_text(col: &Column, row: &CommandRow, now: i64) -> String {
    match col {
        Column::Timestamp => format_relative(row.timestamp, now),
        Column::Duration => format_duration(row.duration_ms),
        Column::Exit => exit_text(row.exit_code).0.to_string(),
        Column::Project => row.project.clone(),
        Column::Command => display_command(&row.command),
        Column::Tags => row.tags.clone(),
        Column::Directory => row.directory.clone(),
    }
}

fn tui_cell(col: &Column, row: &CommandRow, now: i64, width: u16) -> Cell<'static> {
    let text = truncate(&col_text(col, row, now), width as usize);
    let style = match col {
        Column::Timestamp => Style::default().fg(DIM_COLOR),
        Column::Duration => Style::default().fg(ACCENT),
        Column::Exit => Style::default().fg(exit_text(row.exit_code).1),
        Column::Project => Style::default().fg(Color::Blue),
        Column::Directory => Style::default().fg(DIM_COLOR),
        Column::Command | Column::Tags => Style::default(),
    };
    Cell::from(Line::from(vec![Span::styled(text, style)]))
}

pub fn resolve_tui_columns(names: &[String]) -> Vec<Column> {
    use crate::config::default_tui_columns;

    if names.is_empty() {
        eprintln!("thoth: unknown TUI column(s) in config, using defaults");
        return resolve_tui_columns(&default_tui_columns());
    }

    let resolved: Vec<Option<Column>> = names.iter().map(|n| Column::from_name(n)).collect();
    if resolved.iter().any(|c| c.is_none()) {
        eprintln!("thoth: unknown TUI column(s) in config, using defaults");
        return resolve_tui_columns(&default_tui_columns());
    }
    resolved.into_iter().flatten().collect()
}

fn render_tab_bar(frame: &mut Frame, area: ratatui::layout::Rect, app: &App) {
    let accent_style = Style::default().fg(ACCENT).add_modifier(Modifier::BOLD);
    let dim_style = Style::default().fg(DIM_COLOR).add_modifier(Modifier::DIM);
    let active_style = accent_style.add_modifier(Modifier::REVERSED);

    let (history_style, workspaces_style) = match app.tab {
        Tab::History => (active_style, dim_style),
        Tab::Workspaces => (dim_style, active_style),
    };

    let line = Line::from(vec![
        Span::styled(" History ", history_style),
        Span::styled("  ", dim_style),
        Span::styled(" Workspaces ", workspaces_style),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn render_history_pane(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    app: &App,
    now: i64,
    is_bottom: bool,
    columns: &[Column],
    table_state: &mut TableState,
) {
    let dim_style = Style::default().fg(DIM_COLOR).add_modifier(Modifier::DIM);
    let border_style = Style::default().fg(BORDER_COLOR);

    let list_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(Span::styled(" history ", dim_style));

    let inner_list_area = list_block.inner(area);
    frame.render_widget(list_block, area);

    let has_command_col = columns.iter().any(|c| matches!(c, Column::Command));
    let n_cols = columns.len();
    let gaps: u16 = n_cols.saturating_sub(1) as u16;

    let ordered: Vec<usize> = if is_bottom {
        app.filtered.iter().rev().copied().collect()
    } else {
        app.filtered.to_vec()
    };

    let flex_idx: Option<usize> = if has_command_col {
        columns.iter().position(|c| matches!(c, Column::Command))
    } else {
        n_cols.checked_sub(1)
    };

    let content_w: Vec<u16> = columns
        .iter()
        .enumerate()
        .map(|(i, col)| {
            if Some(i) == flex_idx {
                return 0;
            }
            let header_len = tui_header(col).chars().count() as u16;
            let max_content = ordered
                .iter()
                .map(|&fi| col_text(col, &app.all_rows[fi], now).chars().count() as u16)
                .max()
                .unwrap_or(0);
            header_len.max(max_content).min(col_cap(col))
        })
        .collect();

    let fixed_total: u16 = content_w.iter().sum();
    let flex_w: u16 = inner_list_area
        .width
        .saturating_sub(fixed_total + gaps)
        .max(1);

    let widths: Vec<Constraint> = columns
        .iter()
        .enumerate()
        .map(|(i, _)| {
            if Some(i) == flex_idx {
                Constraint::Min(1)
            } else {
                Constraint::Length(content_w[i])
            }
        })
        .collect();

    let rows: Vec<Row> = ordered
        .iter()
        .map(|&fi| {
            let row = &app.all_rows[fi];
            let cells: Vec<Cell> = columns
                .iter()
                .enumerate()
                .map(|(i, col)| {
                    let w = if Some(i) == flex_idx {
                        flex_w
                    } else {
                        content_w[i]
                    };
                    tui_cell(col, row, now, w)
                })
                .collect();
            Row::new(cells)
        })
        .collect();

    let table_header_style = Style::default()
        .fg(DIM_COLOR)
        .add_modifier(Modifier::DIM)
        .add_modifier(Modifier::BOLD);
    let header_cells: Vec<Cell> = columns
        .iter()
        .map(|col| Cell::from(tui_header(col)).style(table_header_style))
        .collect();
    let table_header = Row::new(header_cells);

    let highlight_style = Style::default()
        .add_modifier(Modifier::REVERSED)
        .add_modifier(Modifier::BOLD);

    let table = Table::new(rows, widths)
        .header(table_header)
        .block(Block::default())
        .row_highlight_style(highlight_style);

    if app.filtered.is_empty() {
        table_state.select(None);
    } else {
        let display_idx = if is_bottom {
            app.filtered.len() - 1 - app.selected
        } else {
            app.selected
        };
        table_state.select(Some(display_idx));
    }
    frame.render_stateful_widget(table, inner_list_area, table_state);
}

fn render_workspaces_pane(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    app: &App,
    now: i64,
    table_state: &mut TableState,
    cmd_table_state: &mut TableState,
) {
    let dim_style = Style::default().fg(DIM_COLOR).add_modifier(Modifier::DIM);
    let highlight_style = Style::default()
        .add_modifier(Modifier::REVERSED)
        .add_modifier(Modifier::BOLD);

    let list_focused = app.ws_pane == WsPane::List;
    let accent_border = Style::default().fg(ACCENT);
    let dim_border = Style::default().fg(BORDER_COLOR);

    let h_chunks = Layout::horizontal([Constraint::Percentage(38), Constraint::Min(1)]).split(area);

    let left_area = h_chunks[0];
    let right_area = h_chunks[1];

    let workspaces_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if list_focused {
            accent_border
        } else {
            dim_border
        })
        .title(Span::styled(
            " workspaces ",
            if list_focused {
                Style::default().fg(ACCENT)
            } else {
                dim_style
            },
        ));

    let inner_left = workspaces_block.inner(left_area);
    frame.render_widget(workspaces_block, left_area);

    if app.workspaces.is_empty() {
        let empty = Paragraph::new("no workspaces").style(dim_style);
        frame.render_widget(empty, inner_left);
    } else {
        let rows: Vec<Row> = app.workspaces.iter().map(|w| ws_row(w, now)).collect();

        let widths = vec![
            Constraint::Min(1),
            Constraint::Length(6),
            Constraint::Length(9),
        ];

        let table = Table::new(rows, widths)
            .block(Block::default())
            .row_highlight_style(highlight_style);

        table_state.select(Some(app.ws_selected));
        frame.render_stateful_widget(table, inner_left, table_state);
    }

    let commands_focused = app.ws_pane == WsPane::Commands;
    let commands_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if commands_focused {
            accent_border
        } else {
            dim_border
        })
        .title(Span::styled(
            " commands ",
            if commands_focused {
                Style::default().fg(ACCENT)
            } else {
                dim_style
            },
        ));

    let inner_right = commands_block.inner(right_area);
    frame.render_widget(commands_block, right_area);

    if app.ws_commands.is_empty() {
        let empty = Paragraph::new("no commands").style(dim_style);
        frame.render_widget(empty, inner_right);
    } else {
        let rows: Vec<Row> = app
            .ws_commands
            .iter()
            .map(|cmd| {
                let time_text = truncate(&format_relative(cmd.timestamp, now), 9);
                let exit_text_val = exit_text(cmd.exit_code);
                let cmd_text = truncate(
                    &display_command(&cmd.command),
                    inner_right.width.saturating_sub(16) as usize,
                );
                Row::new(vec![
                    Cell::from(Span::styled(time_text, Style::default().fg(DIM_COLOR))),
                    Cell::from(Span::styled(
                        exit_text_val.0,
                        Style::default().fg(exit_text_val.1),
                    )),
                    Cell::from(cmd_text),
                ])
            })
            .collect();

        let widths = vec![
            Constraint::Length(9),
            Constraint::Length(4),
            Constraint::Min(1),
        ];

        if commands_focused {
            let table = Table::new(rows, widths)
                .block(Block::default())
                .row_highlight_style(highlight_style);
            cmd_table_state.select(Some(app.ws_cmd_selected));
            frame.render_stateful_widget(table, inner_right, cmd_table_state);
        } else {
            let table = Table::new(rows, widths).block(Block::default());
            frame.render_widget(table, inner_right);
        }
    }
}

fn ws_row(w: &WorkspaceRow, now: i64) -> Row<'static> {
    let name_text = truncate(&w.name, 20);
    let count_text = w.command_count.to_string();
    let last_text = truncate(&format_relative(w.last_ts, now), 9);
    Row::new(vec![
        Cell::from(Span::styled(name_text, Style::default().fg(Color::Blue))),
        Cell::from(Span::styled(count_text, Style::default().fg(DIM_COLOR))),
        Cell::from(Span::styled(last_text, Style::default().fg(DIM_COLOR))),
    ])
}

pub fn draw(
    frame: &mut Frame,
    app: &App,
    now: i64,
    is_bottom: bool,
    columns: &[Column],
    table_state: &mut TableState,
) {
    draw_with_cmd_state(
        frame,
        app,
        now,
        is_bottom,
        columns,
        table_state,
        &mut TableState::default(),
    );
}

pub fn draw_with_cmd_state(
    frame: &mut Frame,
    app: &App,
    now: i64,
    is_bottom: bool,
    columns: &[Column],
    table_state: &mut TableState,
    cmd_table_state: &mut TableState,
) {
    let area = frame.area();

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);

    let tabbar_area = chunks[0];
    let header_area = chunks[1];
    let middle_area = chunks[2];
    let query_area = chunks[3];
    let status_area = chunks[4];
    let controls_area = chunks[5];

    render_tab_bar(frame, tabbar_area, app);

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
    frame.render_widget(Paragraph::new(header_line), header_area);

    match app.tab {
        Tab::History => {
            render_history_pane(
                frame,
                middle_area,
                app,
                now,
                is_bottom,
                columns,
                table_state,
            );
        }
        Tab::Workspaces => {
            render_workspaces_pane(frame, middle_area, app, now, table_state, cmd_table_state);
        }
    }

    if app.confirm.is_some() {
        render_confirm_modal(frame, middle_area, app);
    }
    if let Some(ref es) = app.edit {
        render_edit_modal(frame, middle_area, es);
    }

    let query_text = format!("> {}", app.query);
    frame.render_widget(Paragraph::new(query_text), query_area);

    let chips = filter_chips(app);
    let result_count = app.filtered.len();
    let status_text = if chips.is_empty() {
        format!("{result_count} results")
    } else {
        format!("{chips}  {result_count} results")
    };
    frame.render_widget(
        Paragraph::new(status_text).alignment(Alignment::Left),
        status_area,
    );

    let controls_line = match app.tab {
        Tab::History => match app.mode {
            Mode::Insert => Line::from(vec![
                Span::styled(" ↑↓", accent_style),
                Span::styled(" nav", dim_style),
                Span::styled(" · ", dim_style),
                Span::styled("↵", accent_style),
                Span::styled(" run", dim_style),
                Span::styled(" · ", dim_style),
                Span::styled("⇥", accent_style),
                Span::styled(" edit", dim_style),
                Span::styled(" · ", dim_style),
                Span::styled("esc", accent_style),
                Span::styled(" normal", dim_style),
                Span::styled(" · ", dim_style),
                Span::styled("^c", accent_style),
                Span::styled(" quit", dim_style),
            ]),
            Mode::Normal => Line::from(vec![
                Span::styled(" j/k", accent_style),
                Span::styled(" move", dim_style),
                Span::styled(" · ", dim_style),
                Span::styled("d", accent_style),
                Span::styled(" delete", dim_style),
                Span::styled(" · ", dim_style),
                Span::styled("e", accent_style),
                Span::styled(" edit", dim_style),
                Span::styled(" · ", dim_style),
                Span::styled("↵", accent_style),
                Span::styled(" run", dim_style),
                Span::styled(" · ", dim_style),
                Span::styled("i", accent_style),
                Span::styled(" search", dim_style),
                Span::styled(" · ", dim_style),
                Span::styled("q", accent_style),
                Span::styled(" quit", dim_style),
            ]),
        },
        Tab::Workspaces => match app.ws_pane {
            WsPane::List => Line::from(vec![
                Span::styled(" ←→", accent_style),
                Span::styled(" tabs", dim_style),
                Span::styled(" · ", dim_style),
                Span::styled("↑↓", accent_style),
                Span::styled(" workspace", dim_style),
                Span::styled(" · ", dim_style),
                Span::styled("⇥", accent_style),
                Span::styled(" commands", dim_style),
                Span::styled(" · ", dim_style),
                Span::styled("↵", accent_style),
                Span::styled(" replay", dim_style),
                Span::styled(" · ", dim_style),
                Span::styled("esc", accent_style),
                Span::styled(" quit", dim_style),
            ]),
            WsPane::Commands => Line::from(vec![
                Span::styled(" ←→", accent_style),
                Span::styled(" tabs", dim_style),
                Span::styled(" · ", dim_style),
                Span::styled("↑↓", accent_style),
                Span::styled(" command", dim_style),
                Span::styled(" · ", dim_style),
                Span::styled("d", accent_style),
                Span::styled(" delete", dim_style),
                Span::styled(" · ", dim_style),
                Span::styled("e", accent_style),
                Span::styled(" edit", dim_style),
                Span::styled(" · ", dim_style),
                Span::styled("⇥", accent_style),
                Span::styled(" back", dim_style),
                Span::styled(" · ", dim_style),
                Span::styled("esc", accent_style),
                Span::styled(" quit", dim_style),
            ]),
        },
    };
    frame.render_widget(Paragraph::new(controls_line), controls_area);
}

pub fn format_action_line(action: Option<&crate::tui::app::Action>) -> Option<String> {
    use crate::tui::app::Action;
    match action {
        Some(Action::Run(cmd)) => Some(format!("RUN:{cmd}")),
        Some(Action::Edit(cmd)) => Some(format!("EDIT:{cmd}")),
        Some(Action::Replay(path)) => Some(format!("REPLAY:{path}")),
        None => None,
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

fn render_confirm_modal(frame: &mut Frame, area: Rect, app: &crate::tui::app::App) {
    let Some(confirm) = &app.confirm else { return };
    let dim_style = Style::default().fg(DIM_COLOR);
    let accent_style = Style::default().fg(ACCENT).add_modifier(Modifier::BOLD);

    let modal_w = 50u16;
    let modal_h = 4u16;
    let modal_area = centered_rect(modal_w, modal_h, area);

    frame.render_widget(Clear, modal_area);

    match confirm {
        Confirm::Replay(r) => {
            let ws_name = truncate(&r.workspace, 20);
            let title = format!(" Replay '{ws_name}' ({} commands)? ", r.count);
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(ACCENT))
                .title(Span::styled(title, accent_style));

            let inner = block.inner(modal_area);
            frame.render_widget(block, modal_area);

            let hint = Paragraph::new(Line::from(vec![
                Span::styled("[y]", accent_style),
                Span::styled(" run all   ", dim_style),
                Span::styled("[n]", accent_style),
                Span::styled(" cancel", dim_style),
            ]))
            .alignment(Alignment::Center);
            frame.render_widget(hint, inner);
        }
        Confirm::Delete(d) => {
            let label = truncate(&d.label, 30);
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Red))
                .title(Span::styled(
                    " Delete this command? ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ));

            let inner = block.inner(modal_area);
            frame.render_widget(block, modal_area);

            let hint = Paragraph::new(Line::from(vec![
                Span::styled(&label, dim_style),
                Span::styled("  ", dim_style),
                Span::styled("[y]", accent_style),
                Span::styled(" delete   ", dim_style),
                Span::styled("[n]", accent_style),
                Span::styled(" cancel", dim_style),
            ]))
            .alignment(Alignment::Center);
            frame.render_widget(hint, inner);
        }
    }
}

fn render_edit_modal(frame: &mut Frame, area: Rect, es: &crate::tui::app::EditState) {
    let dim_style = Style::default().fg(DIM_COLOR);
    let accent_style = Style::default().fg(ACCENT).add_modifier(Modifier::BOLD);

    let modal_area = centered_rect(EDIT_MODAL_W, EDIT_MODAL_H, area);
    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .title(Span::styled(" Edit command ", accent_style));

    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).split(inner);

    let display = format!("{}_", es.buffer);
    frame.render_widget(
        Paragraph::new(Span::styled(display, Style::default())),
        chunks[0],
    );

    let hint = Paragraph::new(Line::from(vec![
        Span::styled("↵", accent_style),
        Span::styled(" save   ", dim_style),
        Span::styled("esc", accent_style),
        Span::styled(" cancel", dim_style),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(hint, chunks[1]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::default_tui_columns;
    use crate::tui::app::{Action, App, Confirm, ConfirmDelete, DeleteOrigin, Tab, WsPane};
    use crate::workspaces::WorkspaceRow;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    const TEST_NOW: i64 = 1_000_000_000;
    const TEST_WIDTH: u16 = 80;
    const TEST_HEIGHT: u16 = 24;

    fn make_row(cmd: &str, ts: i64, exit: i64, project: &str) -> CommandRow {
        CommandRow {
            id: 0,
            command: cmd.to_string(),
            timestamp: ts,
            exit_code: exit,
            project: project.to_string(),
            directory: "/tmp".to_string(),
            tags: "[]".to_string(),
            session_id: "s1".to_string(),
            duration_ms: 100,
            workspace: None,
        }
    }

    fn make_workspace(name: &str, count: i64, last_ts: i64) -> WorkspaceRow {
        WorkspaceRow {
            name: name.to_string(),
            command_count: count,
            first_ts: last_ts - 3600,
            last_ts,
        }
    }

    fn default_cols() -> Vec<Column> {
        resolve_tui_columns(&default_tui_columns())
    }

    fn render_app(app: &App) -> String {
        let backend = TestBackend::new(TEST_WIDTH, TEST_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut ts = TableState::default();
        let cols = default_cols();
        terminal
            .draw(|f| draw(f, app, TEST_NOW, true, &cols, &mut ts))
            .unwrap();
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

    fn render_app_buf(app: &App) -> ratatui::buffer::Buffer {
        let backend = TestBackend::new(TEST_WIDTH, TEST_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut ts = TableState::default();
        let cols = default_cols();
        terminal
            .draw(|f| draw(f, app, TEST_NOW, true, &cols, &mut ts))
            .unwrap();
        terminal.backend().buffer().clone()
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

    fn app_with_workspaces() -> App {
        let mut app = App::new();
        app.tab = Tab::Workspaces;
        app.workspaces = vec![
            make_workspace("proj-alpha", 5, TEST_NOW - 3600),
            make_workspace("proj-beta", 3, TEST_NOW - 7200),
        ];
        app.ws_selected = 0;
        app.ws_commands = vec![
            make_row("git status", TEST_NOW - 3500, 0, "proj-alpha"),
            make_row("cargo build", TEST_NOW - 3400, 0, "proj-alpha"),
        ];
        app
    }

    #[test]
    fn tab_bar_shows_both_labels() {
        let app = app_with_rows();
        let text = render_app(&app);
        assert!(text.contains("History"), "tab bar must show 'History'");
        assert!(
            text.contains("Workspaces"),
            "tab bar must show 'Workspaces'"
        );
    }

    #[test]
    fn tab_bar_active_history_is_reversed() {
        let app = app_with_rows();
        let buf = render_app_buf(&app);

        let any_reversed_row0 = (0..TEST_WIDTH).any(|x| {
            buf[(x, 0)]
                .style()
                .add_modifier
                .contains(Modifier::REVERSED)
        });
        assert!(
            any_reversed_row0,
            "tab bar row 0 must have REVERSED on active History tab label"
        );

        let reversed_count_row0 = (0..TEST_WIDTH)
            .filter(|&x| {
                buf[(x, 0)]
                    .style()
                    .add_modifier
                    .contains(Modifier::REVERSED)
            })
            .count();

        assert!(
            reversed_count_row0 < TEST_WIDTH as usize,
            "tab bar must NOT be fully reversed; only active label should be reversed (count={reversed_count_row0})"
        );
    }

    #[test]
    fn header_has_no_full_width_reversed_bar() {
        let app = app_with_rows();
        let buf = render_app_buf(&app);

        let reversed_count = (0..TEST_WIDTH)
            .filter(|&x| {
                buf[(x, 1)]
                    .style()
                    .add_modifier
                    .contains(Modifier::REVERSED)
            })
            .count();
        assert_eq!(
            reversed_count, 0,
            "header row (y=1) must have zero REVERSED cells; found {reversed_count}"
        );
    }

    #[test]
    fn table_header_not_reversed() {
        let app = app_with_rows();
        let buf = render_app_buf(&app);

        let header_row_y = 3u16;
        let reversed_count = (0..TEST_WIDTH)
            .filter(|&x| {
                buf[(x, header_row_y)]
                    .style()
                    .add_modifier
                    .contains(Modifier::REVERSED)
            })
            .count();
        assert_eq!(
            reversed_count, 0,
            "table header row must have zero REVERSED cells; found {reversed_count}"
        );
    }

    #[test]
    fn workspaces_tab_renders_two_panes() {
        let app = app_with_workspaces();
        let text = render_app(&app);
        assert!(
            text.contains("workspaces"),
            "Workspaces tab must show 'workspaces' pane title"
        );
        assert!(
            text.contains("commands"),
            "Workspaces tab must show 'commands' pane title"
        );
    }

    #[test]
    fn workspaces_tab_shows_workspace_row_highlighted() {
        let app = app_with_workspaces();
        let buf = render_app_buf(&app);

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
            "Workspaces tab must have REVERSED on selected row"
        );
    }

    #[test]
    fn workspaces_tab_shows_command_text() {
        let app = app_with_workspaces();
        let text = render_app(&app);
        assert!(
            text.contains("git status"),
            "Workspaces tab right pane must show workspace command text"
        );
    }

    #[test]
    fn history_tab_still_renders_history_block() {
        let app = app_with_rows();
        let text = render_app(&app);
        assert!(
            text.contains("history"),
            "History tab must show 'history' block title"
        );
        assert!(
            text.contains("git status"),
            "History tab must show commands"
        );
    }

    #[test]
    fn controls_history_insert_shows_nav_and_run() {
        let app = app_with_rows();
        let text = render_app(&app);
        assert!(
            text.contains("nav"),
            "History Insert controls must contain 'nav'"
        );
        assert!(
            text.contains("run"),
            "History Insert controls must contain 'run'"
        );
        assert!(
            text.contains("edit"),
            "History Insert controls must contain 'edit'"
        );
        assert!(
            text.contains("normal"),
            "History Insert controls must hint 'normal' for esc"
        );
    }

    #[test]
    fn controls_history_normal_shows_jk_and_delete() {
        let mut app = app_with_rows();
        app.mode = crate::tui::app::Mode::Normal;
        let text = render_app(&app);
        assert!(
            text.contains("move"),
            "History Normal controls must contain 'move'"
        );
        assert!(
            text.contains("delete"),
            "History Normal controls must contain 'delete'"
        );
        assert!(
            text.contains("run"),
            "History Normal controls must contain 'run'"
        );
        assert!(
            text.contains("search"),
            "History Normal controls must contain 'search'"
        );
        assert!(
            text.contains("quit"),
            "History Normal controls must contain 'quit'"
        );
    }

    #[test]
    fn controls_workspaces_shows_workspace_and_quit() {
        let app = app_with_workspaces();
        let text = render_app(&app);
        assert!(
            text.contains("workspace"),
            "Workspaces controls must contain 'workspace'"
        );
        assert!(
            !text.contains("open"),
            "Workspaces controls must NOT contain 'open' in PR2"
        );
        assert!(
            text.contains("quit"),
            "Workspaces controls must contain 'quit'"
        );
        assert!(
            text.contains("tabs"),
            "Workspaces controls must hint 'tabs'"
        );
    }

    #[test]
    fn session_chip_shows_when_session_filter_set() {
        let mut app = app_with_rows();
        app.filters.session = Some("abcdefgh-1234-5678".into());
        let text = render_app(&app);
        assert!(
            text.contains("[session:abcdefgh]"),
            "status bar must show session chip with 8-char prefix; got:\n{text}"
        );
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

        let buf = render_app_buf(&app);

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
        assert!(!text.contains('✓'), "exit 0 must NOT render as glyph");
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
        assert!(!text.contains('✗'), "exit nonzero must NOT render as glyph");
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
        const SMALL_HEIGHT: u16 = 10;
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
        let cols = default_cols();
        terminal
            .draw(|f| draw(f, &app, TEST_NOW, true, &cols, &mut ts))
            .unwrap();
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

        let (default_buf, _) = render_app_small(&app, 80, 12);
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

        let (after_up3, _) = render_app_small(&app, 80, 12);
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
        let cols = default_cols();
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| draw(f, app, TEST_NOW, true, &cols, ts))
            .unwrap();
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
    fn top_orientation_renders_newest_above_oldest() {
        let app = app_bottom_anchored();
        let cols = default_cols();
        let backend = TestBackend::new(TEST_WIDTH, TEST_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut ts = TableState::default();
        terminal
            .draw(|f| draw(f, &app, TEST_NOW, false, &cols, &mut ts))
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let newest_y = row_y_of("newest-cmd", &buf, TEST_WIDTH, TEST_HEIGHT)
            .expect("newest-cmd must appear in buffer");
        let oldest_y = row_y_of("oldest-cmd", &buf, TEST_WIDTH, TEST_HEIGHT)
            .expect("oldest-cmd must appear in buffer");
        assert!(
            newest_y < oldest_y,
            "with top orientation newest must be above oldest; newest_y={newest_y} oldest_y={oldest_y}"
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

        const H: u16 = 10;
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
        const H: u16 = 12;

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
        const H: u16 = 12;

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
        const H: u16 = 12;

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

    #[test]
    fn table_header_shows_default_column_labels() {
        let app = app_with_rows();
        let text = render_app(&app);
        assert!(
            text.contains("time"),
            "table header must contain label 'time'; got:\n{text}"
        );
        assert!(
            text.contains("dur"),
            "table header must contain label 'dur'; got:\n{text}"
        );
        assert!(
            text.contains("exit"),
            "table header must contain label 'exit'; got:\n{text}"
        );
        assert!(
            text.contains("project"),
            "table header must contain label 'project'; got:\n{text}"
        );
        assert!(
            text.contains("command"),
            "table header must contain label 'command'; got:\n{text}"
        );
    }

    #[test]
    fn reduced_columns_only_show_selected_headers() {
        let mut app = App::new();
        app.all_rows = vec![make_row("my-cmd", TEST_NOW - 10, 1, "proj")];
        app.recompute();

        let cols = resolve_tui_columns(&["exit".to_string(), "command".to_string()]);
        let backend = TestBackend::new(TEST_WIDTH, TEST_HEIGHT);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut ts = TableState::default();
        terminal
            .draw(|f| draw(f, &app, TEST_NOW, true, &cols, &mut ts))
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let mut lines: Vec<String> = Vec::new();
        for row in 0..TEST_HEIGHT {
            let mut line = String::new();
            for col in 0..TEST_WIDTH {
                line.push(buf[(col, row)].symbol().chars().next().unwrap_or(' '));
            }
            lines.push(line.trim_end().to_string());
        }
        let text = lines.join("\n");

        assert!(
            text.contains("exit"),
            "reduced cols must show 'exit' header; got:\n{text}"
        );
        assert!(
            text.contains("command"),
            "reduced cols must show 'command' header; got:\n{text}"
        );
        assert!(
            !text.contains("time"),
            "reduced cols must NOT show 'time' header; got:\n{text}"
        );
        assert!(
            !text.contains("dur"),
            "reduced cols must NOT show 'dur' header; got:\n{text}"
        );
        assert!(
            text.contains("my-cmd"),
            "command cell must render; got:\n{text}"
        );
    }

    #[test]
    fn resolve_tui_columns_all_valid_returns_them() {
        let cols = resolve_tui_columns(&["exit".to_string(), "command".to_string()]);
        assert_eq!(cols.len(), 2);
        assert!(matches!(cols[0], Column::Exit));
        assert!(matches!(cols[1], Column::Command));
    }

    #[test]
    fn resolve_tui_columns_unknown_name_returns_defaults() {
        let cols = resolve_tui_columns(&["exit".to_string(), "bogus".to_string()]);
        let defaults = resolve_tui_columns(&default_tui_columns());
        assert_eq!(cols.len(), defaults.len());
    }

    #[test]
    fn resolve_tui_columns_empty_returns_defaults() {
        let cols = resolve_tui_columns(&[]);
        let defaults = resolve_tui_columns(&default_tui_columns());
        assert_eq!(cols.len(), defaults.len());
    }

    #[test]
    fn dump_reduced_columns_buffer() {
        let mut app = App::new();
        app.all_rows = vec![
            make_row("git status", TEST_NOW - 10, 0, "proj"),
            make_row("cargo build", TEST_NOW - 60, 1, "proj"),
            make_row("docker run nginx", TEST_NOW - 300, 0, "proj"),
        ];
        app.recompute();

        let cols_default = default_cols();
        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut ts = TableState::default();
        terminal
            .draw(|f| draw(f, &app, TEST_NOW, true, &cols_default, &mut ts))
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let mut lines: Vec<String> = Vec::new();
        for row in 0..10u16 {
            let mut line = String::new();
            for col in 0..80u16 {
                line.push(buf[(col, row)].symbol().chars().next().unwrap_or(' '));
            }
            lines.push(line.trim_end().to_string());
        }
        println!("=== DEFAULT COLUMNS (80x10) ===");
        println!("{}", lines.join("\n"));

        let cols_reduced = resolve_tui_columns(&["exit".to_string(), "command".to_string()]);
        let backend2 = TestBackend::new(80, 10);
        let mut terminal2 = Terminal::new(backend2).unwrap();
        let mut ts2 = TableState::default();
        terminal2
            .draw(|f| draw(f, &app, TEST_NOW, true, &cols_reduced, &mut ts2))
            .unwrap();
        let buf2 = terminal2.backend().buffer().clone();
        let mut lines2: Vec<String> = Vec::new();
        for row in 0..10u16 {
            let mut line = String::new();
            for col in 0..80u16 {
                line.push(buf2[(col, row)].symbol().chars().next().unwrap_or(' '));
            }
            lines2.push(line.trim_end().to_string());
        }
        println!("\n=== REDUCED COLUMNS [exit, command] (80x10) ===");
        println!("{}", lines2.join("\n"));

        assert!(lines.iter().any(|l| l.contains("time")));
        assert!(lines2
            .iter()
            .any(|l| l.contains("exit") && l.contains("command")));
        assert!(lines2.iter().all(|l| !l.contains("time")));
    }

    #[test]
    fn format_action_line_replay_produces_correct_prefix() {
        let action = Action::Replay("( cd '/x' && ls )".to_string());
        let result = format_action_line(Some(&action));
        assert_eq!(result, Some("REPLAY:( cd '/x' && ls )".to_string()));
    }

    #[test]
    fn controls_workspaces_shows_replay_hint() {
        let app = app_with_workspaces();
        let text = render_app(&app);
        assert!(
            text.contains("replay"),
            "Workspaces controls must contain 'replay'; got:\n{text}"
        );
    }

    #[test]
    fn confirm_modal_shows_when_confirm_is_set() {
        let mut app = app_with_workspaces();
        app.confirm = Some(crate::tui::app::Confirm::Replay(
            crate::tui::app::ConfirmReplay {
                workspace: "proj-alpha".into(),
                count: 5,
            },
        ));
        let text = render_app(&app);
        assert!(
            text.contains("proj-alpha"),
            "confirm modal must show workspace name; got:\n{text}"
        );
        assert!(
            text.contains("5 commands") || text.contains("5"),
            "confirm modal must show command count; got:\n{text}"
        );
        assert!(
            text.contains("[y]") || text.contains("y"),
            "confirm modal must show y hint; got:\n{text}"
        );
        assert!(
            text.contains("[n]") || text.contains("n"),
            "confirm modal must show n hint; got:\n{text}"
        );
    }

    #[test]
    fn dump_workspaces_tab_buffer() {
        const NOW: i64 = 1_000_000_000;
        const W: u16 = 100;
        const H: u16 = 16;

        let mut app = App::new();
        app.tab = Tab::Workspaces;
        app.workspaces = vec![
            make_workspace("proj-alpha", 5, NOW - 3600),
            make_workspace("proj-beta", 3, NOW - 7200),
        ];
        app.ws_selected = 0;
        app.ws_commands = vec![
            make_row("git status", NOW - 3500, 0, "proj-alpha"),
            make_row("cargo build --release", NOW - 3400, 0, "proj-alpha"),
            make_row("cargo test", NOW - 3200, 1, "proj-alpha"),
        ];

        let cols = default_cols();
        let backend = TestBackend::new(W, H);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut ts = TableState::default();
        terminal
            .draw(|f| draw(f, &app, NOW, true, &cols, &mut ts))
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let mut lines: Vec<String> = Vec::new();
        for row in 0..H {
            let mut line = String::new();
            for col in 0..W {
                line.push(buf[(col, row)].symbol().chars().next().unwrap_or(' '));
            }
            lines.push(line.trim_end().to_string());
        }
        let text = lines.join("\n");
        println!("=== WORKSPACES TAB (100x16) ===");
        println!("{text}");

        assert!(text.contains("workspaces"), "must show workspaces pane");
        assert!(text.contains("commands"), "must show commands pane");
    }

    #[test]
    fn delete_confirm_modal_shows_delete_text() {
        let mut app = app_with_workspaces();
        app.confirm = Some(Confirm::Delete(ConfirmDelete {
            id: 42,
            label: "cargo build --release".into(),
            origin: DeleteOrigin::History,
        }));
        let text = render_app(&app);
        assert!(
            text.contains("Delete"),
            "delete confirm modal must show 'Delete'; got:\n{text}"
        );
        assert!(
            text.contains("[y]") || text.contains("y"),
            "delete confirm modal must show y hint; got:\n{text}"
        );
        assert!(
            text.contains("[n]") || text.contains("n"),
            "delete confirm modal must show n hint; got:\n{text}"
        );
    }

    #[test]
    fn ws_list_pane_focused_has_accent_border() {
        let app = app_with_workspaces();
        assert_eq!(app.ws_pane, WsPane::List);
        let buf = render_app_buf(&app);
        let list_pane_area_x = 1u16;
        let any_accent_in_left_border = (0..TEST_HEIGHT).any(|row_y| {
            let cell = &buf[(list_pane_area_x, row_y)];
            cell.style().fg == Some(ACCENT)
        });
        assert!(
            any_accent_in_left_border,
            "focused List pane must have ACCENT border somewhere; ws_pane=List"
        );
    }

    #[test]
    fn ws_commands_pane_focused_has_accent_border() {
        let mut app = app_with_workspaces();
        app.ws_pane = WsPane::Commands;
        let buf = render_app_buf(&app);
        let cmd_pane_x = (TEST_WIDTH as f32 * 0.38) as u16 + 1;
        let any_accent_in_cmd_border = (0..TEST_HEIGHT).any(|row_y| {
            let cell = &buf[(cmd_pane_x, row_y)];
            cell.style().fg == Some(ACCENT)
        });
        assert!(
            any_accent_in_cmd_border,
            "focused Commands pane must have ACCENT border somewhere; ws_pane=Commands"
        );
    }

    #[test]
    fn controls_workspaces_commands_pane_shows_delete_hint() {
        let mut app = app_with_workspaces();
        app.ws_pane = WsPane::Commands;
        let text = render_app(&app);
        assert!(
            text.contains("delete"),
            "Workspaces Commands controls must contain 'delete'; got:\n{text}"
        );
        assert!(
            text.contains("back"),
            "Workspaces Commands controls must contain 'back'; got:\n{text}"
        );
    }

    #[test]
    fn dump_pr4a_screenshots() {
        const NOW: i64 = 1_000_000_000;
        const W: u16 = 100;
        const H: u16 = 18;

        let make_r = |cmd: &str, ts: i64, exit: i64| CommandRow {
            id: 0,
            command: cmd.to_string(),
            timestamp: ts,
            exit_code: exit,
            project: "proj".to_string(),
            directory: "/home/user/proj".to_string(),
            tags: "[]".to_string(),
            session_id: "s1".to_string(),
            duration_ms: 100,
            workspace: None,
        };

        let make_rows = || {
            vec![
                make_r("git status", NOW - 60, 0),
                make_r("cargo build --release", NOW - 300, 0),
                make_r("cargo test", NOW - 1200, 0),
                make_r("docker run -p 8080:80 nginx", NOW - 3600, 1),
            ]
        };

        let render_text = |app: &App| -> String {
            let backend = TestBackend::new(W, H);
            let mut terminal = Terminal::new(backend).unwrap();
            let mut ts = TableState::default();
            let mut cts = TableState::default();
            let cols = default_cols();
            terminal
                .draw(|f| draw_with_cmd_state(f, app, NOW, true, &cols, &mut ts, &mut cts))
                .unwrap();
            let buf = terminal.backend().buffer().clone();
            let mut lines: Vec<String> = Vec::new();
            for row in 0..H {
                let mut line = String::new();
                for col in 0..W {
                    line.push(buf[(col, row)].symbol().chars().next().unwrap_or(' '));
                }
                lines.push(line.trim_end().to_string());
            }
            lines.join("\n")
        };

        let mut app_a = App::new();
        app_a.all_rows = make_rows();
        app_a.recompute();
        app_a.enter_normal_mode();
        app_a.selected = 1;
        let dump_a = render_text(&app_a);
        println!("\n=== DUMP (a): History in Normal mode ===\n{dump_a}");

        let mut app_b = App::new();
        app_b.all_rows = make_rows();
        app_b.recompute();
        app_b.selected = 0;
        app_b.confirm = Some(Confirm::Delete(ConfirmDelete {
            id: 42,
            label: "cargo test".into(),
            origin: DeleteOrigin::History,
        }));
        let dump_b = render_text(&app_b);
        println!("\n=== DUMP (b): Delete confirm modal ===\n{dump_b}");

        let ws_rows = vec![
            CommandRow {
                id: 1,
                command: "git status".into(),
                timestamp: NOW - 500,
                exit_code: 0,
                project: "proj".into(),
                directory: "/home/user".into(),
                tags: "[]".into(),
                session_id: "s1".into(),
                duration_ms: 50,
                workspace: Some("my-workspace".into()),
            },
            CommandRow {
                id: 2,
                command: "cargo build --release".into(),
                timestamp: NOW - 400,
                exit_code: 0,
                project: "proj".into(),
                directory: "/home/user".into(),
                tags: "[]".into(),
                session_id: "s1".into(),
                duration_ms: 8000,
                workspace: Some("my-workspace".into()),
            },
            CommandRow {
                id: 3,
                command: "cargo test -- --nocapture".into(),
                timestamp: NOW - 300,
                exit_code: 1,
                project: "proj".into(),
                directory: "/home/user".into(),
                tags: "[]".into(),
                session_id: "s1".into(),
                duration_ms: 3000,
                workspace: Some("my-workspace".into()),
            },
        ];
        use crate::workspaces::WorkspaceRow;
        let mut app_c = App::new();
        app_c.tab = Tab::Workspaces;
        app_c.ws_pane = WsPane::Commands;
        app_c.workspaces = vec![WorkspaceRow {
            name: "my-workspace".into(),
            command_count: 3,
            first_ts: NOW - 500,
            last_ts: NOW - 300,
        }];
        app_c.ws_selected = 0;
        app_c.ws_commands = ws_rows;
        app_c.ws_cmd_selected = 1;
        let dump_c = render_text(&app_c);
        println!("\n=== DUMP (c): Workspaces with Commands pane focused ===\n{dump_c}");

        assert!(dump_a.contains("j/k"), "Normal mode controls must show j/k");
        assert!(dump_b.contains("Delete"), "Delete modal must show 'Delete'");
        assert!(
            dump_c.contains("back"),
            "Commands pane controls must show 'back'"
        );
    }

    #[test]
    fn edit_modal_shows_buffer_text_and_hints() {
        use crate::tui::app::EditState;

        let mut app = App::new();
        app.all_rows = vec![make_row("git status", TEST_NOW - 60, 0, "p")];
        app.recompute();
        app.edit = Some(EditState {
            id: 1,
            buffer: "git status --short".into(),
        });
        let text = render_app(&app);
        assert!(
            text.contains("git status --short"),
            "edit modal must show buffer text; got:\n{text}"
        );
        assert!(
            text.contains("save"),
            "edit modal must show 'save' hint; got:\n{text}"
        );
        assert!(
            text.contains("cancel"),
            "edit modal must show 'cancel' hint; got:\n{text}"
        );
        assert!(
            text.contains("Edit command"),
            "edit modal must show 'Edit command' title; got:\n{text}"
        );
    }

    #[test]
    fn history_normal_controls_show_edit_hint() {
        let mut app = app_with_rows();
        app.mode = crate::tui::app::Mode::Normal;
        let text = render_app(&app);
        assert!(
            text.contains("edit"),
            "History Normal controls must contain 'edit' hint; got:\n{text}"
        );
    }

    #[test]
    fn ws_commands_controls_show_edit_hint() {
        let mut app = App::new();
        app.tab = Tab::Workspaces;
        app.ws_pane = WsPane::Commands;
        let text = render_app(&app);
        assert!(
            text.contains("edit"),
            "Workspaces Commands controls must contain 'edit' hint; got:\n{text}"
        );
    }
}
