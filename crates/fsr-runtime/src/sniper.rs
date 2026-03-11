#![allow(dead_code)]
//! Sniper Mode (Phase 3 §5): TTCP-gated execution with progressive risk scaling.
//!
//! Concept: The system observes passively and only executes when TTCP crystallizes.
//! Progressive risk escalation: position size scales with crystal confidence.
//!
//! States: Idle → Observing → Armed → Executing → Cooldown → Idle
//!
//! Scale factor computation (spec §5.3):
//!   scale_factor = min(1.0, convergence_score / crystal_psi_threshold) * (1 - cooldown_penalty)
//!
//! Cooldown: after a loss, scale_factor is penalized for `cooldown_ticks` ticks.
//! Daily loss limit: if exceeded, enter Cooldown until next 24h window reset.

use fsr_fixed::{q32_to_f64_display_only, ONE};
use fsr_types::Q32;
use serde::{Deserialize, Serialize};

/// Sniper state machine states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SniperState {
    /// Sniper mode disabled.
    Disabled,
    /// Sniper enabled, waiting for TTCP crystal.
    Observing,
    /// TTCP crystal detected, gate open — ready to execute.
    Armed,
    /// Execution in progress.
    Executing,
    /// Post-loss cooldown.
    Cooldown,
}

impl std::fmt::Display for SniperState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SniperState::Disabled => write!(f, "DISABLED"),
            SniperState::Observing => write!(f, "OBSERVING"),
            SniperState::Armed => write!(f, "ARMED"),
            SniperState::Executing => write!(f, "EXECUTING"),
            SniperState::Cooldown => write!(f, "COOLDOWN"),
        }
    }
}

/// Configuration for sniper mode.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SniperConfig {
    /// Number of ticks to stay in cooldown after a loss.
    pub cooldown_ticks: u64,
    /// Cooldown penalty fraction on scale_factor (0.0..1.0 as Q32).
    pub cooldown_penalty: Q32,
    /// Maximum scale_factor (Q32; default = ONE = 1.0).
    pub max_scale_factor: Q32,
    /// TTCP crystal psi threshold for scale normalisation (Q32).
    pub crystal_psi_threshold: Q32,
    /// Daily loss limit in basis points (Q32). 0 = disabled.
    pub daily_loss_limit_bps: Q32,
    /// Require cross-venue cycles only if scale_factor >= this value.
    pub cross_venue_min_scale: Q32,
}

impl Default for SniperConfig {
    fn default() -> Self {
        SniperConfig {
            cooldown_ticks: 50,
            cooldown_penalty: ONE / 2, // 50% penalty
            max_scale_factor: ONE,
            crystal_psi_threshold: (ONE as f64 * 0.30) as Q32,
            daily_loss_limit_bps: 1500i64 * ONE, // -15 bp
            cross_venue_min_scale: (ONE as f64 * 0.30) as Q32,
        }
    }
}

/// Sniper mode controller (spec §5).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SniperMode {
    pub state: SniperState,
    pub scale_factor: Q32,
    pub cooldown_remaining: u64,
    pub last_crystal_tick: Option<u64>,
    pub crystal_validity_remaining: u64,
    pub config: SniperConfig,
    pub daily_loss_bps: Q32,
    pub daily_loss_window_start: u64,
    pub total_sniper_executions: u64,
    pub sniper_losses: u64,
}

impl SniperMode {
    pub fn new(config: SniperConfig) -> Self {
        SniperMode {
            state: SniperState::Disabled,
            scale_factor: 0,
            cooldown_remaining: 0,
            last_crystal_tick: None,
            crystal_validity_remaining: 0,
            daily_loss_bps: 0,
            daily_loss_window_start: 0,
            total_sniper_executions: 0,
            sniper_losses: 0,
            config,
        }
    }

    /// Enable sniper mode (called by 's' key or --sniper flag).
    pub fn enable(&mut self) {
        if self.state == SniperState::Disabled {
            self.state = SniperState::Observing;
        }
    }

    /// Disable sniper mode.
    pub fn disable(&mut self) {
        self.state = SniperState::Disabled;
        self.scale_factor = 0;
    }

    /// Toggle sniper mode on/off.
    pub fn toggle(&mut self) {
        if self.state == SniperState::Disabled {
            self.enable();
        } else {
            self.disable();
        }
    }

    pub fn is_active(&self) -> bool {
        self.state != SniperState::Disabled
    }

    /// Called each tick with current system metrics.
    /// Returns true if sniper should execute this tick.
    pub fn tick(
        &mut self,
        current_tick: u64,
        gate_open: bool,
        ttcp_crystal_tick: Option<u64>,
        ttcp_convergence_score: Q32,
        pnl_delta: Q32, // P&L change this tick
    ) -> bool {
        if self.state == SniperState::Disabled {
            return false;
        }

        // Reset daily loss window (24h = 86400 ticks at 1 tick/sec)
        if current_tick > self.daily_loss_window_start + 86400 {
            self.daily_loss_bps = 0;
            self.daily_loss_window_start = current_tick;
        }

        // Check daily loss limit
        if self.config.daily_loss_limit_bps > 0
            && self.daily_loss_bps.abs() >= self.config.daily_loss_limit_bps
            && self.state != SniperState::Cooldown
        {
            self.state = SniperState::Cooldown;
            self.cooldown_remaining = self.config.cooldown_ticks;
            return false;
        }

        // Tick down cooldown
        if self.state == SniperState::Cooldown {
            if self.cooldown_remaining > 0 {
                self.cooldown_remaining -= 1;
            }
            if self.cooldown_remaining == 0 {
                self.state = SniperState::Observing;
            }
            return false;
        }

        // Tick down crystal validity
        if self.crystal_validity_remaining > 0 {
            self.crystal_validity_remaining -= 1;
        }

        // Detect new crystal
        let new_crystal = ttcp_crystal_tick
            .map(|t| Some(t) != self.last_crystal_tick)
            .unwrap_or(false);

        if new_crystal {
            if let Some(crystal_tick) = ttcp_crystal_tick {
                self.last_crystal_tick = Some(crystal_tick);
                self.crystal_validity_remaining = 50; // valid for 50 ticks
                self.scale_factor = self.compute_scale_factor(ttcp_convergence_score);
            }
        }

        // Transition to Armed if crystal is valid and gate is open
        if self.state == SniperState::Observing
            && self.crystal_validity_remaining > 0
            && gate_open
        {
            self.state = SniperState::Armed;
        }

        // Disarm if crystal expires or gate closes
        if self.state == SniperState::Armed
            && (self.crystal_validity_remaining == 0 || !gate_open)
        {
            self.state = SniperState::Observing;
        }

        // Execute when Armed
        if self.state == SniperState::Armed && gate_open {
            self.state = SniperState::Executing;
            self.total_sniper_executions += 1;
            self.state = SniperState::Observing; // reset after execution
            return true;
        }

        // Track P&L for daily loss
        if pnl_delta < 0 {
            self.daily_loss_bps = self.daily_loss_bps.saturating_add(pnl_delta);
            if pnl_delta < -ONE {
                // A loss: enter cooldown
                self.sniper_losses += 1;
                self.state = SniperState::Cooldown;
                self.cooldown_remaining = self.config.cooldown_ticks;
                // Reduce scale_factor for next armed state
                self.scale_factor = ((self.scale_factor as i128
                    * (ONE - self.config.cooldown_penalty) as i128)
                    / ONE as i128) as Q32;
            }
        }

        false
    }

    /// Compute scale_factor = min(1.0, convergence_score / psi_threshold).
    /// A higher convergence_score → larger position.
    pub fn compute_scale_factor(&self, convergence_score: Q32) -> Q32 {
        if self.config.crystal_psi_threshold <= 0 {
            return self.config.max_scale_factor;
        }
        // Use i128 intermediate to avoid overflow when multiplying two Q32 values.
        let scale = ((convergence_score as i128 * ONE as i128)
            / self.config.crystal_psi_threshold as i128) as Q32;
        scale.min(self.config.max_scale_factor).max(0)
    }

    /// True if cross-venue cycles are allowed at current scale.
    pub fn allow_cross_venue(&self) -> bool {
        self.scale_factor >= self.config.cross_venue_min_scale
    }

    /// One-line status for TUI display.
    pub fn status_line(&self) -> String {
        match self.state {
            SniperState::Disabled => "DISABLED".to_string(),
            SniperState::Observing => "OBSERVING".to_string(),
            SniperState::Armed => format!(
                "ARMED scale={:.2}",
                q32_to_f64_display_only(self.scale_factor)
            ),
            SniperState::Executing => "EXECUTING".to_string(),
            SniperState::Cooldown => format!("COOLDOWN ({} ticks)", self.cooldown_remaining),
        }
    }
}

impl Default for SniperMode {
    fn default() -> Self {
        Self::new(SniperConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sniper_disabled_by_default() {
        let s = SniperMode::default();
        assert_eq!(s.state, SniperState::Disabled);
        assert!(!s.is_active());
    }

    #[test]
    fn test_sniper_enable_disable() {
        let mut s = SniperMode::default();
        s.enable();
        assert_eq!(s.state, SniperState::Observing);
        s.disable();
        assert_eq!(s.state, SniperState::Disabled);
    }

    #[test]
    fn test_sniper_toggle() {
        let mut s = SniperMode::default();
        s.toggle();
        assert!(s.is_active());
        s.toggle();
        assert!(!s.is_active());
    }

    #[test]
    fn test_sniper_no_execute_without_crystal() {
        let mut s = SniperMode::default();
        s.enable();
        // No crystal → should not execute
        let exec = s.tick(1, true, None, 0, 0);
        assert!(!exec, "no execution without crystal");
        assert_eq!(s.state, SniperState::Observing);
    }

    #[test]
    fn test_sniper_arms_with_crystal() {
        let mut s = SniperMode::default();
        s.enable();
        // Feed a crystal at tick 5
        let _ = s.tick(1, false, None, 0, 0);
        let _ = s.tick(5, true, Some(5), ONE / 2, 0); // gate open, crystal at tick 5
        // Armed → should execute immediately when gate open
        // In our implementation, Armed transitions to Executing in same tick
        // Next tick with gate open should execute
        assert_eq!(s.total_sniper_executions, 1);
    }

    #[test]
    fn test_scale_factor_computation() {
        let config = SniperConfig {
            crystal_psi_threshold: ONE / 2, // 0.5
            max_scale_factor: ONE,
            ..SniperConfig::default()
        };
        let s = SniperMode::new(config);
        // convergence_score = 0.25 → scale = 0.5
        let scale = s.compute_scale_factor(ONE / 4);
        assert_eq!(scale, ONE / 2);
        // convergence_score = 0.75 → scale = 1.5 → clamped to 1.0
        let scale_max = s.compute_scale_factor(ONE * 3 / 4);
        assert_eq!(scale_max, ONE);
    }

    #[test]
    fn test_cooldown_after_loss() {
        let mut s = SniperMode::new(SniperConfig {
            cooldown_ticks: 5,
            ..SniperConfig::default()
        });
        s.enable();
        // Arm the sniper with a crystal
        s.tick(1, true, Some(1), ONE / 2, 0);
        // Simulate a big loss to trigger cooldown
        s.tick(2, false, None, 0, -(2 * ONE));
        assert_eq!(s.state, SniperState::Cooldown);
        assert_eq!(s.cooldown_remaining, 5);
        // Tick down
        for i in 0..5 {
            let _ = s.tick(3 + i, false, None, 0, 0);
        }
        assert_eq!(s.state, SniperState::Observing, "cooldown should expire");
    }
}
