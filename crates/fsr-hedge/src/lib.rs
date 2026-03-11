//! fsr-hedge: Hedge FSM, sizing, trigger, unwind (spec Appendix B.4).
//!
//! States: Safe → Armed → Triggered → Disarmed → Safe
//! INV-10: hedge leverage may not exceed configured caps.

use fsr_types::{EventTag, HedgeState, Q32};
use serde::{Deserialize, Serialize};

/// Hedge configuration (spec §24, INV-10).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HedgeConfig {
    /// Maximum drawdown before hedge triggers (Q32, 0..1)
    pub dd_max: Q32,
    /// Maximum leverage cap (Q32, as multiplier)
    pub max_leverage: Q32,
    /// Hedge size as fraction of position (Q32, 0..1)
    pub hedge_fraction: Q32,
}

impl Default for HedgeConfig {
    fn default() -> Self {
        use fsr_fixed::ONE;
        HedgeConfig {
            dd_max: ONE / 5,
            max_leverage: 2 * ONE,
            hedge_fraction: ONE / 2,
        }
    }
}

/// Hedge FSM inputs for transition evaluation.
#[derive(Clone, Debug)]
pub struct HedgeInputs {
    pub regime_is_alpha: bool,
    pub drawdown: Q32,
    pub mirror_degraded: bool,
    pub unwind_complete: bool,
    pub recovery_stable: bool,
    pub no_residual_exposure: bool,
}

/// Hedge FSM controller (spec Appendix B.4).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HedgeFsm {
    pub state: HedgeState,
    pub config: HedgeConfig,
    /// Current hedge size (Q32, as notional fraction).
    pub hedge_size: Q32,
}

impl HedgeFsm {
    pub fn new(config: HedgeConfig) -> Self {
        HedgeFsm {
            state: HedgeState::Safe,
            config,
            hedge_size: 0,
        }
    }

    /// Step the hedge FSM. Returns emitted event if a transition occurred.
    pub fn step(&mut self, inputs: &HedgeInputs) -> Option<EventTag> {
        let next = self.next_state(inputs);
        if next != self.state {
            let event = match next {
                HedgeState::Armed => Some(EventTag::HedgeArmed),
                HedgeState::Triggered => Some(EventTag::HedgeTriggered),
                HedgeState::Disarmed => Some(EventTag::HedgeClosed),
                HedgeState::Safe => Some(EventTag::HedgeRecovered),
            };
            self.state = next;
            // Update hedge size on transitions
            self.update_hedge_size(inputs);
            event
        } else {
            None
        }
    }

    fn next_state(&self, i: &HedgeInputs) -> HedgeState {
        match self.state {
            HedgeState::Safe => {
                // Safe → Armed: regime ≠ Alpha or drawdown rising
                if !i.regime_is_alpha || i.drawdown > 0 {
                    HedgeState::Armed
                } else {
                    HedgeState::Safe
                }
            }
            HedgeState::Armed => {
                // Armed → Triggered: drawdown > dd_max or mirror degradation
                if i.drawdown > self.config.dd_max || i.mirror_degraded {
                    HedgeState::Triggered
                } else if i.regime_is_alpha && i.drawdown == 0 {
                    // de-arm if conditions recover
                    HedgeState::Safe
                } else {
                    HedgeState::Armed
                }
            }
            HedgeState::Triggered => {
                // Triggered → Disarmed: unwind complete or forced stop
                if i.unwind_complete {
                    HedgeState::Disarmed
                } else {
                    HedgeState::Triggered
                }
            }
            HedgeState::Disarmed => {
                // Disarmed → Safe: no residual exposure, recovery stable
                if i.no_residual_exposure && i.recovery_stable {
                    HedgeState::Safe
                } else {
                    HedgeState::Disarmed
                }
            }
        }
    }

    fn update_hedge_size(&mut self, inputs: &HedgeInputs) {
        match self.state {
            HedgeState::Armed => {
                // Size up hedge proportional to drawdown
                let raw = fsr_fixed::q32_mul(self.config.hedge_fraction, inputs.drawdown.max(0));
                // INV-10: enforce leverage cap
                self.hedge_size = raw.min(self.config.max_leverage);
            }
            HedgeState::Triggered => {
                // Full hedge size
                self.hedge_size = fsr_fixed::q32_clamp(self.config.hedge_fraction, 0, self.config.max_leverage);
            }
            HedgeState::Disarmed | HedgeState::Safe => {
                self.hedge_size = 0;
            }
        }
    }

    /// Check INV-10: hedge leverage cap.
    pub fn invariant_leverage_ok(&self) -> bool {
        self.hedge_size <= self.config.max_leverage
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_fixed::ONE;

    fn inputs(alpha: bool, dd: i64, mirror_deg: bool, unwind: bool, recovery: bool, no_exp: bool) -> HedgeInputs {
        HedgeInputs {
            regime_is_alpha: alpha,
            drawdown: dd,
            mirror_degraded: mirror_deg,
            unwind_complete: unwind,
            recovery_stable: recovery,
            no_residual_exposure: no_exp,
        }
    }

    #[test]
    fn test_hedge_safe_stays_safe_alpha_no_drawdown() {
        let mut fsm = HedgeFsm::new(HedgeConfig::default());
        let ev = fsm.step(&inputs(true, 0, false, false, false, false));
        assert!(ev.is_none());
        assert_eq!(fsm.state, HedgeState::Safe);
    }

    #[test]
    fn test_hedge_safe_to_armed_non_alpha() {
        let mut fsm = HedgeFsm::new(HedgeConfig::default());
        let ev = fsm.step(&inputs(false, 0, false, false, false, false));
        assert_eq!(ev, Some(EventTag::HedgeArmed));
        assert_eq!(fsm.state, HedgeState::Armed);
    }

    #[test]
    fn test_hedge_armed_to_triggered_drawdown() {
        let mut fsm = HedgeFsm::new(HedgeConfig::default());
        fsm.state = HedgeState::Armed;
        let ev = fsm.step(&inputs(false, ONE / 4, false, false, false, false));
        // drawdown = 0.25 > dd_max = 0.2
        assert_eq!(ev, Some(EventTag::HedgeTriggered));
        assert_eq!(fsm.state, HedgeState::Triggered);
    }

    #[test]
    fn test_hedge_triggered_to_disarmed() {
        let mut fsm = HedgeFsm::new(HedgeConfig::default());
        fsm.state = HedgeState::Triggered;
        let ev = fsm.step(&inputs(false, ONE / 4, false, true, false, false));
        assert_eq!(ev, Some(EventTag::HedgeClosed));
        assert_eq!(fsm.state, HedgeState::Disarmed);
    }

    #[test]
    fn test_hedge_disarmed_to_safe() {
        let mut fsm = HedgeFsm::new(HedgeConfig::default());
        fsm.state = HedgeState::Disarmed;
        let ev = fsm.step(&inputs(true, 0, false, false, true, true));
        assert_eq!(ev, Some(EventTag::HedgeRecovered));
        assert_eq!(fsm.state, HedgeState::Safe);
    }

    #[test]
    fn test_inv10_leverage_cap_respected() {
        let mut fsm = HedgeFsm::new(HedgeConfig::default());
        fsm.state = HedgeState::Armed;
        fsm.step(&inputs(false, ONE / 4, false, false, false, false));
        assert!(fsm.invariant_leverage_ok(), "INV-10: leverage cap must hold");
    }
}
