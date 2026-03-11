//! Leakage and cross-talk metrics (spec §12.3, TMCP Definitions 11.12-11.14).

use fsr_fixed::{q32_div, q32_mul, EPSILON};
use fsr_types::Q32;

/// Compute reconstruction residual r(x) for a single layer.
/// r_l = x - w_l' * x̂_l where x̂_l = I_l(P_l(x))
/// Returns the component-wise residual magnitude.
pub fn reconstruction_residual(
    market_signals: &[Q32],
    layer_reconstruction: &[Q32],
    weight: Q32,
) -> Q32 {
    if market_signals.len() != layer_reconstruction.len() {
        return 0;
    }
    let mut residual_norm: i128 = 0;
    let mut signal_norm: i128 = 0;
    for (&x, &x_hat) in market_signals.iter().zip(layer_reconstruction.iter()) {
        let w_x_hat = fsr_fixed::q32_mul(weight, x_hat);
        let diff = (x as i128 - w_x_hat as i128).abs();
        residual_norm += diff;
        signal_norm += x.abs() as i128;
    }
    let n = market_signals.len().max(1) as i128;
    let r = (residual_norm / n) as i64;
    let x_norm = (signal_norm / n) as i64;
    // Leak(x) = ||r(x)|| / (||x|| + ε)
    q32_div(r, x_norm.saturating_add(EPSILON))
}

/// Compute Leakage = ||r(x)|| / (||x|| + ε) across all active layers (spec §12.3 eq. 6-7).
/// market_signals: the raw market observation vector.
/// reconstructions: per-layer (weight, reconstruction) pairs.
pub fn compute_leakage(market_signals: &[Q32], reconstructions: &[(Q32, Vec<Q32>)]) -> Q32 {
    if market_signals.is_empty() || reconstructions.is_empty() {
        return 0;
    }
    let n = market_signals.len();
    // Aggregate reconstruction: Σ w_l' * x̂_l
    let mut agg = vec![0i64; n];
    for (w, recon) in reconstructions {
        if recon.len() != n {
            continue;
        }
        for (i, &x_hat) in recon.iter().enumerate() {
            agg[i] = agg[i].saturating_add(q32_mul(*w, x_hat));
        }
    }
    // r(x) = x - agg
    let mut residual_norm: i128 = 0;
    let mut signal_norm: i128 = 0;
    for (i, &x) in market_signals.iter().enumerate() {
        let diff = (x as i128 - agg[i] as i128).abs();
        residual_norm += diff;
        signal_norm += x.abs() as i128;
    }
    let n_i = n as i128;
    let r = (residual_norm / n_i) as i64;
    let x_n = (signal_norm / n_i) as i64;
    q32_div(r, x_n.saturating_add(EPSILON))
}

/// Compute cross-talk XTℓm = |⟨Pℓ(x), Pm(x)⟩| / (||Pℓ(x)|| · ||Pm(x)|| + ε) (spec §12.3 eq. 7).
pub fn compute_cross_talk(layer_a: &[Q32], layer_b: &[Q32]) -> Q32 {
    if layer_a.len() != layer_b.len() || layer_a.is_empty() {
        return 0;
    }
    let n = layer_a.len() as i128;
    // Inner product
    let mut dot: i128 = 0;
    let mut norm_a: i128 = 0;
    let mut norm_b: i128 = 0;
    for (&a, &b) in layer_a.iter().zip(layer_b.iter()) {
        dot += (a as i128 * b as i128) >> 32;
        norm_a += (a.abs() as i128 * a.abs() as i128) >> 32;
        norm_b += (b.abs() as i128 * b.abs() as i128) >> 32;
    }
    let abs_dot = (dot.abs() / n) as i64;
    // Denominator: product of L2 norms + ε
    // sqrt approximation: use avg squared norm as proxy for product, scaled back
    let avg_norm_a = (norm_a / n) as i64;
    let avg_norm_b = (norm_b / n) as i64;
    // product = sqrt(norm_a) * sqrt(norm_b) ≈ sqrt(norm_a * norm_b)
    // Use integer approximation: (norm_a + norm_b) / 2 as upper bound
    let denom_approx = avg_norm_a.saturating_add(avg_norm_b) / 2 + EPSILON;
    q32_div(abs_dot, denom_approx)
}

/// Maximum cross-talk across all layer pairs.
pub fn max_cross_talk(layers: &[Vec<Q32>]) -> Q32 {
    let mut max_xt: Q32 = 0;
    for i in 0..layers.len() {
        for j in (i + 1)..layers.len() {
            let xt = compute_cross_talk(&layers[i], &layers[j]);
            if xt > max_xt {
                max_xt = xt;
            }
        }
    }
    max_xt
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_fixed::{HALF, ONE};

    #[test]
    fn test_leakage_zero_when_perfect_reconstruction() {
        let signals = vec![HALF, ONE, HALF];
        // Perfect reconstruction with weight=1
        let recons = vec![(ONE, signals.clone())];
        let leak = compute_leakage(&signals, &recons);
        assert_eq!(leak, 0, "perfect reconstruction → zero leakage");
    }

    #[test]
    fn test_leakage_nonzero_when_imperfect() {
        let signals = vec![ONE, ONE, ONE];
        // Reconstruction is zero
        let recons = vec![(ONE, vec![0i64, 0i64, 0i64])];
        let leak = compute_leakage(&signals, &recons);
        assert!(leak > 0, "imperfect reconstruction → positive leakage");
    }

    #[test]
    fn test_cross_talk_identical_layers() {
        let layer = vec![ONE, HALF, ONE];
        let xt = compute_cross_talk(&layer, &layer);
        // Identical layers → cross-talk = 1 (fully correlated)
        assert!(xt > HALF, "identical layers → high cross-talk");
    }

    #[test]
    fn test_cross_talk_orthogonal() {
        let a = vec![ONE, 0i64, 0i64];
        let b = vec![0i64, ONE, 0i64];
        let xt = compute_cross_talk(&a, &b);
        assert_eq!(xt, 0, "orthogonal layers → zero cross-talk");
    }
}
