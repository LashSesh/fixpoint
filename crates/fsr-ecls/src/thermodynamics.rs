//! Thermodynamic stability: free energy, reaction pathways.
//!
//! Lower free energy = more stable constraint lattice.
//! High free energy = high disorder = constraint breaking likely.

use crate::constraint::ConstraintCandidate;
use fsr_fixed::ONE;
use fsr_types::Q32;

/// Compute free energy of a constraint group.
/// Free energy = (1 - mean_stability) * ONE.
/// Low free energy → stable crystal.
pub fn compute_free_energy(candidates: &[ConstraintCandidate]) -> Q32 {
    if candidates.is_empty() {
        return ONE;
    }
    let total_stability: i128 = candidates.iter().map(|c| c.stability as i128).sum();
    let mean_stability = total_stability / candidates.len() as i128;
    ONE - mean_stability as Q32
}

/// Compute energy barrier: how much energy needed to break a constraint.
pub fn energy_barrier(candidate: &ConstraintCandidate) -> Q32 {
    candidate.stability
}

/// Check if a constraint is breaking (satisfaction rate dropped significantly).
pub fn is_breaking(candidate: &ConstraintCandidate, prev_rate: Q32, threshold: Q32) -> bool {
    let drop = prev_rate.saturating_sub(candidate.satisfaction_rate);
    drop >= threshold
}
