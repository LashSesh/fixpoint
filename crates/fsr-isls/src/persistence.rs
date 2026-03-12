//! ISLS persistence coordinator.
//!
//! IslsPersistence is the top-level coordinator for all Phase 5 storage.
//! It manages the PersistentGraph, TieredStorage, CrystalRegistry, and
//! the two EvidenceChains (shadow + commitment, migrated from fsr-chain).

use crate::config::IslsConfig;
use crate::consensus::{ConsensusConfig, CommitDecision, resonant_consensus};
use crate::crystal::{CrystalRegistry, SemanticCrystal};
use crate::evidence::EvidenceChain;
use crate::observation::Observation;
use crate::storage::TieredStorage;
use crate::types::{PersistentGraph, SubGraph};
use fsr_types::{EventTag, Hash256, Q32, TemporalKey};
use serde::{Deserialize, Serialize};

/// ISLS persistence top-level coordinator.
pub struct IslsPersistence {
    pub graph: PersistentGraph,
    pub storage: TieredStorage,
    pub crystals: CrystalRegistry,
    pub shadow_chain: EvidenceChain,
    pub commitment_chain: EvidenceChain,
    pub consensus_config: ConsensusConfig,
    pub config: IslsConfig,
    pub observation_count: u64,
    pub tick: u64,
}

impl IslsPersistence {
    pub fn new(config: IslsConfig) -> Self {
        let hot_retention = config.hot_retention_ticks;
        let enabled = config.enabled;
        IslsPersistence {
            graph: PersistentGraph::new(),
            storage: TieredStorage::new(hot_retention, enabled),
            crystals: CrystalRegistry::new(),
            shadow_chain: EvidenceChain::new("shadow"),
            commitment_chain: EvidenceChain::new("commitment"),
            consensus_config: ConsensusConfig {
                commit_threshold: config.consensus_threshold,
            },
            config,
            observation_count: 0,
            tick: 0,
        }
    }

    /// Write an observation to the hot storage tier.
    pub fn write_observation(&mut self, tick: u64, observation: Observation) {
        self.tick = tick;
        self.observation_count += 1;
        self.storage.append_observation(tick, observation);
    }

    /// Append to shadow chain (delegated from fsr-chain).
    pub fn shadow_append(&mut self, tag: EventTag, payload: Vec<u8>, tk: TemporalKey) -> Hash256 {
        self.shadow_chain.append(tag, payload, tk).digest
    }

    /// Append to commitment chain (delegated from fsr-chain).
    pub fn commit_append(&mut self, tag: EventTag, payload: Vec<u8>, tk: TemporalKey) -> Hash256 {
        self.commitment_chain.append(tag, payload, tk).digest
    }

    /// Verify both chains.
    pub fn verify_both(&self) -> Result<(), String> {
        self.shadow_chain.verify().map_err(|i| format!("shadow chain broken at {}", i))?;
        self.commitment_chain.verify().map_err(|i| format!("commitment chain broken at {}", i))?;
        Ok(())
    }

    /// Run consensus on a pending crystal candidate.
    pub fn try_commit_crystal(&mut self, region: SubGraph, stability: Q32) -> Option<&SemanticCrystal> {
        let temp_crystal = SemanticCrystal::new(
            self.crystals.next_id,
            region.clone(),
            stability,
            self.tick,
        );
        match resonant_consensus(&temp_crystal, &self.graph, &self.consensus_config) {
            CommitDecision::Commit(_proof) => {
                Some(self.crystals.register(region, stability, self.tick))
            }
            CommitDecision::Defer => None,
        }
    }

    /// Compact hot tier to warm (called from macro-cycle step 18).
    pub fn compact(&mut self) {
        self.storage.compact(self.tick);
    }

    pub fn crystal_count(&self) -> u64 {
        self.crystals.count()
    }

    pub fn vertex_count(&self) -> u64 {
        self.graph.vertices.len() as u64
    }

    pub fn edge_count(&self) -> u64 {
        self.graph.edges.len() as u64
    }

    pub fn shadow_head(&self) -> Hash256 {
        self.shadow_chain.head
    }

    pub fn observation_count_total(&self) -> u64 {
        self.observation_count
    }
}

impl Default for IslsPersistence {
    fn default() -> Self {
        Self::new(IslsConfig::default())
    }
}
