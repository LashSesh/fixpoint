//! Spread topology: SpreadKind and SpreadDef.

use crate::date::ContractExpiry;
use serde::{Deserialize, Serialize};

/// Crack spread recipe (ratio of crude to products).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrackRecipe {
    /// 3 crude → 2 gasoline + 1 heating oil
    ThreeTwoOne,
    /// 5 crude → 3 gasoline + 2 heating oil
    FiveThreeTwo,
    /// 1 crude → 1 gasoline (gasoline crack)
    OneToOne,
}

/// Board crush recipe (soybeans to products).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CrushRecipe {
    /// Standard: 11 lb oil + 44 lb meal per bushel
    Standard,
}

/// Spread topology — defines the legs and their economic relationship.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SpreadKind {
    /// Two contracts of the same commodity, different expiries.
    /// Signal: observed basis vs theoretical cost-of-carry.
    Calendar {
        near: ContractExpiry,
        far: ContractExpiry,
    },
    /// Two different commodities with an economic relationship.
    /// Signal: observed ratio vs historical mean.
    InterCommodity {
        leg_a: ContractExpiry,
        leg_b: ContractExpiry,
        /// Integer ratio: leg_a × ratio.0 vs leg_b × ratio.1
        ratio: (i32, i32),
    },
    /// Crude oil vs refined products (crack spread).
    /// All three legs must be the same delivery month.
    Crack {
        crude: ContractExpiry,
        gasoline: ContractExpiry,
        heat_oil: ContractExpiry,
        recipe: CrackRecipe,
    },
    /// Soybeans vs oil + meal (board crush).
    Crush {
        beans: ContractExpiry,
        oil: ContractExpiry,
        meal: ContractExpiry,
        recipe: CrushRecipe,
    },
    /// Calendar butterfly: near − 2×mid + far
    Butterfly {
        near: ContractExpiry,
        mid: ContractExpiry,
        far: ContractExpiry,
    },
    /// Single outright contract (no spread; for reference in the spread universe).
    Outright {
        contract: ContractExpiry,
    },
}

/// A named, fully-specified spread with associated metadata.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpreadDef {
    /// Human-readable label, e.g. "CL Jul-Aug 2026" or "3-2-1 Jun 2026".
    pub label: String,
    pub kind: SpreadKind,
    /// Expected fair-value of the spread in basis-points (initial prior).
    /// Updated by MCCE as market data accumulates.
    pub prior_fair_value_bp: i64,
    /// Expected standard deviation of deviation signal, in basis-points.
    pub prior_sigma_bp: i64,
}

impl SpreadDef {
    pub fn calendar(
        near: ContractExpiry,
        far: ContractExpiry,
        prior_fair_value_bp: i64,
        prior_sigma_bp: i64,
    ) -> Self {
        let label = format!("{} / {} Calendar", near.short_ticker(), far.short_ticker());
        SpreadDef {
            label,
            kind: SpreadKind::Calendar { near, far },
            prior_fair_value_bp,
            prior_sigma_bp,
        }
    }

    pub fn crack_321(
        crude: ContractExpiry,
        gasoline: ContractExpiry,
        heat_oil: ContractExpiry,
        prior_fair_value_bp: i64,
        prior_sigma_bp: i64,
    ) -> Self {
        let label = format!("3-2-1 Crack {}", crude.short_ticker());
        SpreadDef {
            label,
            kind: SpreadKind::Crack {
                crude,
                gasoline,
                heat_oil,
                recipe: CrackRecipe::ThreeTwoOne,
            },
            prior_fair_value_bp,
            prior_sigma_bp,
        }
    }

    pub fn crush(
        beans: ContractExpiry,
        oil: ContractExpiry,
        meal: ContractExpiry,
        prior_fair_value_bp: i64,
        prior_sigma_bp: i64,
    ) -> Self {
        let label = format!("Board Crush {}", beans.short_ticker());
        SpreadDef {
            label,
            kind: SpreadKind::Crush {
                beans,
                oil,
                meal,
                recipe: CrushRecipe::Standard,
            },
            prior_fair_value_bp,
            prior_sigma_bp,
        }
    }
}
