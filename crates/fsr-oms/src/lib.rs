//! fsr-oms: Order Management System.
//!
//! ExecutionVenue trait, order lifecycle FSM, position ledger, and P&L tracking.
//!
//! The OMS sits between the macro-cycle engine and the actual broker (IBKR or
//! paper). It maintains a deterministic record of all orders and fills, feeds
//! into fsr-chain for the audit trail, and enforces pre-trade checks
//! (margin cap, FND guard).

pub mod exec;
pub mod manager;
pub mod order;
pub mod position;

pub use exec::{ExecEvent, ExecutionVenue};
pub use manager::OrderManager;
pub use order::{ManagedOrder, OrderState};
pub use position::{PnlLedger, Position};
