//! fsr-ibkr: Interactive Brokers venue adapter.
//!
//! Default (no feature): IbkrSimAdapter — deterministic commodity futures
//! simulator that generates realistic correlated price movements.
//!
//! Feature "live-data": IbkrLiveAdapter — connects to TWS or IB Gateway
//! running locally on the configured host:port.
//!
//! Both implement fsr_types::market::VenueBroker.

pub mod normalize;
pub mod sim;

pub use sim::IbkrSimAdapter;

#[cfg(feature = "live-data")]
pub mod live;

#[cfg(feature = "live-data")]
pub use live::IbkrLiveAdapter;
