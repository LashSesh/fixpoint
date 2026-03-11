//! System-wide error taxonomy (spec §21, F-01 through F-11).

use thiserror::Error;

/// Failure class taxonomy (spec §21).
#[derive(Debug, Error, Clone)]
pub enum FailureClass {
    #[error("F-01 CandidateFailure: {0}")]
    CandidateFailure(String),
    #[error("F-02 QuorumFailure: {0}")]
    QuorumFailure(String),
    #[error("F-03 ExecutionFailure: {0}")]
    ExecutionFailure(String),
    #[error("F-04 HedgeFailure: {0}")]
    HedgeFailure(String),
    #[error("F-05 ReplayFailure: {0}")]
    ReplayFailure(String),
    #[error("F-06 IntegrityFailure: {0}")]
    IntegrityFailure(String),
    #[error("F-07 SchemaFailure: {0}")]
    SchemaFailure(String),
    #[error("F-08 PromotionFailure: {0}")]
    PromotionFailure(String),
    #[error("F-09 TemporalFailure: {0}")]
    TemporalFailure(String),
    #[error("F-10 ExternalDepFailure: {0}")]
    ExternalDepFailure(String),
    #[error("F-11 ResourceFailure: {0}")]
    ResourceFailure(String),
}

/// General FSR error wrapper.
#[derive(Debug, Error)]
pub enum FsrError {
    #[error("failure: {0}")]
    Failure(#[from] FailureClass),
    #[error("invariant violated: INV-{invariant_id:02} {message}")]
    InvariantViolation { invariant_id: u8, message: String },
    #[error("temporal key expired")]
    TemporalKeyExpired,
    #[error("nullcenter rejected: {0}")]
    NullcenterRejected(String),
    #[error("gate closed: {0}")]
    GateClosed(String),
}
