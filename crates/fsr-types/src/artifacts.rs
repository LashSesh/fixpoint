//! Artifact model types: NullcenterCert, route bundles, etc.

use crate::{Hash256, Q32};
use serde::{Deserialize, Serialize};

/// NullcenterCert — the central admissibility certificate (spec §11.2).
/// Every jump, commit, promotion, rollback must carry one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NullcenterCert {
    /// All gates passed
    pub gate_open: bool,
    /// SI value at certification time (Q32)
    pub si_at_cert: Q32,
    /// Temporal phase window index
    pub phase_bin: u16,
    /// w(x): windnarbe winding counter at certification
    pub wind_count: u64,
    /// χ(x): rolling digest of NC traversals
    pub rolling_digest: Hash256,
    /// Shadow-chain head before this traversal
    pub pre_digest: Hash256,
    /// Shadow-chain head after this traversal
    pub post_digest: Hash256,
}

/// Trumpet layer identifier (spec §12.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrumpetLayer {
    /// L1: Triangular arbitrage (A→B→C→A)
    L1,
    /// L2: Quad-leg arbitrage (4-leg cycles)
    L2,
    /// L3: Hedge handoff routes
    L3,
    /// L4: EQ-only smoothing
    L4,
}

/// A single route candidate with SI ranking.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RouteCandidate {
    pub route_id: crate::ids::RouteId,
    pub layer: TrumpetLayer,
    /// SI score (Q32)
    pub si_score: Q32,
    /// Net edge (Q32, ≥ τ_edge to be admissible)
    pub net_edge: Q32,
    /// Legs: (venue, pair, side, quantity_lots, price_bp)
    pub legs: Vec<RouteLeg>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RouteLeg {
    pub venue: crate::ids::VenueId,
    pub pair: crate::ids::TradingPair,
    pub side: crate::market::OrderSide,
    pub quantity_lots: u64,
    /// Limit price in basis points
    pub price_bp: i64,
    /// Fee in basis points (integer)
    pub fee_bp: i64,
    /// Slippage in basis points (integer)
    pub slip_bp: i64,
}
