//! Integrity posture FSM (spec Appendix B.6).
//!
//! States: Healthy → Degraded → SafeHold | RollbackPending → Healthy | Killed
//! Any → Killed on unrecoverable violation or operator kill.

use fsr_types::{EventTag, IntegrityPosture};
use serde::{Deserialize, Serialize};

/// Integrity FSM inputs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntegrityInputs {
    pub non_blocking_failure: bool,
    pub slo_breach: bool,
    pub blocking_invariant_breach: bool,
    pub rollback_target_valid: bool,
    pub promotion_regression: bool,
    pub no_corruption: bool,
    pub issue_corrected: bool,
    pub checks_pass: bool,
    pub rollback_completed: bool,
    pub rollback_validated: bool,
    pub unrecoverable_violation: bool,
    pub operator_kill: bool,
}

/// Integrity posture FSM controller.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntegrityFsm {
    pub state: IntegrityPosture,
}

impl IntegrityFsm {
    pub fn new() -> Self {
        IntegrityFsm { state: IntegrityPosture::Healthy }
    }

    pub fn step(&mut self, inputs: &IntegrityInputs) -> Option<EventTag> {
        let next = self.next_state(inputs);
        if next != self.state {
            let event = match next {
                IntegrityPosture::Degraded => Some(EventTag::IntegrityDegraded),
                IntegrityPosture::SafeHold => Some(EventTag::SafeHoldEntered),
                IntegrityPosture::RollbackPending => Some(EventTag::RollbackRequested),
                IntegrityPosture::Healthy => {
                    if self.state == IntegrityPosture::SafeHold {
                        Some(EventTag::SafeHoldReleased)
                    } else {
                        Some(EventTag::RollbackCompleted)
                    }
                }
                IntegrityPosture::Killed => Some(EventTag::KillActivated),
            };
            self.state = next;
            event
        } else {
            None
        }
    }

    fn next_state(&self, i: &IntegrityInputs) -> IntegrityPosture {
        // Kill takes priority in any state
        if i.unrecoverable_violation || i.operator_kill {
            return IntegrityPosture::Killed;
        }
        match self.state {
            IntegrityPosture::Healthy => {
                if i.non_blocking_failure || i.slo_breach {
                    IntegrityPosture::Degraded
                } else {
                    IntegrityPosture::Healthy
                }
            }
            IntegrityPosture::Degraded => {
                if i.blocking_invariant_breach && !i.rollback_target_valid {
                    IntegrityPosture::SafeHold
                } else if (i.blocking_invariant_breach || i.promotion_regression)
                    && i.rollback_target_valid
                    && i.no_corruption
                {
                    IntegrityPosture::RollbackPending
                } else {
                    IntegrityPosture::Degraded
                }
            }
            IntegrityPosture::SafeHold => {
                if i.issue_corrected && i.checks_pass {
                    IntegrityPosture::Healthy
                } else {
                    IntegrityPosture::SafeHold
                }
            }
            IntegrityPosture::RollbackPending => {
                if i.rollback_completed && i.rollback_validated {
                    IntegrityPosture::Healthy
                } else {
                    IntegrityPosture::RollbackPending
                }
            }
            IntegrityPosture::Killed => IntegrityPosture::Killed,
        }
    }

    /// INV-12: safe-hold must freeze actuation while preserving observation and audit.
    pub fn actuation_frozen(&self) -> bool {
        matches!(self.state, IntegrityPosture::SafeHold | IntegrityPosture::Killed)
    }
}

impl Default for IntegrityFsm {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_inputs() -> IntegrityInputs {
        IntegrityInputs {
            non_blocking_failure: false,
            slo_breach: false,
            blocking_invariant_breach: false,
            rollback_target_valid: false,
            promotion_regression: false,
            no_corruption: true,
            issue_corrected: false,
            checks_pass: false,
            rollback_completed: false,
            rollback_validated: false,
            unrecoverable_violation: false,
            operator_kill: false,
        }
    }

    #[test]
    fn test_integrity_healthy_stays() {
        let mut fsm = IntegrityFsm::new();
        let ev = fsm.step(&base_inputs());
        assert!(ev.is_none());
        assert_eq!(fsm.state, IntegrityPosture::Healthy);
    }

    #[test]
    fn test_healthy_to_degraded() {
        let mut fsm = IntegrityFsm::new();
        let mut i = base_inputs();
        i.non_blocking_failure = true;
        let ev = fsm.step(&i);
        assert_eq!(ev, Some(EventTag::IntegrityDegraded));
        assert_eq!(fsm.state, IntegrityPosture::Degraded);
    }

    #[test]
    fn test_degraded_to_safehold() {
        let mut fsm = IntegrityFsm::new();
        fsm.state = IntegrityPosture::Degraded;
        let mut i = base_inputs();
        i.blocking_invariant_breach = true;
        i.rollback_target_valid = false;
        let ev = fsm.step(&i);
        assert_eq!(ev, Some(EventTag::SafeHoldEntered));
        assert_eq!(fsm.state, IntegrityPosture::SafeHold);
    }

    #[test]
    fn test_degraded_to_rollback_pending() {
        let mut fsm = IntegrityFsm::new();
        fsm.state = IntegrityPosture::Degraded;
        let mut i = base_inputs();
        i.blocking_invariant_breach = true;
        i.rollback_target_valid = true;
        i.no_corruption = true;
        let ev = fsm.step(&i);
        assert_eq!(ev, Some(EventTag::RollbackRequested));
        assert_eq!(fsm.state, IntegrityPosture::RollbackPending);
    }

    #[test]
    fn test_safehold_to_healthy() {
        let mut fsm = IntegrityFsm::new();
        fsm.state = IntegrityPosture::SafeHold;
        let mut i = base_inputs();
        i.issue_corrected = true;
        i.checks_pass = true;
        let ev = fsm.step(&i);
        assert_eq!(ev, Some(EventTag::SafeHoldReleased));
        assert_eq!(fsm.state, IntegrityPosture::Healthy);
    }

    #[test]
    fn test_kill_from_any_state() {
        let mut fsm = IntegrityFsm::new();
        let mut i = base_inputs();
        i.operator_kill = true;
        let ev = fsm.step(&i);
        assert_eq!(ev, Some(EventTag::KillActivated));
        assert_eq!(fsm.state, IntegrityPosture::Killed);
    }

    #[test]
    fn test_inv12_actuation_frozen_in_safehold() {
        let mut fsm = IntegrityFsm::new();
        fsm.state = IntegrityPosture::SafeHold;
        assert!(fsm.actuation_frozen(), "INV-12: SafeHold must freeze actuation");
        fsm.state = IntegrityPosture::Healthy;
        assert!(!fsm.actuation_frozen());
    }
}
