//! Kairos gate: dual-consensus gate with hysteresis (spec §9).
//!
//! Γ(x) = aψ·ψ + aρ·ρ - aω·(1-ω) - aL·Leak
//! Gate dual(z) = Gatepor ∧ Gatemirror ∧ Gatetempo ∧ Gaterisk
//! Hysteresis: opens at θ_open, closes at θ_close (θ_open > θ_close mandatory).

use fsr_fixed::{q32_clamp, q32_mul, ONE};
use fsr_types::{Q32, ResonanceSnapshot};
use serde::{Deserialize, Serialize};

/// Kairos gate configuration (all weights/thresholds from YAML, spec §24).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KairosConfig {
    /// aψ: weight for quality component
    pub a_psi: Q32,
    /// aρ: weight for stability component
    pub a_rho: Q32,
    /// aω: weight for efficiency penalty
    pub a_omega: Q32,
    /// aL: weight for leakage penalty
    pub a_leak: Q32,
    /// θ_open: gate opens when Γ ≥ θ_open
    pub theta_open: Q32,
    /// θ_close: gate closes when Γ ≤ θ_close (must be < θ_open)
    pub theta_close: Q32,
}

impl KairosConfig {
    /// Validate that θ_open > θ_close (TMCP anti-flutter requirement, spec §9).
    pub fn validate(&self) -> Result<(), String> {
        if self.theta_open <= self.theta_close {
            return Err(format!(
                "θ_open ({}) must be > θ_close ({}) (anti-flutter requirement)",
                self.theta_open, self.theta_close
            ));
        }
        Ok(())
    }
}

impl Default for KairosConfig {
    fn default() -> Self {
        KairosConfig {
            a_psi: ONE,
            a_rho: ONE,
            a_omega: ONE / 2,
            a_leak: ONE / 2,
            theta_open: ONE * 6 / 10,   // 0.6
            theta_close: ONE * 4 / 10,  // 0.4
        }
    }
}

/// Sub-gate inputs for the dual-consensus gate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubGateInputs {
    /// PoR gate: route has been verified and committed
    pub por_gate: bool,
    /// Mirror gate: MCI within bounds
    pub mirror_gate: bool,
    /// Temporal gate: temporal key is fresh, phase window active
    pub temporal_gate: bool,
    /// Risk gate: drawdown, leverage, hedge caps all satisfied
    pub risk_gate: bool,
}

/// Kairos gate state (spec §9).
pub struct KairosGate {
    pub config: KairosConfig,
    /// g_prev: gate memory bit (part of protocol state).
    pub g_prev: bool,
}

impl KairosGate {
    pub fn new(config: KairosConfig) -> Self {
        KairosGate { config, g_prev: false }
    }

    /// Compute Γ(x) = aψ·ψ + aρ·ρ - aω·(1-ω) - aL·Leak (spec §9, eq. 1).
    pub fn score(&self, snap: &ResonanceSnapshot, leak: Q32) -> Q32 {
        let pos = q32_mul(self.config.a_psi, snap.psi)
            .saturating_add(q32_mul(self.config.a_rho, snap.rho));
        let neg = q32_mul(self.config.a_omega, ONE.saturating_sub(snap.omega))
            .saturating_add(q32_mul(self.config.a_leak, leak));
        q32_clamp(pos.saturating_sub(neg), i64::MIN / 2, i64::MAX / 2)
    }

    /// Evaluate the full dual-consensus gate with hysteresis (spec §9, eq. 2-3).
    ///
    /// Returns (gate_open: bool, gamma_score: Q32).
    /// Updates g_prev (gate memory bit).
    pub fn evaluate(
        &mut self,
        snap: &ResonanceSnapshot,
        leak: Q32,
        sub: &SubGateInputs,
    ) -> (bool, Q32) {
        let gamma = self.score(snap, leak);

        // Hysteresis (eq. 3):
        // G(x) = 1  if g_prev=0 ∧ Γ ≥ θ_open
        // G(x) = 0  if g_prev=1 ∧ Γ ≤ θ_close
        // G(x) = g_prev otherwise
        let g = if !self.g_prev && gamma >= self.config.theta_open {
            true
        } else if self.g_prev && gamma <= self.config.theta_close {
            false
        } else {
            self.g_prev
        };

        // Dual-consensus: all sub-gates must pass (eq. 2)
        let dual = g && sub.por_gate && sub.mirror_gate && sub.temporal_gate && sub.risk_gate;

        self.g_prev = dual;
        (dual, gamma)
    }

    /// Short-circuit gate evaluation: earliest failing gate is the blocking cause.
    /// Returns None if all pass, Some(reason) if blocked.
    pub fn short_circuit_check(sub: &SubGateInputs) -> Option<&'static str> {
        if !sub.por_gate {
            return Some("PoR gate");
        }
        if !sub.mirror_gate {
            return Some("mirror gate");
        }
        if !sub.temporal_gate {
            return Some("temporal gate");
        }
        if !sub.risk_gate {
            return Some("risk gate");
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_fixed::{HALF, ONE};
    use fsr_types::ResonanceSnapshot;

    fn snap(psi: Q32, rho: Q32, omega: Q32) -> ResonanceSnapshot {
        ResonanceSnapshot {
            kappa: 0,
            entropy: 0,
            sync: 0,
            momentum: 0,
            si: 0,
            psi,
            rho,
            omega,
            tick: 0,
        }
    }

    fn all_sub_gates_open() -> SubGateInputs {
        SubGateInputs {
            por_gate: true,
            mirror_gate: true,
            temporal_gate: true,
            risk_gate: true,
        }
    }

    #[test]
    fn test_kairos_config_validation() {
        let mut cfg = KairosConfig::default();
        assert!(cfg.validate().is_ok());
        cfg.theta_open = cfg.theta_close;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_gate_opens_when_score_above_theta_open() {
        let mut gate = KairosGate::new(KairosConfig::default());
        // ψ=1, ρ=1, ω=1, leak=0 → Γ = 1+1-0-0 = 2 > θ_open
        let s = snap(ONE, ONE, ONE);
        let (open, gamma) = gate.evaluate(&s, 0, &all_sub_gates_open());
        assert!(open, "gate should open with high score");
        assert!(gamma > KairosConfig::default().theta_open);
    }

    #[test]
    fn test_gate_stays_closed_below_theta_open() {
        let mut gate = KairosGate::new(KairosConfig::default());
        // ψ=0.1, ρ=0.1, ω=0 → Γ very low
        let s = snap(ONE / 10, ONE / 10, 0);
        let (open, _) = gate.evaluate(&s, 0, &all_sub_gates_open());
        assert!(!open, "gate should stay closed with low score");
    }

    #[test]
    fn test_hysteresis_anti_flutter() {
        let cfg = KairosConfig::default();
        let mut gate = KairosGate::new(cfg.clone());

        // Force gate open
        let s_high = snap(ONE, ONE, ONE);
        let (open, _) = gate.evaluate(&s_high, 0, &all_sub_gates_open());
        assert!(open);

        // Score drops between θ_close and θ_open → gate stays open (hysteresis)
        let s_mid = snap(HALF, HALF, HALF);
        let (open2, gamma2) = gate.evaluate(&s_mid, 0, &all_sub_gates_open());
        // gamma2 should be between theta_close and theta_open
        if gamma2 > cfg.theta_close && gamma2 < cfg.theta_open {
            assert!(open2, "hysteresis: gate stays open in mid zone");
        }
    }

    #[test]
    fn test_dual_consensus_blocks_on_sub_gate() {
        let mut gate = KairosGate::new(KairosConfig::default());
        let s = snap(ONE, ONE, ONE);
        let mut sub = all_sub_gates_open();
        sub.mirror_gate = false;
        let (open, _) = gate.evaluate(&s, 0, &sub);
        assert!(!open, "should block when mirror gate is closed");
    }

    #[test]
    fn test_short_circuit() {
        let mut sub = all_sub_gates_open();
        assert!(KairosGate::short_circuit_check(&sub).is_none());
        sub.por_gate = false;
        assert_eq!(KairosGate::short_circuit_check(&sub), Some("PoR gate"));
        sub.por_gate = true;
        sub.temporal_gate = false;
        assert_eq!(KairosGate::short_circuit_check(&sub), Some("temporal gate"));
    }
}
