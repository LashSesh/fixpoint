//! fsr-dshae: DSHAE Dual-Simplex Holographic Arbitrage Engine (Phase 4).
//!
//! Modules:
//!   basket    — CurrencyBasket, CrossRateTensor, DeviationTensor
//!   simplex   — DualSimplex Alpha/Beta cycle computation
//!   him       — Holographic Interference Manifold
//!   crystal   — Crystal formation via DK/WT/Pi/Press cascade
//!   axle      — Axle-invariant enforcement
//!   config    — DshaeConfig structs
//!   sandbox_gen — Validation sandbox generator and runner

pub mod axle;
pub mod basket;
pub mod config;
pub mod crystal;
pub mod him;
pub mod sandbox_gen;
pub mod simplex;

// Re-export key public types.
pub use basket::{CrossRateTensor, CurrencyBasket, DeviationTensor, RATE_SCALE};
pub use config::{
    BasketConfig, CascadeConfig, CrystalConfig, DshaeConfig, DshaeMode, DualSimplexConfig,
    HolographicConfig,
};
pub use crystal::{CrystalFormation, DshaeCrystal};
pub use him::{Him, HimPoint, expected_him_points};
pub use sandbox_gen::{SandboxResult, SandboxRunner, Scenario};
pub use simplex::{DualSimplex, SimplexObservation};

use fsr_types::{Q32, market::OrderBook};
use serde::{Deserialize, Serialize};

/// The main DSHAE engine: orchestrates all components per tick.
pub struct DshaeEngine {
    pub config: DshaeConfig,
    pub basket: CurrencyBasket,
    simplex: DualSimplex,
    formation: CrystalFormation,
    /// Total crystals found since engine start.
    pub crystals_found: u64,
    /// Tick of the most recent crystal.
    pub last_crystal_tick: Option<u64>,
}

impl DshaeEngine {
    /// Create a new DSHAE engine with the given configuration.
    pub fn new(config: DshaeConfig) -> Self {
        let basket = CurrencyBasket::default_4();
        let simplex = DualSimplex::new(config.dual_simplex.clone());
        let formation = CrystalFormation::new(config.cascade.clone(), config.crystal.clone());
        DshaeEngine {
            config,
            basket,
            simplex,
            formation,
            crystals_found: 0,
            last_crystal_tick: None,
        }
    }

    /// Process a set of OrderBooks and return any new crystals.
    ///
    /// Builds the CrossRateTensor from the books' mid prices,
    /// then runs the full DSHAE pipeline.
    pub fn push_books(&mut self, books: &[OrderBook], tick: u64) -> Vec<DshaeCrystal> {
        if books.is_empty() || self.basket.n < 3 {
            return vec![];
        }

        // Build rate pairs from books (up to n*(n-1)/2 pairs).
        let mids = extract_mids(books, self.basket.n);
        if mids.is_empty() {
            return vec![];
        }

        let tensor = CrossRateTensor::from_mid_prices(&mids, self.basket.n, tick);
        let deviations = DeviationTensor::from_rates(&tensor);
        let obs = self.simplex.observe(&tensor, &deviations);
        let him = Him::construct(self.basket.n, &tensor, &deviations, &obs, tick, 20);

        // Check minimum points constraint.
        if him.len() < self.config.holographic.min_points {
            return vec![];
        }

        let new_crystals = self.formation.process_him(&him);
        self.crystals_found += new_crystals.len() as u64;
        if !new_crystals.is_empty() {
            self.last_crystal_tick = Some(tick);
        }
        new_crystals
    }

    /// Process a raw rate slice (mid prices normalized to RATE_SCALE).
    /// Used in sandbox and bridge.
    pub fn push_mids(&mut self, mids: &[(usize, usize, i64)], tick: u64) -> Vec<DshaeCrystal> {
        if mids.is_empty() {
            return vec![];
        }

        let tensor = CrossRateTensor::from_mid_prices(mids, self.basket.n, tick);
        let deviations = DeviationTensor::from_rates(&tensor);
        let obs = self.simplex.observe(&tensor, &deviations);
        let him = Him::construct(self.basket.n, &tensor, &deviations, &obs, tick, 20);

        if him.len() < self.config.holographic.min_points {
            return vec![];
        }

        let new_crystals = self.formation.process_him(&him);
        self.crystals_found += new_crystals.len() as u64;
        if !new_crystals.is_empty() {
            self.last_crystal_tick = Some(tick);
        }
        new_crystals
    }
}

/// Extract mid prices from OrderBooks for the first `n` currency slots.
///
/// Maps book indices to currency pairs: book[k] → pair (i, j) by position.
fn extract_mids(books: &[OrderBook], n: usize) -> Vec<(usize, usize, i64)> {
    let mut result = Vec::new();
    let mut pos = 0usize;
    'outer: for i in 0..n {
        for j in (i + 1)..n {
            if let Some(book) = books.get(pos) {
                if let Some(mid) = book.mid_bp() {
                    if mid > 0 {
                        // Normalize: divide by max_mid to get relative rates.
                        // We use max_mid / RATE_SCALE to bring into RATE_SCALE range.
                        let normalized = normalize_mid(mid);
                        result.push((i, j, normalized));
                    }
                }
            }
            pos += 1;
            if pos >= books.len() {
                break 'outer;
            }
        }
    }
    result
}

/// Normalize an arbitrary mid_bp to RATE_SCALE range for deviation computation.
///
/// Uses log-ratio normalization: all rates relative to first book's mid.
/// The absolute scale doesn't matter for deviation D_ijk computation.
fn normalize_mid(mid: i64) -> i64 {
    // Bring mid into [RATE_SCALE/2, RATE_SCALE*2] by scaling.
    // Use a reference of RATE_SCALE * 1000 = 10_000_000 (typical crypto mid_bp).
    let reference = RATE_SCALE * 1000; // 10_000_000
    // normalized = mid * RATE_SCALE / reference
    let normalized = mid * RATE_SCALE / reference.max(1);
    normalized.max(1) // ensure positive
}

/// Summary of DSHAE state for dashboard display.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DshaeSummary {
    pub enabled: bool,
    pub mode: String,
    pub crystals_found: u64,
    pub last_crystal_tick: Option<u64>,
    pub active_crystals: usize,
    pub basket_size: usize,
}

// Re-export SandboxExpected for use in fsr-gui.
pub use sandbox_gen::{SandboxExpected, SandboxFrame, SandboxPairSnap};
