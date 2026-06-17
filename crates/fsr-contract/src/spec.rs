//! ContractSpec: per-symbol tick/multiplier/settlement metadata.

use serde::{Deserialize, Serialize};

/// Exchange on which the contract is listed.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Exchange {
    NYMEX,
    CME,
    CBOT,
    COMEX,
    ICE,
}

impl std::fmt::Display for Exchange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Exchange::NYMEX  => "NYMEX",
            Exchange::CME    => "CME",
            Exchange::CBOT   => "CBOT",
            Exchange::COMEX  => "COMEX",
            Exchange::ICE    => "ICE",
        };
        write!(f, "{}", s)
    }
}

/// Native price unit of the contract.
///
/// The price_bp wire format is always: native_unit_price × 100 → i64.
/// E.g. CL at $75.00/bbl → price_bp = 7500.
///      ZC at 450¢/bu    → price_bp = 45000.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PriceUnit {
    UsdPerBbl,           // Crude oil, Brent
    UsdPerMmBtu,         // Natural gas
    UsdCentsPerBushel,   // Corn (ZC), Wheat (ZW), Soybeans (ZS)
    UsdCentsPerLb,       // Soybean oil (ZL), Cotton (CT)
    UsdPerShortTon,      // Soybean meal (ZM)
    UsdCentsPerGallon,   // RBOB gasoline (RB), Heating oil (HO)
    UsdPerTroyOz,        // Gold (GC), Silver (SI)
    IndexPoints,         // E-mini S&P (ES), Nasdaq (NQ)
}

/// How a contract is settled at expiry.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Settlement {
    /// Physical delivery — must roll or close before FND.
    Physical,
    /// Cash-settled against reference index.
    Cash,
}

/// All static metadata for one futures root symbol.
///
/// Prices are in price_bp = native_unit × 100 as i64.
/// Monetary values (tick_value_cents) are in integer US cents.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContractSpec {
    /// Root symbol, e.g. "CL", "NG", "ZC".
    pub symbol: String,
    pub exchange: Exchange,
    pub price_unit: PriceUnit,
    pub currency: String,

    /// Minimum price increment in basis-points (price × 100).
    ///
    /// CL:  $0.01/bbl   → tick_bp = 1
    /// NG:  $0.001/MMBtu → tick_bp = 0  (sub-bp; use 1 as floor)
    /// ZC:  0.25¢/bu    → tick_bp = 25 (because ¢×100 = 0.25×100 = 25)
    /// ZL:  0.01¢/lb    → tick_bp = 1
    /// ZM:  $0.10/ton   → tick_bp = 10
    /// GC:  $0.10/oz    → tick_bp = 10
    /// RB/HO: $0.0001/gal → tick_bp = 0 (use 1 as floor)
    pub tick_bp: i64,

    /// Dollar value of one tick in integer cents.
    ///
    /// CL:  1 tick × 1000 bbl = $10.00 → 1000 cents
    /// ZC:  0.25¢ × 5000 bu  = $12.50 → 1250 cents
    /// GC:  $0.10 × 100 oz   = $10.00 → 1000 cents
    pub tick_value_cents: i64,

    /// Contract lot size (physical units per contract).
    ///
    /// CL: 1000 bbl  · NG: 10000 MMBtu  · ZC/ZS/ZW: 5000 bu
    /// ZL: 60000 lb  · ZM: 100 short ton · GC: 100 oz · SI: 5000 oz
    /// RB/HO: 42000 gal · ES: 50 (index multiplier)
    pub lot_size: u64,

    pub settlement: Settlement,

    /// Approximate days before contract month end that the contract stops trading.
    /// Used for soft FND warning; exact dates must come from exchange calendar.
    pub ltd_day_of_prior_month: u8,
    /// For physical contracts: approximate day-of-month of First Notice Day.
    pub fnd_day_of_prior_month: Option<u8>,
}

impl ContractSpec {
    /// Convert a price in basis-points to a human-readable f64 in native units.
    pub fn bp_to_price(&self, price_bp: i64) -> f64 {
        price_bp as f64 / 100.0
    }

    /// Convert a native-unit price (f64) to basis-points (i64).
    pub fn price_to_bp(&self, price: f64) -> i64 {
        (price * 100.0).round() as i64
    }

    /// Dollar P&L for a position change of `delta_bp` (price movement in bp)
    /// on `lots` contracts.
    pub fn pnl_cents(&self, lots: i64, delta_bp: i64) -> i64 {
        // 1 bp = 1 tick if tick_bp == 1, else delta_bp / tick_bp ticks
        // P&L = (delta_bp / tick_bp) * tick_value_cents * lots
        // Use saturating to avoid overflow on large positions
        let tick = self.tick_bp.max(1);
        let ticks = delta_bp / tick;
        ticks.saturating_mul(self.tick_value_cents).saturating_mul(lots)
    }
}
