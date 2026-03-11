//! DSHAE bridge: integrates DshaeEngine into the macro-cycle (Phase 4).
//!
//! Called once per macro-cycle tick from engine.rs.
//! Builds the CrossRateTensor from OrderBooks and runs the full DSHAE pipeline.

use fsr_dshae::{DshaeConfig, DshaeEngine, DshaeCrystal};
use fsr_types::market::OrderBook;
use fsr_fixed::ONE;

/// State held by the bridge across ticks.
pub struct DshaeBridge {
    pub engine: DshaeEngine,
    pub enabled: bool,
}

impl DshaeBridge {
    pub fn new(enabled: bool) -> Self {
        let mut cfg = DshaeConfig::default();
        cfg.enabled = enabled;
        cfg.holographic.min_points = 1;
        cfg.dual_simplex.anti_phase_tolerance = ONE / 10;
        DshaeBridge {
            engine: DshaeEngine::new(cfg),
            enabled,
        }
    }

    /// Process current market books, return any new DSHAE crystals.
    pub fn tick(&mut self, books: &[OrderBook], tick: u64) -> Vec<DshaeCrystal> {
        if !self.enabled {
            return vec![];
        }
        self.engine.push_books(books, tick)
    }

    /// Process raw mid prices directly (for testing or sandbox paths).
    pub fn tick_mids(&mut self, mids: &[(usize, usize, i64)], tick: u64) -> Vec<DshaeCrystal> {
        if !self.enabled {
            return vec![];
        }
        self.engine.push_mids(mids, tick)
    }

    pub fn crystals_found(&self) -> u64 {
        self.engine.crystals_found
    }
}

impl Default for DshaeBridge {
    fn default() -> Self {
        Self::new(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_disabled_returns_empty() {
        let mut bridge = DshaeBridge::new(false);
        let result = bridge.tick_mids(
            &[(0, 1, 9200), (0, 2, 7912), (1, 2, 8600)],
            0,
        );
        assert!(result.is_empty(), "disabled bridge must return no crystals");
    }

    #[test]
    fn test_bridge_enabled_no_arb_no_crystal() {
        let mut bridge = DshaeBridge::new(true);
        for tick in 0..10 {
            // No-arb baseline rates.
            let mids = vec![(0usize, 1usize, 9200i64), (0, 2, 7912), (0, 3, 7516), (1, 2, 8600), (1, 3, 8170), (2, 3, 9500)];
            let crystals = bridge.tick_mids(&mids, tick);
            assert_eq!(crystals.len(), 0, "no-arb baseline must not produce crystals at tick {}", tick);
        }
    }
}
