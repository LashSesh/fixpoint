//! Observation canonicalization (ISLS).
//!
//! Observations are the fundamental data-ingestion events. Each observation
//! is hash-stamped to ensure append-only, replay-verifiable storage.

use crate::types::EntityId;
use fsr_types::Hash256;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Source identifier (which venue, chain, or feed provided this data).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceId(pub String);

/// Provenance record: who/what produced this observation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Provenance {
    pub source: SourceId,
    pub schema_version: u32,
    pub sequence: u64,
}

/// Measurement context (precision, staleness, etc.)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MeasureContext {
    pub staleness_ticks: u32,
    pub precision_bits: u8,
}

/// Observation payload variants.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ObsPayload {
    /// Mid-price observation: (entity_id, price_q32).
    Price(EntityId, i64),
    /// Order-book snapshot: (entity_id, bids_count, asks_count).
    OrderBook(EntityId, u32, u32),
    /// Correlation observation: (from, to, rho_q32).
    Correlation(EntityId, EntityId, i64),
    /// Raw bytes (for future extension).
    Raw(Vec<u8>),
}

/// A canonical, hash-stamped observation (ISLS Definition 4.1).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Observation {
    /// Microseconds since epoch.
    pub time: u64,
    /// Which venue/chain/feed provided this data.
    pub source: SourceId,
    /// Provenance record.
    pub provenance: Provenance,
    /// The actual observation data.
    pub payload: ObsPayload,
    /// Measurement context.
    pub context: MeasureContext,
    /// Hash digest: H(time || source || payload).
    pub digest: Hash256,
}

impl Observation {
    pub fn new(time: u64, source: SourceId, payload: ObsPayload) -> Self {
        let provenance = Provenance {
            source: source.clone(),
            schema_version: 1,
            sequence: 0,
        };
        let context = MeasureContext::default();
        let digest = compute_obs_digest(time, &source, &payload);
        Observation { time, source, provenance, payload, context, digest }
    }

    pub fn with_provenance(mut self, provenance: Provenance) -> Self {
        self.provenance = provenance;
        self
    }
}

fn compute_obs_digest(time: u64, source: &SourceId, payload: &ObsPayload) -> Hash256 {
    let mut hasher = Sha256::new();
    hasher.update(time.to_le_bytes());
    hasher.update(source.0.as_bytes());
    let payload_bytes = bincode::serialize(payload).unwrap_or_default();
    hasher.update((payload_bytes.len() as u32).to_le_bytes());
    hasher.update(&payload_bytes);
    Hash256(hasher.finalize().into())
}
