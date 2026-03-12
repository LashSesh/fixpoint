//! GuiState: shared state sent from engine thread → GUI render thread.

use fsr_dshae::{DshaeCrystal, DshaeSummary};
use fsr_types::Q32;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Maximum P&L history points kept.
pub const MAX_PNL_HISTORY: usize = 1000;
/// Maximum crystal history.
pub const MAX_CRYSTAL_HISTORY: usize = 100;

// ── Phase 5 display structs ───────────────────────────────────────────────────

/// MCCE display state (Phase 5).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct McceDisplay {
    pub vertex_count: u64,
    pub edge_count: u64,
    pub graph_density: f32,
    pub cluster_count: usize,
    pub total_signals: u64,
}

/// ECLS display state (Phase 5).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EclsDisplay {
    pub active_constraints: usize,
    pub lattice_crystals: u64,
    pub breaking_events: u64,
    pub recent_events: VecDeque<String>,
}

/// ISLS display state (Phase 5).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct IslsDisplay {
    pub hot_count: u64,
    pub warm_count: u64,
    pub cold_count: u64,
    pub total_observations: u64,
    pub crystal_count: u64,
    pub replay_verified: bool,
    pub shadow_head: String,
    pub shadow_event_count: u64,
    pub commit_event_count: u64,
    /// Rolling vertex count history for growth chart (max 200 pts).
    pub vertex_history: VecDeque<u64>,
}

/// Run mode string.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunMode {
    Paper,
    Sandbox,
    Idle,
}

impl std::fmt::Display for RunMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunMode::Paper => write!(f, "paper"),
            RunMode::Sandbox => write!(f, "sandbox"),
            RunMode::Idle => write!(f, "idle"),
        }
    }
}

/// Resonance metrics snapshot for display.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ResonanceDisplay {
    pub si: f32,
    pub psi: f32,
    pub rho: f32,
    pub omega: f32,
    pub kappa: f32,
    pub entropy: f32,
}

/// TTCP status for display.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TtcpDisplay {
    pub crystals_found: u64,
    pub last_crystal_tick: Option<u64>,
    pub level: u8,
    pub convergence_score: f32,
}

/// Risk/P&L summary for display.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PnlDisplay {
    /// Net P&L in basis points (float for display).
    pub net_pnl_bps: f32,
    /// Current drawdown in bp.
    pub drawdown_bps: f32,
    /// Settled trade count.
    pub settled: u64,
    /// Aborted trade count.
    pub aborted: u64,
}

/// Summary of a DSHAE crystal for display.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrystalDisplay {
    pub triangle: (usize, usize, usize),
    pub net_edge_bps: f32,
    pub tick: u64,
    pub age: u64,
}

impl From<&DshaeCrystal> for CrystalDisplay {
    fn from(c: &DshaeCrystal) -> Self {
        use fsr_fixed::q32_to_f64_display_only;
        CrystalDisplay {
            triangle: c.triangle,
            net_edge_bps: (q32_to_f64_display_only(c.net_edge) * 10000.0) as f32,
            tick: c.tick,
            age: c.age,
        }
    }
}

/// Complete GUI state: one snapshot per engine tick.
/// Sent via mpsc channel from engine thread → GUI render thread.
#[derive(Clone, Debug)]
pub struct GuiState {
    pub tick: u64,
    pub mode: RunMode,

    // FSM states (strings for display).
    pub regime: String,
    pub integrity: String,
    pub resource: String,
    pub gate_open: bool,
    pub gamma_score: f32,

    // Resonance metrics.
    pub resonance: ResonanceDisplay,

    // TTCP.
    pub ttcp: TtcpDisplay,

    // P&L.
    pub pnl: PnlDisplay,

    // DSHAE.
    pub dshae: DshaeSummary,

    // Rolling histories.
    pub pnl_history: VecDeque<(u64, f32)>, // (tick, cumulative_pnl_bps)
    pub delta_history: VecDeque<(u64, f32, f32, f32)>, // (tick, d1, d2, d3)
    pub crystal_log: VecDeque<CrystalDisplay>,
    pub event_log: VecDeque<String>,

    // Candidates.
    pub candidates_found: usize,

    // Phase 5: MCCE/ECLS/ISLS display.
    pub mcce: McceDisplay,
    pub ecls: EclsDisplay,
    pub isls: IslsDisplay,

    // Control.
    pub quit_requested: bool,
}

impl Default for GuiState {
    fn default() -> Self {
        GuiState {
            tick: 0,
            mode: RunMode::Idle,
            regime: "Alpha".into(),
            integrity: "Healthy".into(),
            resource: "Nominal".into(),
            gate_open: false,
            gamma_score: 0.0,
            resonance: ResonanceDisplay::default(),
            ttcp: TtcpDisplay::default(),
            pnl: PnlDisplay::default(),
            dshae: DshaeSummary::default(),
            pnl_history: VecDeque::new(),
            delta_history: VecDeque::new(),
            crystal_log: VecDeque::new(),
            event_log: VecDeque::new(),
            candidates_found: 0,
            mcce: McceDisplay::default(),
            ecls: EclsDisplay::default(),
            isls: IslsDisplay::default(),
            quit_requested: false,
        }
    }
}

impl GuiState {
    pub fn push_crystal(&mut self, c: CrystalDisplay) {
        if self.crystal_log.len() >= MAX_CRYSTAL_HISTORY {
            self.crystal_log.pop_front();
        }
        self.crystal_log.push_back(c);
    }

    pub fn push_event(&mut self, msg: impl Into<String>) {
        if self.event_log.len() >= 200 {
            self.event_log.pop_front();
        }
        self.event_log.push_back(msg.into());
    }

    pub fn push_pnl(&mut self, tick: u64, pnl_bps: f32) {
        if self.pnl_history.len() >= MAX_PNL_HISTORY {
            self.pnl_history.pop_front();
        }
        self.pnl_history.push_back((tick, pnl_bps));
    }

    pub fn push_deltas(&mut self, tick: u64, d1: f32, d2: f32, d3: f32) {
        if self.delta_history.len() >= MAX_PNL_HISTORY {
            self.delta_history.pop_front();
        }
        self.delta_history.push_back((tick, d1, d2, d3));
    }
}
