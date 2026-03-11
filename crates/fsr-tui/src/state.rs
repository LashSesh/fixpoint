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
    /// Average delta1 (Q32)
    pub avg_delta1: Q32,
    /// Average delta2 (Q32)
    pub avg_delta2: Q32,
    /// Average delta3 (Q32)
    pub avg_delta3: Q32,
    /// Crystal validity countdown
    pub crystal_validity_remaining: u64,
}

/// Sniper mode status for TUI display.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SniperStatus {
    pub enabled: bool,
    /// Display string: "DISABLED", "OBSERVING", "ARMED scale=0.31", "COOLDOWN (N ticks)"
    pub status_line: String,
    pub scale_factor: Q32,
    pub total_executions: u64,
    pub cooldown_remaining: u64,
    pub state_name: String,
}

/// Risk and P&L metrics for panel 4.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RiskStatus {
    /// Net P&L in basis points (Q32)
    pub net_pnl_bps: Q32,
    /// Current drawdown (Q32 bps)
    pub drawdown_bps: Q32,
    /// Max allowed drawdown (Q32 bps)
    pub max_drawdown_limit_bps: Q32,
    /// Current leverage (Q32 ratio)
    pub leverage: Q32,
    /// Max allowed leverage (Q32)
    pub max_leverage: Q32,
    /// Risk budget consumed (0..1 as Q32)
    pub risk_budget_fraction: Q32,
    /// Daily loss consumed (Q32 bps, positive = loss)
    pub daily_loss_bps: Q32,
    /// Daily loss limit (Q32 bps)
    pub daily_loss_limit_bps: Q32,
    /// True if daily loss limit hit (auto-pause)
    pub daily_limit_hit: bool,
    /// Inventory summary string e.g. "BTC+.003 ETH-.012"
    pub inventory_summary: String,
    /// Exposure in Q32 basis points
    pub exposure_bps: Q32,
}

/// Complete dashboard state: one snapshot per tick, cloned into TUI thread.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DashboardState {
    // ── Tick / identity ────────────────────────────────────────────────────
    pub tick: u64,
    pub run_id: String,
    /// Mode: "paper", "live", "replay", "live|sniper"
    pub mode: String,

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

    // ── Phase 3: Sniper ────────────────────────────────────────────────────
    pub sniper: SniperStatus,

    // ── Phase 3: Risk Dashboard ────────────────────────────────────────────
    pub risk: RiskStatus,

    // ── Phase 3: Multi-venue counts ────────────────────────────────────────
    /// Binance L1 (single-venue) candidate count
    pub binance_l1_count: usize,
    /// Binance L2 candidate count
    pub binance_l2_count: usize,
    /// Kraken L1 candidate count
    pub kraken_l1_count: usize,
    /// Kraken L2 candidate count
    pub kraken_l2_count: usize,
    /// Cross-venue candidate count
    pub cross_venue_count: usize,

    // ── Control ────────────────────────────────────────────────────────────
    /// If true, the engine pauses between ticks (set by 'p' key, cleared by 'r').
    pub paused: bool,
    /// Requested ticks-per-second speed multiplier (1 = normal, 0 = max speed).
    pub speed_multiplier: u32,
    /// If true, the TUI has been asked to quit (set by 'q' key).
    pub quit_requested: bool,
    /// If true, sniper toggle was requested (set by 's' key).
    pub sniper_toggle_requested: bool,
}

impl DashboardState {
    pub fn push_event(&mut self, msg: impl Into<String>) {
        if self.event_log.len() >= MAX_EVENT_LOG {
            self.event_log.pop_front();
        }
        self.event_log.push_back(msg.into());
    }
}
