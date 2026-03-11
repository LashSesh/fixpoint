//! Configuration loading and YAML schema (spec §24).
//!
//! Four profiles: conservative (paper default), balanced (live default), aggressive, minimal.
//! Every numeric threshold referenced in gates/filters/scoring must appear in YAML.

use fsr_fixed::{q32_from_f64_boundary, ONE};
use serde::{Deserialize, Serialize};

/// Master configuration (all parameters sourced from YAML, spec §24).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FsrConfig {
    pub profile: ConfigProfile,

    // Gate weights (aψ, aρ, aω, aL)
    pub gate_a_psi: f64,
    pub gate_a_rho: f64,
    pub gate_a_omega: f64,
    pub gate_a_leak: f64,

    // Gate thresholds (θ_open > θ_close mandatory)
    pub theta_open: f64,
    pub theta_close: f64,

    // Trumpet layer weights w_l' (L1..L4, must sum to 1.0)
    pub trumpet_w_l1: f64,
    pub trumpet_w_l2: f64,
    pub trumpet_w_l3: f64,
    pub trumpet_w_l4: f64,

    // Leakage bounds
    pub tau_leak: f64,
    pub tau_xt: f64,

    // PoR parameters
    pub por_rho_min: f64,
    pub por_epsilon: f64,
    pub por_ttl_ticks: u64,

    // Regime thresholds
    pub regime_si_weaken: f64,
    pub regime_gamma_trigger: f64,
    pub regime_si_recovery: f64,
    pub regime_dd_max: f64,

    // Hedge
    pub hedge_dd_max: f64,
    pub hedge_max_leverage: f64,
    pub hedge_fraction: f64,

    // Resonance
    pub psi_amp: f64,
    pub si_max_bp: f64,
    pub fees_bp: f64,
    pub slip_bp: f64,
    pub latency_decay_bp: f64,

    // Calibration
    pub calibration_window: u64,
    pub max_step_per_update: f64,
    pub calibration_rho_min: f64,
    pub calibration_epsilon: f64,

    // Execution
    pub press_top_k: usize,
    pub tau_edge: f64,

    // Resource limits
    pub resource_soft_limit: f64,
    pub resource_hard_limit: f64,

    // Temporal
    pub phase_bins: u16,
    pub omega_drift: f64,
    pub freshness_ttl: u64,
    pub stability_window: u64,
    pub efficiency_window: u64,

    // MCI
    pub max_mci_divergence: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfigProfile {
    Conservative,
    Balanced,
    Aggressive,
    Minimal,
}

impl FsrConfig {
    pub fn conservative() -> Self {
        FsrConfig {
            profile: ConfigProfile::Conservative,
            gate_a_psi: 1.0,
            gate_a_rho: 1.0,
            gate_a_omega: 0.5,
            gate_a_leak: 0.5,
            theta_open: 0.6,
            theta_close: 0.4,
            trumpet_w_l1: 0.4,
            trumpet_w_l2: 0.3,
            trumpet_w_l3: 0.2,
            trumpet_w_l4: 0.1,
            tau_leak: 0.3,
            tau_xt: 0.5,
            por_rho_min: 0.5,
            por_epsilon: 0.05,
            por_ttl_ticks: 10,
            regime_si_weaken: 0.3,
            regime_gamma_trigger: 0.15,
            regime_si_recovery: 0.6,
            regime_dd_max: 0.2,
            hedge_dd_max: 0.2,
            hedge_max_leverage: 2.0,
            hedge_fraction: 0.5,
            psi_amp: 1.0,
            si_max_bp: 100.0,
            fees_bp: 10.0,
            slip_bp: 5.0,
            latency_decay_bp: 2.0,
            calibration_window: 100,
            max_step_per_update: 0.1,
            calibration_rho_min: 0.5,
            calibration_epsilon: 0.05,
            press_top_k: 3,
            tau_edge: 0.0,
            resource_soft_limit: 0.7,
            resource_hard_limit: 0.9,
            phase_bins: 64,
            omega_drift: 0.1,
            freshness_ttl: 10,
            stability_window: 100,
            efficiency_window: 100,
            max_mci_divergence: 0.2,
        }
    }

    pub fn balanced() -> Self {
        let mut c = Self::conservative();
        c.profile = ConfigProfile::Balanced;
        c.theta_open = 0.5;
        c.theta_close = 0.3;
        c.press_top_k = 5;
        c
    }

    pub fn aggressive() -> Self {
        let mut c = Self::balanced();
        c.profile = ConfigProfile::Aggressive;
        c.theta_open = 0.4;
        c.theta_close = 0.2;
        c.press_top_k = 8;
        c.gate_a_psi = 1.5;
        c
    }

    pub fn minimal() -> Self {
        let mut c = Self::conservative();
        c.profile = ConfigProfile::Minimal;
        // Laptop-class: reduced resource limits
        c.resource_soft_limit = 0.5;
        c.resource_hard_limit = 0.7;
        c.press_top_k = 2;
        c
    }

    /// Load from YAML string.
    pub fn from_yaml(yaml: &str) -> Result<Self, String> {
        serde_yaml::from_str(yaml).map_err(|e| e.to_string())
    }

    /// Convert to Q32 gate config.
    pub fn to_kairos_config(&self) -> fsr_gate::KairosConfig {
        fsr_gate::KairosConfig {
            a_psi: q32_from_f64_boundary(self.gate_a_psi),
            a_rho: q32_from_f64_boundary(self.gate_a_rho),
            a_omega: q32_from_f64_boundary(self.gate_a_omega),
            a_leak: q32_from_f64_boundary(self.gate_a_leak),
            theta_open: q32_from_f64_boundary(self.theta_open),
            theta_close: q32_from_f64_boundary(self.theta_close),
        }
    }

    pub fn to_resonance_config(&self) -> fsr_resonance::ResonanceConfig {
        fsr_resonance::ResonanceConfig {
            psi_amp: q32_from_f64_boundary(self.psi_amp),
            si_max: (self.si_max_bp as i64) * ONE,
            fees_bp: q32_from_f64_boundary(self.fees_bp),
            slip_bp: q32_from_f64_boundary(self.slip_bp),
            latency_decay_bp: q32_from_f64_boundary(self.latency_decay_bp),
            stability_window: self.stability_window,
            efficiency_window: self.efficiency_window,
        }
    }

    pub fn to_trumpet_weights(&self) -> fsr_candidates::TrumpetWeights {
        fsr_candidates::TrumpetWeights {
            w: [
                q32_from_f64_boundary(self.trumpet_w_l1),
                q32_from_f64_boundary(self.trumpet_w_l2),
                q32_from_f64_boundary(self.trumpet_w_l3),
                q32_from_f64_boundary(self.trumpet_w_l4),
            ],
        }
    }

    pub fn to_hedge_config(&self) -> fsr_hedge::HedgeConfig {
        fsr_hedge::HedgeConfig {
            dd_max: q32_from_f64_boundary(self.hedge_dd_max),
            max_leverage: q32_from_f64_boundary(self.hedge_max_leverage),
            hedge_fraction: q32_from_f64_boundary(self.hedge_fraction),
        }
    }

    pub fn to_mci_config(&self) -> fsr_mirror::MciConfig {
        fsr_mirror::MciConfig {
            max_mci_divergence: q32_from_f64_boundary(self.max_mci_divergence),
        }
    }

    pub fn to_discovery_config(&self) -> fsr_candidates::DiscoveryConfig {
        fsr_candidates::DiscoveryConfig {
            fee_bp: self.fees_bp as i64,
            slip_bp: self.slip_bp as i64,
            tau_edge: q32_from_f64_boundary(self.tau_edge),
        }
    }

    pub fn to_por_accept_config(&self) -> fsr_mirror::PorAcceptConfig {
        fsr_mirror::PorAcceptConfig {
            rho_min: q32_from_f64_boundary(self.por_rho_min),
            epsilon: q32_from_f64_boundary(self.por_epsilon),
        }
    }
}
