//! PaperFeed: Deterministic synthetic order book generator (spec §10).
//!
//! Paper mode must be fully operational with synthetic data alone.
//! PaperFeed implements VenueAdapter with a deterministic pseudo-random order book.

use fsr_types::{
    ids::{OrderId, TradingPair, VenueId},
    market::{OrderBook, OrderReceipt, OrderRequest, PriceLevel},
};
use serde::{Deserialize, Serialize};

/// Deterministic LCG for reproducible synthetic data (paper mode).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DetRng {
    state: u64,
}

impl DetRng {
    pub fn new(seed: u64) -> Self {
        DetRng { state: seed }
    }

    /// Next u64 (Knuth LCG).
    pub fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state
    }

    /// Next value in [0, range).
    pub fn next_range(&mut self, range: u64) -> u64 {
        self.next_u64() % range
    }

    /// Next value in [min_val, max_val] (basis points).
    pub fn next_bp(&mut self, min_val: i64, max_val: i64) -> i64 {
        let range = (max_val - min_val).max(1) as u64;
        min_val + self.next_range(range) as i64
    }
}

/// Synthetic paper venue adapter (spec §10).
#[allow(dead_code)]
pub struct PaperVenue {
    pub venue_id: VenueId,
    pub pair: TradingPair,
    pub rng: DetRng,
    pub tick: u64,
    /// Next order ID.
    next_order_id: u64,
    /// Base price in basis points (moves with synthetic market).
    base_price_bp: i64,
}

impl PaperVenue {
    pub fn new(venue_id: VenueId, pair: TradingPair, seed: u64, base_price_bp: i64) -> Self {
        PaperVenue {
            venue_id,
            pair,
            rng: DetRng::new(seed),
            tick: 0,
            next_order_id: 1,
            base_price_bp,
        }
    }

    /// Advance the synthetic market by one tick.
    pub fn advance(&mut self) {
        self.tick += 1;
        // Brownian-like price movement: ±0.1% per tick
        let delta = self.rng.next_bp(-100, 100); // ±100 bp = ±1%
        self.base_price_bp = (self.base_price_bp + delta).max(1000);
    }

    /// Generate a synthetic order book snapshot.
    pub fn order_book(&mut self) -> OrderBook {
        let spread = self.rng.next_bp(5, 50); // 5..50 bp spread
        let bid = self.base_price_bp - spread / 2;
        let ask = self.base_price_bp + spread / 2;
        let bid_qty = self.rng.next_range(1000) + 1;
        let ask_qty = self.rng.next_range(1000) + 1;

        let timestamp_us = self.tick * 1_000_000;
        OrderBook {
            venue: self.venue_id.clone(),
            pair: self.pair.clone(),
            bids: vec![
                PriceLevel { price_bp: bid, quantity_lots: bid_qty },
                PriceLevel { price_bp: bid - spread, quantity_lots: bid_qty * 2 },
            ],
            asks: vec![
                PriceLevel { price_bp: ask, quantity_lots: ask_qty },
                PriceLevel { price_bp: ask + spread, quantity_lots: ask_qty * 2 },
            ],
            timestamp_us,
        }
    }

    /// Execute a paper order (always succeeds with synthetic slippage).
    #[allow(dead_code)]
    pub fn execute_order(&mut self, req: &OrderRequest) -> OrderReceipt {
        let slip = self.rng.next_bp(0, req.limit_price_bp.unwrap_or(0).abs() / 100 + 1);
        let executed_price = req.limit_price_bp.unwrap_or(self.base_price_bp);
        let id = self.next_order_id;
        self.next_order_id += 1;
        OrderReceipt {
            order_id: OrderId(id),
            venue: req.venue.clone(),
            filled_lots: req.quantity_lots,
            executed_price_bp: executed_price + slip,
            timestamp_us: self.tick * 1_000_000,
        }
    }
}

/// Multi-venue paper broker: manages several PaperVenues.
pub struct PaperBroker {
    pub venues: Vec<PaperVenue>,
}

impl PaperBroker {
    pub fn new() -> Self {
        PaperBroker { venues: Vec::new() }
    }

    pub fn add_venue(&mut self, venue: PaperVenue) {
        self.venues.push(venue);
    }

    /// Advance all venues by one tick.
    pub fn advance_all(&mut self) {
        for v in &mut self.venues {
            v.advance();
        }
    }

    /// Collect all order books from all venues.
    pub fn all_books(&mut self) -> Vec<OrderBook> {
        self.venues.iter_mut().map(|v| v.order_book()).collect()
    }

    /// Get all mid-prices for MCI computation.
    #[allow(dead_code)]
    pub fn mid_prices(&mut self) -> Vec<i64> {
        self.venues
            .iter_mut()
            .filter_map(|v| {
                let book = v.order_book();
                book.mid_bp()
            })
            .collect()
    }
}

impl Default for PaperBroker {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a default 3-venue paper broker for testing.
pub fn default_paper_broker() -> PaperBroker {
    let mut broker = PaperBroker::new();
    broker.add_venue(PaperVenue::new(
        VenueId("paper-v1".to_string()),
        TradingPair::new("BTC", "USDT"),
        42,
        10_000_000, // 10,000,000 bp = $100,000 BTC price in bp
    ));
    broker.add_venue(PaperVenue::new(
        VenueId("paper-v2".to_string()),
        TradingPair::new("ETH", "USDT"),
        1337,
        300_000, // ~$3000 ETH
    ));
    broker.add_venue(PaperVenue::new(
        VenueId("paper-v3".to_string()),
        TradingPair::new("BTC", "ETH"),
        9999,
        3_333, // BTC/ETH ratio
    ));
    broker
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_det_rng_deterministic() {
        let mut rng1 = DetRng::new(42);
        let mut rng2 = DetRng::new(42);
        for _ in 0..100 {
            assert_eq!(rng1.next_u64(), rng2.next_u64(), "same seed → same sequence");
        }
    }

    #[test]
    fn test_paper_venue_order_book_valid() {
        let mut venue = PaperVenue::new(
            VenueId("test".to_string()),
            TradingPair::new("BTC", "USD"),
            1,
            100_000,
        );
        let book = venue.order_book();
        let bid = book.best_bid_bp().unwrap();
        let ask = book.best_ask_bp().unwrap();
        assert!(bid < ask, "bid must be less than ask");
        assert!(bid > 0, "prices must be positive");
    }

    #[test]
    fn test_paper_broker_advances_deterministically() {
        let mut b1 = default_paper_broker();
        let mut b2 = default_paper_broker();
        for _ in 0..10 {
            b1.advance_all();
            b2.advance_all();
        }
        let books1 = b1.all_books();
        let books2 = b2.all_books();
        for (bk1, bk2) in books1.iter().zip(books2.iter()) {
            assert_eq!(bk1.bids[0].price_bp, bk2.bids[0].price_bp, "deterministic advance");
        }
    }
}
