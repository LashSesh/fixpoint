//! Constraint candidates and types.

use crate::templates::ConstraintTemplate;
use fsr_isls::types::EntityId;
use fsr_types::Q32;
use serde::{Deserialize, Serialize};

/// Unique ID for a constraint candidate.
pub type ConstraintId = u64;

/// A discovered constraint candidate binding a template to specific entities.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConstraintCandidate {
    pub id: ConstraintId,
    pub template: ConstraintTemplate,
    /// Which vertices this constraint binds.
    pub entities: Vec<EntityId>,
    /// Fraction of observation window satisfying the constraint.
    pub satisfaction_rate: Q32,
    /// Number of ticks in the observation window.
    pub window_size: u64,
    /// Tick when first seen.
    pub first_seen: u64,
    /// Tick of last confirmation.
    pub last_confirmed: u64,
    /// Stability score (how persistent is this constraint?).
    pub stability: Q32,
}

impl ConstraintCandidate {
    pub fn new(
        id: ConstraintId,
        template: ConstraintTemplate,
        entities: Vec<EntityId>,
        satisfaction_rate: Q32,
        window_size: u64,
        tick: u64,
    ) -> Self {
        ConstraintCandidate {
            id,
            template,
            entities,
            satisfaction_rate,
            window_size,
            first_seen: tick,
            last_confirmed: tick,
            stability: satisfaction_rate,
        }
    }

    /// Update the constraint with a new satisfaction rate.
    pub fn update(&mut self, new_rate: Q32, tick: u64) {
        // Exponential smoothing for stability.
        self.stability = ((self.stability as i128 * 7 + new_rate as i128 * 3) / 10) as Q32;
        self.satisfaction_rate = new_rate;
        self.last_confirmed = tick;
    }

    pub fn age(&self, current_tick: u64) -> u64 {
        current_tick.saturating_sub(self.first_seen)
    }
}
