//! ECLS configuration.

use crate::templates::ConstraintTemplate;
use fsr_fixed::ONE;
use fsr_types::Q32;
use serde::{Deserialize, Serialize};

/// ECLS configuration section (from YAML config).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EclsConfig {
    pub enabled: bool,
    /// Run scanner every N ticks.
    pub scan_interval: u64,
    /// Minimum satisfaction rate to report a constraint (Q32, e.g. 0.80 * ONE).
    pub alpha_min: Q32,
    /// Window size (ticks) for constraint evaluation.
    pub window_size: u64,
    /// Minimum constraints to form a Lattice Crystal.
    pub lattice_min_constraints: usize,
    /// Free energy threshold below which crystals are accepted.
    pub free_energy_threshold: Q32,
    // Template enable flags.
    pub band_enabled: bool,
    pub ratio_enabled: bool,
    pub correlation_enabled: bool,
    pub granger_enabled: bool,
    pub spectral_enabled: bool,
    pub topological_enabled: bool,
    pub phase_lock_enabled: bool,
}

impl Default for EclsConfig {
    fn default() -> Self {
        EclsConfig {
            enabled: true,
            scan_interval: 100,
            alpha_min: ONE * 4 / 5,
            window_size: 500,
            lattice_min_constraints: 2,
            free_energy_threshold: ONE / 2,
            band_enabled: true,
            ratio_enabled: true,
            correlation_enabled: true,
            granger_enabled: false,
            spectral_enabled: false,
            topological_enabled: true,
            phase_lock_enabled: true,
        }
    }
}

impl EclsConfig {
    /// Build the list of active constraint templates.
    pub fn active_templates(&self) -> Vec<ConstraintTemplate> {
        let mut templates = Vec::new();
        if self.band_enabled {
            templates.push(ConstraintTemplate::Band {
                axis: 0,
                k_sigma: ONE * 2,
            });
        }
        if self.ratio_enabled {
            templates.push(ConstraintTemplate::Ratio {
                axis_i: 0,
                axis_j: 1,
                ratio: ONE,
                tolerance: ONE / 10,
            });
        }
        if self.correlation_enabled {
            templates.push(ConstraintTemplate::Correlation {
                rho_target: ONE * 7 / 10,
                delta: ONE / 5,
            });
        }
        if self.topological_enabled {
            templates.push(ConstraintTemplate::Topological {
                betti_dim: 0,
                expected_value: 1,
            });
        }
        if self.phase_lock_enabled {
            templates.push(ConstraintTemplate::PhaseLock {
                phase_offset: 0,
                tolerance: ONE / 10,
            });
        }
        templates
    }
}
