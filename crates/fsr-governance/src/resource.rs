//! Resource posture FSM (spec Appendix B.7, §22).
//!
//! States: Nominal → Constrained → Scarce → Emergency (and recovery path).
//! INV-13: resource degradation must never silently weaken blocking gates.
//! INV-14: resource exhaustion may degrade breadth, never legality.

use fsr_types::{EventTag, ResourcePosture};
use serde::{Deserialize, Serialize};

/// Resource budget readings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceBudget {
    /// CPU usage 0..1 (Q32)
    pub cpu: i64,
    /// Memory usage 0..1 (Q32)
    pub mem: i64,
    /// Storage usage 0..1 (Q32)
    pub storage: i64,
    /// Soft limit threshold (Q32)
    pub soft_limit: i64,
    /// Hard limit threshold (Q32)
    pub hard_limit: i64,
}

impl ResourceBudget {
    /// Fraction of budgets approaching soft limit.
    pub fn approaching_soft(&self) -> bool {
        self.cpu > self.soft_limit || self.mem > self.soft_limit || self.storage > self.soft_limit
    }

    /// Multiple budgets exceeding soft or one exceeding hard.
    pub fn constrained_to_scarce(&self) -> bool {
        let soft_hits = [self.cpu, self.mem, self.storage]
            .iter()
            .filter(|&&v| v > self.soft_limit)
            .count();
        let hard_hit = self.cpu > self.hard_limit
            || self.mem > self.hard_limit
            || self.storage > self.hard_limit;
        soft_hits >= 2 || hard_hit
    }

    /// Legality-preserving operation at risk (→ Emergency).
    pub fn emergency_condition(&self) -> bool {
        // Emergency if any budget is critically high
        let critical = self.hard_limit + (self.hard_limit / 10); // 10% over hard
        self.cpu > critical || self.mem > critical || self.storage > critical
    }

    /// Emergency cleared and legality preserved.
    pub fn emergency_cleared(&self) -> bool {
        !self.emergency_condition()
    }

    /// Usage returns below hard limits.
    pub fn below_hard(&self) -> bool {
        self.cpu <= self.hard_limit
            && self.mem <= self.hard_limit
            && self.storage <= self.hard_limit
    }

    /// Sustained healthy margin.
    pub fn healthy_margin(&self) -> bool {
        let margin = self.soft_limit * 7 / 10; // 70% of soft limit
        self.cpu <= margin && self.mem <= margin && self.storage <= margin
    }
}

/// Resource posture FSM controller.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceFsm {
    pub state: ResourcePosture,
}

impl ResourceFsm {
    pub fn new() -> Self {
        ResourceFsm { state: ResourcePosture::Nominal }
    }

    pub fn step(&mut self, budget: &ResourceBudget) -> Option<EventTag> {
        let next = self.next_state(budget);
        if next != self.state {
            let posture_rank = |p: ResourcePosture| -> u8 {
                match p {
                    ResourcePosture::Nominal => 0,
                    ResourcePosture::Constrained => 1,
                    ResourcePosture::Scarce => 2,
                    ResourcePosture::Emergency => 3,
                }
            };
            let event = if posture_rank(next) > posture_rank(self.state) {
                // Degrading
                if next == ResourcePosture::Emergency {
                    Some(EventTag::ResourceEmergency)
                } else {
                    Some(EventTag::ResourceDegraded)
                }
            } else {
                // Recovering
                Some(EventTag::ResourceRecovered)
            };
            self.state = next;
            event
        } else {
            None
        }
    }

    fn next_state(&self, b: &ResourceBudget) -> ResourcePosture {
        match self.state {
            ResourcePosture::Nominal => {
                if b.approaching_soft() {
                    ResourcePosture::Constrained
                } else {
                    ResourcePosture::Nominal
                }
            }
            ResourcePosture::Constrained => {
                if b.constrained_to_scarce() {
                    ResourcePosture::Scarce
                } else if b.healthy_margin() {
                    ResourcePosture::Nominal
                } else {
                    ResourcePosture::Constrained
                }
            }
            ResourcePosture::Scarce => {
                if b.emergency_condition() {
                    ResourcePosture::Emergency
                } else if b.below_hard() {
                    ResourcePosture::Constrained
                } else {
                    ResourcePosture::Scarce
                }
            }
            ResourcePosture::Emergency => {
                if b.emergency_cleared() && b.below_hard() {
                    ResourcePosture::Scarce
                } else {
                    ResourcePosture::Emergency
                }
            }
        }
    }
}

impl Default for ResourceFsm {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_fixed::ONE;

    fn budget(cpu: i64, mem: i64, storage: i64) -> ResourceBudget {
        ResourceBudget {
            cpu,
            mem,
            storage,
            soft_limit: ONE * 7 / 10,  // 70%
            hard_limit: ONE * 9 / 10,  // 90%
        }
    }

    #[test]
    fn test_nominal_stays_nominal() {
        let mut fsm = ResourceFsm::new();
        let b = budget(ONE / 2, ONE / 2, ONE / 2); // all at 50%
        let ev = fsm.step(&b);
        assert!(ev.is_none());
        assert_eq!(fsm.state, ResourcePosture::Nominal);
    }

    #[test]
    fn test_nominal_to_constrained() {
        let mut fsm = ResourceFsm::new();
        let b = budget(ONE * 8 / 10, ONE / 2, ONE / 2); // cpu above soft
        let ev = fsm.step(&b);
        assert_eq!(ev, Some(EventTag::ResourceDegraded));
        assert_eq!(fsm.state, ResourcePosture::Constrained);
    }

    #[test]
    fn test_constrained_to_scarce_multiple_soft() {
        let mut fsm = ResourceFsm::new();
        fsm.state = ResourcePosture::Constrained;
        let b = budget(ONE * 8 / 10, ONE * 8 / 10, ONE / 2); // 2 above soft
        let ev = fsm.step(&b);
        assert_eq!(ev, Some(EventTag::ResourceDegraded));
        assert_eq!(fsm.state, ResourcePosture::Scarce);
    }

    #[test]
    fn test_scarce_to_emergency() {
        let mut fsm = ResourceFsm::new();
        fsm.state = ResourcePosture::Scarce;
        let b = budget(ONE, ONE / 2, ONE / 2); // cpu at 100% (over hard + 10%)
        let ev = fsm.step(&b);
        assert_eq!(ev, Some(EventTag::ResourceEmergency));
        assert_eq!(fsm.state, ResourcePosture::Emergency);
    }

    #[test]
    fn test_emergency_to_scarce_recovery() {
        let mut fsm = ResourceFsm::new();
        fsm.state = ResourcePosture::Emergency;
        let b = budget(ONE / 2, ONE / 2, ONE / 2); // all clear
        let ev = fsm.step(&b);
        assert_eq!(ev, Some(EventTag::ResourceRecovered));
        assert_eq!(fsm.state, ResourcePosture::Scarce);
    }
}
