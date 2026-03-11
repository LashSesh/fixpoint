//! fsr-fixed: Q32 fixed-point arithmetic (signed 32.32, i64 backing).
//!
//! All decision-relevant quantities use Q32. Floating-point is forbidden
//! in decision paths (spec §23).
//!
//! Q32 representation: i64 where the integer part occupies the upper 32 bits
//! and the fractional part occupies the lower 32 bits.
//! Value = raw / 2^32

use fsr_types::Q32;

/// 1.0 in Q32 representation.
pub const ONE: Q32 = 1i64 << 32;
/// 0.5 in Q32.
pub const HALF: Q32 = 1i64 << 31;
/// Maximum Q32 value.
pub const Q32_MAX: Q32 = i64::MAX;
/// Minimum Q32 value.
pub const Q32_MIN: Q32 = i64::MIN;
/// Epsilon (smallest positive Q32 value above zero).
pub const EPSILON: Q32 = 1i64;
/// Basis-point multiplier: 1 bp = 0.0001 = 1/10000 in Q32.
pub const BP_ONE: Q32 = (ONE as i128 / 10_000) as i64;

/// Multiply two Q32 values. Panics on overflow in debug.
#[inline]
pub fn q32_mul(a: Q32, b: Q32) -> Q32 {
    // Use i128 intermediate to avoid overflow.
    ((a as i128 * b as i128) >> 32) as i64
}

/// Divide a by b in Q32. Panics if b == 0.
#[inline]
pub fn q32_div(a: Q32, b: Q32) -> Q32 {
    debug_assert!(b != 0, "Q32 division by zero");
    (((a as i128) << 32) / b as i128) as i64
}

/// Saturating multiply (does not panic on overflow).
#[inline]
pub fn q32_mul_sat(a: Q32, b: Q32) -> Q32 {
    let result = (a as i128 * b as i128) >> 32;
    result.clamp(Q32_MIN as i128, Q32_MAX as i128) as i64
}

/// Clamp x to [lo, hi].
#[inline]
pub fn q32_clamp(x: Q32, lo: Q32, hi: Q32) -> Q32 {
    x.clamp(lo, hi)
}

/// Convert a rational n/d to Q32. Returns 0 if d == 0.
#[inline]
pub fn q32_from_ratio(n: i64, d: i64) -> Q32 {
    if d == 0 {
        return 0;
    }
    (((n as i128) << 32) / d as i128) as i64
}

/// Convert Q32 to f64 (for display only — never use in decision paths).
#[inline]
pub fn q32_to_f64_display_only(x: Q32) -> f64 {
    x as f64 / (1u64 << 32) as f64
}

/// Convert f64 to Q32 (for observation boundary conversion only).
#[inline]
pub fn q32_from_f64_boundary(x: f64) -> Q32 {
    (x * (1u64 << 32) as f64) as i64
}

/// tanh approximation in Q32 using rational approximation.
/// Valid for input in Q32 domain.
/// tanh(x) ≈ x*(27+x²)/(27+9x²) for |x| ≤ 3, else ±1.
#[inline]
pub fn q32_tanh(x: Q32) -> Q32 {
    // Clamp to [-3, 3] in Q32
    let three = 3 * ONE;
    if x >= three {
        return ONE;
    }
    if x <= -three {
        return -ONE;
    }
    // x² in Q32
    let x2 = q32_mul(x, x);
    // 27 + x²
    let num_inner = 27 * ONE + x2;
    // numerator = x * (27 + x²)
    let num = q32_mul(x, num_inner);
    // 9 * x²
    let nine_x2 = q32_mul(9 * ONE, x2);
    // 27 + 9*x²
    let den = 27 * ONE + nine_x2;
    q32_div(num, den)
}

/// Natural logarithm approximation in Q32.
/// Uses ln(x) ≈ 2 * (y + y³/3 + y⁵/5) where y = (x-1)/(x+1).
/// Input x must be > 0 (as Q32 > 0).
/// Returns 0 if x <= 0.
pub fn q32_ln(x: Q32) -> Q32 {
    if x <= 0 {
        return i64::MIN / 2; // large negative
    }
    if x == ONE {
        return 0;
    }
    // y = (x - 1) / (x + 1)
    let y = q32_div(x - ONE, x + ONE);
    let y2 = q32_mul(y, y);
    let y3 = q32_mul(y2, y);
    let y5 = q32_mul(q32_mul(y2, y2), y);
    // 2 * (y + y³/3 + y⁵/5)
    let term1 = y;
    let term2 = q32_div(y3, 3 * ONE);
    let term3 = q32_div(y5, 5 * ONE);
    2 * (term1 + term2 + term3)
}

/// Compute Shannon entropy of a slice of Q32 values in [0, ONE].
/// H = -Σ p_i * ln(p_i) where p_i are normalized probabilities.
/// Input values are treated as unnormalized weights.
pub fn q32_entropy(values: &[Q32]) -> Q32 {
    if values.is_empty() {
        return 0;
    }
    let sum: i128 = values.iter().map(|&v| v as i128).sum();
    if sum <= 0 {
        return 0;
    }
    let mut h: i64 = 0;
    for &v in values {
        if v <= 0 {
            continue;
        }
        let p = q32_div(v, sum as i64);
        if p <= 0 {
            continue;
        }
        let ln_p = q32_ln(p);
        h -= q32_mul(p, ln_p);
    }
    h
}

/// Pairwise coherence: 1 - variance / (mean + ε).
/// All inputs in Q32 [0, ONE].
pub fn q32_coherence(values: &[Q32]) -> Q32 {
    if values.is_empty() {
        return ONE;
    }
    let n = values.len() as i64;
    let sum: i64 = values.iter().copied().sum::<i64>().clamp(0, i64::MAX / 2);
    let mean = q32_div(sum, n * ONE);
    if mean <= 0 {
        return 0;
    }
    // variance = Σ(v - mean)² / n
    let mut var_sum: i128 = 0;
    for &v in values {
        let diff = (v - mean) as i128;
        var_sum += diff * diff;
    }
    let variance_raw = var_sum / (n as i128);
    // variance in Q32 (shifted back)
    let variance = (variance_raw >> 32) as i64;
    let ratio = q32_div(variance, mean + EPSILON);
    q32_clamp(ONE - ratio, 0, ONE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_q32_mul_identity() {
        assert_eq!(q32_mul(ONE, ONE), ONE);
        assert_eq!(q32_mul(ONE, 2 * ONE), 2 * ONE);
    }

    #[test]
    fn test_q32_div_identity() {
        assert_eq!(q32_div(ONE, ONE), ONE);
        assert_eq!(q32_div(2 * ONE, 2 * ONE), ONE);
    }

    #[test]
    fn test_q32_clamp() {
        assert_eq!(q32_clamp(-ONE, 0, ONE), 0);
        assert_eq!(q32_clamp(2 * ONE, 0, ONE), ONE);
        assert_eq!(q32_clamp(HALF, 0, ONE), HALF);
    }

    #[test]
    fn test_q32_tanh() {
        let zero = q32_tanh(0);
        assert_eq!(zero, 0);
        let large = q32_tanh(5 * ONE);
        assert_eq!(large, ONE);
        let neg_large = q32_tanh(-5 * ONE);
        assert_eq!(neg_large, -ONE);
    }

    #[test]
    fn test_q32_coherence_identical() {
        let vals = vec![HALF, HALF, HALF, HALF];
        let coh = q32_coherence(&vals);
        // All identical → coherence = 1 (variance = 0)
        assert_eq!(coh, ONE);
    }

    #[test]
    fn test_q32_entropy_uniform() {
        // Uniform distribution has maximum entropy
        let vals = vec![ONE, ONE, ONE, ONE];
        let h = q32_entropy(&vals);
        assert!(h > 0, "uniform distribution should have positive entropy");
    }

    #[test]
    fn test_q32_from_ratio() {
        let half = q32_from_ratio(1, 2);
        assert_eq!(half, HALF);
    }
}
