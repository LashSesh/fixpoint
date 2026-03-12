//! MCCE configuration.

use fsr_fixed::ONE;
use fsr_types::Q32;
use serde::{Deserialize, Serialize};

/// MCCE configuration section (from YAML config).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McceConfig {
    pub enabled: bool,
    /// Auto-discover pairs from exchange adapters.
    pub spore_auto_discover: bool,
    /// Window size (ticks) for rolling Pearson correlation.
    pub hypha_window_ticks: usize,
    /// Minimum absolute Pearson correlation to create an edge (Q32).
    pub hypha_min_rho: Q32,
    /// Decay rate per tick for correlation edges (Q32 fraction).
    pub hypha_decay_rate: Q32,
    /// Interval (ticks) between embedding updates.
    pub embedding_update_interval: u64,
    /// Interval (ticks) between fruiting signal emissions.
    pub fruiting_interval: u64,
}

impl Default for McceConfig {
    fn default() -> Self {
        McceConfig {
            enabled: true,
            spore_auto_discover: true,
            hypha_window_ticks: 500,
            hypha_min_rho: ONE * 3 / 10,       // 0.30 minimum correlation
            hypha_decay_rate: ONE / 200,         // 0.005 decay per tick
            embedding_update_interval: 10,
            fruiting_interval: 100,
        }
    }
}
