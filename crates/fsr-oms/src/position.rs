//! Position and P&L ledger.

use fsr_contract::spec::ContractSpec;
use fsr_types::ids::TradingPair;
use fsr_types::market::OrderReceipt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Open position in a single futures contract.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Position {
    pub pair: TradingPair,
    /// Net long (+) / short (-) in lots.
    pub net_lots: i64,
    /// Volume-weighted average fill price in basis-points.
    pub avg_fill_bp: i64,
    /// Realized P&L in cents (accumulated from closed trades).
    pub realized_pnl_cents: i64,
}

impl Position {
    pub fn new(pair: TradingPair) -> Self {
        Position { pair, net_lots: 0, avg_fill_bp: 0, realized_pnl_cents: 0 }
    }

    /// Apply a fill: update net_lots, avg_fill_bp, realized_pnl.
    pub fn apply_fill(&mut self, lots: i64, fill_bp: i64, spec: &ContractSpec) {
        if lots == 0 {
            return;
        }

        let prev_net = self.net_lots;
        let new_net = prev_net + lots;

        if prev_net == 0 || prev_net.signum() == lots.signum() {
            // Opening or adding to an existing position (same direction)
            let total = prev_net.abs() + lots.abs();
            self.avg_fill_bp = (self.avg_fill_bp * prev_net.abs() + fill_bp * lots.abs()) / total;
        } else {
            // Reducing or flipping the position
            let closing_lots = prev_net.abs().min(lots.abs());
            let closing_pnl = spec.pnl_cents(closing_lots * lots.signum(), fill_bp - self.avg_fill_bp);
            self.realized_pnl_cents += closing_pnl;

            let remaining_open = lots.abs() - closing_lots;
            if remaining_open > 0 {
                // Flip: new direction with remaining lots
                self.avg_fill_bp = fill_bp;
            }
        }

        self.net_lots = new_net;
    }

    /// Mark-to-market unrealized P&L in cents.
    pub fn unrealized_pnl_cents(&self, mark_bp: i64, spec: &ContractSpec) -> i64 {
        if self.net_lots == 0 {
            return 0;
        }
        spec.pnl_cents(self.net_lots, mark_bp - self.avg_fill_bp)
    }

    pub fn is_flat(&self) -> bool {
        self.net_lots == 0
    }
}

/// Complete position ledger across all contracts.
pub struct PnlLedger {
    positions: HashMap<TradingPair, Position>,
    /// Spec lookup for P&L calculation.
    specs: HashMap<String, ContractSpec>,
}

impl PnlLedger {
    pub fn new() -> Self {
        PnlLedger {
            positions: HashMap::new(),
            specs: HashMap::new(),
        }
    }

    /// Register a ContractSpec for a root symbol (needed for P&L math).
    pub fn register_spec(&mut self, spec: ContractSpec) {
        self.specs.insert(spec.symbol.clone(), spec);
    }

    /// Apply an OrderReceipt fill to the ledger.
    ///
    /// `pair` is the contract pair (e.g. TradingPair("CL", "202606")).
    /// `side_multiplier` is +1 for Buy, -1 for Sell.
    pub fn apply_receipt(&mut self, pair: &TradingPair, receipt: &OrderReceipt, side_multiplier: i64) {
        self.apply_fill(pair, side_multiplier * receipt.filled_lots as i64, receipt.executed_price_bp);
    }

    /// Apply a fill directly with pair + sign.
    pub fn apply_fill(
        &mut self,
        pair: &TradingPair,
        lots_signed: i64,
        fill_bp: i64,
    ) {
        let spec_key = &pair.0;
        let spec = match self.specs.get(spec_key) {
            Some(s) => s.clone(),
            None => return, // no spec registered → skip
        };
        let pos = self.positions
            .entry(pair.clone())
            .or_insert_with(|| Position::new(pair.clone()));
        pos.apply_fill(lots_signed, fill_bp, &spec);
    }

    /// Total realized P&L across all positions, in cents.
    pub fn total_realized_cents(&self) -> i64 {
        self.positions.values().map(|p| p.realized_pnl_cents).sum()
    }

    /// Total unrealized P&L given a map of mark prices in basis-points.
    pub fn total_unrealized_cents(&self, marks: &HashMap<TradingPair, i64>) -> i64 {
        self.positions.iter().map(|(pair, pos)| {
            if let (Some(mark), Some(spec)) = (marks.get(pair), self.specs.get(&pair.0)) {
                pos.unrealized_pnl_cents(*mark, spec)
            } else {
                0
            }
        }).sum()
    }

    pub fn get_position(&self, pair: &TradingPair) -> Option<&Position> {
        self.positions.get(pair)
    }

    pub fn all_positions(&self) -> impl Iterator<Item = (&TradingPair, &Position)> {
        self.positions.iter()
    }

    /// Number of non-flat positions.
    pub fn open_count(&self) -> usize {
        self.positions.values().filter(|p| !p.is_flat()).count()
    }
}

impl Default for PnlLedger {
    fn default() -> Self {
        Self::new()
    }
}
