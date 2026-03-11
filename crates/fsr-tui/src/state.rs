//! DashboardState: shared state between the engine thread and TUI render thread.
//!
//! The engine holds an Arc<Mutex<DashboardState>> and updates it after each
//! macro-cycle. The TUI render loop reads a clone each frame.

use fsr_ttcp::TtcpCrystal;
use fsr_types::Q32;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Maximum event log entries kept in dashboard state.
pub const MAX_EVENT_LOG: usize = 100;

/// TTCP cascade status shown in panel 6.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TtcpStatus {
    /// Cascade level currently active (0 = inactive, 1–3 = cascade depth).
    pub level: u8,
    /// Total crystals found since run start.
    pub crystals_found: u64,
    /// Tick of last crystal, if any.
    pub last_crystal_tick: Option<u64>,
    /// Most recent crystal (for display).
    pub last_crystal: Option<TtcpCrystal>,
}

/// Complete dashboard state: one snapshot per tick, cloned into TUI thread.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DashboardState {
    // ── Tick / identity ────────────────────────────────────────────────────
    pub tick: u64,
    pub run_id: String,

    // ── FSM states ─────────────────────────────────────────────────────────
    pub regime: String,
    pub integrity: String,
    pub resource: String,

    // ── Gate ───────────────────────────────────────────────────────────────
    pub gate_open: bool,
    pub gamma_score: Q32,

    // ── Resonance metrics ──────────────────────────────────────────────────
    pub si: Q32,
    pub psi: Q32,
    pub rho: Q32,
    pub omega: Q32,
    pub kappa: Q32,
    pub entropy: Q32,
    pub momentum: Q32,

    // ── Candidates / P&L ───────────────────────────────────────────────────
    pub candidates_found: usize,
    pub wind_count: u64,
    pub settled_cycles: u64,
    pub aborted_cycles: u64,
    pub current_drawdown: Q32,

    // ── Evidence chain ─────────────────────────────────────────────────────
    pub shadow_head: String,
    pub shadow_event_count: u64,
    pub commitment_event_count: u64,

    // ── Event log (last MAX_EVENT_LOG entries) ─────────────────────────────
    pub event_log: VecDeque<String>,

    // ── TTCP ───────────────────────────────────────────────────────────────
    pub ttcp: TtcpStatus,

    // ── Control ────────────────────────────────────────────────────────────
    /// If true, the engine pauses between ticks (set by 'p' key, cleared by 'r').
    pub paused: bool,
    /// Requested ticks-per-second speed multiplier (1 = normal, 0 = max speed).
    pub speed_multiplier: u32,
    /// If true, the TUI has been asked to quit (set by 'q' key).
    pub quit_requested: bool,
}

impl DashboardState {
    pub fn push_event(&mut self, msg: impl Into<String>) {
        if self.event_log.len() >= MAX_EVENT_LOG {
            self.event_log.pop_front();
        }
        self.event_log.push_back(msg.into());
    }
}
