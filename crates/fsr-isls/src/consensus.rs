//! Resonant consensus for ISLS (ISLS Section 9).
//!
//! Determines whether a SemanticCrystal should be committed to the ledger
//! based on stability, coherence, and evidence completeness.

use crate::crystal::SemanticCrystal;
use crate::types::PersistentGraph;
use fsr_fixed::ONE;
use fsr_types::Q32;
use serde::{Deserialize, Serialize};

/// Consensus configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsensusConfig {
    /// Minimum composite score to commit (Q32, e.g. 0.75 * ONE = 3221225472).
    pub commit_threshold: Q32,
}

impl Default for ConsensusConfig {
    fn default() -> Self {
        ConsensusConfig {
            commit_threshold: ONE * 3 / 4,
        }
    }
}

/// Commit proof attached to a committed SemanticCrystal.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CommitProof {
    pub score: Q32,
    pub stability: Q32,
    pub coherence: Q32,
    pub evidence_completeness: Q32,
    pub commit_tick: u64,
}

impl CommitProof {
    pub fn new(score: Q32, stability: Q32, coherence: Q32, evidence: Q32, tick: u64) -> Self {
        CommitProof { score, stability, coherence, evidence_completeness: evidence, commit_tick: tick }
    }
}

/// Decision from resonant consensus.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CommitDecision {
    Commit(CommitProof),
    Defer,
}

/// Resonant consensus function (ISLS Section 9).
///
/// score = stability * coherence * evidence_completeness
/// If score >= commit_threshold → Commit(CommitProof)
/// else → Defer
pub fn resonant_consensus(
    crystal: &SemanticCrystal,
    graph: &PersistentGraph,
    config: &ConsensusConfig,
) -> CommitDecision {
    let stability = crystal.stability;
    let coherence = compute_local_coherence(crystal, graph);
    let evidence = crystal.evidence.completeness_score();

    // score = stability * coherence * evidence (Q32 multiplication)
    let score = q32_mul3(stability, coherence, evidence);

    if score >= config.commit_threshold {
        let proof = CommitProof::new(score, stability, coherence, evidence, 0);
        CommitDecision::Commit(proof)
    } else {
        CommitDecision::Defer
    }
}

/// Compute local coherence: fraction of crystal vertices that have edges to each other.
fn compute_local_coherence(crystal: &SemanticCrystal, graph: &PersistentGraph) -> Q32 {
    let verts = crystal.entities();
    if verts.len() < 2 {
        return ONE;
    }
    let possible_edges = verts.len() * (verts.len() - 1);
    let actual_edges = graph.edges.iter().filter(|e| {
        verts.contains(&e.from) && verts.contains(&e.to)
    }).count();
    if possible_edges == 0 {
        return ONE;
    }
    (actual_edges as i64 * ONE) / possible_edges as i64
}

/// Q32 three-way multiplication (normalised to [0, ONE]).
fn q32_mul3(a: Q32, b: Q32, c: Q32) -> Q32 {
    // a, b, c ∈ [0, ONE]. To avoid overflow, divide progressively.
    let ab = (a as i128 * b as i128) / ONE as i128;
    ((ab * c as i128) / ONE as i128) as Q32
}
