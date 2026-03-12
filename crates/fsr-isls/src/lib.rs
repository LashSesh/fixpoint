//! fsr-isls: Intelligent Semantic Ledger Substrate (Phase 5).
//!
//! Unifying persistence and consensus layer that binds everything into a
//! replay-verifiable, structurally grounded knowledge substrate.
//!
//! ISLS Integration Rule: After Phase 5, all system persistence flows through ISLS.
//! The existing fsr-chain crate becomes a thin wrapper delegating to fsr-isls::evidence.
//! Existing chain semantics (shadow/commit, hash-linking, deterministic replay)
//! are preserved exactly.

pub mod config;
pub mod consensus;
pub mod crystal;
pub mod evidence;
pub mod observation;
pub mod persistence;
pub mod storage;
pub mod types;

pub use config::IslsConfig;
pub use consensus::{CommitDecision, CommitProof, ConsensusConfig, resonant_consensus};
pub use crystal::SemanticCrystal;
pub use evidence::{EvidenceChain, EvidenceEntry};
pub use observation::{Observation, ObsPayload, Provenance, SourceId};
pub use persistence::{IslsPersistence};
pub use storage::{StorageTier, TieredStorage};
pub use types::{EntityId, TripolarState, VertexRecord, TypedEdge, EdgeId, TopoSignature, PersistentGraph};
