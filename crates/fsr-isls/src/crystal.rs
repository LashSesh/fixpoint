//! SemanticCrystal artifact (ISLS Definition 4.6).
//!
//! A SemanticCrystal is a condensed, stable subgraph region that has passed
//! resonant consensus. It carries topological signatures, Betti numbers,
//! and commit proofs for replay verification.

use crate::evidence::EvidenceChain;
use crate::types::{EntityId, SubGraph, TopoSignature};
use crate::consensus::CommitProof;
use fsr_types::{Hash256, Q32};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Unique identifier for a SemanticCrystal.
pub type CrystalId = u64;

/// SemanticCrystal: a stable, hash-verified knowledge condensate.
/// (ISLS Definition 4.6)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SemanticCrystal {
    /// Crystal identifier.
    pub id: CrystalId,
    /// The condensed subgraph region.
    pub condensed_region: SubGraph,
    /// Constraint programs bound to this crystal.
    pub constraint_program: Vec<u64>,
    /// Stability score τ(C) ∈ [0, ONE].
    pub stability: Q32,
    /// Topological signature of the crystal.
    pub topo_signature: TopoSignature,
    /// Betti numbers [β₀, β₁, β₂].
    pub betti: Vec<u32>,
    /// Hash digest of this crystal's canonical encoding.
    pub digest: Hash256,
    /// Evidence chain for this crystal.
    pub evidence: EvidenceChain,
    /// Commit proof π(C).
    pub commit_proof: CommitProof,
    /// Tick at which this crystal was created.
    pub created_at: u64,
}

impl SemanticCrystal {
    pub fn new(
        id: CrystalId,
        condensed_region: SubGraph,
        stability: Q32,
        tick: u64,
    ) -> Self {
        let topo_signature = TopoSignature {
            betti: [1, 0, 0],
            stability,
            digest: Hash256::ZERO,
        };
        let evidence = EvidenceChain::new(format!("crystal-{}", id));
        let commit_proof = CommitProof::default();
        let betti = vec![1, 0, 0];
        let mut crystal = SemanticCrystal {
            id,
            condensed_region,
            constraint_program: vec![],
            stability,
            topo_signature,
            betti,
            digest: Hash256::ZERO,
            evidence,
            commit_proof,
            created_at: tick,
        };
        crystal.digest = crystal.compute_digest();
        crystal
    }

    fn compute_digest(&self) -> Hash256 {
        let mut hasher = Sha256::new();
        hasher.update(self.id.to_le_bytes());
        hasher.update(self.stability.to_le_bytes());
        hasher.update(self.created_at.to_le_bytes());
        for v in &self.condensed_region.vertices {
            hasher.update(v.to_le_bytes());
        }
        Hash256(hasher.finalize().into())
    }

    pub fn entities(&self) -> &[EntityId] {
        &self.condensed_region.vertices
    }
}

/// Registry of all committed SemanticCrystals.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CrystalRegistry {
    pub crystals: Vec<SemanticCrystal>,
    pub next_id: CrystalId,
}

impl CrystalRegistry {
    pub fn new() -> Self {
        CrystalRegistry::default()
    }

    pub fn register(&mut self, region: SubGraph, stability: Q32, tick: u64) -> &SemanticCrystal {
        let id = self.next_id;
        self.next_id += 1;
        let crystal = SemanticCrystal::new(id, region, stability, tick);
        self.crystals.push(crystal);
        self.crystals.last().unwrap()
    }

    pub fn count(&self) -> u64 {
        self.crystals.len() as u64
    }
}
