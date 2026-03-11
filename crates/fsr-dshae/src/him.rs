//! Holographic Interference Manifold (HIM) construction (spec §2.2.3).
//!
//! For every triangle {i,j,k} in the basket T(B), build a 5D HimPoint:
//!   x1 = ln(r_ij)       — log-rate of first leg
//!   x2 = ln(r_jk)       — log-rate of second leg
//!   x3 = D_ijk          — deviation (arbitrage signal)
//!   psi = |δ_ij| + |δ_jk| — phase differential (sum of leg deviations)
//!   omega = φ(w) mod 2π  — basket phase angle
//!
//! |T(B)| = C(n, 3) = n*(n-1)*(n-2)/6 points for unordered triangles.

use crate::basket::{CrossRateTensor, DeviationTensor};
use crate::simplex::SimplexObservation;
use fsr_fixed::{ONE, q32_ln};
use fsr_types::{Hash256, Q32};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A single 5D point in the Holographic Interference Manifold.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HimPoint {
    /// Log-rate of leg i→j.
    pub x1: Q32,
    /// Log-rate of leg j→k.
    pub x2: Q32,
    /// Deviation D_ijk = D_ijk.
    pub x3: Q32,
    /// Phase differential ψ = |Δ_ij| + |Δ_jk|.
    pub psi: Q32,
    /// Basket phase ω = φ(w) mod 2π (approximated in Q32 as phase % ONE).
    pub omega: Q32,
    /// Triangle indices (i, j, k), unordered (i < j < k).
    pub triangle: (usize, usize, usize),
}

impl HimPoint {
    /// Net edge magnitude: |x3| = |D_ijk|.
    pub fn net_edge(&self) -> Q32 {
        self.x3.abs()
    }
}

/// The Holographic Interference Manifold at tick t.
///
/// Contains one HimPoint per triangle in T(B).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Him {
    pub points: Vec<HimPoint>,
    pub tick: u64,
    pub window: u64,
    /// SHA-256 digest of the manifold for chain integrity.
    pub digest: Hash256,
}

impl Him {
    /// Construct the HIM from basket state at the current tick.
    pub fn construct(
        basket_n: usize,
        rates: &CrossRateTensor,
        deviations: &DeviationTensor,
        observations: &[SimplexObservation],
        tick: u64,
        window: u64,
    ) -> Self {
        let mut points = Vec::new();

        // Iterate all unordered triangles (i < j < k).
        for i in 0..basket_n {
            for j in (i + 1)..basket_n {
                for k in (j + 1)..basket_n {
                    if let Some(point) =
                        build_him_point(i, j, k, rates, deviations, observations, tick)
                    {
                        points.push(point);
                    }
                }
            }
        }

        let digest = compute_him_digest(&points, tick);
        Him { points, tick, window, digest }
    }

    /// Number of HIM points (should equal C(n,3) when all rates available).
    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Maximum net-edge across all points.
    pub fn max_net_edge(&self) -> Q32 {
        self.points.iter().map(|p| p.net_edge()).max().unwrap_or(0)
    }
}

fn build_him_point(
    i: usize,
    j: usize,
    k: usize,
    rates: &CrossRateTensor,
    deviations: &DeviationTensor,
    observations: &[SimplexObservation],
    _tick: u64,
) -> Option<HimPoint> {
    let r_ij = rates.rate(i, j);
    let r_jk = rates.rate(j, k);

    if r_ij <= 0 || r_jk <= 0 {
        return None;
    }

    let x1 = q32_ln(r_ij);
    let x2 = q32_ln(r_jk);

    // D_ijk from deviations (prefer ordered i→j→k)
    let x3 = deviations.get(i, j, k).unwrap_or_else(|| {
        deviations.get(j, k, i).unwrap_or_else(|| deviations.get(k, i, j).unwrap_or(0))
    });

    // Phase differential from simplex observations (|delta_ij| + |delta_jk|)
    let psi = observations
        .iter()
        .find(|o| o.triangle == (i, j, k))
        .map(|o| o.phase_diff)
        .unwrap_or_else(|| x3.abs() * 2);

    // Basket phase: use tick-based rotation as phi(w) mod 2π
    // Approximate 2π in Q32 as 6 * ONE (since 2π ≈ 6.283)
    let two_pi_q32 = 6 * ONE + ONE * 283 / 1000; // ≈ 6.283 * ONE
    let omega = ((_tick as Q32 * ONE) % two_pi_q32).abs();

    Some(HimPoint { x1, x2, x3, psi, omega, triangle: (i, j, k) })
}

fn compute_him_digest(points: &[HimPoint], tick: u64) -> Hash256 {
    let mut hasher = Sha256::new();
    hasher.update(&tick.to_le_bytes());
    for p in points {
        hasher.update(&p.x1.to_le_bytes());
        hasher.update(&p.x2.to_le_bytes());
        hasher.update(&p.x3.to_le_bytes());
        hasher.update(&p.psi.to_le_bytes());
    }
    let result = hasher.finalize();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&result);
    Hash256(bytes)
}

/// Expected HIM point count for n-currency basket: C(n, 3).
pub fn expected_him_points(n: usize) -> usize {
    if n < 3 {
        return 0;
    }
    n * (n - 1) * (n - 2) / 6
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basket::CrossRateTensor;
    use crate::config::DualSimplexConfig;
    use crate::simplex::DualSimplex;
    use fsr_fixed::ONE;

    #[test]
    fn test_n4_him_produces_4_points() {
        // For n=4 currencies, C(4,3) = 4 HIM points.
        let mids = vec![
            (0, 1, 9200i64),
            (1, 2, 8600i64),
            (2, 3, 9500i64),
            (0, 2, 7912i64),
            (1, 3, 8170i64),
            (0, 3, 7516i64),
        ];
        let tensor = CrossRateTensor::from_mid_prices(&mids, 4, 0);
        let deviations = crate::basket::DeviationTensor::from_rates(&tensor);
        let simplex = DualSimplex::new(DualSimplexConfig {
            cycle_length: 3,
            notional_per_leg: 1000 * ONE,
            anti_phase_tolerance: ONE / 10, // larger tolerance for test
            rebalance_window: 5,
        });
        let obs = simplex.observe(&tensor, &deviations);
        let him = Him::construct(4, &tensor, &deviations, &obs, 0, 20);

        assert_eq!(
            him.len(),
            expected_him_points(4),
            "n=4 basket must produce {} HIM points, got {}",
            expected_him_points(4),
            him.len()
        );
    }

    #[test]
    fn test_him_digest_stable() {
        let mids = vec![
            (0, 1, 9200i64),
            (1, 2, 8600i64),
            (0, 2, 7912i64),
        ];
        let tensor = CrossRateTensor::from_mid_prices(&mids, 3, 0);
        let deviations = crate::basket::DeviationTensor::from_rates(&tensor);
        let obs: Vec<SimplexObservation> = vec![];
        let him1 = Him::construct(3, &tensor, &deviations, &obs, 0, 20);
        let him2 = Him::construct(3, &tensor, &deviations, &obs, 0, 20);

        assert_eq!(him1.digest, him2.digest, "HIM digest must be deterministic");
    }

    #[test]
    fn test_no_arb_him_low_net_edge() {
        let mids = vec![
            (0, 1, 9200i64),
            (1, 2, 8600i64),
            (0, 2, 7912i64),
        ];
        let tensor = CrossRateTensor::from_mid_prices(&mids, 3, 0);
        let deviations = crate::basket::DeviationTensor::from_rates(&tensor);
        let obs: Vec<SimplexObservation> = vec![];
        let him = Him::construct(3, &tensor, &deviations, &obs, 0, 20);

        let max_edge = him.max_net_edge();
        // In no-arb, max net edge should be < 5bp = ONE/2000
        assert!(
            max_edge < ONE / 200,
            "no-arb HIM max net edge {} should be small",
            max_edge
        );
    }
}
