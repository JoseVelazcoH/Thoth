pub mod time;

use std::fs::OpenOptions;
use std::panic;

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use rusqlite::Connection;

use crate::error::ThothError;

struct TerminalGuard;

impl TerminalGuard {
    fn new() -> Result<Self, ThothError> {
        let mut tty = OpenOptions::new()
            .write(true)
            .open("/dev/tty")
            .map_err(|e| ThothError::Tui(format!("cannot open /dev/tty for restore: {e}")))?;
        enable_raw_mode().map_err(|e| ThothError::Tui(format!("enable_raw_mode failed: {e}")))?;
        execute!(tty, EnterAlternateScreen)
            .map_err(|e| ThothError::Tui(format!("EnterAlternateScreen failed: {e}")))?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        if let Ok(mut tty) = OpenOptions::new().write(true).open("/dev/tty") {
            let _ = execute!(tty, LeaveAlternateScreen);
        }
    }
}

fn history_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM commands", [], |r| r.get(0))
        .unwrap_or(0)
}

pub fn run(conn: &Connection, _now: i64) -> Result<(), ThothError> {
    let tty = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .map_err(|e| ThothError::Tui(format!("cannot open /dev/tty: {e}")))?;

    let prev_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        if let Ok(mut t) = OpenOptions::new().write(true).open("/dev/tty") {
            let _ = execute!(t, LeaveAlternateScreen);
        }
        prev_hook(info);
    }));

    let _guard = TerminalGuard::new()?;

    let backend = CrosstermBackend::new(tty);
    let mut terminal = Terminal::new(backend)
        .map_err(|e| ThothError::Tui(format!("terminal init failed: {e}")))?;

    let count = history_count(conn);

    loop {
        terminal
            .draw(|f| {
                let area = f.area();
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(3), Constraint::Length(1)])
                    .split(area);

                let block = Block::default().title(" Thoth ").borders(Borders::ALL);
                let inner = block.inner(chunks[0]);
                f.render_widget(block, chunks[0]);

                let body =
                    Paragraph::new(format!("History count: {count}\n\npress q or Esc to quit"))
                        .alignment(Alignment::Left);
                f.render_widget(body, inner);
            })
            .map_err(|e| ThothError::Tui(format!("draw failed: {e}")))?;

        if event::poll(std::time::Duration::from_millis(200))
            .map_err(|e| ThothError::Tui(format!("event poll failed: {e}")))?
        {
            if let Event::Key(key) =
                event::read().map_err(|e| ThothError::Tui(format!("event read failed: {e}")))?
            {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
