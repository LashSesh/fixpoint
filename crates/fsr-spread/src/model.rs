//! SpreadModel trait and shared types.

use fsr_types::market::OrderBook;
use fsr_types::ids::TradingPair;

/// Current market quotes needed to evaluate a spread.
///
/// Keys are TradingPair(root, yyyymm) matching the IbkrSimAdapter convention.
pub struct QuoteSet<'a> {
    books: &'a [OrderBook],
}

impl<'a> QuoteSet<'a> {
    pub fn new(books: &'a [OrderBook]) -> Self {
        QuoteSet { books }
    }

    /// Mid-price in basis-points for a given contract pair, or None if not found.
    pub fn mid_bp(&self, pair: &TradingPair) -> Option<i64> {
        self.books.iter().find(|b| &b.pair == pair).and_then(|b| b.mid_bp())
    }

    /// Best-bid in basis-points.
    pub fn bid_bp(&self, pair: &TradingPair) -> Option<i64> {
        self.books.iter().find(|b| &b.pair == pair).and_then(|b| b.best_bid_bp())
    }

    /// Best-ask in basis-points.
    pub fn ask_bp(&self, pair: &TradingPair) -> Option<i64> {
        self.books.iter().find(|b| &b.pair == pair).and_then(|b| b.best_ask_bp())
    }
}

/// Output of a spread evaluation for one tick.
#[derive(Clone, Debug)]
pub struct SpreadSignal {
    /// Spread label (e.g. "3-2-1 Crack CLM6").
    pub label: String,
    /// Observed spread value in basis-points.
    pub observed_bp: i64,
    /// Theoretical fair-value in basis-points.
    pub fair_value_bp: i64,
    /// Deviation D = observed − fair_value, in basis-points.
    pub deviation_bp: i64,
    /// |D| / prior_sigma as Q32 (standardised signal strength, 0=no signal).
    pub z_score_q32: i64,
    /// Evaluation tick.
    pub tick: u64,
}

impl SpreadSignal {
    pub fn is_rich(&self) -> bool {
        self.deviation_bp > 0
    }
    pub fn is_cheap(&self) -> bool {
        self.deviation_bp < 0
    }
}

/// Core trait for spread signal models.
pub trait SpreadModel: Send + Sync {
    fn label(&self) -> &str;

    /// Theoretical fair-value of the spread in basis-points.
    fn fair_value_bp(&self, quotes: &QuoteSet) -> Option<i64>;

    /// Observed spread value in basis-points (from live order books).
    fn observed_bp(&self, quotes: &QuoteSet) -> Option<i64>;

    /// Evaluate: compute fair, observed, and deviation.
    fn evaluate(&self, quotes: &QuoteSet, prior_sigma_bp: i64, tick: u64) -> Option<SpreadSignal> {
        let obs = self.observed_bp(quotes)?;
        let fv  = self.fair_value_bp(quotes)?;
        let dev = obs - fv;
        let z = if prior_sigma_bp > 0 {
            // z-score as Q32: (|dev| << 32) / prior_sigma_bp
            (dev.abs() << 32) / prior_sigma_bp
        } else {
            0
        };
        Some(SpreadSignal {
            label: self.label().to_string(),
            observed_bp: obs,
            fair_value_bp: fv,
            deviation_bp: dev,
            z_score_q32: z,
            tick,
        })
    }
}
