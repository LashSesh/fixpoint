//! Dual-Simplex Alpha/Beta cycle computation (spec §2.2.2).
//!
//! Alpha cycle: i→j→k→i (forward triangle).
//! Beta cycle:  i→k→j→i (reverse triangle).
//!
//! p_alpha = ln(r_ij) + ln(r_jk) + ln(r_ki) = D_ijk
//! p_beta  = ln(r_ik) + ln(r_kj) + ln(r_ji) = -D_ijk
//!
//! phase_diff = |p_alpha - p_beta| = 2 * |D_ijk|
//!
//! Axle invariant: net exposure = 0 (see axle.rs).
//! Anti-phase condition: |p_alpha + p_beta| <= epsilon_phi.

use crate::axle::{check_axle_invariant, triangle_legs};
use crate::basket::{CrossRateTensor, DeviationTensor};
use crate::config::DualSimplexConfig;
use fsr_fixed::{ONE, q32_ln};
use fsr_types::Q32;
use serde::{Deserialize, Serialize};

/// A single observation from the dual-simplex on triangle (i, j, k).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimplexObservation {
    pub triangle: (usize, usize, usize),
    /// Alpha cycle log-return: Σ ln(r_ij) around the forward loop.
    pub p_alpha: Q32,
    /// Beta cycle log-return: Σ ln(r_ij) around the reverse loop.
    pub p_beta: Q32,
    /// Phase differential: |p_alpha - p_beta| = 2 * |D_ijk|.
    pub phase_diff: Q32,
    /// True if the axle invariant holds for this observation.
    pub axle_ok: bool,
}

impl SimplexObservation {
    /// Deviation magnitude: half of phase_diff = |D_ijk|.
    pub fn deviation_magnitude(&self) -> Q32 {
        self.phase_diff / 2
    }
}

/// The Dual-Simplex engine: computes Alpha/Beta observations for all triangles.
pub struct DualSimplex {
    pub config: DualSimplexConfig,
}

impl DualSimplex {
    pub fn new(config: DualSimplexConfig) -> Self {
        DualSimplex { config }
    }

    /// Compute observations for all unordered triangles {i,j,k} in the basket.
    ///
    /// Returns one SimplexObservation per unique triangle (i<j<k).
    pub fn observe(
        &self,
        tensor: &CrossRateTensor,
        _deviations: &DeviationTensor,
    ) -> Vec<SimplexObservation> {
        let n = tensor.n;
        let mut observations = Vec::new();

        for i in 0..n {
            for j in (i + 1)..n {
                for k in (j + 1)..n {
                    if let Some(obs) = self.observe_triangle(tensor, i, j, k) {
                        observations.push(obs);
                    }
                }
            }
        }

        observations
    }

    fn observe_triangle(
        &self,
        tensor: &CrossRateTensor,
        i: usize,
        j: usize,
        k: usize,
    ) -> Option<SimplexObservation> {
        let r_ij = tensor.rate(i, j);
        let r_jk = tensor.rate(j, k);
        let r_ki = tensor.rate(k, i);
        let r_ik = tensor.rate(i, k);
        let r_kj = tensor.rate(k, j);
        let r_ji = tensor.rate(j, i);

        if r_ij <= 0 || r_jk <= 0 || r_ki <= 0 || r_ik <= 0 || r_kj <= 0 || r_ji <= 0 {
            return None;
        }

        // Alpha cycle: i→j→k→i
        // p_alpha = ln(r_ij) + ln(r_jk) + ln(r_ki)
        let p_alpha = q32_ln(r_ij).saturating_add(q32_ln(r_jk)).saturating_add(q32_ln(r_ki));

        // Beta cycle: i→k→j→i
        // p_beta = ln(r_ik) + ln(r_kj) + ln(r_ji)
        let p_beta = q32_ln(r_ik).saturating_add(q32_ln(r_kj)).saturating_add(q32_ln(r_ji));

        // Phase differential = |p_alpha - p_beta|
        let phase_diff = (p_alpha - p_beta).abs();

        // Axle invariant check
        let legs = triangle_legs(i, j, k, self.config.notional_per_leg);
        let axle_ok = check_axle_invariant(&legs, tensor.n);

        // Anti-phase condition: |p_alpha + p_beta| <= anti_phase_tolerance
        // p_alpha + p_beta should be ≈ 0 for a proper triangle (log-return round trip ≈ 0)
        let anti_phase_sum = (p_alpha + p_beta).abs();
        let anti_phase_ok = anti_phase_sum <= self.config.anti_phase_tolerance;

        if !anti_phase_ok {
            // If anti-phase condition badly violated, skip (not a valid simplex).
            return None;
        }

        Some(SimplexObservation {
            triangle: (i, j, k),
            p_alpha,
            p_beta,
            phase_diff,
            axle_ok,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basket::CrossRateTensor;
    use crate::config::DualSimplexConfig;
    use fsr_fixed::ONE;

    fn default_config() -> DualSimplexConfig {
        DualSimplexConfig {
            cycle_length: 3,
            notional_per_leg: 1000 * ONE,
            anti_phase_tolerance: ONE / 100, // 1% tolerance
            rebalance_window: 5,
        }
    }

    #[test]
    fn test_axle_invariant_always_satisfied() {
        // No-arb basket: p_alpha ≈ 0, p_beta ≈ 0, axle_ok = true.
        let mids = vec![
            (0, 1, 9200i64),
            (1, 2, 8600i64),
            (0, 2, 7912i64),
        ];
        let tensor = CrossRateTensor::from_mid_prices(&mids, 3, 0);
        let dev = crate::basket::DeviationTensor::from_rates(&tensor);
        let simplex = DualSimplex::new(default_config());
        let obs = simplex.observe(&tensor, &dev);

        assert!(!obs.is_empty(), "should have at least one observation");
        for ob in &obs {
            assert!(ob.axle_ok, "axle invariant must hold for triangle {:?}", ob.triangle);
        }
    }

    #[test]
    fn test_no_arb_phase_diff_near_zero() {
        let mids = vec![
            (0, 1, 9200i64),
            (1, 2, 8600i64),
            (0, 2, 7912i64),
        ];
        let tensor = CrossRateTensor::from_mid_prices(&mids, 3, 0);
        let dev = crate::basket::DeviationTensor::from_rates(&tensor);
        let simplex = DualSimplex::new(default_config());
        let obs = simplex.observe(&tensor, &dev);

        for ob in &obs {
            // In no-arb, |D_ijk| ≈ 0 → phase_diff ≈ 0
            assert!(
                ob.phase_diff < ONE / 50, // < 2%
                "no-arb phase_diff {} should be near 0",
                ob.phase_diff
            );
        }
    }

    #[test]
    fn test_arb_increases_phase_diff() {
        // Inject 10bp arb
        let mids_arb = vec![
            (0, 1, 9200i64),
            (1, 2, 8600i64),
            (0, 2, 7920i64), // mispriced ~10bp
        ];
        let tensor_arb = CrossRateTensor::from_mid_prices(&mids_arb, 3, 0);
        let dev_arb = crate::basket::DeviationTensor::from_rates(&tensor_arb);
        let simplex = DualSimplex::new(default_config());
        let obs_arb = simplex.observe(&tensor_arb, &dev_arb);

        // With arb, some observation should have significant phase_diff
        let max_phase = obs_arb.iter().map(|o| o.phase_diff).max().unwrap_or(0);
        // 10bp = 0.001 * ONE ≈ 4_294_967
        let threshold = ONE / 2000; // 5bp
        assert!(
            max_phase >= threshold,
            "10bp arb should produce phase_diff {} >= {}",
            max_phase,
            threshold
        );
    }
}
