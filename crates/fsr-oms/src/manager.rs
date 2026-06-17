//! OrderManager: central OMS that ties together venue, order FSM, and P&L ledger.

use crate::exec::{ExecError, ExecEvent, ExecutionVenue};
use crate::order::{ManagedOrder, OrderState};
use crate::position::PnlLedger;
use fsr_contract::date::ContractDate;
use fsr_contract::registry::ContractRegistry;
use fsr_types::market::{OrderRequest, OrderReceipt, OrderSide};
use std::collections::HashMap;
use tracing::{info, warn};

/// Pre-trade check results.
#[derive(Debug)]
pub enum PreTradeCheck {
    Approved,
    Blocked(String),
}

/// The central order manager.
///
/// Each macro-cycle tick:
///   1. Call `advance(books)` to drain venue events and update positions.
///   2. Call `submit(req)` to place new orders (with pre-trade checks).
pub struct OrderManager {
    venue: Box<dyn ExecutionVenue>,
    orders: HashMap<u64, ManagedOrder>,
    ledger: PnlLedger,
    registry: ContractRegistry,
    next_oms_id: u64,
    tick: u64,
    /// Maximum margin utilization before blocking new orders (INV-17).
    max_open_orders: usize,
    /// Current system date for FND checks (INV-16).
    /// Updated externally each tick.
    pub today: Option<ContractDate>,
}

impl OrderManager {
    pub fn new(venue: Box<dyn ExecutionVenue>, registry: ContractRegistry) -> Self {
        let mut ledger = PnlLedger::new();
        for sym in registry.symbols() {
            if let Some(spec) = registry.get(sym) {
                ledger.register_spec(spec.clone());
            }
        }
        OrderManager {
            venue,
            orders: HashMap::new(),
            ledger,
            registry,
            next_oms_id: 1,
            tick: 0,
            max_open_orders: 20,
            today: None,
        }
    }

    /// Advance one tick: drain venue events, apply fills to ledger.
    pub fn advance(&mut self, books: &[fsr_types::market::OrderBook]) {
        self.tick += 1;

        // If simulated venue, trigger fills
        // For live venues, fills arrive asynchronously via poll_events.
        // We process events from a common poll.
        let events = self.venue.poll_events();

        for event in events {
            match event {
                ExecEvent::Acked { oms_id, broker_order_id, .. } => {
                    if let Some(order) = self.orders.get_mut(&oms_id) {
                        order.transition_acked(broker_order_id.clone());
                        info!(oms_id, %broker_order_id, "order acked");
                    }
                }
                ExecEvent::PartialFill { oms_id, filled_lots, executed_price_bp, remaining_lots, .. } => {
                    if let Some(order) = self.orders.get_mut(&oms_id) {
                        let sign = match order.request.side {
                            OrderSide::Buy  =>  1i64,
                            OrderSide::Sell => -1i64,
                        };
                        self.ledger.apply_fill(
                            &order.request.pair,
                            sign * filled_lots as i64,
                            executed_price_bp,
                        );
                        order.transition_partial_fill(filled_lots, remaining_lots, executed_price_bp);
                    }
                }
                ExecEvent::Filled { oms_id, ref receipt } => {
                    if let Some(order) = self.orders.get_mut(&oms_id) {
                        let sign = match order.request.side {
                            OrderSide::Buy  =>  1i64,
                            OrderSide::Sell => -1i64,
                        };
                        let pair = order.request.pair.clone();
                        self.ledger.apply_fill(
                            &pair,
                            sign * receipt.filled_lots as i64,
                            receipt.executed_price_bp,
                        );
                        order.transition_filled(receipt.clone(), self.tick);
                        info!(oms_id, price_bp = receipt.executed_price_bp, "order filled");
                    }
                }
                ExecEvent::Rejected { oms_id, reason, .. } => {
                    if let Some(order) = self.orders.get_mut(&oms_id) {
                        order.transition_rejected(reason.clone(), self.tick);
                        warn!(oms_id, %reason, "order rejected");
                    }
                }
                ExecEvent::Cancelled { oms_id, .. } => {
                    if let Some(order) = self.orders.get_mut(&oms_id) {
                        order.transition_cancelled(self.tick);
                        info!(oms_id, "order cancelled");
                    }
                }
            }
        }

        let _ = books; // future: update mark prices for unrealized P&L
    }

    /// Submit a new order with pre-trade checks.
    pub fn submit(&mut self, req: OrderRequest) -> Result<u64, ExecError> {
        match self.pre_trade_check(&req) {
            PreTradeCheck::Blocked(reason) => {
                warn!(%reason, "pre-trade check blocked order");
                return Err(ExecError::Rejected(reason));
            }
            PreTradeCheck::Approved => {}
        }

        let oms_id = self.next_oms_id;
        self.next_oms_id += 1;

        let mut order = ManagedOrder::new(oms_id, req.clone(), self.tick);
        order.transition_submitted();

        self.venue.submit(oms_id, &req)?;
        self.orders.insert(oms_id, order);

        Ok(oms_id)
    }

    /// Cancel an active order.
    pub fn cancel(&mut self, oms_id: u64) -> Result<(), ExecError> {
        if let Some(order) = self.orders.get(&oms_id) {
            if order.state.is_active() {
                return self.venue.cancel(oms_id);
            }
        }
        Ok(())
    }

    /// Run pre-trade checks (INV-16 FND guard, INV-17 open order cap).
    fn pre_trade_check(&self, req: &OrderRequest) -> PreTradeCheck {
        // INV-17: cap on simultaneous open orders
        let open_count = self.orders.values()
            .filter(|o| o.state.is_active())
            .count();
        if open_count >= self.max_open_orders {
            return PreTradeCheck::Blocked(
                format!("open order cap {} reached", self.max_open_orders)
            );
        }

        // INV-16: FND guard for physical contracts
        if let (Some(today), Some(spec)) = (&self.today, self.registry.get(&req.pair.0)) {
            if spec.settlement == fsr_contract::spec::Settlement::Physical {
                if let Some(fnd_day) = spec.fnd_day_of_prior_month {
                    // Expiry month from pair.1 = "YYYYMM"
                    if let Ok(exp_year_month) = parse_yyyymm(&req.pair.1) {
                        // FND is approximately fnd_day of the month prior to expiry
                        let (fnd_year, fnd_month) = prev_month(exp_year_month.0, exp_year_month.1);
                        let fnd = ContractDate::new(fnd_year, fnd_month, fnd_day);
                        if !today.is_before(&fnd) && req.side == OrderSide::Buy {
                            return PreTradeCheck::Blocked(format!(
                                "INV-16: {} {} is at/past FND ({}) — close only",
                                req.pair.0, req.pair.1, fnd
                            ));
                        }
                    }
                }
            }
        }

        PreTradeCheck::Approved
    }

    pub fn ledger(&self) -> &PnlLedger {
        &self.ledger
    }

    pub fn orders(&self) -> &HashMap<u64, ManagedOrder> {
        &self.orders
    }

    pub fn is_connected(&self) -> bool {
        self.venue.is_connected()
    }

    pub fn open_order_count(&self) -> usize {
        self.orders.values().filter(|o| o.state.is_active()).count()
    }
}

fn parse_yyyymm(s: &str) -> Result<(u16, u8), ()> {
    if s.len() < 6 {
        return Err(());
    }
    let year: u16 = s[..4].parse().map_err(|_| ())?;
    let month: u8 = s[4..6].parse().map_err(|_| ())?;
    Ok((year, month))
}

fn prev_month(year: u16, month: u8) -> (u16, u8) {
    if month == 1 {
        (year.saturating_sub(1), 12)
    } else {
        (year, month - 1)
    }
}
