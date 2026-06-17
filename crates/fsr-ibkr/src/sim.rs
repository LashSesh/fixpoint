//! IbkrSimAdapter: deterministic commodity futures price simulator.
//!
//! Generates realistic spread-correlated Brownian-like price movements for
//! commodity futures contracts. Uses the same LCG determinism philosophy as
//! PaperBroker in fsr-runtime.
//!
//! Default universe (configurable):
//!   Energy:  CL Jun/Jul, RB Jun, HO Jun  (for 3-2-1 crack + calendar)
//!   Grains:  ZS Jul/Aug, ZL Jul, ZM Jul  (for crush + calendar)
//!   Metals:  GC Jun
//!
//! TradingPair convention: TradingPair(root, "YYYYMM")
//!   e.g. TradingPair("CL", "202606")

use fsr_contract::registry::ContractRegistry;
use fsr_types::ids::{TradingPair, VenueId};
use fsr_types::market::{OrderBook, PriceLevel, VenueBroker};

const VENUE_ID: &str = "IBKR-SIM";

/// Configuration for one simulated futures instrument.
#[derive(Clone)]
struct SimInstrument {
    pair: TradingPair,
    /// Current mid-price in basis-points.
    price_bp: i64,
    /// Tick size in basis-points (min price movement).
    tick_bp: i64,
    /// Per-tick volatility in basis-points (1-sigma move).
    vol_bp: i64,
    /// Correlation coefficient with the first instrument in group [-100, 100].
    /// 0 = independent.
    corr: i8,
    /// LCG internal state.
    lcg_state: u64,
}

impl SimInstrument {
    fn new(
        root: &str,
        yyyymm: &str,
        initial_price_bp: i64,
        tick_bp: i64,
        vol_bp: i64,
        corr: i8,
        seed: u64,
    ) -> Self {
        SimInstrument {
            pair: TradingPair(root.into(), yyyymm.into()),
            price_bp: initial_price_bp,
            tick_bp: tick_bp.max(1),
            vol_bp,
            corr,
            lcg_state: seed,
        }
    }

    /// LCG step: returns a signed noise in [-vol_bp, +vol_bp].
    fn next_noise(&mut self) -> i64 {
        // 64-bit LCG (same params as PaperBroker)
        self.lcg_state = self.lcg_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let r = ((self.lcg_state >> 33) as i64) % (self.vol_bp.max(1) + 1);
        // Alternate sign based on bit 32
        if (self.lcg_state >> 32) & 1 == 0 { r } else { -r }
    }

    fn advance(&mut self, common_noise: i64) {
        let own_noise = self.next_noise();
        // Blend correlated + independent noise
        let c = self.corr.abs() as i64;
        let blended = (common_noise * c + own_noise * (100 - c)) / 100;
        // Snap to tick grid
        let raw = self.price_bp + blended;
        self.price_bp = ((raw / self.tick_bp) * self.tick_bp).max(self.tick_bp);
    }

    fn order_book(&self) -> OrderBook {
        let half_spread = self.tick_bp.max(1);
        OrderBook {
            venue: VenueId(VENUE_ID.into()),
            pair: self.pair.clone(),
            bids: vec![PriceLevel {
                price_bp: self.price_bp - half_spread,
                quantity_lots: 50,
            }],
            asks: vec![PriceLevel {
                price_bp: self.price_bp + half_spread,
                quantity_lots: 50,
            }],
            timestamp_us: 0,
        }
    }
}

/// Deterministic commodity futures simulator implementing VenueBroker.
///
/// Groups of instruments share a common noise component to simulate realistic
/// within-group correlations (e.g. CL and RB move together).
pub struct IbkrSimAdapter {
    /// Groups of correlated instruments. First instrument in each group is the
    /// "leader"; others have corr ≠ 0.
    groups: Vec<Vec<SimInstrument>>,
    /// Shared LCG for group-level common noise.
    group_seed: u64,
    tick: u64,
}

impl IbkrSimAdapter {
    /// Build the default commodity universe for 2026.
    pub fn default_2026() -> Self {
        let mut adapter = IbkrSimAdapter {
            groups: Vec::new(),
            group_seed: 0xDEAD_BEEF_CAFE_1337,
            tick: 0,
        };

        // ── Energy group ─────────────────────────────────────────────────────
        // CL Jun/Jul: ~$75 contango ~$0.30/month
        // RB Jun: ~$2.50/gal → 250bp; HO Jun: ~$2.40/gal → 240bp
        adapter.groups.push(vec![
            SimInstrument::new("CL", "202606", 7500,  1, 15, 0,   0x1111_2222_3333_4444),
            SimInstrument::new("CL", "202607", 7530,  1, 14, 85,  0x2222_3333_4444_5555),
            SimInstrument::new("RB", "202606",  250,  1,  6, 75,  0x3333_4444_5555_6666),
            SimInstrument::new("HO", "202606",  240,  1,  5, 72,  0x4444_5555_6666_7777),
        ]);

        // ── Grains group ─────────────────────────────────────────────────────
        // ZS Jul/Aug: ~1050¢/bu; ZL Jul: ~45¢/lb → 4500bp; ZM Jul: ~$350/ton → 35000bp
        adapter.groups.push(vec![
            SimInstrument::new("ZS", "202607", 105000, 25, 300, 0,  0x5555_6666_7777_8888),
            SimInstrument::new("ZS", "202608", 105200, 25, 290, 90, 0x6666_7777_8888_9999),
            SimInstrument::new("ZL", "202607",   4500,  1,  20, 60, 0x7777_8888_9999_AAAA),
            SimInstrument::new("ZM", "202607",  35000, 10, 150, 65, 0x8888_9999_AAAA_BBBB),
        ]);

        // ── Metals group ─────────────────────────────────────────────────────
        adapter.groups.push(vec![
            SimInstrument::new("GC", "202606", 200000, 10, 400, 0, 0x9999_AAAA_BBBB_CCCC),
        ]);

        // ── Grains continuation: Corn ─────────────────────────────────────
        adapter.groups.push(vec![
            SimInstrument::new("ZC", "202607", 45000, 25, 200, 0,   0xAAAA_BBBB_CCCC_DDDD),
            SimInstrument::new("ZC", "202609", 45200, 25, 195, 88,  0xBBBB_CCCC_DDDD_EEEE),
        ]);

        adapter
    }

    /// Build a custom adapter from a ContractRegistry.
    /// Adds one near + one far monthly instrument for each symbol in the registry.
    pub fn from_registry(registry: &ContractRegistry, base_year: u16, base_month: u8) -> Self {
        let mut adapter = IbkrSimAdapter {
            groups: Vec::new(),
            group_seed: 0xFEED_FACE_DEAD_BEEF,
            tick: 0,
        };

        let mut seed: u64 = 0x1234_5678_9ABC_DEF0;
        let next_month = if base_month == 12 { 1 } else { base_month + 1 };
        let next_year  = if base_month == 12 { base_year + 1 } else { base_year };

        for sym in registry.symbols() {
            let spec = registry.get(sym).unwrap();
            let near_mm = format!("{:04}{:02}", base_year, base_month);
            let far_mm  = format!("{:04}{:02}", next_year, next_month);
            // Rough initial price based on tick_value (just for initialization)
            let init_bp = spec.tick_bp * 1000; // placeholder; real price from config
            let group = vec![
                SimInstrument::new(sym, &near_mm, init_bp, spec.tick_bp, spec.tick_bp * 5, 0,  seed),
                SimInstrument::new(sym, &far_mm, init_bp + spec.tick_bp * 3, spec.tick_bp, spec.tick_bp * 5, 80, seed ^ 0xFFFF),
            ];
            adapter.groups.push(group);
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        }

        adapter
    }

    /// LCG step for group common noise.
    fn group_noise(&mut self, vol: i64) -> i64 {
        self.group_seed = self.group_seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let r = ((self.group_seed >> 33) as i64) % (vol.max(1) + 1);
        if (self.group_seed >> 32) & 1 == 0 { r } else { -r }
    }
}

impl VenueBroker for IbkrSimAdapter {
    fn advance_all(&mut self) {
        self.tick += 1;
        for group in &mut self.groups {
            let leader_vol = group.first().map(|i| i.vol_bp).unwrap_or(10);
            let common = {
                self.group_seed = self.group_seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let r = ((self.group_seed >> 33) as i64) % (leader_vol.max(1) + 1);
                if (self.group_seed >> 32) & 1 == 0 { r } else { -r }
            };
            for inst in group.iter_mut() {
                inst.advance(common);
            }
        }
    }

    fn all_books(&mut self) -> Vec<OrderBook> {
        let ts = self.tick * 1_000_000; // fake microsecond timestamp
        let mut books: Vec<OrderBook> = self.groups.iter()
            .flat_map(|g| g.iter().map(|i| {
                let mut b = i.order_book();
                b.timestamp_us = ts;
                b
            }))
            .collect();
        books
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_2026_produces_books() {
        let mut adapter = IbkrSimAdapter::default_2026();
        adapter.advance_all();
        let books = adapter.all_books();
        assert!(!books.is_empty());
        // All books must have valid bid < ask
        for b in &books {
            let bid = b.best_bid_bp().unwrap();
            let ask = b.best_ask_bp().unwrap();
            assert!(bid < ask, "{:?}: bid={} ask={}", b.pair, bid, ask);
        }
    }

    #[test]
    fn deterministic_replay() {
        let mut a = IbkrSimAdapter::default_2026();
        let mut b = IbkrSimAdapter::default_2026();
        for _ in 0..100 {
            a.advance_all();
            b.advance_all();
        }
        let ba = a.all_books();
        let bb = b.all_books();
        assert_eq!(ba.len(), bb.len());
        for (x, y) in ba.iter().zip(bb.iter()) {
            assert_eq!(x.pair, y.pair);
            assert_eq!(x.best_bid_bp(), y.best_bid_bp());
        }
    }
}
