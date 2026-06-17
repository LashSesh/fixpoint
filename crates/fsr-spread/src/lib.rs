//! fsr-spread: Commodity spread signal engine.
//!
//! Computes deviation signals for calendar, crack, and crush spreads.
//! Plugs into the macro-cycle engine as a bridge step (analog to DshaeBridge).
//!
//! The deviation signal D = observed − fair_value is the fundamental trading
//! signal. Positive D → spread is rich (sell); negative D → spread is cheap (buy).

pub mod bridge;
pub mod calendar;
pub mod crack;
pub mod crush;
pub mod model;

pub use bridge::{SpreadBridge, SpreadCrystal};
pub use model::{QuoteSet, SpreadSignal};
