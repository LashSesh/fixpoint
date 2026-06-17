//! Default ContractSpec registry for major commodity futures.
//!
//! Tick values and lot sizes are from official CME/NYMEX exchange specs.
//! FND/LTD day approximations are for reference — use exchange calendar for
//! production.

use crate::spec::{ContractSpec, Exchange, PriceUnit, Settlement};
use std::collections::HashMap;

/// Registry mapping root symbol → ContractSpec.
pub struct ContractRegistry {
    specs: HashMap<String, ContractSpec>,
}

impl ContractRegistry {
    pub fn new() -> Self {
        ContractRegistry { specs: HashMap::new() }
    }

    pub fn insert(&mut self, spec: ContractSpec) {
        self.specs.insert(spec.symbol.clone(), spec);
    }

    pub fn get(&self, symbol: &str) -> Option<&ContractSpec> {
        self.specs.get(symbol)
    }

    pub fn symbols(&self) -> impl Iterator<Item = &str> {
        self.specs.keys().map(|s| s.as_str())
    }
}

impl Default for ContractRegistry {
    fn default() -> Self {
        default_registry()
    }
}

/// Build a registry with the most-liquid commodity futures.
pub fn default_registry() -> ContractRegistry {
    let mut r = ContractRegistry::new();

    // ── Energy ────────────────────────────────────────────────────────────────

    r.insert(ContractSpec {
        symbol: "CL".into(),
        exchange: Exchange::NYMEX,
        price_unit: PriceUnit::UsdPerBbl,
        currency: "USD".into(),
        tick_bp: 1,          // $0.01/bbl
        tick_value_cents: 1000, // $10.00
        lot_size: 1000,
        settlement: Settlement::Physical,
        ltd_day_of_prior_month: 20,
        fnd_day_of_prior_month: Some(25),
    });

    r.insert(ContractSpec {
        symbol: "NG".into(),
        exchange: Exchange::NYMEX,
        price_unit: PriceUnit::UsdPerMmBtu,
        currency: "USD".into(),
        tick_bp: 1,          // $0.001 rounds to 1bp floor
        tick_value_cents: 1000, // $10.00 per $0.001 move × 10000 MMBtu
        lot_size: 10000,
        settlement: Settlement::Physical,
        ltd_day_of_prior_month: 28,
        fnd_day_of_prior_month: Some(28),
    });

    r.insert(ContractSpec {
        symbol: "RB".into(),   // RBOB Gasoline
        exchange: Exchange::NYMEX,
        price_unit: PriceUnit::UsdCentsPerGallon,
        currency: "USD".into(),
        tick_bp: 1,            // $0.0001/gal → 0.01¢/gal → 1bp floor
        tick_value_cents: 420, // $0.0001 × 42000 gal = $4.20
        lot_size: 42000,
        settlement: Settlement::Physical,
        ltd_day_of_prior_month: 31,
        fnd_day_of_prior_month: Some(31),
    });

    r.insert(ContractSpec {
        symbol: "HO".into(),   // Heating Oil / ULSD
        exchange: Exchange::NYMEX,
        price_unit: PriceUnit::UsdCentsPerGallon,
        currency: "USD".into(),
        tick_bp: 1,
        tick_value_cents: 420, // $0.0001 × 42000 gal = $4.20
        lot_size: 42000,
        settlement: Settlement::Physical,
        ltd_day_of_prior_month: 31,
        fnd_day_of_prior_month: Some(31),
    });

    // ── Metals ────────────────────────────────────────────────────────────────

    r.insert(ContractSpec {
        symbol: "GC".into(),   // Gold
        exchange: Exchange::COMEX,
        price_unit: PriceUnit::UsdPerTroyOz,
        currency: "USD".into(),
        tick_bp: 10,           // $0.10/oz
        tick_value_cents: 1000, // $0.10 × 100 oz = $10.00
        lot_size: 100,
        settlement: Settlement::Physical,
        ltd_day_of_prior_month: 28,
        fnd_day_of_prior_month: Some(28),
    });

    r.insert(ContractSpec {
        symbol: "SI".into(),   // Silver
        exchange: Exchange::COMEX,
        price_unit: PriceUnit::UsdCentsPerLb,  // actually ¢/troy oz
        currency: "USD".into(),
        tick_bp: 50,           // $0.005/oz = 0.5¢/oz
        tick_value_cents: 2500, // $0.005 × 5000 oz = $25.00
        lot_size: 5000,
        settlement: Settlement::Physical,
        ltd_day_of_prior_month: 28,
        fnd_day_of_prior_month: Some(28),
    });

    // ── Grains ────────────────────────────────────────────────────────────────

    r.insert(ContractSpec {
        symbol: "ZC".into(),   // Corn
        exchange: Exchange::CBOT,
        price_unit: PriceUnit::UsdCentsPerBushel,
        currency: "USD".into(),
        tick_bp: 25,           // 0.25¢/bu × 100 = 25bp
        tick_value_cents: 1250, // 0.25¢ × 5000 bu = $12.50
        lot_size: 5000,
        settlement: Settlement::Physical,
        ltd_day_of_prior_month: 14,
        fnd_day_of_prior_month: Some(1),
    });

    r.insert(ContractSpec {
        symbol: "ZS".into(),   // Soybeans
        exchange: Exchange::CBOT,
        price_unit: PriceUnit::UsdCentsPerBushel,
        currency: "USD".into(),
        tick_bp: 25,           // 0.25¢/bu
        tick_value_cents: 1250,
        lot_size: 5000,
        settlement: Settlement::Physical,
        ltd_day_of_prior_month: 14,
        fnd_day_of_prior_month: Some(1),
    });

    r.insert(ContractSpec {
        symbol: "ZW".into(),   // Wheat (CBOT)
        exchange: Exchange::CBOT,
        price_unit: PriceUnit::UsdCentsPerBushel,
        currency: "USD".into(),
        tick_bp: 25,
        tick_value_cents: 1250,
        lot_size: 5000,
        settlement: Settlement::Physical,
        ltd_day_of_prior_month: 14,
        fnd_day_of_prior_month: Some(1),
    });

    r.insert(ContractSpec {
        symbol: "ZL".into(),   // Soybean Oil
        exchange: Exchange::CBOT,
        price_unit: PriceUnit::UsdCentsPerLb,
        currency: "USD".into(),
        tick_bp: 1,            // 0.01¢/lb
        tick_value_cents: 600, // 0.01¢ × 60000 lb = $6.00
        lot_size: 60000,
        settlement: Settlement::Physical,
        ltd_day_of_prior_month: 14,
        fnd_day_of_prior_month: Some(1),
    });

    r.insert(ContractSpec {
        symbol: "ZM".into(),   // Soybean Meal
        exchange: Exchange::CBOT,
        price_unit: PriceUnit::UsdPerShortTon,
        currency: "USD".into(),
        tick_bp: 10,           // $0.10/ton
        tick_value_cents: 1000, // $0.10 × 100 tons = $10.00
        lot_size: 100,
        settlement: Settlement::Physical,
        ltd_day_of_prior_month: 14,
        fnd_day_of_prior_month: Some(1),
    });

    // ── Equity Index ─────────────────────────────────────────────────────────

    r.insert(ContractSpec {
        symbol: "ES".into(),   // E-mini S&P 500
        exchange: Exchange::CME,
        price_unit: PriceUnit::IndexPoints,
        currency: "USD".into(),
        tick_bp: 25,           // 0.25 index points × 100 = 25bp
        tick_value_cents: 1250, // 0.25 × $50 multiplier = $12.50
        lot_size: 50,
        settlement: Settlement::Cash,
        ltd_day_of_prior_month: 21,
        fnd_day_of_prior_month: None,
    });

    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_has_major_contracts() {
        let r = default_registry();
        for sym in &["CL", "NG", "RB", "HO", "GC", "ZC", "ZS", "ZL", "ZM", "ES"] {
            assert!(r.get(sym).is_some(), "missing {}", sym);
        }
    }

    #[test]
    fn cl_pnl_math() {
        let r = default_registry();
        let cl = r.get("CL").unwrap();
        // 1 contract, price moves $1.00 = 100bp
        // P&L = (100 / 1) ticks × $10 = $1000 = 100000 cents
        assert_eq!(cl.pnl_cents(1, 100), 100_000);
    }

    #[test]
    fn zc_pnl_math() {
        let r = default_registry();
        let zc = r.get("ZC").unwrap();
        // 1 contract, price moves 1¢/bu = 100bp
        // ticks = 100 / 25 = 4; P&L = 4 × $12.50 = $50.00 = 5000 cents
        assert_eq!(zc.pnl_cents(1, 100), 5000);
    }
}
