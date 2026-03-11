//! Regime FSM (spec Appendix B.1).
//!
//! States: Alpha, Beta, Gamma.
//! All transitions emit RegimeChanged events.

use fsr_types::{EventTag, RegimeState, Q32};
use serde::{Deserialize, Serialize};

/// Inputs needed to evaluate regime transitions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegimeInputs {
    /// Current SI score (Q32)
    pub si: Q32,
    /// SI threshold for weakening (Alpha→Beta)
    pub si_weaken_threshold: Q32,
    /// SI threshold for Gamma trigger
    pub gamma_trigger_threshold: Q32,
    /// Current drawdown (Q32, 0..1)
    pub drawdown: Q32,
    /// Max drawdown allowed in Alpha (Q32)
    pub dd_max: Q32,
    /// Is mirror consistent?
    pub mirror_consistent: bool,
    /// Is entropy collapsed?
    pub entropy_collapsed: bool,
    /// Is hedge inactive or safely closed?
    pub hedge_inactive: bool,
    /// SI recovery threshold (Beta→Alpha)
    pub si_recovery_threshold: Q32,
}

/// Regime FSM controller.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegimeFsm {
    pub state: RegimeState,
}

impl RegimeFsm {
    pub fn new() -> Self {
        RegimeFsm {
            state: RegimeState::Alpha,
        }
    }

    /// Step the regime FSM given current inputs.
    /// Returns Some(EventTag::RegimeChanged) if a transition occurred.
    pub fn step(&mut self, inputs: &RegimeInputs) -> Option<EventTag> {
        let next = self.next_state(inputs);
        if next != self.state {
            self.state = next;
            Some(EventTag::RegimeChanged)
        } else {
            None
        }
    }

    fn next_state(&self, i: &RegimeInputs) -> RegimeState {
        match self.state {
            RegimeState::Alpha => {
                // Alpha → Gamma: SI < gamma_trigger or drawdown > dd_max or entropy collapse
                if i.si < i.gamma_trigger_threshold
                    || i.drawdown > i.dd_max
                    || i.entropy_collapsed
                {
                    return RegimeState::Gamma;
                }
                // Alpha → Beta: SI weakens or mirror degrades; Gamma trigger not met
                if i.si < i.si_weaken_threshold || !i.mirror_consistent {
                    return RegimeState::Beta;
                }
                RegimeState::Alpha
            }
            RegimeState::Beta => {
                // Beta → Gamma: Gamma trigger fires
                if i.si < i.gamma_trigger_threshold
                    || i.drawdown > i.dd_max
                    || i.entropy_collapsed
                {
                    return RegimeState::Gamma;
                }
                // Beta → Alpha: SI recovery, mirror consistent, risk safe
                if i.si >= i.si_recovery_threshold && i.mirror_consistent && i.drawdown <= i.dd_max
                {
                    return RegimeState::Alpha;
                }
                RegimeState::Beta
            }
            RegimeState::Gamma => {
                // Gamma → Alpha: full recovery and hedge inactive or safely closed
                if i.si >= i.si_recovery_threshold
                    && i.mirror_consistent
                    && i.drawdown <= i.dd_max
                    && i.hedge_inactive
                    && !i.entropy_collapsed
                {
                    return RegimeState::Alpha;
                }
                // Gamma → Beta: partial recovery; full Alpha conditions not met
                if i.si >= i.si_weaken_threshold && i.mirror_consistent {
                    return RegimeState::Beta;
                }
                RegimeState::Gamma
            }
        }
    }
}

impl Default for RegimeFsm {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_fixed::ONE;

    fn inputs(si: Q32, drawdown: Q32, mirror_ok: bool, hedge_ok: bool, entropy_col: bool) -> RegimeInputs {
        RegimeInputs {
            si,
            si_weaken_threshold: ONE / 2,
            gamma_trigger_threshold: ONE / 4,
            drawdown,
            dd_max: ONE / 5,
            mirror_consistent: mirror_ok,
            entropy_collapsed: entropy_col,
            hedge_inactive: hedge_ok,
            si_recovery_threshold: ONE * 3 / 4,
        }
    }

    #[test]
    fn test_alpha_stays_alpha() {
        let mut fsm = RegimeFsm::new();
        let i = inputs(ONE, 0, true, true, false);
        assert!(fsm.step(&i).is_none());
        assert_eq!(fsm.state, RegimeState::Alpha);
    }

    #[test]
    fn test_alpha_to_beta_weak_si() {
        let mut fsm = RegimeFsm::new();
        let i = inputs(ONE / 3, 0, true, true, false);
        let ev = fsm.step(&i);
        assert_eq!(ev, Some(EventTag::RegimeChanged));
        assert_eq!(fsm.state, RegimeState::Beta);
    }

    #[test]
    fn test_alpha_to_gamma_entropy_collapse() {
        let mut fsm = RegimeFsm::new();
        let i = inputs(ONE, 0, true, true, true);
        let ev = fsm.step(&i);
        assert_eq!(ev, Some(EventTag::RegimeChanged));
        assert_eq!(fsm.state, RegimeState::Gamma);
    }

    #[test]
    fn test_beta_to_alpha_recovery() {
        let mut fsm = RegimeFsm { state: RegimeState::Beta };
        let i = inputs(ONE, 0, true, true, false);
        let ev = fsm.step(&i);
        assert_eq!(ev, Some(EventTag::RegimeChanged));
        assert_eq!(fsm.state, RegimeState::Alpha);
    }

    #[test]
    fn test_gamma_to_beta_partial_recovery() {
        let mut fsm = RegimeFsm { state: RegimeState::Gamma };
        // SI above weaken but below recovery, no hedge inactive
        let i = inputs(ONE * 2 / 3, 0, true, false, false);
        let ev = fsm.step(&i);
        assert_eq!(ev, Some(EventTag::RegimeChanged));
        assert_eq!(fsm.state, RegimeState::Beta);
    }

    #[test]
    fn test_gamma_to_alpha_full_recovery() {
        let mut fsm = RegimeFsm { state: RegimeState::Gamma };
        let i = inputs(ONE, 0, true, true, false);
        let ev = fsm.step(&i);
        assert_eq!(ev, Some(EventTag::RegimeChanged));
        assert_eq!(fsm.state, RegimeState::Alpha);
    }
}
