//! EvidenceChain: replaces fsr-chain internals (ISLS).
//!
//! The EvidenceChain is the append-only hash-linked event log that provides
//! replay-verifiability. Existing chain semantics (shadow/commit, hash-linking,
//! deterministic replay) are preserved exactly.

use fsr_types::{ChainEvent, EventTag, Hash256, TemporalKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const TMCP_GENESIS: &[u8] = b"TMCP_GENESIS_FIXPOINT_SWARM_R_v3.0.0";

/// Completeness score for an EvidenceChain (fraction of expected observations present).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CompletenessScore {
    pub score: i64,
    pub total_events: u64,
    pub valid_events: u64,
}

impl CompletenessScore {
    pub fn compute(chain: &EvidenceChain) -> Self {
        let valid = chain.events.iter().filter(|e| !e.payload.is_empty()).count() as u64;
        let total = chain.events.len() as u64;
        let score = if total == 0 {
            fsr_fixed::ONE
        } else {
            (valid as i64 * fsr_fixed::ONE) / total.max(1) as i64
        };
        CompletenessScore { score, total_events: total, valid_events: valid }
    }
}

/// A single entry in the EvidenceChain (re-exports fsr_types::ChainEvent).
pub type EvidenceEntry = ChainEvent;

/// EvidenceChain: append-only hash-linked evidence log.
/// Replaces fsr-chain HashChain internals.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EvidenceChain {
    pub name: String,
    pub events: Vec<EvidenceEntry>,
    pub head: Hash256,
    pub event_count: u64,
}

impl EvidenceChain {
    pub fn new(name: impl Into<String>) -> Self {
        let genesis = compute_genesis();
        EvidenceChain {
            name: name.into(),
            events: Vec::new(),
            head: genesis,
            event_count: 0,
        }
    }

    /// Append an event, computing the new digest.
    pub fn append(&mut self, tag: EventTag, payload: Vec<u8>, temporal_key: TemporalKey) -> &EvidenceEntry {
        let prev = self.head;
        let digest = compute_digest(tag, &payload, &temporal_key, prev);
        let event = ChainEvent {
            tag,
            payload,
            temporal_key,
            prev_digest: prev,
            digest,
        };
        self.head = digest;
        self.event_count += 1;
        self.events.push(event);
        self.events.last().unwrap()
    }

    /// Verify the entire chain for hash consistency.
    pub fn verify(&self) -> Result<(), usize> {
        let genesis = compute_genesis();
        let mut expected_prev = genesis;
        for (i, event) in self.events.iter().enumerate() {
            if event.prev_digest != expected_prev {
                return Err(i);
            }
            let expected_digest = compute_digest(event.tag, &event.payload, &event.temporal_key, event.prev_digest);
            if event.digest != expected_digest {
                return Err(i);
            }
            expected_prev = event.digest;
        }
        Ok(())
    }

    pub fn head_digest(&self) -> Hash256 {
        self.head
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Compute completeness score for this chain.
    pub fn completeness_score(&self) -> i64 {
        CompletenessScore::compute(self).score
    }
}

impl Default for EvidenceChain {
    fn default() -> Self {
        Self::new("default")
    }
}

pub fn compute_genesis() -> Hash256 {
    let mut hasher = Sha256::new();
    hasher.update(TMCP_GENESIS);
    Hash256(hasher.finalize().into())
}

pub fn compute_digest(
    tag: EventTag,
    payload: &[u8],
    temporal_key: &TemporalKey,
    prev: Hash256,
) -> Hash256 {
    let mut hasher = Sha256::new();
    hasher.update([tag as u8]);
    hasher.update((payload.len() as u32).to_le_bytes());
    hasher.update(payload);
    hasher.update(temporal_key.commit_tick.to_le_bytes());
    hasher.update(temporal_key.intrinsic_tick.to_le_bytes());
    hasher.update(temporal_key.phase_bin.to_le_bytes());
    hasher.update(temporal_key.wind_count.to_le_bytes());
    hasher.update(prev.0);
    Hash256(hasher.finalize().into())
}
