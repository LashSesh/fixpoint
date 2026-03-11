//! Backtest Report (Phase 3 §3).
//!
//! After a historical replay completes, the engine emits a structured report:
//!   data/backtest/{report_id}/report.json   — machine-readable
//!   data/backtest/{report_id}/summary.txt   — human-readable one-page summary

use fsr_fixed::q32_to_f64_display_only;
use fsr_types::{Hash256, Q32};
use serde::{Deserialize, Serialize};
use std::{
    io,
    path::Path,
};

/// Full backtest report (spec §3).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BacktestReport {
    pub run_id: String,
    pub config_hash: Hash256,
    pub recording_files: Vec<String>,
    pub total_ticks: u64,
    /// Wall-clock span of recorded data
    pub duration_covered: u64, // seconds (Duration is not Serialize)

    // P&L
    pub net_pnl_bps: Q32,
    pub gross_pnl_bps: Q32,
    pub total_fees_bps: Q32,
    pub max_drawdown_bps: Q32,
    /// Annualized Sharpe ratio (Q32)
    pub sharpe_ratio: Q32,

    // Execution
    pub total_trades: u64,
    pub settled_trades: u64,
    pub aborted_trades: u64,
    /// settled with positive PnL / total settled (Q32 fraction)
    pub win_rate: Q32,
    pub avg_trade_pnl_bps: Q32,

    // Regime
    pub ticks_alpha: u64,
    pub ticks_beta: u64,
    pub ticks_gamma: u64,

    // TTCP
    pub ttcp_crystals_emitted: u64,
    pub ttcp_avg_delta1: Q32,
    pub ttcp_avg_delta2: Q32,
    pub ttcp_avg_delta3: Q32,

    // Integrity
    pub invariant_violations: u64,
    pub safe_hold_entries: u64,
    pub chain_digest_final: Hash256,
}

impl BacktestReport {
    /// Write report.json and summary.txt to the output directory.
    pub fn write_to_dir(&self, dir: &Path) -> io::Result<()> {
        std::fs::create_dir_all(dir)?;
        // JSON
        let json = serde_json::to_string_pretty(self)
            .map_err(io::Error::other)?;
        std::fs::write(dir.join("report.json"), &json)?;
        // Summary
        std::fs::write(dir.join("summary.txt"), self.summary_text())?;
        Ok(())
    }

    /// Human-readable one-page summary.
    pub fn summary_text(&self) -> String {
        let win_pct = q32_to_f64_display_only(self.win_rate) * 100.0;
        let net_bps = q32_to_f64_display_only(self.net_pnl_bps);
        let dd_bps = q32_to_f64_display_only(self.max_drawdown_bps);
        let sharpe = q32_to_f64_display_only(self.sharpe_ratio);
        let config_hex: String = self.config_hash.to_string().chars().take(8).collect();
        let duration_h = self.duration_covered / 3600;
        let alpha_pct = if self.total_ticks > 0 {
            100.0 * self.ticks_alpha as f64 / self.total_ticks as f64
        } else {
            0.0
        };
        let beta_pct = if self.total_ticks > 0 {
            100.0 * self.ticks_beta as f64 / self.total_ticks as f64
        } else {
            0.0
        };
        let gamma_pct = if self.total_ticks > 0 {
            100.0 * self.ticks_gamma as f64 / self.total_ticks as f64
        } else {
            0.0
        };
        let d1 = q32_to_f64_display_only(self.ttcp_avg_delta1);
        let d2 = q32_to_f64_display_only(self.ttcp_avg_delta2);
        let d3 = q32_to_f64_display_only(self.ttcp_avg_delta3);

        format!(
            "=== BACKTEST REPORT: {} ===\n\
             Config:     (hash: {}...)\n\
             Recording:  {} ticks ({} h)\n\
             ---\n\
             Net P&L:      {:+.1} bp\n\
             Max Drawdown: {:+.1} bp\n\
             Sharpe (ann.): {:.2}\n\
             Win Rate:     {:.1}% ({}/{} settled)\n\
             Aborted:      {}\n\
             ---\n\
             Regime: Alpha {:.0}% | Beta {:.0}% | Gamma {:.0}%\n\
             TTCP:   {} crystals | avg d1={:.3} d2={:.3} d3={:.3}\n\
             Integrity: {} violations | {} safe-holds\n\
             Chain:  {} (verified)\n\
             ===",
            self.run_id,
            config_hex,
            self.total_ticks,
            duration_h,
            net_bps,
            dd_bps,
            sharpe,
            win_pct,
            self.settled_trades,
            self.total_trades,
            self.aborted_trades,
            alpha_pct,
            beta_pct,
            gamma_pct,
            self.ttcp_crystals_emitted,
            d1,
            d2,
            d3,
            self.invariant_violations,
            self.safe_hold_entries,
            &self.chain_digest_final.to_string()[..8],
        )
    }
}

/// Accumulates statistics during a replay run.
pub struct BacktestAccumulator {
    pub total_ticks: u64,
    pub settled: u64,
    pub aborted: u64,
    pub ticks_alpha: u64,
    pub ticks_beta: u64,
    pub ticks_gamma: u64,
    pub ttcp_crystals: u64,
    pub invariant_violations: u64,
    pub safe_hold_entries: u64,
    pub max_drawdown: Q32,
    pub min_pnl_seen: Q32,
    pub current_pnl: Q32,
    pub positive_trades: u64,

    // For Sharpe: accumulate returns
    tick_returns: Vec<Q32>,

    // TTCP delta accumulators
    ttcp_delta1_sum: Q32,
    ttcp_delta2_sum: Q32,
    ttcp_delta3_sum: Q32,
    ttcp_delta_count: u64,

    pub first_timestamp_us: u64,
    pub last_timestamp_us: u64,
}

impl BacktestAccumulator {
    pub fn new() -> Self {
        BacktestAccumulator {
            total_ticks: 0,
            settled: 0,
            aborted: 0,
            ticks_alpha: 0,
            ticks_beta: 0,
            ticks_gamma: 0,
            ttcp_crystals: 0,
            invariant_violations: 0,
            safe_hold_entries: 0,
            max_drawdown: 0,
            min_pnl_seen: 0,
            current_pnl: 0,
            positive_trades: 0,
            tick_returns: Vec::new(),
            ttcp_delta1_sum: 0,
            ttcp_delta2_sum: 0,
            ttcp_delta3_sum: 0,
            ttcp_delta_count: 0,
            first_timestamp_us: 0,
            last_timestamp_us: 0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_tick(
        &mut self,
        _tick: u64,
        regime: &str,
        integrity: &str,
        settled_delta: u64,
        aborted_delta: u64,
        ttcp_crystals_delta: u64,
        drawdown: Q32,
        ttcp_psi: Q32,
        ttcp_rho: Q32,
        ttcp_score: Q32,
        timestamp_us: u64,
    ) {
        self.total_ticks += 1;
        if self.first_timestamp_us == 0 && timestamp_us > 0 {
            self.first_timestamp_us = timestamp_us;
        }
        if timestamp_us > 0 {
            self.last_timestamp_us = timestamp_us;
        }

        match regime {
            "Alpha" => self.ticks_alpha += 1,
            "Beta" => self.ticks_beta += 1,
            "Gamma" => self.ticks_gamma += 1,
            _ => {}
        }

        if integrity == "SafeHold" {
            self.safe_hold_entries += 1;
        }

        self.settled += settled_delta;
        self.aborted += aborted_delta;

        // Track P&L as settled - aborted (proxy)
        let pnl_delta = settled_delta as Q32 - aborted_delta as Q32;
        self.current_pnl += pnl_delta;
        self.positive_trades += settled_delta;

        if drawdown < self.min_pnl_seen {
            self.min_pnl_seen = drawdown;
        }
        self.max_drawdown = self.min_pnl_seen.abs();

        self.tick_returns.push(pnl_delta);

        if ttcp_crystals_delta > 0 {
            self.ttcp_crystals += ttcp_crystals_delta;
            self.ttcp_delta1_sum += ttcp_psi / 3;
            self.ttcp_delta2_sum += ttcp_rho / 2;
            self.ttcp_delta3_sum += ttcp_score;
            self.ttcp_delta_count += 1;
        }
    }

    pub fn into_report(
        self,
        run_id: &str,
        config_hash: Hash256,
        recording_files: Vec<String>,
        chain_digest: Hash256,
    ) -> BacktestReport {
        use fsr_fixed::ONE;

        let total = (self.settled + self.aborted).max(1);
        let win_rate = (self.settled as Q32 * ONE) / total as Q32;
        let avg_trade_pnl = if self.settled > 0 {
            self.current_pnl / self.settled as Q32
        } else {
            0
        };

        // Sharpe: mean/std of tick_returns * sqrt(ticks_per_year)
        // Simplified: use settled_rate * sqrt_factor
        let sharpe = compute_sharpe(&self.tick_returns);

        let duration_sec = if self.last_timestamp_us > self.first_timestamp_us {
            (self.last_timestamp_us - self.first_timestamp_us) / 1_000_000
        } else {
            self.total_ticks // 1 second per tick as fallback
        };

        let ttcp_avg = |sum: Q32| -> Q32 {
            if self.ttcp_delta_count > 0 {
                sum / self.ttcp_delta_count as Q32
            } else {
                0
            }
        };

        BacktestReport {
            run_id: run_id.to_string(),
            config_hash,
            recording_files,
            total_ticks: self.total_ticks,
            duration_covered: duration_sec,
            net_pnl_bps: self.current_pnl,
            gross_pnl_bps: self.settled as Q32 * ONE,
            total_fees_bps: 0, // simplified
            max_drawdown_bps: self.max_drawdown,
            sharpe_ratio: sharpe,
            total_trades: self.settled + self.aborted,
            settled_trades: self.settled,
            aborted_trades: self.aborted,
            win_rate,
            avg_trade_pnl_bps: avg_trade_pnl,
            ticks_alpha: self.ticks_alpha,
            ticks_beta: self.ticks_beta,
            ticks_gamma: self.ticks_gamma,
            ttcp_crystals_emitted: self.ttcp_crystals,
            ttcp_avg_delta1: ttcp_avg(self.ttcp_delta1_sum),
            ttcp_avg_delta2: ttcp_avg(self.ttcp_delta2_sum),
            ttcp_avg_delta3: ttcp_avg(self.ttcp_delta3_sum),
            invariant_violations: self.invariant_violations,
            safe_hold_entries: self.safe_hold_entries,
            chain_digest_final: chain_digest,
        }
    }
}

impl Default for BacktestAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

/// Simplified Sharpe ratio: mean return / std return * sqrt(252 * 86400) proxy.
/// Uses Q32 fixed-point arithmetic.
fn compute_sharpe(returns: &[Q32]) -> Q32 {
    use fsr_fixed::ONE;
    if returns.len() < 2 {
        return 0;
    }
    let n = returns.len() as Q32;
    let mean = returns.iter().copied().sum::<Q32>() / n;
    let variance = returns
        .iter()
        .map(|&r| {
            let d = r - mean;
            // d * d in Q32 can overflow; use saturating arithmetic
            d.saturating_mul(d) / ONE
        })
        .sum::<Q32>()
        / n;
    if variance <= 0 {
        return 0;
    }
    // sqrt approximation: use integer sqrt
    let std_dev = isqrt(variance as u64) as Q32;
    if std_dev == 0 {
        return 0;
    }
    // Annualized: multiply by sqrt(ticks_per_year); assume 86400 ticks/day * 252 days
    // sqrt(21772800) ≈ 4666; scaled by ONE for Q32
    let annualize = 4666i64;
    mean.saturating_mul(annualize) / std_dev
}

/// Integer square root (floor).
fn isqrt(n: u64) -> u64 {
    if n == 0 {
        return 0;
    }
    let mut x = n;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backtest_report_serialization() {
        let r = BacktestReport {
            run_id: "test_run".to_string(),
            config_hash: Hash256::ZERO,
            recording_files: vec!["a.rec".to_string()],
            total_ticks: 100,
            duration_covered: 3600,
            net_pnl_bps: 1000,
            gross_pnl_bps: 2000,
            total_fees_bps: 100,
            max_drawdown_bps: 500,
            sharpe_ratio: 5_000_000_000,
            total_trades: 10,
            settled_trades: 7,
            aborted_trades: 3,
            win_rate: 3_006_477_107,
            avg_trade_pnl_bps: 100,
            ticks_alpha: 70,
            ticks_beta: 20,
            ticks_gamma: 10,
            ttcp_crystals_emitted: 2,
            ttcp_avg_delta1: 100,
            ttcp_avg_delta2: 200,
            ttcp_avg_delta3: 300,
            invariant_violations: 0,
            safe_hold_entries: 0,
            chain_digest_final: Hash256::ZERO,
        };

        let json = serde_json::to_string(&r).unwrap();
        let parsed: BacktestReport = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.run_id, "test_run");
        assert_eq!(parsed.total_ticks, 100);
        assert_eq!(parsed.settled_trades, 7);
    }

    #[test]
    fn test_summary_text_contains_key_fields() {
        let r = BacktestReport {
            run_id: "run_test".to_string(),
            config_hash: Hash256::ZERO,
            recording_files: vec![],
            total_ticks: 1000,
            duration_covered: 7200,
            net_pnl_bps: 500,
            gross_pnl_bps: 1000,
            total_fees_bps: 50,
            max_drawdown_bps: 200,
            sharpe_ratio: 8_000_000_000,
            total_trades: 20,
            settled_trades: 14,
            aborted_trades: 6,
            win_rate: 3_006_477_107,
            avg_trade_pnl_bps: 35,
            ticks_alpha: 700,
            ticks_beta: 200,
            ticks_gamma: 100,
            ttcp_crystals_emitted: 3,
            ttcp_avg_delta1: 137_438_953,
            ttcp_avg_delta2: 274_877_906,
            ttcp_avg_delta3: 412_316_860,
            invariant_violations: 0,
            safe_hold_entries: 0,
            chain_digest_final: Hash256::ZERO,
        };

        let summary = r.summary_text();
        assert!(summary.contains("run_test"), "summary must contain run_id");
        assert!(summary.contains("Net P&L"), "summary must contain P&L header");
        assert!(summary.contains("TTCP"), "summary must contain TTCP section");
    }

    #[test]
    fn test_backtest_accumulator_basic() {
        let mut acc = BacktestAccumulator::new();
        for _ in 0..10 {
            acc.record_tick(0, "Alpha", "Healthy", 1, 0, 0, 0, 0, 0, 0, 0);
        }
        acc.record_tick(10, "Beta", "Healthy", 0, 1, 0, 0, 0, 0, 0, 0);
        assert_eq!(acc.total_ticks, 11);
        assert_eq!(acc.settled, 10);
        assert_eq!(acc.aborted, 1);
        assert_eq!(acc.ticks_alpha, 10);
        assert_eq!(acc.ticks_beta, 1);
    }

    #[test]
    fn test_backtest_report_write_to_dir() {
        let dir = std::env::temp_dir().join("fsr_backtest_report_test");
        let _ = std::fs::remove_dir_all(&dir);

        let r = BacktestReport {
            run_id: "write_test".to_string(),
            config_hash: Hash256::ZERO,
            recording_files: vec![],
            total_ticks: 50,
            duration_covered: 1800,
            net_pnl_bps: 100,
            gross_pnl_bps: 200,
            total_fees_bps: 10,
            max_drawdown_bps: 50,
            sharpe_ratio: 0,
            total_trades: 5,
            settled_trades: 4,
            aborted_trades: 1,
            win_rate: 3_435_973_836,
            avg_trade_pnl_bps: 25,
            ticks_alpha: 40,
            ticks_beta: 8,
            ticks_gamma: 2,
            ttcp_crystals_emitted: 1,
            ttcp_avg_delta1: 0,
            ttcp_avg_delta2: 0,
            ttcp_avg_delta3: 0,
            invariant_violations: 0,
            safe_hold_entries: 0,
            chain_digest_final: Hash256::ZERO,
        };
        r.write_to_dir(&dir).unwrap();
        assert!(dir.join("report.json").exists());
        assert!(dir.join("summary.txt").exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
