//! Crack spread model: crude oil vs. refined products.
//!
//! 3-2-1 Crack Spread (industry standard):
//!   Crack = (2×RBOB×42 + 1×HO×42) / 3 − CL
//!
//! All legs normalized to $/bbl for the deviation calculation.
//!
//! Price conventions in fsr-types basis-points (price × 100):
//!   CL  ($/bbl):    bp = dollars × 100
//!   RB  ($/gallon): bp = dollars × 100   (price_bp / 100 × 42 = $/bbl for RB)
//!   HO  ($/gallon): bp = dollars × 100
//!
//! Since RB and HO are quoted in $/gallon and 1 bbl = 42 gallons:
//!   RB_bbl_bp = RB_gal_bp × 42
//!   HO_bbl_bp = HO_gal_bp × 42
//!
//! Crack_3_2_1_bp = (2×RB_bbl_bp + 1×HO_bbl_bp) / 3 − CL_bp
//!
//! Fair value (theoretical):
//!   The crack spread reflects refining margin. We use an exponential moving
//!   average of observed values as the fair-value prior (mean-reversion model).
//!   The initial prior is provided as a constructor argument.

use crate::model::{QuoteSet, SpreadModel};
use fsr_types::ids::TradingPair;

pub struct CrackSpreadModel {
    label: String,
    crude_pair:   TradingPair,   // TradingPair("CL", "YYYYMM")
    gasoline_pair: TradingPair,  // TradingPair("RB", "YYYYMM")
    heat_oil_pair: TradingPair,  // TradingPair("HO", "YYYYMM")
    /// 3-2-1 (true) or 5-3-2 (false).
    three_two_one: bool,
    /// Running EMA of observed crack as fair-value estimate.
    ema_fair_bp: i64,
    /// EMA decay factor as integer (1..99, higher = slower decay).
    /// ema = ema × alpha/100 + observed × (100-alpha)/100
    ema_alpha: i64,
}

impl CrackSpreadModel {
    /// Create a 3-2-1 crack spread model.
    ///
    /// `yyyymm`: delivery month for all three legs, e.g. "202606".
    /// `initial_fair_bp`: prior estimate of fair crack in bp (e.g. 2500 = $25/bbl).
    pub fn new_321(yyyymm: impl Into<String>, initial_fair_bp: i64) -> Self {
        let mm = yyyymm.into();
        CrackSpreadModel {
            label: format!("3-2-1 Crack {}", &mm),
            crude_pair:    TradingPair("CL".into(), mm.clone()),
            gasoline_pair: TradingPair("RB".into(), mm.clone()),
            heat_oil_pair: TradingPair("HO".into(), mm),
            three_two_one: true,
            ema_fair_bp: initial_fair_bp,
            ema_alpha: 95,
        }
    }

    /// Update the EMA fair-value with a new observation.
    /// Call this each tick after evaluation.
    pub fn update_ema(&mut self, observed_bp: i64) {
        self.ema_fair_bp = self.ema_fair_bp * self.ema_alpha / 100
            + observed_bp * (100 - self.ema_alpha) / 100;
    }

    fn crack_bp_from_quotes(&self, quotes: &QuoteSet) -> Option<i64> {
        let cl_bp = quotes.mid_bp(&self.crude_pair)?;
        let rb_bp = quotes.mid_bp(&self.gasoline_pair)?;
        let ho_bp = quotes.mid_bp(&self.heat_oil_pair)?;

        // Convert gasoline and heating oil from $/gallon to $/bbl (×42)
        let rb_bbl = rb_bp * 42;
        let ho_bbl = ho_bp * 42;

        if self.three_two_one {
            // (2 × RB + 1 × HO) / 3 − CL
            Some((2 * rb_bbl + ho_bbl) / 3 - cl_bp)
        } else {
            // 5-3-2: (3 × RB + 2 × HO) / 5 − CL
            Some((3 * rb_bbl + 2 * ho_bbl) / 5 - cl_bp)
        }
    }
}

impl SpreadModel for CrackSpreadModel {
    fn label(&self) -> &str {
        &self.label
    }

    /// Fair value = EMA of historical crack (mean-reversion anchor).
    fn fair_value_bp(&self, _quotes: &QuoteSet) -> Option<i64> {
        Some(self.ema_fair_bp)
    }

    fn observed_bp(&self, quotes: &QuoteSet) -> Option<i64> {
        self.crack_bp_from_quotes(quotes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_types::market::{OrderBook, PriceLevel};
    use fsr_types::ids::VenueId;

    fn book(root: &str, mm: &str, mid: i64) -> OrderBook {
        OrderBook {
            venue: VenueId("SIM".into()),
            pair: TradingPair(root.into(), mm.into()),
            bids: vec![PriceLevel { price_bp: mid - 1, quantity_lots: 5 }],
            asks: vec![PriceLevel { price_bp: mid + 1, quantity_lots: 5 }],
            timestamp_us: 0,
        }
    }

    #[test]
    fn crack_321_calculation() {
        // CL=$75, RB=$2.50/gal, HO=$2.40/gal
        // RB_bbl = $2.50 × 42 = $105.00
        // HO_bbl = $2.40 × 42 = $100.80
        // Crack = (2×105 + 100.80) / 3 - 75 = 310.80/3 - 75 = 103.60 - 75 = $28.60
        // In bp: CL=7500, RB=250, HO=240
        // RB_bbl_bp = 250 × 42 = 10500; HO_bbl_bp = 240 × 42 = 10080
        // Crack_bp = (2×10500 + 10080)/3 - 7500 = 31080/3 - 7500 = 10360 - 7500 = 2860
        let model = CrackSpreadModel::new_321("202606", 2500);
        let books = vec![
            book("CL", "202606", 7500),
            book("RB", "202606",  250),
            book("HO", "202606",  240),
        ];
        let qs = QuoteSet::new(&books);
        let obs = model.observed_bp(&qs).unwrap();
        assert_eq!(obs, 2860);
    }
}
