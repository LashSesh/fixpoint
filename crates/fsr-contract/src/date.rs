//! Lightweight date and contract-expiry types (no external date library).

use serde::{Deserialize, Serialize};
use std::fmt;

/// Calendar date (no timezone, no time-of-day).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ContractDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl ContractDate {
    pub const fn new(year: u16, month: u8, day: u8) -> Self {
        ContractDate { year, month, day }
    }

    pub fn is_before(&self, other: &ContractDate) -> bool {
        self < other
    }

    pub fn is_after(&self, other: &ContractDate) -> bool {
        self > other
    }
}

impl fmt::Display for ContractDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// Contract expiry identifier: symbol root + YYYYMM.
///
/// Matches the TradingPair convention used in fsr-ibkr:
///   `TradingPair(root, "YYYYMM")` e.g. TradingPair("CL", "202606")
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContractExpiry {
    /// Symbol root, e.g. "CL", "NG", "ZC".
    pub root: String,
    /// Delivery year.
    pub year: u16,
    /// Delivery month (1-based).
    pub month: u8,
}

impl ContractExpiry {
    pub fn new(root: impl Into<String>, year: u16, month: u8) -> Self {
        ContractExpiry { root: root.into(), year, month }
    }

    /// Compact YYYYMM string used as TradingPair.1.
    pub fn yyyymm(&self) -> String {
        format!("{:04}{:02}", self.year, self.month)
    }

    /// Standard CME month code letter (F G H J K M N Q U V X Z).
    pub fn month_code(&self) -> char {
        const CODES: [char; 12] = ['F','G','H','J','K','M','N','Q','U','V','X','Z'];
        CODES[(self.month as usize).saturating_sub(1).min(11)]
    }

    /// Bloomberg/CME short ticker, e.g. "CLM6".
    pub fn short_ticker(&self) -> String {
        format!("{}{}{}", self.root, self.month_code(), self.year % 10)
    }
}

impl fmt::Display for ContractExpiry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.root, self.yyyymm())
    }
}
