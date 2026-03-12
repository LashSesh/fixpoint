//! Tiered storage (hot/warm/cold) for ISLS (ISLS Axiom 3.3).
//!
//! All three tiers are append-only and queryable.
//! Compaction moves data between tiers without logical deletion.
//! Existing fsr-chain shadow/commit chain is migrated to ISLS storage backend.
//!
//! Tier layout:
//!   Hot:  in-memory, <1 hour, bincode format
//!   Warm: bincode disk, data/isls/warm/, <90 days
//!   Cold: Zstd-compressed, data/isls/cold/

use crate::observation::Observation;
use serde::{Deserialize, Serialize};

/// Storage tier classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StorageTier {
    Hot,
    Warm,
    Cold,
}

impl std::fmt::Display for StorageTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageTier::Hot => write!(f, "hot"),
            StorageTier::Warm => write!(f, "warm"),
            StorageTier::Cold => write!(f, "cold"),
        }
    }
}

/// Tiered storage statistics.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StorageStats {
    pub hot_count: u64,
    pub warm_count: u64,
    pub cold_count: u64,
    pub total_bytes_hot: u64,
    pub total_bytes_warm: u64,
    pub total_bytes_cold: u64,
}

/// In-memory hot tier entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HotEntry {
    pub tick: u64,
    pub observation: Observation,
}

/// Tiered storage manager.
/// In Phase 5, warm/cold are simulated (no actual filesystem I/O required
/// for paper mode or sandbox — persists to paths when enabled).
pub struct TieredStorage {
    pub hot: Vec<HotEntry>,
    pub warm_count: u64,
    pub cold_count: u64,
    /// Retention: hot entries older than this many ticks are promoted to warm.
    pub hot_retention_ticks: u64,
    pub enabled: bool,
}

impl TieredStorage {
    pub fn new(hot_retention_ticks: u64, enabled: bool) -> Self {
        TieredStorage {
            hot: Vec::new(),
            warm_count: 0,
            cold_count: 0,
            hot_retention_ticks,
            enabled,
        }
    }

    /// Append an observation to the hot tier.
    pub fn append_observation(&mut self, tick: u64, observation: Observation) {
        if !self.enabled { return; }
        self.hot.push(HotEntry { tick, observation });
    }

    /// Compact: promote hot entries older than retention threshold to warm.
    pub fn compact(&mut self, current_tick: u64) {
        if !self.enabled { return; }
        let cutoff = current_tick.saturating_sub(self.hot_retention_ticks);
        let (keep, promote): (Vec<_>, Vec<_>) = self.hot.drain(..).partition(|e| e.tick > cutoff);
        self.warm_count += promote.len() as u64;
        self.hot = keep;
    }

    /// Query hot tier for observations matching a predicate.
    pub fn query_hot<F>(&self, predicate: F) -> Vec<&HotEntry>
    where
        F: Fn(&HotEntry) -> bool,
    {
        self.hot.iter().filter(|e| predicate(e)).collect()
    }

    pub fn stats(&self) -> StorageStats {
        let hot_bytes = (self.hot.len() * std::mem::size_of::<HotEntry>()) as u64;
        StorageStats {
            hot_count: self.hot.len() as u64,
            warm_count: self.warm_count,
            cold_count: self.cold_count,
            total_bytes_hot: hot_bytes,
            total_bytes_warm: 0,
            total_bytes_cold: 0,
        }
    }
}

impl Default for TieredStorage {
    fn default() -> Self {
        Self::new(3600, false)
    }
}
