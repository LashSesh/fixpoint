//! Board crush spread model: soybeans → soybean oil + soybean meal.
//!
//! Board Crush Gross Processing Margin (GPM) per bushel of beans:
//!
//!   GPM = Oil_contribution + Meal_contribution − Bean_cost
//!
//! Yields per bushel (standard):
//!   11 lb soybean oil  (ZL, quoted in ¢/lb)
//!   44 lb soybean meal (ZM, quoted in $/short ton = $/2000 lb)
//!
//! GPM ($/bu) = 11 × ZL_cents_per_lb / 100
//!            + 44 × ZM_per_ton / 2000
//!            − ZS_cents_per_bu / 100
//!
//! In basis-points (all prices × 100):
//!   ZL_bp = ZL_cents_per_lb × 100  → ZL_cents_per_lb = ZL_bp / 100
//!   ZM_bp = ZM_per_ton × 100       → ZM_per_ton = ZM_bp / 100
//!   ZS_bp = ZS_cents_per_bu × 100  → ZS_cents_per_bu = ZS_bp / 100
//!
//! GPM_bp (in units of ¢/bu × 100 = same scale as ZS):
//!   GPM_bp = 11 × ZL_bp / 100 + 44 × ZM_bp / 2000 - ZS_bp
//!
//! Positive GPM → profitable to crush; negative → reverse crush.

use crate::model::{QuoteSet, SpreadModel};
use fsr_types::ids::TradingPair;

pub struct CrushSpreadModel {
    label: String,
    beans_pair: TradingPair,   // ZS
    oil_pair:   TradingPair,   // ZL
    meal_pair:  TradingPair,   // ZM
    /// EMA of observed GPM as fair-value estimate.
    ema_fair_bp: i64,
    ema_alpha: i64,
}

impl CrushSpreadModel {
    /// Create a board crush model for the given delivery month.
    ///
    /// `yyyymm`: e.g. "202607".
    /// `initial_fair_bp`: prior GPM estimate in bp (e.g. 200 = 2¢/bu = ~$0.02/bu).
    pub fn new(yyyymm: impl Into<String>, initial_fair_bp: i64) -> Self {
        let mm = yyyymm.into();
        CrushSpreadModel {
            label: format!("Board Crush {}", &mm),
            beans_pair: TradingPair("ZS".into(), mm.clone()),
            oil_pair:   TradingPair("ZL".into(), mm.clone()),
            meal_pair:  TradingPair("ZM".into(), mm),
            ema_fair_bp: initial_fair_bp,
            ema_alpha: 95,
        }
    }

    pub fn update_ema(&mut self, observed_bp: i64) {
        self.ema_fair_bp = self.ema_fair_bp * self.ema_alpha / 100
            + observed_bp * (100 - self.ema_alpha) / 100;
    }
}

impl SpreadModel for CrushSpreadModel {
    fn label(&self) -> &str {
        &self.label
    }

    fn fair_value_bp(&self, _quotes: &QuoteSet) -> Option<i64> {
        Some(self.ema_fair_bp)
    }

    fn observed_bp(&self, quotes: &QuoteSet) -> Option<i64> {
        let zs_bp = quotes.mid_bp(&self.beans_pair)?;
        let zl_bp = quotes.mid_bp(&self.oil_pair)?;
        let zm_bp = quotes.mid_bp(&self.meal_pair)?;

        // Convert to GPM in bp (¢/bu × 100), same unit as zs_bp:
        //   oil_bp  = 11 lb × (zl_bp/100 ¢/lb) × 100  = 11 × zl_bp
        //   meal_bp = 44 lb × (zm_bp/100 $/ton) / 2000 × 100¢/$ × 100
        //           = 44 × zm_bp / 20
        //   GPM_bp  = oil_bp + meal_bp − zs_bp
        let oil_contrib_bp  = 11 * zl_bp;
        let meal_contrib_bp = 44 * zm_bp / 20;

        Some(oil_contrib_bp + meal_contrib_bp - zs_bp)
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
    fn crush_gpm_calculation() {
        // ZS=1050¢/bu, ZL=45¢/lb, ZM=$350/ton
        // Oil: 11 × 45 = 495 ¢/bu
        // Meal: 44 × 350/2000 × 100 = 44 × 17.5 = 770 ¢/bu (but meal is $/ton so /100 not needed)
        // Actually in bp: ZS_bp=105000, ZL_bp=4500, ZM_bp=35000
        // oil_contrib = 11 × 4500 / 100 = 495
        // meal_contrib = 44 × 35000 / 2000 = 770
        // GPM_bp = 495 + 770 - 105000 = -103735  → strongly negative (loss on crush)
        // That seems wrong. Let me recalculate:
        // Wait: ZS is in ¢/bu, so ZS_bp = 1050 * 100 = 105000. That means $10.50/bu.
        // ZL is in ¢/lb, so ZL_bp = 45 * 100 = 4500. That means $0.45/lb.
        // ZM is in $/ton, so ZM_bp = 350 * 100 = 35000. That means $350/ton.
        // oil_contrib_bp = 11 × 4500 / 100 = 495 (in ¢/bu scale)
        // meal_contrib_bp = 44 × 35000 / 2000 = 770 (in ¢/bu scale)
        // GPM_bp = 495 + 770 - 105000 = -103735
        // That's clearly wrong - beans can't be worth $1050/bu while oil+meal gives 495+770=1265 ¢/bu = $12.65/bu.
        // Wait, ZS at $10.50/bu... oil at $0.45/lb × 11lb = $4.95; meal at $350/ton × 0.022 ton = $7.70
        // Oil+Meal = $4.95 + $7.70 = $12.65. Crush GPM = $12.65 - $10.50 = $2.15/bu = 215¢/bu
        // So GPM_bp should be 21500 (215¢ × 100).
        // My formula is wrong because I'm mixing units. Let me fix:
        // GPM in ¢/bu:
        //   oil = 11 lb × ZL_cents_per_lb (i.e. ZL_bp/100 ¢/lb) = 11 × (ZL_bp/100)
        //   meal = 44 lb × ZM_dollars_per_ton / 2000 lb_per_ton × 100 ¢/dollar = 44 × (ZM_bp/100) / 2000 × 100
        //        = 44 × ZM_bp / 2000
        //   gpm_cents = oil + meal - ZS_cents_per_bu = 11 × ZL_bp/100 + 44 × ZM_bp/2000 - ZS_bp/100
        // Then GPM_bp = gpm_cents * 100:
        //   GPM_bp = 11 × ZL_bp + 44 × ZM_bp / 20 - ZS_bp
        // With ZS_bp=105000, ZL_bp=4500, ZM_bp=35000:
        //   oil = 11 × 4500 = 49500
        //   meal = 44 × 35000 / 20 = 77000
        //   GPM_bp = 49500 + 77000 - 105000 = 21500 ✓
        // So the formula in observed_bp is wrong. Let me check it again.
        // Current: oil_contrib = 11 * zl_bp / 100 (this gives ¢/bu not bp)
        //          meal_contrib = 44 * zm_bp / 2000 (this gives ¢/bu not bp)
        //          GPM_bp = oil + meal - zs_bp (where zs_bp is ¢ × 100, not ¢)
        // The unit mismatch: oil_contrib is in ¢/bu but zs_bp is in (¢ × 100)
        // Fix: GPM_bp should be in same unit as zs_bp (¢ × 100 = bp)
        //   oil_contrib_bp = 11 × zl_bp  (lb × ¢/lb × 100 = ¢/bu × 100 = bp/bu)
        //   meal_contrib_bp = 44 × zm_bp / 20  (lb × $/ton × /2000 × /1 × 100 = ... 44/2000 × 100 = 44/20)
        //   GPM_bp = oil_contrib_bp + meal_contrib_bp - zs_bp
        // This gives units of (¢/bu × 100) = same as zs_bp. ✓

        let model = CrushSpreadModel::new("202607", 0);
        let books = vec![
            book("ZS", "202607", 105000), // 1050¢/bu
            book("ZL", "202607",   4500), // 45¢/lb
            book("ZM", "202607",  35000), // $350/ton
        ];
        let qs = QuoteSet::new(&books);
        // oil_bp  = 11 × 4500 = 49500
        // meal_bp = 44 × 35000 / 20 = 77000
        // GPM_bp  = 49500 + 77000 - 105000 = 21500  (= 215 ¢/bu = $2.15/bu)
        let gpm = model.observed_bp(&qs).unwrap();
        assert_eq!(gpm, 21500);
    }
}
