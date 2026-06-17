//! ExecutionVenue trait and ExecEvent — the boundary to the actual broker.

use fsr_types::ids::{OrderId, TradingPair, VenueId};
use fsr_types::market::{OrderRequest, OrderReceipt};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors from the execution venue.
#[derive(Debug, Error, Clone, Serialize, Deserialize)]
pub enum ExecError {
    #[error("not connected to broker: {0}")]
    NotConnected(String),
    #[error("order rejected by venue: {0}")]
    Rejected(String),
    #[error("insufficient margin")]
    InsufficientMargin,
    #[error("FND guard: contract {0}/{1} is at or past First Notice Day")]
    FndGuard(String, String),
    #[error("rate limited")]
    RateLimited,
    #[error("internal error: {0}")]
    Internal(String),
}

/// An event emitted by the execution venue after order submission.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ExecEvent {
    /// Order acknowledged by venue (assigned broker_order_id).
    Acked {
        oms_id: u64,
        broker_order_id: String,
        timestamp_us: u64,
    },
    /// Partial fill.
    PartialFill {
        oms_id: u64,
        filled_lots: u64,
        executed_price_bp: i64,
        remaining_lots: u64,
        timestamp_us: u64,
    },
    /// Full fill — order is complete.
    Filled {
        oms_id: u64,
        receipt: OrderReceipt,
    },
    /// Order rejected by the venue.
    Rejected {
        oms_id: u64,
        reason: String,
        timestamp_us: u64,
    },
    /// Order cancelled (by OMS request or venue).
    Cancelled {
        oms_id: u64,
        timestamp_us: u64,
    },
}

/// Trait for execution venues (IBKR live, IBKR paper, simulated).
///
/// Thread-safety is NOT required — the OMS calls this synchronously from the
/// single macro-cycle thread. Async venues use an internal channel.
pub trait ExecutionVenue: Send {
    /// Submit an order. Returns the OMS-assigned order ID on acceptance.
    /// Pre-trade checks (margin, FND) should be done BEFORE calling this.
    fn submit(&mut self, oms_id: u64, req: &OrderRequest) -> Result<(), ExecError>;

    /// Request cancellation of an open order.
    fn cancel(&mut self, oms_id: u64) -> Result<(), ExecError>;

    /// Drain all pending events since the last call (non-blocking).
    fn poll_events(&mut self) -> Vec<ExecEvent>;

    /// Is the venue connection healthy?
    fn is_connected(&self) -> bool;
}

/// Simulated execution venue — fills immediately at mid price with slippage.
pub struct SimulatedVenue {
    pending: Vec<(u64, OrderRequest)>,
    events: Vec<ExecEvent>,
    tick: u64,
    /// Slippage in basis-points applied to each fill.
    slippage_bp: i64,
    next_broker_id: u64,
}

impl SimulatedVenue {
    pub fn new(slippage_bp: i64) -> Self {
        SimulatedVenue {
            pending: Vec::new(),
            events: Vec::new(),
            tick: 0,
            slippage_bp,
            next_broker_id: 1,
        }
    }

    /// Advance the simulation clock, filling all pending orders immediately.
    pub fn advance(&mut self, current_books: &[fsr_types::market::OrderBook]) {
        self.tick += 1;
        let ts = self.tick * 1_000_000;

        let pending = std::mem::take(&mut self.pending);
        for (oms_id, req) in pending {
            // Find matching book for execution price
            let exec_price = current_books
                .iter()
                .find(|b| b.venue == req.venue && b.pair == req.pair)
                .and_then(|b| match req.side {
                    fsr_types::market::OrderSide::Buy  => b.best_ask_bp(),
                    fsr_types::market::OrderSide::Sell => b.best_bid_bp(),
                })
                .unwrap_or_else(|| req.limit_price_bp.unwrap_or(10000));

            let slip = match req.side {
                fsr_types::market::OrderSide::Buy  =>  self.slippage_bp,
                fsr_types::market::OrderSide::Sell => -self.slippage_bp,
            };

            let broker_id = self.next_broker_id.to_string();
            self.next_broker_id += 1;

            self.events.push(ExecEvent::Acked {
                oms_id,
                broker_order_id: broker_id,
                timestamp_us: ts,
            });
            self.events.push(ExecEvent::Filled {
                oms_id,
                receipt: OrderReceipt {
                    order_id: OrderId(oms_id),
                    venue: req.venue.clone(),
                    filled_lots: req.quantity_lots,
                    executed_price_bp: exec_price + slip,
                    timestamp_us: ts,
                },
            });
        }
    }
}

impl ExecutionVenue for SimulatedVenue {
    fn submit(&mut self, oms_id: u64, req: &OrderRequest) -> Result<(), ExecError> {
        self.pending.push((oms_id, req.clone()));
        Ok(())
    }

    fn cancel(&mut self, oms_id: u64) -> Result<(), ExecError> {
        let before = self.pending.len();
        self.pending.retain(|(id, _)| *id != oms_id);
        if self.pending.len() < before {
            self.events.push(ExecEvent::Cancelled {
                oms_id,
                timestamp_us: self.tick * 1_000_000,
            });
        }
        Ok(())
    }

    fn poll_events(&mut self) -> Vec<ExecEvent> {
        std::mem::take(&mut self.events)
    }

    fn is_connected(&self) -> bool {
        true
    }
}
