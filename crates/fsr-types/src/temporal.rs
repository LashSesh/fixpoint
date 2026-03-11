//! Temporal key and tri-carrier types (spec §17).

use serde::{Deserialize, Serialize};

/// Freshness of a temporal key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Freshness {
    Fresh,
    Stale,
    Expired,
}

/// TemporalKey — mandatory on every consequential event (spec §17.3).
/// (commit_tick, intrinsic_tick, phase_bin, wind_count, freshness)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TemporalKey {
    /// t1: commit clock (microseconds since epoch)
    pub commit_tick: u64,
    /// t2: exploration tick (macro-cycle counter)
    pub intrinsic_tick: u64,
    /// φ bin: phase window index j(x) ∈ {0..M-1}
    pub phase_bin: u16,
    /// w(x): windnarbe winding counter
    pub wind_count: u64,
    /// Freshness assessment
    pub freshness: Freshness,
}

impl TemporalKey {
    pub fn is_valid(&self) -> bool {
        self.freshness != Freshness::Expired
    }
}
