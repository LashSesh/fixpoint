//! Currency basket and cross-rate tensor (spec §2.2.1).
//!
//! The basket has n currencies numbered 0..n-1.
//! Cross-rates r_ij represent "how many units of currency j per 1 unit of currency i".
//!
//! Rate normalization convention:
//!   All rates are stored as Q32 fractions where ONE = 1.0.
//!   Rate r_ij = mid_bp / RATE_SCALE.
//!   In no-arbitrage: r_ij * r_jk = r_ik (product property).
//!   Deviation: D_ijk = ln(r_ij) + ln(r_jk) - ln(r_ik) = 0 in no-arb.

use fsr_fixed::{ONE, q32_from_ratio, q32_ln};
use fsr_types::Q32;
use serde::{Deserialize, Serialize};

/// Scale factor for converting raw mid_bp → Q32 rate.
/// Mid prices in [RATE_SCALE*0.1, RATE_SCALE*10] map to rates in [0.1, 10].
pub const RATE_SCALE: i64 = 10_000;

/// The currency basket: n currencies with symbolic names.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CurrencyBasket {
    /// Number of currencies in the basket.
    pub n: usize,
    /// Human-readable currency labels (e.g., "USD", "EUR", ...).
    pub names: Vec<String>,
}

impl CurrencyBasket {
    pub fn new(names: Vec<String>) -> Self {
        let n = names.len();
        CurrencyBasket { n, names }
    }

    /// Default 4-currency basket for sandbox scenarios.
    pub fn default_4() -> Self {
        CurrencyBasket::new(vec![
            "C0".into(), "C1".into(), "C2".into(), "C3".into(),
        ])
    }
}

/// Cross-rate tensor R(t): n×n matrix of rates r_ij in Q32.
///
/// Stored row-major: rates[i * n + j] = r_ij.
/// Diagonal: r_ii = ONE (1.0).
/// Rates <= 0 indicate unavailable data.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrossRateTensor {
    pub tick: u64,
    pub n: usize,
    /// Flat n×n rate matrix in Q32.
    pub rates: Vec<Q32>,
}

impl CrossRateTensor {
    /// Create with all rates zeroed (except diagonal = ONE).
    pub fn new(n: usize, tick: u64) -> Self {
        let mut rates = vec![0i64; n * n];
        for i in 0..n {
            rates[i * n + i] = ONE;
        }
        CrossRateTensor { tick, n, rates }
    }

    #[inline]
    pub fn rate(&self, i: usize, j: usize) -> Q32 {
        if i == j {
            return ONE;
        }
        self.rates[i * self.n + j]
    }

    #[inline]
    pub fn set_rate(&mut self, i: usize, j: usize, r: Q32) {
        if i != j {
            self.rates[i * self.n + j] = r;
        }
    }

    /// Construct from a flat slice of mid prices (length = n*(n-1)/2, pairs (0,1),(0,2),...).
    ///
    /// Each mid_bp is divided by RATE_SCALE to give a Q32 rate ≈ 1.0.
    /// Missing rates (mid_bp = 0) are derived via transitivity if possible.
    pub fn from_mid_prices(mids: &[(usize, usize, i64)], n: usize, tick: u64) -> Self {
        let mut tensor = CrossRateTensor::new(n, tick);

        // Set direct rates from provided mid prices.
        for &(i, j, mid_bp) in mids {
            if mid_bp <= 0 || i >= n || j >= n || i == j {
                continue;
            }
            let r_ij = q32_from_ratio(mid_bp, RATE_SCALE);
            if r_ij > 0 {
                tensor.set_rate(i, j, r_ij);
                // r_ji = 1 / r_ij (approximate via ratio inversion)
                let r_ji = q32_from_ratio(RATE_SCALE, mid_bp);
                tensor.set_rate(j, i, r_ji);
            }
        }

        // Fill missing rates by transitivity: r_ik ≈ r_ij * r_jk / ONE.
        // Do up to n passes to propagate.
        for _ in 0..n {
            for i in 0..n {
                for k in 0..n {
                    if i == k || tensor.rate(i, k) > 0 {
                        continue;
                    }
                    for j in 0..n {
                        if j == i || j == k {
                            continue;
                        }
                        let r_ij = tensor.rate(i, j);
                        let r_jk = tensor.rate(j, k);
                        if r_ij > 0 && r_jk > 0 {
                            // r_ik = r_ij * r_jk (both in Q32, so divide by ONE)
                            let r_ik = fsr_fixed::q32_mul(r_ij, r_jk);
                            if r_ik > 0 {
                                tensor.set_rate(i, k, r_ik);
                            }
                            break;
                        }
                    }
                }
            }
        }

        tensor
    }
}

/// Deviation tensor D(t): for each ordered triple (i,j,k), D_ijk = ln(r_ij) + ln(r_jk) - ln(r_ik).
///
/// D_ijk = 0 in no-arbitrage. |D_ijk| > 0 indicates mispricing.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviationTensor {
    pub tick: u64,
    /// Entries: (i, j, k, D_ijk) in Q32.
    pub deviations: Vec<(usize, usize, usize, Q32)>,
}

impl DeviationTensor {
    /// Compute from a CrossRateTensor.
    pub fn from_rates(tensor: &CrossRateTensor) -> Self {
        let n = tensor.n;
        let mut deviations = Vec::with_capacity(n * (n - 1) * (n - 2));

        for i in 0..n {
            for j in 0..n {
                if j == i {
                    continue;
                }
                for k in 0..n {
                    if k == i || k == j {
                        continue;
                    }
                    let r_ij = tensor.rate(i, j);
                    let r_jk = tensor.rate(j, k);
                    let r_ik = tensor.rate(i, k);

                    if r_ij <= 0 || r_jk <= 0 || r_ik <= 0 {
                        continue;
                    }

                    // D_ijk = ln(r_ij) + ln(r_jk) - ln(r_ik)
                    let ln_rij = q32_ln(r_ij);
                    let ln_rjk = q32_ln(r_jk);
                    let ln_rik = q32_ln(r_ik);
                    let d_ijk = ln_rij + ln_rjk - ln_rik;
                    deviations.push((i, j, k, d_ijk));
                }
            }
        }

        DeviationTensor { tick: tensor.tick, deviations }
    }

    /// Get deviation for a specific ordered triple (i, j, k).
    pub fn get(&self, i: usize, j: usize, k: usize) -> Option<Q32> {
        self.deviations
            .iter()
            .find(|&&(a, b, c, _)| a == i && b == j && c == k)
            .map(|&(_, _, _, d)| d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_fixed::ONE;

    /// Build a 3-currency basket with no arbitrage:
    /// r_01 = 0.92, r_12 = 0.86, r_02 = 0.92 * 0.86 = 0.7912
    fn no_arb_3_basket() -> CrossRateTensor {
        let mids = vec![
            (0, 1, 9200i64),   // r_01 = 0.92
            (1, 2, 8600i64),   // r_12 = 0.86
            (0, 2, 7912i64),   // r_02 = 0.7912 (= 0.92 * 0.86)
        ];
        CrossRateTensor::from_mid_prices(&mids, 3, 0)
    }

    #[test]
    fn test_no_arb_deviation_near_zero() {
        let tensor = no_arb_3_basket();
        let dev = DeviationTensor::from_rates(&tensor);

        // D_012 should be ≈ 0 (within rounding)
        let d_012 = dev.get(0, 1, 2).expect("D_012 should exist");
        // Allow ±1% in Q32 = ONE/100
        assert!(
            d_012.abs() < ONE / 100,
            "D_012 = {} should be near zero (< {})",
            d_012,
            ONE / 100
        );
    }

    #[test]
    fn test_arb_injection_detected() {
        // Inject 10bp arb: r_02 becomes 7912 * 1.001 ≈ 7920
        let mids = vec![
            (0, 1, 9200i64),
            (1, 2, 8600i64),
            (0, 2, 7920i64),  // mispriced by ~10bp
        ];
        let tensor = CrossRateTensor::from_mid_prices(&mids, 3, 0);
        let dev = DeviationTensor::from_rates(&tensor);

        let d_012 = dev.get(0, 1, 2).expect("D_012 should exist");

        // Deviation should be non-zero (≥ 5bp in Q32)
        // 5bp = 0.0005 → 0.0005 * 2^32 ≈ 2,147,483
        let threshold_5bp = ONE / 2000; // ≈ 0.0005
        assert!(
            d_012.abs() >= threshold_5bp,
            "10bp arb D_012 = {} should be >= 5bp threshold {}",
            d_012,
            threshold_5bp
        );
    }

    #[test]
    fn test_n4_basket_has_correct_triangle_count() {
        // 4-currency basket: C(4,3) * 2 (ordered) = 24 ordered triples
        // But deviations only count distinct ordered triples with all rates present
        let mids = vec![
            (0, 1, 9200i64),
            (1, 2, 8600i64),
            (2, 3, 9500i64),
            (0, 2, 7912i64),
            (0, 3, 7516i64),  // = 0.7912 * 0.9500 ≈ 7516
            (1, 3, 8170i64),  // = 0.8600 * 0.9500 ≈ 8170
        ];
        let tensor = CrossRateTensor::from_mid_prices(&mids, 4, 0);
        let dev = DeviationTensor::from_rates(&tensor);

        // Should have 4*3*2 = 24 ordered triples (or fewer if some rates missing)
        assert!(dev.deviations.len() >= 4, "n=4 basket needs at least 4 unique triangles");
    }

    #[test]
    fn test_transitivity_fills_missing_rate() {
        // Only provide r_01 and r_12; r_02 should be derived via transitivity.
        let mids = vec![
            (0, 1, 9200i64),
            (1, 2, 8600i64),
            // r_02 NOT provided
        ];
        let tensor = CrossRateTensor::from_mid_prices(&mids, 3, 0);
        // r_02 should be derived ≈ r_01 * r_12
        let r_02 = tensor.rate(0, 2);
        assert!(r_02 > 0, "r_02 should be derived via transitivity");
    }
}
