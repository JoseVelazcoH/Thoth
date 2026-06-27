pub mod app;
pub mod event;
pub mod fuzzy;
pub mod render;
pub mod time;

use std::fs::OpenOptions;
use std::panic;

use crossterm::{
    event::{self as ct_event, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, widgets::TableState, Terminal};
use rusqlite::Connection;

use crate::error::ThothError;
use crate::tui::app::App;
use crate::tui::event::{handle_key, Outcome};
use crate::tui::render::format_action_line;

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

pub fn run(conn: &Connection, now: i64, is_bottom: bool) -> Result<(), ThothError> {
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

    let mut app = App::new();
    app.reload(conn, now)?;
    let mut table_state = TableState::default();

    loop {
        terminal
            .draw(|f| render::draw(f, &app, now, is_bottom, &mut table_state))
            .map_err(|e| ThothError::Tui(format!("draw failed: {e}")))?;

        if ct_event::poll(std::time::Duration::from_millis(200))
            .map_err(|e| ThothError::Tui(format!("event poll failed: {e}")))?
        {
            if let Event::Key(key) =
                ct_event::read().map_err(|e| ThothError::Tui(format!("event read failed: {e}")))?
            {
                match handle_key(key, &mut app) {
                    Outcome::Exit => break,
                    Outcome::Continue => {}
                }
            }
        }
    }

    drop(_guard);

    if let Some(line) = format_action_line(app.action.as_ref()) {
        println!("{line}");
    }

    Ok(())
}
