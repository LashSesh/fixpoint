//! DSHAE configuration structs (spec §2.3).

use serde::{Deserialize, Serialize};
use fsr_types::Q32;

/// Operation mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DshaeMode {
    /// Integrated: replaces trumpet expansion as signal source.
    Integrated,
    /// Shadow: runs in parallel but does not gate execution.
    Shadow,
}

/// Basket configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BasketConfig {
    pub min_basket_size: usize,
    pub max_basket_size: usize,
}

/// Dual-simplex cycle configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DualSimplexConfig {
    /// Number of legs per cycle (3 = triangle).
    pub cycle_length: usize,
    /// Notional per leg in Q32.
    pub notional_per_leg: Q32,
    /// Anti-phase tolerance ε_φ in Q32.
    pub anti_phase_tolerance: Q32,
    /// Ticks window to restore net-neutral.
    pub rebalance_window: u64,
}

/// Holographic manifold configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HolographicConfig {
    /// Scale factor for deviation tensor embedding (Q32 multiplier).
    pub q32_scale: Q32,
    /// Minimum HIM points required to run the cascade.
    pub min_points: usize,
}

/// Cascade configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CascadeConfig {
    /// Maximum cascade depth.
    pub max_depth: usize,
    /// DK contraction rate (Q32, 0.85 default).
    pub dk_contraction_rate: Q32,
    /// Press top-k: keep at most k candidates per tick.
    pub press_top_k: usize,
}

/// Crystal formation configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrystalConfig {
    /// Minimum net-edge in basis points (integer).
    pub tau_edge_bp: i64,
    /// Crystal validity window in ticks.
    pub max_age_ticks: u64,
    /// Maximum number of concurrently active crystals.
    pub max_concurrent: usize,
}

/// Master DSHAE configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DshaeConfig {
    pub enabled: bool,
    pub mode: DshaeMode,
    pub basket: BasketConfig,
    pub dual_simplex: DualSimplexConfig,
    pub holographic: HolographicConfig,
    pub cascade: CascadeConfig,
    pub crystal: CrystalConfig,
}

impl Default for DshaeConfig {
    fn default() -> Self {
        use fsr_fixed::{ONE, q32_from_f64_boundary};
        DshaeConfig {
            enabled: false,
            mode: DshaeMode::Shadow,
            basket: BasketConfig {
                min_basket_size: 3,
                max_basket_size: 8,
            },
            dual_simplex: DualSimplexConfig {
                cycle_length: 3,
                notional_per_leg: 1000 * ONE,
                anti_phase_tolerance: 100,
                rebalance_window: 5,
            },
            holographic: HolographicConfig {
                q32_scale: 1_000_000,
                min_points: 4,
            },
            cascade: CascadeConfig {
                max_depth: 3,
                dk_contraction_rate: q32_from_f64_boundary(0.85),
                press_top_k: 16,
            },
            crystal: CrystalConfig {
                tau_edge_bp: 5,
                max_age_ticks: 3,
                max_concurrent: 2,
            },
        }
    }
}
