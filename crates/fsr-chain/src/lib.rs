//! fsr-chain: Shadow-chain and commitment-chain (evidence monoid E*) (spec §18).
//!
//! Both chains are hash-linked evidence monoids.
//! Shadow-chain: operational events (every macro-cycle state change).
//! Commitment-chain: irreversible fills/receipts/promotions (crystallization).
//!
//! Hash chaining (spec §18.2):
//!   h_0 = H(TMCP_GENESIS)
//!   h_i = H(tag_i || payload_i || ctx_i || h_{i-1})
//!
//! Evidence Law: two replays with identical inputs must produce identical digests.

pub mod persist;
pub use persist::{ChainFileWriter, ChainReader, SystemSnapshot, unix_ms};

use fsr_types::{ChainEvent, EventTag, Hash256, TemporalKey};
use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};

const TMCP_GENESIS: &[u8] = b"TMCP_GENESIS_FIXPOINT_SWARM_R_v3.0.0";

/// A single hash-linked chain (usable for both shadow and commitment chains).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HashChain {
    pub name: String,
    pub events: Vec<ChainEvent>,
    pub head: Hash256,
    pub event_count: u64,
}

impl HashChain {
    pub fn new(name: impl Into<String>) -> Self {
        let genesis = compute_genesis();
        HashChain {
            name: name.into(),
            events: Vec::new(),
            head: genesis,
            event_count: 0,
        }
    }

    /// Append an event, computing the new digest (spec §18.2).
    /// INV-08: chain digests must remain hash-consistent.
    pub fn append(&mut self, tag: EventTag, payload: Vec<u8>, temporal_key: TemporalKey) -> &ChainEvent {
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
    /// Returns Ok(()) if consistent, Err(index) if first broken link found.
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
}

/// Compute the genesis hash h_0 = H(TMCP_GENESIS).
fn compute_genesis() -> Hash256 {
    let mut hasher = Sha256::new();
    hasher.update(TMCP_GENESIS);
    Hash256(hasher.finalize().into())
}

/// Compute h_i = H(tag || payload || ctx || h_{i-1}) (spec §18.2).
fn compute_digest(
    tag: EventTag,
    payload: &[u8],
    temporal_key: &TemporalKey,
    prev: Hash256,
) -> Hash256 {
    let mut hasher = Sha256::new();
    // Canonical encoding (spec §18.1)
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

/// Dual-chain container: shadow-chain (operational) + commitment-chain (crystallized).
pub struct DualChain {
    pub shadow: HashChain,
    pub commitment: HashChain,
}

impl DualChain {
    pub fn new() -> Self {
        DualChain {
            shadow: HashChain::new("shadow"),
            commitment: HashChain::new("commitment"),
        }
    }

    /// Append to shadow-chain (operational events, every state change).
    pub fn shadow_append(&mut self, tag: EventTag, payload: Vec<u8>, tk: TemporalKey) -> Hash256 {
        self.shadow.append(tag, payload, tk).digest
    }

    /// Append to commitment-chain (irreversible: fills, receipts, promotions).
    /// INV-01: requires NC cert (checked by caller — commitment MUST be NC-factored).
    pub fn commit(&mut self, tag: EventTag, payload: Vec<u8>, tk: TemporalKey) -> Hash256 {
        self.commitment.append(tag, payload, tk).digest
    }

    /// Verify both chains.
    pub fn verify_both(&self) -> Result<(), String> {
        self.shadow.verify().map_err(|i| format!("shadow chain broken at index {}", i))?;
        self.commitment.verify().map_err(|i| format!("commitment chain broken at index {}", i))?;
        Ok(())
    }
}

impl Default for DualChain {
    fn default() -> Self {
        Self::new()
    }
}

/// ChainSink trait: interface for appending to chains (spec §28).
pub trait ChainSink {
    fn shadow_append(&mut self, tag: EventTag, payload: Vec<u8>, tk: TemporalKey) -> Hash256;
    fn commit(&mut self, tag: EventTag, payload: Vec<u8>, tk: TemporalKey) -> Hash256;
}

impl ChainSink for DualChain {
    fn shadow_append(&mut self, tag: EventTag, payload: Vec<u8>, tk: TemporalKey) -> Hash256 {
        self.shadow_append(tag, payload, tk)
    }
    fn commit(&mut self, tag: EventTag, payload: Vec<u8>, tk: TemporalKey) -> Hash256 {
        self.commit(tag, payload, tk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_types::{EventTag, Freshness, TemporalKey};

    fn make_tk(tick: u64) -> TemporalKey {
        TemporalKey {
            commit_tick: 0,
            intrinsic_tick: tick,
            phase_bin: 0,
            wind_count: 0,
            freshness: Freshness::Fresh,
        }
    }

    #[test]
    fn test_chain_starts_with_genesis() {
        let chain = HashChain::new("test");
        assert_ne!(chain.head, Hash256::ZERO, "genesis hash must not be zero");
        assert!(chain.events.is_empty());
    }

    #[test]
    fn test_chain_append_and_verify() {
        let mut chain = HashChain::new("test");
        chain.append(EventTag::MacroCycleStart, vec![1, 2, 3], make_tk(0));
        chain.append(EventTag::RegimeChanged, vec![4, 5, 6], make_tk(1));
        chain.append(EventTag::MacroCycleEnd, vec![], make_tk(2));
        assert_eq!(chain.len(), 3);
        assert!(chain.verify().is_ok(), "chain should be consistent");
    }

    #[test]
    fn test_chain_detects_tampering() {
        let mut chain = HashChain::new("test");
        chain.append(EventTag::MacroCycleStart, vec![1], make_tk(0));
        chain.append(EventTag::MacroCycleEnd, vec![2], make_tk(1));
        // Tamper with first event's payload
        chain.events[0].payload = vec![99];
        assert!(chain.verify().is_err(), "tampered chain should fail verification");
    }

    #[test]
    fn test_dual_chain_shadow_and_commit() {
        let mut dual = DualChain::new();
        let tk = make_tk(0);
        dual.shadow_append(EventTag::MacroCycleStart, vec![], tk);
        dual.commit(EventTag::ExecutionSettled, vec![1, 2], tk);
        assert_eq!(dual.shadow.len(), 1);
        assert_eq!(dual.commitment.len(), 1);
        assert!(dual.verify_both().is_ok());
    }

    #[test]
    fn test_chain_determinism() {
        // Two chains with same events must produce identical head digests
        let tk = make_tk(5);
        let mut c1 = HashChain::new("chain1");
        let mut c2 = HashChain::new("chain2");
        c1.append(EventTag::RegimeChanged, vec![1, 2, 3], tk);
        c2.append(EventTag::RegimeChanged, vec![1, 2, 3], tk);
        assert_eq!(c1.head, c2.head, "deterministic hashing: same inputs → same digest");
    }

    #[test]
    fn test_chain_append_changes_head() {
        let mut chain = HashChain::new("test");
        let h0 = chain.head;
        chain.append(EventTag::MacroCycleStart, vec![], make_tk(0));
        assert_ne!(chain.head, h0, "head must change after append");
    }
}
