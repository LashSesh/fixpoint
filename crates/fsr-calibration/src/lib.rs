//! fsr-calibration: Benchmark, double-kick update, promotion FSM (spec §14, Appendix B.5).
//!
//! Double-kick: T = ΦV ∘ ΦU where ΦU improves ψ, ΦV improves ρ and ω.
//! Promotion FSM: Candidate → PaperValidated → PromotionPending → LiveEligible → LiveActive.
//! INV-05: no live promotion without paper validation and promotion gate passage.
//!
//! Phase 3 additions:
//!   - promotion_workflow: 5-gate pipeline CLI integration

pub mod promotion_workflow;
pub use promotion_workflow::{
    GateCheckResult, PromotionGateConfig, PromotionProposal, PromotionWorkflow,
    run_gate_checks,
};

use fsr_types::{EventTag, PromotionState, Q32};
use serde::{Deserialize, Serialize};

/// Calibration configuration (spec §14).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalibrationConfig {
    /// Maximum step per update (Q32)
    pub max_step_per_update: Q32,
    /// ρ_min for PoR acceptance
    pub rho_min: Q32,
    /// ε for bounded regression
    pub epsilon: Q32,
    /// Calibration window in ticks
    pub calibration_window: u64,
}

impl Default for CalibrationConfig {
    fn default() -> Self {
        use fsr_fixed::ONE;
        CalibrationConfig {
            max_step_per_update: ONE / 10, // max 10% update per step
            rho_min: ONE / 2,
            epsilon: ONE / 20,
            calibration_window: 100,
        }
    }
}

/// Evaluation triplet for a configuration.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct EvalTriplet {
    /// ψ: quality (Q32, 0..1)
    pub psi: Q32,
    /// ρ: stability (Q32, 0..1)
    pub rho: Q32,
    /// ω: efficiency (Q32, 0..1)
    pub omega: Q32,
}

/// Double-kick update T = ΦV ∘ ΦU (spec §14.2).
/// ΦU: improves ψ (quality step).
/// ΦV: improves ρ (stability step) and ω (efficiency step).
/// Returns the updated triplet, bounded by max_step_per_update.
pub fn double_kick_update(
    current: &EvalTriplet,
    delta_psi: Q32,
    delta_rho: Q32,
    delta_omega: Q32,
    config: &CalibrationConfig,
) -> EvalTriplet {
    use fsr_fixed::{q32_clamp, ONE};
    let max_step = config.max_step_per_update;
    // ΦU: quality improvement
    let new_psi = current.psi + delta_psi.clamp(-max_step, max_step);
    // ΦV: stability + efficiency improvement
    let new_rho = current.rho + delta_rho.clamp(-max_step, max_step);
    let new_omega = current.omega + delta_omega.clamp(-max_step, max_step);
    EvalTriplet {
        psi: q32_clamp(new_psi, 0, ONE),
        rho: q32_clamp(new_rho, 0, ONE),
        omega: q32_clamp(new_omega, 0, ONE),
    }
}

/// PoR acceptance for configuration (spec §14.3, TMCP Definition 11.8).
pub fn por_acceptance(new_t: &EvalTriplet, old_t: &EvalTriplet, config: &CalibrationConfig) -> bool {
    // Monotone: Φ(c_new) ⪰_Φ Φ(c_old)
    let monotone = new_t.psi >= old_t.psi && new_t.rho >= old_t.rho && new_t.omega >= old_t.omega;
    // Bounded regression: ρ ≥ ρ_min and ψ ≥ ψ_old - ε
    let bounded = new_t.rho >= config.rho_min && new_t.psi >= old_t.psi.saturating_sub(config.epsilon);
    monotone || bounded
}

/// Promotion FSM controller (spec Appendix B.5).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromotionFsm {
    pub state: PromotionState,
}

impl PromotionFsm {
    pub fn new() -> Self {
        PromotionFsm { state: PromotionState::Candidate }
    }

    /// Candidate → PaperValidated: paper suite passes.
    pub fn paper_validate(&mut self) -> Option<EventTag> {
        if self.state == PromotionState::Candidate {
            self.state = PromotionState::PaperValidated;
            Some(EventTag::CalibrationAccepted)
        } else {
            None
        }
    }

    /// PaperValidated → PromotionPending: promotion proposal emitted.
    pub fn propose_promotion(&mut self) -> Option<EventTag> {
        if self.state == PromotionState::PaperValidated {
            self.state = PromotionState::PromotionPending;
            Some(EventTag::PromotionProposal)
        } else {
            None
        }
    }

    /// PromotionPending → LiveEligible: all promotion checks pass.
    pub fn gate_pass(&mut self) -> Option<EventTag> {
        if self.state == PromotionState::PromotionPending {
            self.state = PromotionState::LiveEligible;
            Some(EventTag::PromotionGatePass)
        } else {
            None
        }
    }

    /// LiveEligible → LiveActive: live activation approved.
    pub fn activate(&mut self) -> Option<EventTag> {
        if self.state == PromotionState::LiveEligible {
            self.state = PromotionState::LiveActive;
            Some(EventTag::PromotionActivated)
        } else {
            None
        }
    }

    /// Any non-revoked → Demoted: live regression or governance.
    pub fn demote(&mut self) -> Option<EventTag> {
        if self.state != PromotionState::Revoked {
            self.state = PromotionState::Demoted;
            Some(EventTag::PromotionDemoted)
        } else {
            None
        }
    }

    /// Any → Revoked: fatal issue or explicit revocation.
    pub fn revoke(&mut self) -> EventTag {
        self.state = PromotionState::Revoked;
        EventTag::PromotionRevoked
    }

    /// INV-05: no live promotion without paper validation and promotion gate passage.
    pub fn can_activate(&self) -> bool {
        self.state == PromotionState::LiveEligible
    }
}

impl Default for PromotionFsm {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_fixed::ONE;

    #[test]
    fn test_promotion_full_path() {
        let mut fsm = PromotionFsm::new();
        assert_eq!(fsm.state, PromotionState::Candidate);

        assert_eq!(fsm.paper_validate(), Some(EventTag::CalibrationAccepted));
        assert_eq!(fsm.state, PromotionState::PaperValidated);

        assert_eq!(fsm.propose_promotion(), Some(EventTag::PromotionProposal));
        assert_eq!(fsm.state, PromotionState::PromotionPending);

        assert_eq!(fsm.gate_pass(), Some(EventTag::PromotionGatePass));
        assert_eq!(fsm.state, PromotionState::LiveEligible);

        assert!(fsm.can_activate(), "INV-05: eligible after gate pass");

        assert_eq!(fsm.activate(), Some(EventTag::PromotionActivated));
        assert_eq!(fsm.state, PromotionState::LiveActive);
    }

    #[test]
    fn test_inv05_cannot_activate_from_candidate() {
        let mut fsm = PromotionFsm::new();
        assert!(!fsm.can_activate(), "INV-05: cannot activate without paper validation");
        assert!(fsm.activate().is_none(), "activate returns None when not eligible");
    }

    #[test]
    fn test_promotion_revoke_any_state() {
        let mut fsm = PromotionFsm::new();
        let ev = fsm.revoke();
        assert_eq!(ev, EventTag::PromotionRevoked);
        assert_eq!(fsm.state, PromotionState::Revoked);
    }

    #[test]
    fn test_double_kick_update_bounded() {
        let config = CalibrationConfig::default();
        let current = EvalTriplet { psi: ONE / 2, rho: ONE / 2, omega: ONE / 2 };
        // Large delta that should be clamped to max_step
        let updated = double_kick_update(&current, ONE, ONE, ONE, &config);
        let max = config.max_step_per_update;
        assert_eq!(updated.psi, ONE / 2 + max);
        assert_eq!(updated.rho, ONE / 2 + max);
        assert_eq!(updated.omega, ONE / 2 + max);
    }

    #[test]
    fn test_por_acceptance_monotone() {
        let config = CalibrationConfig::default();
        let old = EvalTriplet { psi: ONE / 2, rho: ONE / 2, omega: ONE / 2 };
        let new_t = EvalTriplet { psi: ONE * 3 / 4, rho: ONE * 3 / 4, omega: ONE * 3 / 4 };
        assert!(por_acceptance(&new_t, &old, &config));
    }

    #[test]
    fn test_por_acceptance_bounded_regression() {
        let config = CalibrationConfig::default();
        let old = EvalTriplet { psi: ONE, rho: ONE * 3 / 4, omega: ONE / 2 };
        // psi drops by epsilon
        let new_t = EvalTriplet {
            psi: ONE - config.epsilon,
            rho: ONE * 3 / 4,
            omega: ONE / 2,
        };
        assert!(por_acceptance(&new_t, &old, &config), "bounded regression accepted");
    }

    #[test]
    fn test_por_acceptance_fails_below_rho_min() {
        let config = CalibrationConfig::default();
        let old = EvalTriplet { psi: ONE, rho: ONE, omega: ONE };
        let new_t = EvalTriplet { psi: ONE / 4, rho: ONE / 4, omega: ONE }; // rho < rho_min
        assert!(!por_acceptance(&new_t, &old, &config));
    }
}
