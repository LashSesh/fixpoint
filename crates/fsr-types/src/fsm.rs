//! All 7 finite state machine state enumerations (binding, from spec Appendix B).

use serde::{Deserialize, Serialize};

// ── B.1 Regime FSM ────────────────────────────────────────────────────────────

/// Regime state machine (Appendix B.1).
/// Alpha = favourable, Beta = degraded, Gamma = stressed/emergency.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RegimeState {
    Alpha,
    Beta,
    Gamma,
}

// ── B.2 CSP FSM ───────────────────────────────────────────────────────────────

/// Commitment Schedule Protocol state machine (Appendix B.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CspState {
    Idle,
    Discovered,
    IntentOpen,
    IntentQuorum,
    ExecLockstep,
    Confirm,
    Settled,
    Abort,
    Reset,
}

// ── B.3 PoR FSM ───────────────────────────────────────────────────────────────

/// Proof-of-Route state machine (Appendix B.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PorState {
    Search,
    Lock,
    Verify,
    Commit,
}

// ── B.4 Hedge FSM ─────────────────────────────────────────────────────────────

/// Hedge state machine (Appendix B.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HedgeState {
    Safe,
    Armed,
    Triggered,
    Disarmed,
}

// ── B.5 Promotion FSM ─────────────────────────────────────────────────────────

/// Promotion state machine (Appendix B.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PromotionState {
    Candidate,
    PaperValidated,
    PromotionPending,
    LiveEligible,
    LiveActive,
    Demoted,
    Revoked,
}

// ── B.6 Integrity Posture FSM ─────────────────────────────────────────────────

/// Integrity posture (Appendix B.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IntegrityPosture {
    Healthy,
    Degraded,
    SafeHold,
    RollbackPending,
    Killed,
}

// ── B.7 Resource Posture FSM ──────────────────────────────────────────────────

/// Resource posture (Appendix B.7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourcePosture {
    Nominal,
    Constrained,
    Scarce,
    Emergency,
}
