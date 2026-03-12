//! Constraint lattice construction.
//!
//! Groups mutually consistent constraint candidates into a lattice structure.

use crate::constraint::ConstraintCandidate;
use fsr_fixed::ONE;
use fsr_types::Q32;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Partial order relation between constraints (precedence).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PartialOrder {
    /// (parent_id, child_id) → strength of ordering.
    pub relations: Vec<(u64, u64, Q32)>,
}

/// Check if two constraints are mutually consistent (can coexist in a lattice).
pub fn are_consistent(a: &ConstraintCandidate, b: &ConstraintCandidate) -> bool {
    // Constraints on the same entity pair with different templates are consistent.
    // Constraints with satisfaction_rate both above 80% are considered consistent.
    let same_entities = a.entities == b.entities;
    let both_satisfied = a.satisfaction_rate >= ONE * 4 / 5 && b.satisfaction_rate >= ONE * 4 / 5;
    // Constraints are consistent if they don't share the same template type on same entities.
    !same_entities || both_satisfied
}

/// Build a partial order from consistent constraint candidates.
pub fn build_partial_order(candidates: &[ConstraintCandidate]) -> PartialOrder {
    let mut relations = Vec::new();
    for i in 0..candidates.len() {
        for j in (i + 1)..candidates.len() {
            if are_consistent(&candidates[i], &candidates[j]) {
                let strength = ((candidates[i].stability as i128 + candidates[j].stability as i128) / 2) as Q32;
                relations.push((candidates[i].id, candidates[j].id, strength));
            }
        }
    }
    PartialOrder { relations }
}

/// Group candidates into consistent subsets (proto-crystals).
pub fn group_consistent(candidates: &[ConstraintCandidate], min_size: usize) -> Vec<Vec<usize>> {
    // Simple greedy clustering: group by shared entities.
    let mut groups: HashMap<Vec<u64>, Vec<usize>> = HashMap::new();
    for (i, c) in candidates.iter().enumerate() {
        let mut key = c.entities.clone();
        key.sort();
        groups.entry(key).or_default().push(i);
    }
    groups.into_values()
        .filter(|g| g.len() >= min_size)
        .collect()
}
