//! TuiApp: terminal lifecycle manager + event loop (Phase 2 §2.3).
//!
//! Runs in a dedicated thread. Reads DashboardState via Arc<Mutex<>>.
//! Key bindings: q=quit, p=pause, r=resume, +=speed-up, -=slow-down.

use crate::{panels, state::DashboardState};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{
    io,
    sync::{Arc, Mutex},
    time::Duration,
};

/// TUI application. Owns the terminal handle; `DashboardState` is shared.
pub struct TuiApp {
    state: Arc<Mutex<DashboardState>>,
}

impl TuiApp {
    pub fn new(state: Arc<Mutex<DashboardState>>) -> Self {
        TuiApp { state }
    }

    /// Run the TUI event loop. Blocks until 'q' is pressed or `quit_requested`
    /// is set externally. Called from a dedicated thread.
    pub fn run(&mut self) -> Result<()> {
        // Setup terminal.
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.event_loop(&mut terminal);

        // Restore terminal regardless of error.
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    fn event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        loop {
            // Check quit flag before drawing (engine may have set it).
            {
                let s = self.state.lock().unwrap();
                if s.quit_requested {
                    return Ok(());
                }
            }

            // Draw frame.
            terminal.draw(|f| {
                let state = self.state.lock().unwrap().clone();
                let area = f.size();

                // Title bar (1 line) + panels.
                let chunks = ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Vertical)
                    .constraints([
                        ratatui::layout::Constraint::Length(1),
                        ratatui::layout::Constraint::Min(0),
                    ])
                    .split(area);

                // Title bar.
                let title = ratatui::widgets::Paragraph::new(format!(
                    " FIXPOINT SWARM-R v3.0.0 — Phase 2 Dashboard  tick={}  run={}",
                    state.tick, state.run_id
                ))
                .style(
                    ratatui::style::Style::default()
                        .fg(ratatui::style::Color::White)
                        .bg(ratatui::style::Color::DarkGray)
                        .add_modifier(ratatui::style::Modifier::BOLD),
                );
                f.render_widget(title, chunks[0]);

                panels::render_all(f, chunks[1], &state);
            })?;

            // Poll for key events (16 ms ≈ 60 fps refresh).
            if event::poll(Duration::from_millis(16))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        if self.handle_key(key.code) {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    /// Handle a key press. Returns `true` if the TUI should exit.
    fn handle_key(&mut self, code: KeyCode) -> bool {
        let mut state = self.state.lock().unwrap();
        match code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                state.quit_requested = true;
                true
            }
            KeyCode::Char('p') | KeyCode::Char('P') => {
                state.paused = true;
                false
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                state.paused = false;
                false
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                state.speed_multiplier = state.speed_multiplier.saturating_add(1).min(16);
                false
            }
            KeyCode::Char('-') => {
                state.speed_multiplier = state.speed_multiplier.saturating_sub(1).max(1);
                false
            }
            KeyCode::Esc => {
                state.quit_requested = true;
                true
            }
            _ => false,
        }
    }
}
