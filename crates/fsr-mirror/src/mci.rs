//! Mirror Consensus Index (MCI) — measures consistency across venue views.

use fsr_fixed::{q32_clamp, q32_from_ratio, ONE};
use fsr_types::Q32;
use serde::{Deserialize, Serialize};

/// MCI configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MciConfig {
    /// Maximum tolerable MCI divergence (Q32, 0..1)
    pub max_mci_divergence: Q32,
}

impl Default for MciConfig {
    fn default() -> Self {
        MciConfig {
            max_mci_divergence: ONE / 5, // 20%
        }
    }
}

/// Compute MCI: ratio of price agreement across venues.
/// mid_prices: best mid prices from each venue (basis points).
/// Returns MCI in [0, 1] (Q32), where 1 = perfect consensus.
pub fn compute_mci(mid_prices: &[i64]) -> Q32 {
    if mid_prices.is_empty() {
        return ONE; // vacuous consistency
    }
    if mid_prices.len() == 1 {
        return ONE;
    }
    let max_p = *mid_prices.iter().max().unwrap();
    let min_p = *mid_prices.iter().min().unwrap();
    if max_p <= 0 {
        return 0;
    }
    let spread = (max_p - min_p).abs();
    let spread_ratio = q32_from_ratio(spread, max_p);
    q32_clamp(ONE - spread_ratio, 0, ONE)
}

/// Check if MCI meets the mirror gate requirement.
pub fn mirror_gate_pass(mci: Q32, config: &MciConfig) -> bool {
    // MCI must be ≥ (1 - max_mci_divergence)
    mci >= ONE - config.max_mci_divergence
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_fixed::ONE;

    #[test]
    fn test_mci_perfect_consensus() {
        let prices = vec![1000, 1000, 1000];
        let mci = compute_mci(&prices);
        assert_eq!(mci, ONE, "identical prices → MCI = 1");
    }

    #[test]
    fn test_mci_with_spread() {
        let prices = vec![1000, 900];
        let mci = compute_mci(&prices);
        assert!(mci < ONE, "spread → MCI < 1");
        assert!(mci > 0, "non-zero consensus");
    }

    #[test]
    fn test_mirror_gate_pass_high_mci() {
        let cfg = MciConfig::default();
        assert!(mirror_gate_pass(ONE, &cfg));
        assert!(mirror_gate_pass(ONE * 9 / 10, &cfg));
    }

    #[test]
    fn test_mirror_gate_fail_low_mci() {
        let cfg = MciConfig::default();
        assert!(!mirror_gate_pass(ONE / 2, &cfg));
    }
}
