//! Inverse weaving algorithm: groups compatible constraints into lattice crystals.
//!
//! Inverse weaving takes a set of ConstraintCandidates and "weaves" them
//! into a LatticeCrystal by identifying mutually reinforcing constraints.

use crate::constraint::ConstraintCandidate;
use crate::crystal::LatticeCrystal;
use crate::lattice::{build_partial_order, group_consistent};
use crate::thermodynamics::compute_free_energy;
use fsr_types::Q32;

/// Configuration for inverse weaving.
pub struct InverseWeaveConfig {
    pub min_constraints: usize,
    pub free_energy_threshold: Q32,
    pub next_crystal_id: u64,
}

/// Perform inverse weaving: produce Lattice Crystals from candidates.
pub fn inverse_weave(
    candidates: &[ConstraintCandidate],
    config: &mut InverseWeaveConfig,
    tick: u64,
) -> Vec<LatticeCrystal> {
    let mut crystals = Vec::new();

    // Group consistent candidates.
    let groups = group_consistent(candidates, config.min_constraints);

    for group_indices in groups {
        let group: Vec<ConstraintCandidate> = group_indices.iter()
            .map(|&i| candidates[i].clone())
            .collect();

        let free_energy = compute_free_energy(&group);
        if free_energy >= config.free_energy_threshold {
            continue; // Too unstable (high free energy = high disorder).
        }

        let partial_order = build_partial_order(&group);
        let id = config.next_crystal_id;
        config.next_crystal_id += 1;
        crystals.push(LatticeCrystal::new(id, group, partial_order, free_energy, tick));
    }

    crystals
}
