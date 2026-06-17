//! Calendar spread model: near vs. far contract of the same commodity.
//!
//! Deviation = observed_basis - theoretical_carry_basis
//!
//! Theoretical carry (simplified linear approximation):
//!   FV_basis = F_near × (r + storage) × Δt
//!
//! where Δt is the time between delivery months in years (approximated as
//! month_delta / 12.0) and r + storage is a combined carry rate.
//!
//! Positive deviation → far is trading RICHER than carry implies (contango
//! steeper than cost-of-carry) → potential short the spread.
//! Negative deviation → far is CHEAPER (backwardation stronger than expected)
//! → potential long the spread.

use crate::model::{QuoteSet, SpreadModel};
use fsr_types::ids::TradingPair;

pub struct CalendarSpreadModel {
    label: String,
    /// TradingPair for the near contract, e.g. TradingPair("CL", "202606")
    near_pair: TradingPair,
    /// TradingPair for the far contract, e.g. TradingPair("CL", "202607")
    far_pair: TradingPair,
    /// Number of months between near and far delivery (usually 1).
    month_delta: u8,
    /// Combined carry rate (risk-free + storage − convenience_yield), per year.
    /// Represented as integer basis-points per year (e.g. 500 = 5%/year).
    carry_rate_bppa: i64,
}

impl CalendarSpreadModel {
    /// Create a monthly calendar spread.
    ///
    /// `near_root` / `near_yyyymm` / `far_yyyymm`: e.g. "CL" / "202606" / "202607".
    /// `carry_rate_bppa`: annual carry rate in bp (500 = 5%/year).
    pub fn new(
        near_root: impl Into<String>,
        near_yyyymm: impl Into<String>,
        far_yyyymm: impl Into<String>,
        month_delta: u8,
        carry_rate_bppa: i64,
    ) -> Self {
        let root = near_root.into();
        let near_mm = near_yyyymm.into();
        let far_mm = far_yyyymm.into();
        CalendarSpreadModel {
            label: format!("{} Cal {}/{}", root, &near_mm[4..], &far_mm[4..]),
            near_pair: TradingPair(root.clone(), near_mm),
            far_pair: TradingPair(root, far_mm),
            month_delta,
            carry_rate_bppa,
        }
    }
}

impl SpreadModel for CalendarSpreadModel {
    fn label(&self) -> &str {
        &self.label
    }

    /// Theoretical fair-value basis = F_near × carry_rate × Δt
    fn fair_value_bp(&self, quotes: &QuoteSet) -> Option<i64> {
        let f_near = quotes.mid_bp(&self.near_pair)?;
        // Δt ≈ month_delta / 12 years
        // FV_basis = F_near × carry_rate_bppa × month_delta / 12 / 10000
        // Dividing by 10000 because carry_rate is in bp (1/10000 of price) and
        // we want the result in bp of price movement.
        // Simplified: FV_basis_bp = f_near * carry_rate_bppa * month_delta / (12 * 10000)
        let numer = f_near.saturating_mul(self.carry_rate_bppa)
                          .saturating_mul(self.month_delta as i64);
        let fv = numer / (12 * 10_000);
        Some(fv)
    }

    /// Observed basis = F_far - F_near (positive = contango, negative = backwardation).
    fn observed_bp(&self, quotes: &QuoteSet) -> Option<i64> {
        let f_near = quotes.mid_bp(&self.near_pair)?;
        let f_far  = quotes.mid_bp(&self.far_pair)?;
        Some(f_far - f_near)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_types::market::{OrderBook, PriceLevel};
    use fsr_types::ids::VenueId;

    fn make_book(root: &str, mm: &str, mid: i64) -> OrderBook {
        OrderBook {
            venue: VenueId("IBKR-SIM".into()),
            pair: TradingPair(root.into(), mm.into()),
            bids: vec![PriceLevel { price_bp: mid - 1, quantity_lots: 10 }],
            asks: vec![PriceLevel { price_bp: mid + 1, quantity_lots: 10 }],
            timestamp_us: 0,
        }
    }

    #[test]
    fn contango_positive_deviation() {
        // CL Jun/Jul, 5%/year carry, near=$75 → FV basis ≈ $0.3125 → 31 bp
        let model = CalendarSpreadModel::new("CL", "202606", "202607", 1, 500);
        let books = vec![
            make_book("CL", "202606", 7500), // $75.00
            make_book("CL", "202607", 7560), // $75.60 (observed basis = +60bp)
        ];
        let qs = QuoteSet::new(&books);
        let fv = model.fair_value_bp(&qs).unwrap();
        let obs = model.observed_bp(&qs).unwrap();
        // Observed basis (60bp) > FV basis → positive deviation → spread rich
        assert!(obs > fv, "obs={} fv={}", obs, fv);
    }
}
