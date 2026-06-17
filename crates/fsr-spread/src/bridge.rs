//! SpreadBridge: integrates SpreadModel instances into the macro-cycle engine.
//!
//! Mirrors the DshaeBridge pattern from fsr-runtime/src/dshae_bridge.rs.
//! Called once per macro-cycle tick from engine.rs (Phase 6 step).

use crate::calendar::CalendarSpreadModel;
use crate::crack::CrackSpreadModel;
use crate::crush::CrushSpreadModel;
use crate::model::{QuoteSet, SpreadModel, SpreadSignal};
use fsr_contract::spread::{CrackRecipe, CrushRecipe, SpreadDef, SpreadKind};
use fsr_types::ids::TradingPair;

/// A signal that crossed the z-score threshold this tick: a "crystal" event.
#[derive(Clone, Debug)]
pub struct SpreadCrystal {
    pub signal: SpreadSignal,
    /// Z-score threshold it crossed (configurable).
    pub z_threshold_q32: i64,
}

/// Holds all active spread models and evaluates them each tick.
pub struct SpreadBridge {
    pub enabled: bool,
    /// Minimum z-score (Q32) to emit a SpreadCrystal.
    pub z_threshold_q32: i64,
    calendar_models: Vec<(CalendarSpreadModel, i64)>,   // (model, prior_sigma_bp)
    crack_models:    Vec<(CrackSpreadModel, i64)>,
    crush_models:    Vec<(CrushSpreadModel, i64)>,
}

impl SpreadBridge {
    pub fn new(enabled: bool) -> Self {
        SpreadBridge {
            enabled,
            z_threshold_q32: 2 << 32,   // z > 2.0 triggers a crystal
            calendar_models: Vec::new(),
            crack_models: Vec::new(),
            crush_models: Vec::new(),
        }
    }

    /// Load spread definitions from a set of SpreadDefs.
    pub fn load_spreads(&mut self, defs: &[SpreadDef]) {
        for def in defs {
            match &def.kind {
                SpreadKind::Calendar { near, far } => {
                    let month_delta = month_distance(near.month, near.year, far.month, far.year);
                    let m = CalendarSpreadModel::new(
                        &near.root,
                        near.yyyymm(),
                        far.yyyymm(),
                        month_delta,
                        500, // 5%/year carry default
                    );
                    self.calendar_models.push((m, def.prior_sigma_bp));
                }
                SpreadKind::Crack { crude, gasoline: _, heat_oil: _, recipe } => {
                    let initial_fair = def.prior_fair_value_bp;
                    let m = match recipe {
                        CrackRecipe::ThreeTwoOne | CrackRecipe::FiveThreeTwo | CrackRecipe::OneToOne =>
                            CrackSpreadModel::new_321(crude.yyyymm(), initial_fair),
                    };
                    self.crack_models.push((m, def.prior_sigma_bp));
                }
                SpreadKind::Crush { beans, oil: _, meal: _, recipe: CrushRecipe::Standard } => {
                    let m = CrushSpreadModel::new(beans.yyyymm(), def.prior_fair_value_bp);
                    self.crush_models.push((m, def.prior_sigma_bp));
                }
                _ => {} // Butterfly, InterCommodity, Outright: not yet implemented
            }
        }
    }

    /// Add a simple monthly calendar spread.
    pub fn add_calendar(
        &mut self,
        root: &str,
        near_yyyymm: &str,
        far_yyyymm: &str,
        prior_sigma_bp: i64,
    ) {
        let m = CalendarSpreadModel::new(root, near_yyyymm, far_yyyymm, 1, 500);
        self.calendar_models.push((m, prior_sigma_bp));
    }

    pub fn add_crack_321(&mut self, yyyymm: &str, initial_fair_bp: i64, prior_sigma_bp: i64) {
        let m = CrackSpreadModel::new_321(yyyymm, initial_fair_bp);
        self.crack_models.push((m, prior_sigma_bp));
    }

    pub fn add_crush(&mut self, yyyymm: &str, initial_fair_bp: i64, prior_sigma_bp: i64) {
        let m = CrushSpreadModel::new(yyyymm, initial_fair_bp);
        self.crush_models.push((m, prior_sigma_bp));
    }

    /// Evaluate all spread models against current order books.
    /// Returns all signals, and emits SpreadCrystals for those above z_threshold.
    pub fn tick(
        &mut self,
        books: &[fsr_types::market::OrderBook],
        tick: u64,
    ) -> (Vec<SpreadSignal>, Vec<SpreadCrystal>) {
        if !self.enabled {
            return (vec![], vec![]);
        }

        let qs = QuoteSet::new(books);
        let mut signals = Vec::new();
        let mut crystals = Vec::new();

        for (model, sigma) in &self.calendar_models {
            if let Some(sig) = model.evaluate(&qs, *sigma, tick) {
                if sig.z_score_q32 >= self.z_threshold_q32 {
                    crystals.push(SpreadCrystal {
                        z_threshold_q32: self.z_threshold_q32,
                        signal: sig.clone(),
                    });
                }
                signals.push(sig);
            }
        }

        for (model, sigma) in &mut self.crack_models {
            if let Some(sig) = model.evaluate(&qs, *sigma, tick) {
                model.update_ema(sig.observed_bp);
                if sig.z_score_q32 >= self.z_threshold_q32 {
                    crystals.push(SpreadCrystal {
                        z_threshold_q32: self.z_threshold_q32,
                        signal: sig.clone(),
                    });
                }
                signals.push(sig);
            }
        }

        for (model, sigma) in &mut self.crush_models {
            if let Some(sig) = model.evaluate(&qs, *sigma, tick) {
                model.update_ema(sig.observed_bp);
                if sig.z_score_q32 >= self.z_threshold_q32 {
                    crystals.push(SpreadCrystal {
                        z_threshold_q32: self.z_threshold_q32,
                        signal: sig.clone(),
                    });
                }
                signals.push(sig);
            }
        }

        (signals, crystals)
    }

    pub fn signal_count(&self) -> usize {
        self.calendar_models.len() + self.crack_models.len() + self.crush_models.len()
    }
}

impl Default for SpreadBridge {
    fn default() -> Self {
        Self::new(false)
    }
}

/// Approximate months between two (month, year) pairs.
fn month_distance(m1: u8, y1: u16, m2: u8, y2: u16) -> u8 {
    let total1 = y1 as i32 * 12 + m1 as i32;
    let total2 = y2 as i32 * 12 + m2 as i32;
    (total2 - total1).unsigned_abs() as u8
}
