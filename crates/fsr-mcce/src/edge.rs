//! Edge types, annotations, and lifecycle for MCCE.

use fsr_isls::types::EntityId;
use fsr_types::Q32;
use serde::{Deserialize, Serialize};

/// A correlation edge between two vertices (Hypha layer).
/// Weight decays over time per MCCE Axiom A.1.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorrelationEdge {
    pub from: EntityId,
    pub to: EntityId,
    /// Pearson correlation ρ_uv ∈ [-ONE, ONE] (Q32).
    pub rho: Q32,
    /// Exponentially decayed weight ∈ [0, ONE].
    pub weight: Q32,
    pub created_tick: u64,
    pub last_updated_tick: u64,
    /// Number of ticks this edge has been observed.
    pub observation_ticks: u64,
}

impl CorrelationEdge {
    pub fn new(from: EntityId, to: EntityId, rho: Q32, tick: u64) -> Self {
        use fsr_fixed::ONE;
        CorrelationEdge {
            from,
            to,
            rho,
            weight: ONE,
            created_tick: tick,
            last_updated_tick: tick,
            observation_ticks: 1,
        }
    }

    /// Decay weight by factor (1 - decay_rate) and update correlation.
    /// decay_rate is Q32 fraction (e.g. 0.005 * ONE for 0.5% per tick).
    pub fn decay_and_update(&mut self, new_rho: Q32, tick: u64, decay_rate: Q32) {
        use fsr_fixed::ONE;
        // Exponential decay: weight *= (1 - decay_rate)
        let decay_factor = ONE - decay_rate;
        self.weight = ((self.weight as i128 * decay_factor as i128) / ONE as i128) as Q32;
        // Update correlation with exponential smoothing.
        self.rho = new_rho;
        self.last_updated_tick = tick;
        self.observation_ticks += 1;
    }

    /// Check if edge is still significant (weight above threshold).
    pub fn is_significant(&self, min_weight: Q32) -> bool {
        self.weight >= min_weight
    }
}

/// A triangulation edge (Mycelium layer): connects three vertices via TTCP.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TriangulationEdge {
    pub a: EntityId,
    pub b: EntityId,
    pub c: EntityId,
    pub score: Q32,
    pub created_tick: u64,
    pub persistence: u64,
}
