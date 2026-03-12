//! ISLS configuration.

use fsr_fixed::ONE;
use fsr_types::Q32;
use serde::{Deserialize, Serialize};

/// ISLS configuration section (from YAML config).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IslsConfig {
    pub enabled: bool,
    /// Ticks before hot entries are promoted to warm.
    pub hot_retention_ticks: u64,
    /// Days before warm entries are promoted to cold (not enforced in paper mode).
    pub warm_retention_days: u32,
    /// Consensus commit threshold Q32 (default: 0.75 * ONE).
    pub consensus_threshold: Q32,
}

impl Default for IslsConfig {
    fn default() -> Self {
        IslsConfig {
            enabled: true,
            hot_retention_ticks: 3600,
            warm_retention_days: 90,
            consensus_threshold: ONE * 3 / 4,
        }
    }
}
