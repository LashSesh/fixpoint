//! fsr-contract: Commodity futures domain layer.
//!
//! Provides ContractSpec (tick/multiplier/settlement metadata), SpreadKind
//! (spread topology definitions), a lightweight Date type, and a default
//! registry for the most-liquid commodity futures.
//!
//! All price arithmetic outside this crate uses price_bp = price_in_native_unit × 100,
//! consistent with the fsr-types OrderBook convention.

pub mod date;
pub mod registry;
pub mod spec;
pub mod spread;

pub use date::{ContractDate, ContractExpiry};
pub use registry::{ContractRegistry, default_registry};
pub use spec::{ContractSpec, Exchange, PriceUnit, Settlement};
pub use spread::{CrackRecipe, CrushRecipe, SpreadDef, SpreadKind};
