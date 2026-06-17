//! Order state machine: Pending → Submitted → Acked → [PartialFill…] → Filled|Rejected|Cancelled

use fsr_types::market::{OrderRequest, OrderReceipt};
use serde::{Deserialize, Serialize};

/// States of a managed order lifecycle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderState {
    /// Created locally, not yet submitted to venue.
    Pending,
    /// Submitted to venue, awaiting ack.
    Submitted,
    /// Acknowledged by venue (assigned broker order ID).
    Acked { broker_order_id: String },
    /// Partially filled; waiting for remaining.
    PartialFill { filled_lots: u64, remaining_lots: u64, avg_price_bp: i64 },
    /// Fully filled.
    Filled,
    /// Rejected by venue.
    Rejected { reason: String },
    /// Cancelled (by OMS or venue).
    Cancelled,
}

impl OrderState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, OrderState::Filled | OrderState::Rejected { .. } | OrderState::Cancelled)
    }

    pub fn is_active(&self) -> bool {
        !self.is_terminal()
    }
}

/// An order being managed by the OMS.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ManagedOrder {
    /// OMS-internal ID (monotonically increasing).
    pub oms_id: u64,
    pub request: OrderRequest,
    pub state: OrderState,
    /// Final fill receipt, set when state = Filled.
    pub receipt: Option<OrderReceipt>,
    /// Wall-clock tick at which the order was created.
    pub created_tick: u64,
    /// Wall-clock tick at which terminal state was reached.
    pub completed_tick: Option<u64>,
}

impl ManagedOrder {
    pub fn new(oms_id: u64, request: OrderRequest, created_tick: u64) -> Self {
        ManagedOrder {
            oms_id,
            request,
            state: OrderState::Pending,
            receipt: None,
            created_tick,
            completed_tick: None,
        }
    }

    pub fn transition_submitted(&mut self) {
        if self.state == OrderState::Pending {
            self.state = OrderState::Submitted;
        }
    }

    pub fn transition_acked(&mut self, broker_order_id: String) {
        if matches!(self.state, OrderState::Pending | OrderState::Submitted) {
            self.state = OrderState::Acked { broker_order_id };
        }
    }

    pub fn transition_partial_fill(&mut self, filled: u64, remaining: u64, price_bp: i64) {
        let prev_avg = match &self.state {
            OrderState::PartialFill { avg_price_bp, .. } => *avg_price_bp,
            _ => 0,
        };
        let total_filled = match &self.state {
            OrderState::PartialFill { filled_lots, .. } => *filled_lots + filled,
            _ => filled,
        };
        let new_avg = if total_filled > 0 {
            (prev_avg * (total_filled - filled) as i64 + price_bp * filled as i64)
                / total_filled as i64
        } else {
            price_bp
        };
        self.state = OrderState::PartialFill {
            filled_lots: total_filled,
            remaining_lots: remaining,
            avg_price_bp: new_avg,
        };
    }

    pub fn transition_filled(&mut self, receipt: OrderReceipt, tick: u64) {
        self.receipt = Some(receipt);
        self.state = OrderState::Filled;
        self.completed_tick = Some(tick);
    }

    pub fn transition_rejected(&mut self, reason: String, tick: u64) {
        self.state = OrderState::Rejected { reason };
        self.completed_tick = Some(tick);
    }

    pub fn transition_cancelled(&mut self, tick: u64) {
        self.state = OrderState::Cancelled;
        self.completed_tick = Some(tick);
    }
}
