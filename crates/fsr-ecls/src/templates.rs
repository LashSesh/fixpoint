//! Constraint template families (ECLS §4.2).
//!
//! 7 template families:
//!   Band        — |sigma_j - mu_j| <= k*std_j
//!   Ratio       — |sigma_i/sigma_j - ratio| <= epsilon
//!   Correlation — |rho_uv - rho_0| <= delta
//!   Granger     — tau_uv >= tau_min (DEFERRED: computationally expensive)
//!   Spectral    — kappa_uv(f) >= kappa_min (DEFERRED: requires FFT)
//!   Topological — beta_k(VR_epsilon) = expected_value (Betti invariant)
//!   PhaseLock   — |omega_u - omega_v| <= delta_omega

use fsr_types::Q32;
use serde::{Deserialize, Serialize};

/// All constraint template families.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ConstraintTemplate {
    /// Band constraint: |price - mean| <= k_sigma * std.
    Band {
        axis: usize,
        k_sigma: Q32,
    },
    /// Ratio constraint: |price_i / price_j - ratio| <= tolerance.
    Ratio {
        axis_i: usize,
        axis_j: usize,
        ratio: Q32,
        tolerance: Q32,
    },
    /// Correlation constraint: |rho_uv - rho_0| <= delta.
    Correlation {
        rho_target: Q32,
        delta: Q32,
    },
    /// Granger-like causality (DEFERRED to Phase 6).
    /// min_lag_score: kappa_uv(f) >= kappa_min
    Granger {
        min_lag_score: Q32,
    },
    /// Spectral coherence (DEFERRED to Phase 6, requires FFT).
    Spectral {
        frequency_bin: u16,
        min_coherence: Q32,
    },
    /// Topological constraint: Betti number beta_k(VR_epsilon) = expected.
    Topological {
        betti_dim: u8,
        expected_value: u32,
    },
    /// Phase lock: |omega_u - omega_v| <= delta_omega.
    PhaseLock {
        phase_offset: Q32,
        tolerance: Q32,
    },
}

impl ConstraintTemplate {
    /// Whether this template is active in Phase 5 (Granger and Spectral are deferred).
    pub fn is_active_phase5(&self) -> bool {
        !matches!(self, ConstraintTemplate::Granger { .. } | ConstraintTemplate::Spectral { .. })
    }

    pub fn name(&self) -> &'static str {
        match self {
            ConstraintTemplate::Band { .. } => "Band",
            ConstraintTemplate::Ratio { .. } => "Ratio",
            ConstraintTemplate::Correlation { .. } => "Correlation",
            ConstraintTemplate::Granger { .. } => "Granger",
            ConstraintTemplate::Spectral { .. } => "Spectral",
            ConstraintTemplate::Topological { .. } => "Topological",
            ConstraintTemplate::PhaseLock { .. } => "PhaseLock",
        }
    }
}
