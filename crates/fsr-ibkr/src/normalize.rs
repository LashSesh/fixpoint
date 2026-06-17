//! Price normalization: native unit ↔ basis-points via ContractSpec.

use fsr_contract::spec::ContractSpec;

/// Convert a native-unit price to basis-points.
///
/// price_bp = native_price × 100  (matches fsr-types convention)
pub fn price_to_bp(spec: &ContractSpec, native: f64) -> i64 {
    spec.price_to_bp(native)
}

/// Convert basis-points back to native-unit price (f64).
pub fn bp_to_price(spec: &ContractSpec, bp: i64) -> f64 {
    spec.bp_to_price(bp)
}

/// Snap a price to the nearest valid tick.
///
/// Returns the largest multiple of tick_bp that is ≤ price_bp.
pub fn snap_to_tick(price_bp: i64, tick_bp: i64) -> i64 {
    let tick = tick_bp.max(1);
    (price_bp / tick) * tick
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_contract::registry::default_registry;

    #[test]
    fn cl_round_trip() {
        let r = default_registry();
        let cl = r.get("CL").unwrap();
        let bp = price_to_bp(cl, 75.0);
        assert_eq!(bp, 7500);
        assert!((bp_to_price(cl, bp) - 75.0).abs() < 1e-9);
    }

    #[test]
    fn zc_snap_to_tick() {
        // ZC tick_bp = 25; price 45001 → snaps to 45000
        assert_eq!(snap_to_tick(45001, 25), 45000);
        assert_eq!(snap_to_tick(45025, 25), 45025);
    }
}
