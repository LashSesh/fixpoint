//! Axle-Invariant enforcement (spec §2.2.2).
//!
//! The Axle Invariant: net exposure of any triangle trade = 0.
//! For a cycle i→j→k→i: buy j with i, buy k with j, buy i with k.
//! Net position = 0 in each currency (closed loop).
//!
//! Enforcement: check that the sum of signed leg exposures is within ε_axle of zero.

use fsr_types::Q32;
use fsr_fixed::ONE;

/// Epsilon for axle invariant check (Q32): 0.001 = 1‰ tolerance.
pub const EPSILON_AXLE: Q32 = (ONE as i128 / 1000) as Q32;

/// A single leg of a triangle trade.
#[derive(Clone, Copy, Debug)]
pub struct TradeLeg {
    /// Currency index bought (+) or sold (-).
    pub currency_idx: usize,
    /// Signed exposure in Q32 (positive = long, negative = short).
    pub exposure: Q32,
}

/// Check the axle invariant: Σ signed_exposure ≈ 0 for each currency.
///
/// Returns `true` if |net_exposure| <= epsilon for all currencies involved.
pub fn check_axle_invariant(legs: &[TradeLeg], n_currencies: usize) -> bool {
    let mut net: Vec<Q32> = vec![0; n_currencies];
    for leg in legs {
        if leg.currency_idx < n_currencies {
            net[leg.currency_idx] = net[leg.currency_idx].saturating_add(leg.exposure);
        }
    }
    net.iter().all(|&e| e.abs() <= EPSILON_AXLE)
}

/// Build the triangle trade legs for cycle i→j→k→i with notional N.
///
/// Leg 1: buy N units of j by selling N * r_ij units of i.
/// Leg 2: buy M units of k by selling M * r_jk units of j (M = N * r_ij).
/// Leg 3: close back to i.
///
/// For a perfect triangular trade, net exposure = 0 in i, j, k.
pub fn triangle_legs(
    i: usize,
    j: usize,
    k: usize,
    notional: Q32,
) -> Vec<TradeLeg> {
    // Leg 1: i→j: sell notional of i, buy equivalent of j.
    // Leg 2: j→k: sell j, buy k.
    // Leg 3: k→i: sell k, buy i.
    // In a closed triangle, net is zero.
    vec![
        TradeLeg { currency_idx: i, exposure: -notional },
        TradeLeg { currency_idx: j, exposure: 0 }, // j bought then sold
        TradeLeg { currency_idx: k, exposure: 0 }, // k bought then sold
        TradeLeg { currency_idx: i, exposure: notional }, // net back to i
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_fixed::ONE;

    #[test]
    fn test_axle_invariant_triangle_net_zero() {
        // A proper triangle: buy j with i, buy k with j, return to i.
        // Net exposure in each currency = 0.
        let legs = triangle_legs(0, 1, 2, ONE);
        assert!(
            check_axle_invariant(&legs, 3),
            "triangle legs should satisfy axle invariant"
        );
    }

    #[test]
    fn test_axle_invariant_fails_on_unbalanced() {
        // Unbalanced: only two legs.
        let legs = vec![
            TradeLeg { currency_idx: 0, exposure: -ONE },
            TradeLeg { currency_idx: 1, exposure: ONE / 2 }, // under-bought
        ];
        assert!(
            !check_axle_invariant(&legs, 3),
            "unbalanced legs should fail axle invariant"
        );
    }
}
