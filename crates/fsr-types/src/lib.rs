//! fsr-types: Shared types, enums, traits, and artifact models for FIXPOINT SWARM-R v3.0.0
//!
//! This crate is the single canonical source of all shared types. No other crate
//! may define canonical enum variants or core structs.

pub mod artifacts;
pub mod errors;
pub mod events;
pub mod fsm;
pub mod ids;
pub mod market;
pub mod temporal;

pub use artifacts::*;
pub use errors::*;
pub use events::*;
pub use fsm::*;
pub use ids::*;
pub use market::*;
pub use temporal::*;

use serde::{Deserialize, Serialize};

/// Q32 fixed-point type (32.32 signed). Arithmetic lives in fsr-fixed.
pub type Q32 = i64;

/// Hash256: 32-byte Blake/SHA256 digest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Hash256(pub [u8; 32]);

impl Hash256 {
    pub const ZERO: Hash256 = Hash256([0u8; 32]);
}

impl std::fmt::Display for Hash256 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for b in &self.0 {
            write!(f, "{:02x}", b)?;
        }
        Ok(())
    }
}
