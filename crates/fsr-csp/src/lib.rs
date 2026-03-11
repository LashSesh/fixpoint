//! fsr-csp: Commitment Schedule Protocol (CSP) FSM (spec Appendix B.2).
//!
//! 9 states: Idle → Discovered → IntentOpen → IntentQuorum → ExecLockstep → Confirm → Settled
//!           Any → Abort → Reset → Idle

use fsr_types::{CspState, EventTag, NullcenterCert};
use serde::{Deserialize, Serialize};

/// Quorum requirements for a CSP cycle.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuorumRequirements {
    pub edge_ok: bool,
    pub ttl_ok: bool,
    pub depth_ok: bool,
    pub slip_ok: bool,
    pub mirror_ok: bool,
    pub temporal_ok: bool,
}

impl QuorumRequirements {
    pub fn all_pass(&self) -> bool {
        self.edge_ok && self.ttl_ok && self.depth_ok && self.slip_ok && self.mirror_ok && self.temporal_ok
    }

    pub fn blocking_reason(&self) -> Option<&'static str> {
        if !self.edge_ok { return Some("edge"); }
        if !self.ttl_ok { return Some("TTL"); }
        if !self.depth_ok { return Some("depth"); }
        if !self.slip_ok { return Some("slip"); }
        if !self.mirror_ok { return Some("mirror"); }
        if !self.temporal_ok { return Some("temporal"); }
        None
    }
}

/// Lockstep admissibility check inputs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LockstepAdmissibility {
    pub nc_cert: NullcenterCert,
    pub route_admissible: bool,
}

impl LockstepAdmissibility {
    pub fn is_admissible(&self) -> bool {
        self.nc_cert.gate_open && self.route_admissible
    }
}

/// CSP FSM controller (spec Appendix B.2).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CspFsm {
    pub state: CspState,
    /// Abort reason (set when transitioning to Abort state).
    pub abort_reason: Option<String>,
}

impl CspFsm {
    pub fn new() -> Self {
        CspFsm {
            state: CspState::Idle,
            abort_reason: None,
        }
    }

    /// Idle → Discovered: candidate survives hard filters.
    /// Called when PoR reaches Commit.
    pub fn on_candidate_discovered(&mut self) -> Option<EventTag> {
        if self.state == CspState::Idle {
            self.state = CspState::Discovered;
            Some(EventTag::CandidateDiscovered)
        } else {
            None
        }
    }

    /// Discovered → IntentOpen: all required gates pass.
    pub fn open_intent(&mut self) -> Option<EventTag> {
        if self.state == CspState::Discovered {
            self.state = CspState::IntentOpen;
            Some(EventTag::IntentOpened)
        } else {
            None
        }
    }

    /// IntentOpen → IntentQuorum: every leg satisfies quorum rules.
    /// IntentOpen → Abort: quorum rules fail.
    pub fn check_quorum(&mut self, reqs: &QuorumRequirements) -> EventTag {
        if self.state != CspState::IntentOpen {
            return self.abort("not in IntentOpen state");
        }
        // INV-04: no quorum without edge, TTL, depth, slip, mirror, and temporal validity
        if reqs.all_pass() {
            self.state = CspState::IntentQuorum;
            EventTag::QuorumPassed
        } else {
            let reason = reqs.blocking_reason().unwrap_or("unknown").to_string();
            self.abort_with_reason(format!("quorum failed: {}", reason))
        }
    }

    /// IntentQuorum → ExecLockstep: lockstep admissibility true.
    pub fn start_lockstep(&mut self, admissibility: &LockstepAdmissibility) -> Option<EventTag> {
        if self.state != CspState::IntentQuorum {
            return None;
        }
        if admissibility.is_admissible() {
            self.state = CspState::ExecLockstep;
            Some(EventTag::ExecutionStarted)
        } else {
            self.abort_inner("lockstep admissibility failed");
            None
        }
    }

    /// ExecLockstep → Confirm: all execution receipts available.
    pub fn on_receipts_available(&mut self) -> Option<EventTag> {
        if self.state == CspState::ExecLockstep {
            self.state = CspState::Confirm;
            Some(EventTag::ExecutionConfirmed)
        } else {
            None
        }
    }

    /// Confirm → Settled: all confirmations valid.
    pub fn on_settled(&mut self) -> Option<EventTag> {
        if self.state == CspState::Confirm {
            self.state = CspState::Settled;
            Some(EventTag::ExecutionSettled)
        } else {
            None
        }
    }

    /// Settled → Reset: settlement cleanup.
    pub fn begin_reset_from_settled(&mut self) -> Option<EventTag> {
        if self.state == CspState::Settled {
            self.state = CspState::Reset;
            Some(EventTag::ResetEvent)
        } else {
            None
        }
    }

    /// Any active → Abort: explicit abort class emitted.
    pub fn abort_now(&mut self, reason: impl Into<String>) -> EventTag {
        self.abort_with_reason(reason.into())
    }

    /// Abort → Reset: abort witness appended.
    pub fn begin_reset_from_abort(&mut self) -> Option<EventTag> {
        if self.state == CspState::Abort {
            self.state = CspState::Reset;
            Some(EventTag::ResetEvent)
        } else {
            None
        }
    }

    /// Reset → Idle: state cleanup complete.
    pub fn complete_reset(&mut self) -> Option<EventTag> {
        if self.state == CspState::Reset {
            self.state = CspState::Idle;
            self.abort_reason = None;
            Some(EventTag::ResetComplete)
        } else {
            None
        }
    }

    fn abort(&mut self, reason: &str) -> EventTag {
        self.abort_with_reason(reason.to_string())
    }

    fn abort_with_reason(&mut self, reason: String) -> EventTag {
        self.state = CspState::Abort;
        self.abort_reason = Some(reason);
        EventTag::ExecutionAborted
    }

    fn abort_inner(&mut self, reason: &str) {
        self.state = CspState::Abort;
        self.abort_reason = Some(reason.to_string());
    }
}

impl Default for CspFsm {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_types::{CspState, Hash256, NullcenterCert};
    use fsr_fixed::ONE;

    fn good_quorum() -> QuorumRequirements {
        QuorumRequirements {
            edge_ok: true,
            ttl_ok: true,
            depth_ok: true,
            slip_ok: true,
            mirror_ok: true,
            temporal_ok: true,
        }
    }

    fn good_admissibility() -> LockstepAdmissibility {
        LockstepAdmissibility {
            nc_cert: NullcenterCert {
                gate_open: true,
                si_at_cert: ONE,
                phase_bin: 0,
                wind_count: 1,
                rolling_digest: Hash256::ZERO,
                pre_digest: Hash256::ZERO,
                post_digest: Hash256::ZERO,
            },
            route_admissible: true,
        }
    }

    #[test]
    fn test_csp_full_happy_path() {
        let mut csp = CspFsm::new();
        assert_eq!(csp.state, CspState::Idle);

        csp.on_candidate_discovered();
        assert_eq!(csp.state, CspState::Discovered);

        csp.open_intent();
        assert_eq!(csp.state, CspState::IntentOpen);

        csp.check_quorum(&good_quorum());
        assert_eq!(csp.state, CspState::IntentQuorum);

        csp.start_lockstep(&good_admissibility());
        assert_eq!(csp.state, CspState::ExecLockstep);

        csp.on_receipts_available();
        assert_eq!(csp.state, CspState::Confirm);

        csp.on_settled();
        assert_eq!(csp.state, CspState::Settled);

        csp.begin_reset_from_settled();
        assert_eq!(csp.state, CspState::Reset);

        csp.complete_reset();
        assert_eq!(csp.state, CspState::Idle);
    }

    #[test]
    fn test_csp_quorum_fail_aborts() {
        let mut csp = CspFsm::new();
        csp.on_candidate_discovered();
        csp.open_intent();
        let bad_quorum = QuorumRequirements {
            edge_ok: false,
            ttl_ok: true,
            depth_ok: true,
            slip_ok: true,
            mirror_ok: true,
            temporal_ok: true,
        };
        let ev = csp.check_quorum(&bad_quorum);
        assert_eq!(ev, EventTag::ExecutionAborted);
        assert_eq!(csp.state, CspState::Abort);
    }

    #[test]
    fn test_csp_abort_and_reset_cycle() {
        let mut csp = CspFsm::new();
        csp.abort_now("test abort");
        assert_eq!(csp.state, CspState::Abort);

        csp.begin_reset_from_abort();
        assert_eq!(csp.state, CspState::Reset);

        csp.complete_reset();
        assert_eq!(csp.state, CspState::Idle);
    }

    #[test]
    fn test_csp_discovered_requires_idle() {
        let mut csp = CspFsm::new();
        csp.on_candidate_discovered();
        // Already Discovered, cannot re-discover from non-Idle
        let ev = csp.on_candidate_discovered();
        assert!(ev.is_none(), "cannot discover from non-Idle state");
    }

    #[test]
    fn test_quorum_all_fields_required() {
        let reqs = good_quorum();
        assert!(reqs.all_pass());
        assert!(reqs.blocking_reason().is_none());

        let mut bad = reqs.clone();
        bad.slip_ok = false;
        assert!(!bad.all_pass());
        assert_eq!(bad.blocking_reason(), Some("slip"));
    }
}
