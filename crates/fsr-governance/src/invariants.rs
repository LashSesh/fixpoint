//! 15 Global Invariants (spec Appendix A).
//! All invariants are enforced at runtime. Blocking violations trigger safe-hold/rollback/kill.

use fsr_types::{IntegrityPosture, NullcenterCert};

/// Result of invariant check.
#[derive(Clone, Debug)]
pub struct InvariantResult {
    pub id: u8,
    pub passed: bool,
    pub message: &'static str,
}

/// Full invariant check context.
pub struct InvariantContext<'a> {
    pub has_nc_cert: bool,
    pub nc_cert: Option<&'a NullcenterCert>,
    pub event_emitted_on_transition: bool,
    pub temporal_key_valid: bool,
    pub quorum_checks_done: bool,
    pub paper_validated_before_live: bool,
    pub promotion_gate_passed: bool,
    pub replay_divergence_disclosed: bool,
    pub no_hidden_mutable_state: bool,
    pub shadow_chain_consistent: bool,
    pub commitment_chain_consistent: bool,
    pub schema_migration_preserves_lineage: bool,
    pub hedge_leverage_within_cap: bool,
    pub hard_filter_not_rescued_by_ranking: bool,
    pub safe_hold_freezes_actuation: bool,
    pub resource_does_not_weaken_gates: bool,
    pub resource_degrades_breadth_not_legality: bool,
    pub buildable_without_enterprise_infra: bool,
    pub integrity_posture: IntegrityPosture,
}

/// Check all 15 invariants. Returns list of failed invariant IDs (blocking).
pub fn check_invariants(ctx: &InvariantContext) -> Vec<InvariantResult> {
    vec![
        InvariantResult {
            id: 1,
            passed: ctx.has_nc_cert,
            message: "INV-01: No execution without nullcenter certificate",
        },
        InvariantResult {
            id: 2,
            passed: ctx.event_emitted_on_transition,
            message: "INV-02: No state transition without typed event emission",
        },
        InvariantResult {
            id: 3,
            passed: ctx.temporal_key_valid,
            message: "INV-03: No consequential event without valid temporal key",
        },
        InvariantResult {
            id: 4,
            passed: ctx.quorum_checks_done,
            message: "INV-04: No quorum without edge, TTL, depth, slip, mirror, temporal validity",
        },
        InvariantResult {
            id: 5,
            passed: ctx.paper_validated_before_live && ctx.promotion_gate_passed,
            message: "INV-05: No live promotion without paper validation and promotion gate",
        },
        InvariantResult {
            id: 6,
            passed: ctx.replay_divergence_disclosed,
            message: "INV-06: No replay-success claim if divergence exists and is undisclosed",
        },
        InvariantResult {
            id: 7,
            passed: ctx.no_hidden_mutable_state,
            message: "INV-07: No hidden mutable state may affect decisions",
        },
        InvariantResult {
            id: 8,
            passed: ctx.shadow_chain_consistent && ctx.commitment_chain_consistent,
            message: "INV-08: Shadow-chain and commitment-chain digests must remain hash-consistent",
        },
        InvariantResult {
            id: 9,
            passed: ctx.schema_migration_preserves_lineage,
            message: "INV-09: Schema migration must not destroy replay lineage",
        },
        InvariantResult {
            id: 10,
            passed: ctx.hedge_leverage_within_cap,
            message: "INV-10: Hedge leverage may not exceed configured caps",
        },
        InvariantResult {
            id: 11,
            passed: ctx.hard_filter_not_rescued_by_ranking,
            message: "INV-11: Candidate ranking may not rescue a hard-filter failure",
        },
        InvariantResult {
            id: 12,
            passed: ctx.safe_hold_freezes_actuation,
            message: "INV-12: Safe-hold must freeze actuation while preserving observation and audit",
        },
        InvariantResult {
            id: 13,
            passed: ctx.resource_does_not_weaken_gates,
            message: "INV-13: Resource degradation must never silently weaken blocking gates",
        },
        InvariantResult {
            id: 14,
            passed: ctx.resource_degrades_breadth_not_legality,
            message: "INV-14: Resource exhaustion may degrade breadth, never legality",
        },
        InvariantResult {
            id: 15,
            passed: ctx.buildable_without_enterprise_infra,
            message: "INV-15: Repository must remain buildable without enterprise infrastructure",
        },
    ]
}

/// Returns all blocking invariant failures.
pub fn blocking_failures(ctx: &InvariantContext) -> Vec<InvariantResult> {
    check_invariants(ctx)
        .into_iter()
        .filter(|r| !r.passed)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn good_ctx() -> InvariantContext<'static> {
        InvariantContext {
            has_nc_cert: true,
            nc_cert: None,
            event_emitted_on_transition: true,
            temporal_key_valid: true,
            quorum_checks_done: true,
            paper_validated_before_live: true,
            promotion_gate_passed: true,
            replay_divergence_disclosed: true,
            no_hidden_mutable_state: true,
            shadow_chain_consistent: true,
            commitment_chain_consistent: true,
            schema_migration_preserves_lineage: true,
            hedge_leverage_within_cap: true,
            hard_filter_not_rescued_by_ranking: true,
            safe_hold_freezes_actuation: true,
            resource_does_not_weaken_gates: true,
            resource_degrades_breadth_not_legality: true,
            buildable_without_enterprise_infra: true,
            integrity_posture: IntegrityPosture::Healthy,
        }
    }

    #[test]
    fn test_all_invariants_pass() {
        let ctx = good_ctx();
        let failures = blocking_failures(&ctx);
        assert!(failures.is_empty(), "All invariants should pass: {:?}", failures);
    }

    #[test]
    fn test_inv01_fails_without_nc_cert() {
        let mut ctx = good_ctx();
        ctx.has_nc_cert = false;
        let failures = blocking_failures(&ctx);
        assert!(failures.iter().any(|r| r.id == 1), "INV-01 should fail");
    }

    #[test]
    fn test_inv08_fails_on_chain_inconsistency() {
        let mut ctx = good_ctx();
        ctx.shadow_chain_consistent = false;
        let failures = blocking_failures(&ctx);
        assert!(failures.iter().any(|r| r.id == 8), "INV-08 should fail");
    }

    #[test]
    fn test_inv05_fails_without_paper_validation() {
        let mut ctx = good_ctx();
        ctx.paper_validated_before_live = false;
        let failures = blocking_failures(&ctx);
        assert!(failures.iter().any(|r| r.id == 5));
    }

    #[test]
    fn test_all_15_invariants_covered() {
        let ctx = good_ctx();
        let results = check_invariants(&ctx);
        assert_eq!(results.len(), 15, "All 15 invariants must be checked");
        let ids: Vec<u8> = results.iter().map(|r| r.id).collect();
        for i in 1u8..=15 {
            assert!(ids.contains(&i), "INV-{:02} must be in results", i);
        }
    }
}
