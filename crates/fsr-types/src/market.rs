//! Market data types: OrderBook, OrderRequest, OrderReceipt, etc.

use crate::ids::{OrderId, TradingPair, VenueId};
use crate::Q32;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A single price level in an order book.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceLevel {
    /// Price in basis points (integer)
    pub price_bp: i64,
    /// Quantity in integer lots
    pub quantity_lots: u64,
}

/// Normalized order book snapshot.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrderBook {
    pub venue: VenueId,
    pub pair: TradingPair,
    pub bids: Vec<PriceLevel>,
    pub asks: Vec<PriceLevel>,
    /// Capture time in microseconds since epoch
    pub timestamp_us: u64,
}

impl OrderBook {
    /// Best bid price in basis points, or None if empty.
    pub fn best_bid_bp(&self) -> Option<i64> {
        self.bids.first().map(|l| l.price_bp)
    }
    /// Best ask price in basis points, or None if empty.
    pub fn best_ask_bp(&self) -> Option<i64> {
        self.asks.first().map(|l| l.price_bp)
    }
    /// Mid-price in basis points (integer average).
    pub fn mid_bp(&self) -> Option<i64> {
        match (self.best_bid_bp(), self.best_ask_bp()) {
            (Some(b), Some(a)) => Some((b + a) / 2),
            _ => None,
        }
    }
}

/// An order request submitted to a venue.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrderRequest {
    pub venue: VenueId,
    pub pair: TradingPair,
    pub side: OrderSide,
    /// Quantity in integer lots
    pub quantity_lots: u64,
    /// Limit price in basis points (None = market order)
    pub limit_price_bp: Option<i64>,
    pub route_id: crate::ids::RouteId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Receipt returned by a venue after order submission.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrderReceipt {
    pub order_id: OrderId,
    pub venue: VenueId,
    pub filled_lots: u64,
    /// Executed price in basis points
    pub executed_price_bp: i64,
    pub timestamp_us: u64,
}

/// Receipt returned by a venue after order cancellation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CancelReceipt {
    pub order_id: OrderId,
    pub cancelled: bool,
    pub timestamp_us: u64,
}

/// Venue-level error.
#[derive(Debug, Error, Clone, Serialize, Deserialize)]
pub enum VenueError {
    #[error("venue unavailable: {0}")]
    Unavailable(String),
    #[error("order rejected: {0}")]
    OrderRejected(String),
    #[error("insufficient liquidity")]
    InsufficientLiquidity,
    #[error("rate limited")]
    RateLimited,
    #[error("network error: {0}")]
    NetworkError(String),
}

/// Trade side for cross-venue candidate legs (spec §7.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

/// A single leg of a (potentially cross-venue) candidate route (spec §7.2).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CandidateLeg {
    pub venue: crate::ids::VenueId,
    pub pair: crate::ids::TradingPair,
    pub side: Side,
    /// Price in basis points.
    pub price: Q32,
    /// Available quantity in lots.
    pub available_qty: Q32,
}

/// Normalized route signal observation, ri ∈ [0,1] (as Q32 ∈ [0, ONE]).
/// ONE = 1 << 32.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteSignal(pub Q32);

/// ResonanceSnapshot — computed each macro-cycle tick (spec §16, §8).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResonanceSnapshot {
    /// Pairwise coherence (Q32)
    pub kappa: Q32,
    /// Signal entropy (Q32)
    pub entropy: Q32,
    /// Mean signal sync (Q32)
    pub sync: Q32,
    /// Momentum M = tanh(2*sync - 1) (Q32)
    pub momentum: Q32,
    /// Extended Spiral Index SI (Q32)
    pub si: Q32,
    /// Evaluation triplet: ψ (quality)
    pub psi: Q32,
    /// Evaluation triplet: ρ (stability)
    pub rho: Q32,
    /// Evaluation triplet: ω (efficiency)
    pub omega: Q32,
    /// Tick this snapshot was computed at
    pub tick: u64,
}
