//! Lattice Crystal artifact.

use crate::constraint::ConstraintCandidate;
use crate::lattice::PartialOrder;
use fsr_types::{Hash256, Q32};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A Lattice Crystal: a stable, mutually consistent set of constraints.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LatticeCrystal {
    pub id: u64,
    pub constraints: Vec<ConstraintCandidate>,
    pub lattice_structure: PartialOrder,
    /// Thermodynamic free energy (lower = more stable).
    pub free_energy: Q32,
    /// How many ticks it took to crystallize.
    pub formation_ticks: u64,
    pub digest: Hash256,
    pub created_at: u64,
}

impl LatticeCrystal {
    pub fn new(
        id: u64,
        constraints: Vec<ConstraintCandidate>,
        lattice_structure: PartialOrder,
        free_energy: Q32,
        tick: u64,
    ) -> Self {
        let digest = compute_crystal_digest(id, &constraints, free_energy);
        LatticeCrystal {
            id,
            constraints,
            lattice_structure,
            free_energy,
            formation_ticks: tick,
            digest,
            created_at: tick,
        }
    }

    pub fn constraint_count(&self) -> usize {
        self.constraints.len()
    }
}

fn compute_crystal_digest(id: u64, constraints: &[ConstraintCandidate], free_energy: Q32) -> Hash256 {
    let mut hasher = Sha256::new();
    hasher.update(id.to_le_bytes());
    hasher.update(free_energy.to_le_bytes());
    for c in constraints {
        hasher.update(c.id.to_le_bytes());
        hasher.update(c.satisfaction_rate.to_le_bytes());
    }
    Hash256(hasher.finalize().into())
}
