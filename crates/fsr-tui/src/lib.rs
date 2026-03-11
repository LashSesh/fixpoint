//! fsr-tui: TUI dashboard for FIXPOINT SWARM-R Phase 2 (spec §2.3, §2.4).
//!
//! 6-panel ratatui dashboard driven by shared DashboardState.
//! Runs in a dedicated thread; engine pushes updates via Arc<Mutex<DashboardState>>.
//!
//! Usage:
//! ```no_run
//! use fsr_tui::{DashboardState, TuiApp};
//! use std::sync::{Arc, Mutex};
//!
//! let state = Arc::new(Mutex::new(DashboardState::default()));
//! let state_clone = Arc::clone(&state);
//!
//! std::thread::spawn(move || {
//!     TuiApp::new(state_clone).run().ok();
//! });
//!
//! // Engine updates state each tick:
//! state.lock().unwrap().tick += 1;
//! ```

pub mod app;
pub mod panels;
pub mod state;

pub use app::TuiApp;
pub use state::{DashboardState, TtcpStatus};

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_dashboard_state_push_event() {
        let mut ds = DashboardState::default();
        ds.push_event("tick 1: gate open");
        ds.push_event("tick 2: settled");
        assert_eq!(ds.event_log.len(), 2);
        assert_eq!(ds.event_log[0], "tick 1: gate open");
    }

    #[test]
    fn test_dashboard_state_event_log_capped() {
        let mut ds = DashboardState::default();
        for i in 0..=state::MAX_EVENT_LOG + 10 {
            ds.push_event(format!("event {}", i));
        }
        assert_eq!(ds.event_log.len(), state::MAX_EVENT_LOG);
    }

    #[test]
    fn test_dashboard_state_arc_mutex_shared() {
        let state = Arc::new(Mutex::new(DashboardState::default()));
        let state2 = Arc::clone(&state);

        {
            let mut s = state.lock().unwrap();
            s.tick = 42;
            s.regime = "Alpha".to_string();
        }

        let s = state2.lock().unwrap();
        assert_eq!(s.tick, 42);
        assert_eq!(s.regime, "Alpha");
    }
}
