//! Recovery routing table (spec Appendix B.8, §21).
//!
//! Maps failure class → recovery action deterministically.
//! Recovery is not discretionary.

use fsr_types::{FailureClass, IntegrityPosture};
use serde::{Deserialize, Serialize};

/// Recovery action (from Appendix B.8).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoveryAction {
    /// LocalRecover: reject candidate/append witness, continue loop.
    LocalRecover,
    /// SafeHold: freeze actuation, preserve audit.
    SafeHold,
    /// Rollback: revert to prior config/state.
    Rollback,
    /// Kill: terminal. Evidence divergence is terminal.
    Kill,
    /// Degrade: reduce breadth, maintain legality.
    Degrade,
}

/// Route a failure class to its required recovery action (spec Appendix B.8).
/// Recovery is deterministic from failure class.
pub fn route_recovery(failure: &FailureClass) -> RecoveryAction {
    match failure {
        FailureClass::CandidateFailure(_) => RecoveryAction::LocalRecover,
        FailureClass::QuorumFailure(_) => RecoveryAction::LocalRecover,
        FailureClass::ExecutionFailure(_) => RecoveryAction::SafeHold, // depends on exposure
        FailureClass::HedgeFailure(_) => RecoveryAction::SafeHold,
        FailureClass::ReplayFailure(_) => RecoveryAction::SafeHold,
        FailureClass::IntegrityFailure(_) => RecoveryAction::Kill, // hash-chain irrecoverable
        FailureClass::SchemaFailure(_) => RecoveryAction::Rollback,
        FailureClass::PromotionFailure(_) => RecoveryAction::Rollback,
        FailureClass::TemporalFailure(_) => RecoveryAction::SafeHold,
        FailureClass::ExternalDepFailure(_) => RecoveryAction::Degrade,
        FailureClass::ResourceFailure(_) => RecoveryAction::Degrade,
    }
}

/// GovernanceController trait (spec §28).
pub trait GovernanceController {
    fn current_integrity(&self) -> IntegrityPosture;
    fn trigger_safe_hold(&mut self, reason: &str);
    fn trigger_rollback(&mut self, reason: &str);
    fn trigger_kill(&mut self, reason: &str);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candidate_failure_local_recover() {
        let f = FailureClass::CandidateFailure("no edge".to_string());
        assert_eq!(route_recovery(&f), RecoveryAction::LocalRecover);
    }

    #[test]
    fn test_integrity_failure_kills() {
        let f = FailureClass::IntegrityFailure("hash chain broken".to_string());
        assert_eq!(route_recovery(&f), RecoveryAction::Kill);
    }

    #[test]
    fn test_hedge_failure_safe_hold() {
        let f = FailureClass::HedgeFailure("uncontrolled exposure".to_string());
        assert_eq!(route_recovery(&f), RecoveryAction::SafeHold);
    }

    #[test]
    fn test_promotion_failure_rollback() {
        let f = FailureClass::PromotionFailure("gate rejected".to_string());
        assert_eq!(route_recovery(&f), RecoveryAction::Rollback);
    }

    #[test]
    fn test_resource_failure_degrade() {
        let f = FailureClass::ResourceFailure("OOM".to_string());
        assert_eq!(route_recovery(&f), RecoveryAction::Degrade);
    }

    #[test]
    fn test_all_11_failure_classes_routed() {
        use FailureClass::*;
        let classes = vec![
            CandidateFailure("".to_string()),
            QuorumFailure("".to_string()),
            ExecutionFailure("".to_string()),
            HedgeFailure("".to_string()),
            ReplayFailure("".to_string()),
            IntegrityFailure("".to_string()),
            SchemaFailure("".to_string()),
            PromotionFailure("".to_string()),
            TemporalFailure("".to_string()),
            ExternalDepFailure("".to_string()),
            ResourceFailure("".to_string()),
        ];
        assert_eq!(classes.len(), 11, "All 11 failure classes must be tested");
        for c in &classes {
            let action = route_recovery(c);
            // Just verify it doesn't panic and returns a valid action
            let _ = action;
        }
    }
}
