//! Basket advisor: dynamic basket recommendation from MCCE graph structure.
//!
//! Uses cluster cohesion and graph topology to suggest basket compositions
//! for DSHAE triangle arbitrage.

use crate::hdag::MycelialHdag;
use fsr_isls::types::EntityId;
use fsr_types::Q32;

/// A basket recommendation from MCCE.
#[derive(Clone, Debug)]
pub struct BasketRecommendation {
    pub entities: Vec<EntityId>,
    pub expected_cohesion: Q32,
    pub tick: u64,
}

/// Basket advisor: reads HDAG cluster state and recommends baskets.
pub struct BasketAdvisor;

impl BasketAdvisor {
    /// Recommend baskets from the current HDAG state.
    pub fn recommend(hdag: &MycelialHdag) -> Vec<BasketRecommendation> {
        let tick = hdag.tick;
        hdag.mycelium.clusters.iter()
            .filter(|c| c.members.len() >= 3)
            .map(|c| BasketRecommendation {
                entities: c.members.clone(),
                expected_cohesion: c.cohesion,
                tick,
            })
            .collect()
    }
}
