//! Typed event tags for all state machine transitions and consequential actions.

use serde::{Deserialize, Serialize};
use crate::Hash256;
use crate::temporal::TemporalKey;

/// All event tags used across the system (spec §18, Appendix B).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventTag {
    // Regime (B.1)
    RegimeChanged,
    // CSP (B.2)
    CandidateDiscovered,
    IntentOpened,
    QuorumPassed,
    QuorumFailed,
    ExecutionStarted,
    ExecutionConfirmed,
    ExecutionSettled,
    ExecutionAborted,
    ResetEvent,
    ResetComplete,
    // PoR (B.3)
    RouteLocked,
    RouteVerified,
    RouteCommitted,
    RouteRejected,
    // Hedge (B.4)
    HedgeArmed,
    HedgeTriggered,
    HedgeClosed,
    HedgeRecovered,
    // Promotion (B.5)
    CalibrationAccepted,
    PromotionProposal,
    PromotionGatePass,
    PromotionActivated,
    PromotionDemoted,
    PromotionRevoked,
    // Integrity (B.6)
    IntegrityDegraded,
    SafeHoldEntered,
    SafeHoldReleased,
    RollbackRequested,
    RollbackCompleted,
    KillActivated,
    // Resource (B.7)
    ResourceDegraded,
    ResourceEmergency,
    ResourceRecovered,
    // Nullcenter
    NullcenterTraversal,
    // Calibration
    CalibrationUpdate,
    // General
    MacroCycleStart,
    MacroCycleEnd,
    StatusReport,
    // Phase 2 additions (additive only — do not remove or reorder above variants)
    SnapshotWritten,
    ConfigReloaded,
    TtcpCrystal,
    // Phase 3 additions (additive only — do not remove or reorder above variants)
    RecordingStarted,
    RecordingFrame,
    BacktestCompleted,
    PromotionGateChecked,
    PromotionApproved,
    SniperArmed,
    SniperExecuted,
    SniperCooldown,
    SniperDisarmed,
    DailyLossLimitHit,
    RiskLimitBreached,
    ReplayStarted,
    ReplayCompleted,
}

/// A hash-chained evidence event (spec §18).
/// ε = (tag, payload, ctx, h_prev)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChainEvent {
    pub tag: EventTag,
    pub payload: Vec<u8>,
    pub temporal_key: TemporalKey,
    pub prev_digest: Hash256,
    pub digest: Hash256,
}
