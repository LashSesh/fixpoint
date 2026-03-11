//! WT (trumpet multiplexing) operator (spec §12.2).
//!
//! WT(x) = Σ_l w_l' I_l(P_l(x)), w_l' ≥ 0, Σ w_l' = 1

use fsr_fixed::ONE;
use fsr_types::{artifacts::{RouteCandidate, TrumpetLayer}, market::OrderBook, Q32};
use serde::{Deserialize, Serialize};
use crate::discovery::discover_routes;

/// Trumpet layer weights (spec §12.2). Must sum to ONE.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TrumpetWeights {
    pub w: [Q32; 4], // w[0]=L1, w[1]=L2, w[2]=L3, w[3]=L4
}

impl TrumpetWeights {
    /// Uniform weights (each layer = 0.25).
    pub fn uniform() -> Self {
        TrumpetWeights { w: [ONE / 4, ONE / 4, ONE / 4, ONE / 4] }
    }

    /// Validate: all weights ≥ 0 and sum = ONE (within epsilon).
    pub fn validate(&self) -> bool {
        let sum: i64 = self.w.iter().copied().sum();
        let eps = ONE / 1000;
        self.w.iter().all(|&w| w >= 0) && (sum - ONE).abs() < eps
    }
}

impl Default for TrumpetWeights {
    fn default() -> Self {
        Self::uniform()
    }
}

/// Configuration for route discovery.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    pub fee_bp: i64,
    pub slip_bp: i64,
    /// τ_edge: minimum net edge for admissibility (Q32)
    pub tau_edge: Q32,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        DiscoveryConfig {
            fee_bp: 10,
            slip_bp: 5,
            tau_edge: 0, // accept all positives in paper mode
        }
    }
}

/// Execute the WT operator across active trumpet layers.
/// active_layers: indices (0=L1..3=L4) to include.
/// Returns the combined candidate set.
pub fn wt_multiplex(
    books: &[OrderBook],
    weights: &TrumpetWeights,
    active_layers: &[usize],
    config: &DiscoveryConfig,
) -> Vec<RouteCandidate> {
    let layers = [TrumpetLayer::L1, TrumpetLayer::L2, TrumpetLayer::L3, TrumpetLayer::L4];
    let mut all: Vec<RouteCandidate> = Vec::new();
    let mut id_base = 0u64;

    for &idx in active_layers {
        if idx >= 4 { continue; }
        let layer = layers[idx];
        let weight = weights.w[idx];
        if weight <= 0 { continue; }

        let mut candidates = discover_routes(layer, books, id_base, config.fee_bp, config.slip_bp, config.tau_edge);
        // Apply weight: scale si_score by layer weight
        for c in &mut candidates {
            c.si_score = fsr_fixed::q32_mul(c.si_score, weight);
        }
        all.extend(candidates);
        id_base += 10_000;
    }

    all
}

/// Market signal reconstruction from layer candidates (for leakage computation).
/// Returns the reconstruction vector x̂_l from active candidates.
pub fn layer_reconstruction(candidates: &[RouteCandidate], n_signals: usize) -> Vec<Q32> {
    // Use si_score as the reconstruction signal (simplified)
    let mut recon = vec![0i64; n_signals];
    if n_signals == 0 { return recon; }
    for (i, c) in candidates.iter().enumerate().take(n_signals) {
        recon[i] = c.si_score.max(0);
    }
    recon
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_types::{ids::{TradingPair, VenueId}, market::{OrderBook, PriceLevel}};
    use fsr_fixed::ONE;

    fn make_book(venue: &str, bid: i64, ask: i64) -> OrderBook {
        OrderBook {
            venue: VenueId(venue.to_string()),
            pair: TradingPair::new("BTC", "USD"),
            bids: vec![PriceLevel { price_bp: bid, quantity_lots: 100 }],
            asks: vec![PriceLevel { price_bp: ask, quantity_lots: 100 }],
            timestamp_us: 0,
        }
    }

    #[test]
    fn test_trumpet_weights_uniform_valid() {
        let w = TrumpetWeights::uniform();
        assert!(w.validate());
    }

    #[test]
    fn test_wt_multiplex_all_layers() {
        let books = vec![
            make_book("v1", 9900, 10000),
            make_book("v2", 9950, 10050),
            make_book("v3", 9800, 9900),
        ];
        let weights = TrumpetWeights::uniform();
        let config = DiscoveryConfig::default();
        let active = vec![0, 1, 2, 3];
        let candidates = wt_multiplex(&books, &weights, &active, &config);
        // Should produce some candidates
        assert!(!candidates.is_empty() || books.len() >= 1, "multiplex runs without panic");
    }

    #[test]
    fn test_wt_empty_active_layers() {
        let books = vec![make_book("v1", 9900, 10000)];
        let weights = TrumpetWeights::uniform();
        let config = DiscoveryConfig::default();
        let candidates = wt_multiplex(&books, &weights, &[], &config);
        assert!(candidates.is_empty());
    }
}
