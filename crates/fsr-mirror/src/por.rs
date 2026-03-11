//! Proof-of-Route (PoR) FSM (spec Appendix B.3).
//!
//! States: Search → Lock → Verify → Commit
//!   Any → Search on rejection/TTL expiry.
//! PoR reaches Commit → CSP enters Discovered.

use fsr_types::{EventTag, PorState, Q32};
use serde::{Deserialize, Serialize};

/// PoR FSM controller.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PorFsm {
    pub state: PorState,
    /// TTL countdown in ticks.
    pub ttl: u64,
    /// SI at lock time (for PoR acceptance check).
    pub si_at_lock: Q32,
}

/// PoR acceptance config (spec §14.3).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PorAcceptConfig {
    /// ρ_min: minimum stability for bounded regression acceptance
    pub rho_min: Q32,
    /// ε: maximum allowed quality regression
    pub epsilon: Q32,
}

impl PorFsm {
    pub fn new(ttl: u64) -> Self {
        PorFsm {
            state: PorState::Search,
            ttl,
            si_at_lock: 0,
        }
    }

    /// Attempt to lock a route candidate (Search → Lock).
    /// Returns RouteLocked event if successful.
    pub fn try_lock(&mut self, has_sufficient_edge: bool, si: Q32) -> Option<EventTag> {
        if self.state == PorState::Search && has_sufficient_edge {
            self.state = PorState::Lock;
            self.si_at_lock = si;
            Some(EventTag::RouteLocked)
        } else {
            None
        }
    }

    /// Verify depth/slip/mirror check (Lock → Verify).
    /// Returns RouteVerified or falls back to Search on failure.
    pub fn try_verify(&mut self, depth_ok: bool, slip_ok: bool, mirror_ok: bool) -> EventTag {
        match self.state {
            PorState::Lock => {
                if depth_ok && slip_ok && mirror_ok {
                    self.state = PorState::Verify;
                    EventTag::RouteVerified
                } else {
                    self.state = PorState::Search;
                    EventTag::RouteRejected
                }
            }
            _ => {
                self.state = PorState::Search;
                EventTag::RouteRejected
            }
        }
    }

    /// Commit route (Verify → Commit) if temporal key valid and NC cert issued.
    /// Returns RouteCommitted or RouteRejected.
    pub fn try_commit(&mut self, temporal_valid: bool, nc_cert_issued: bool) -> EventTag {
        match self.state {
            PorState::Verify => {
                if temporal_valid && nc_cert_issued {
                    self.state = PorState::Commit;
                    EventTag::RouteCommitted
                } else {
                    self.state = PorState::Search;
                    EventTag::RouteRejected
                }
            }
            _ => {
                self.state = PorState::Search;
                EventTag::RouteRejected
            }
        }
    }

    /// Tick TTL countdown; reject if expired. Any state → Search on expiry.
    pub fn tick_ttl(&mut self) -> Option<EventTag> {
        if self.ttl == 0 {
            if self.state != PorState::Search {
                self.state = PorState::Search;
                return Some(EventTag::RouteRejected);
            }
            return None;
        }
        self.ttl -= 1;
        None
    }

    /// Reset to Search state (on any failure).
    pub fn reset(&mut self, new_ttl: u64) {
        self.state = PorState::Search;
        self.ttl = new_ttl;
        self.si_at_lock = 0;
    }

    /// PoR acceptance rule (spec §14.3, TMCP Definition 11.8).
    /// Returns true if new_snap is acceptable relative to old_snap.
    pub fn acceptance_check(
        new_psi: Q32,
        old_psi: Q32,
        new_rho: Q32,
        config: &PorAcceptConfig,
    ) -> bool {
        // Monotone: new ≥ old under stability-dominant preorder
        let monotone = new_psi >= old_psi && new_rho >= config.rho_min;
        // Bounded regression: ρ_new ≥ ρ_min and ψ_new ≥ ψ_old - ε
        let bounded = new_rho >= config.rho_min && new_psi >= old_psi.saturating_sub(config.epsilon);
        monotone || bounded
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_fixed::ONE;

    #[test]
    fn test_por_full_happy_path() {
        let mut por = PorFsm::new(10);
        assert_eq!(por.state, PorState::Search);

        // Lock
        let ev = por.try_lock(true, ONE);
        assert_eq!(ev, Some(EventTag::RouteLocked));
        assert_eq!(por.state, PorState::Lock);

        // Verify
        let ev = por.try_verify(true, true, true);
        assert_eq!(ev, EventTag::RouteVerified);
        assert_eq!(por.state, PorState::Verify);

        // Commit
        let ev = por.try_commit(true, true);
        assert_eq!(ev, EventTag::RouteCommitted);
        assert_eq!(por.state, PorState::Commit);
    }

    #[test]
    fn test_por_verify_fails_resets_to_search() {
        let mut por = PorFsm::new(10);
        por.try_lock(true, ONE);
        let ev = por.try_verify(false, true, true);
        assert_eq!(ev, EventTag::RouteRejected);
        assert_eq!(por.state, PorState::Search);
    }

    #[test]
    fn test_por_commit_fails_without_nc_cert() {
        let mut por = PorFsm::new(10);
        por.try_lock(true, ONE);
        por.try_verify(true, true, true);
        let ev = por.try_commit(true, false);
        assert_eq!(ev, EventTag::RouteRejected);
        assert_eq!(por.state, PorState::Search);
    }

    #[test]
    fn test_por_ttl_expiry() {
        let mut por = PorFsm::new(1);
        por.try_lock(true, ONE);
        por.tick_ttl(); // ttl now 0
        let ev = por.tick_ttl();
        assert_eq!(ev, Some(EventTag::RouteRejected));
        assert_eq!(por.state, PorState::Search);
    }

    #[test]
    fn test_por_acceptance_monotone() {
        let cfg = PorAcceptConfig { rho_min: ONE / 2, epsilon: ONE / 10 };
        assert!(PorFsm::acceptance_check(ONE, ONE, ONE, &cfg));
        assert!(PorFsm::acceptance_check(ONE, ONE / 2, ONE, &cfg));
    }

    #[test]
    fn test_por_acceptance_bounded_regression() {
        let cfg = PorAcceptConfig { rho_min: ONE / 2, epsilon: ONE / 10 };
        // new_psi = old_psi - epsilon (exactly at boundary)
        let old_psi = ONE;
        let new_psi = old_psi - ONE / 10;
        assert!(PorFsm::acceptance_check(new_psi, old_psi, ONE * 3 / 4, &cfg));
    }
}
