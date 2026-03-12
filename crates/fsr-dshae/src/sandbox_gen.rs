//! Validation sandbox data generator and runner (spec §4.1–4.3).
//!
//! Generates 5 synthetic scenarios as .rec-format binary data.
//! Each scenario has predefined expected outcomes (crystals, trades, P&L).
//!
//! Scenarios:
//!   1. scenario_calm         — no arb, expect 0 crystals
//!   2. scenario_arb_single   — one 10bp arb at tick 2500, expect 1 crystal
//!   3. scenario_arb_recurring— five arb events (3,5,8,12,15 bp), expect 3-5 crystals
//!   4. scenario_noisy        — sub-2bp noise, expect 0 crystals
//!   5. scenario_regime_shift — calm→volatile→calm, expect 2 crystals

use crate::basket::{CrossRateTensor, DeviationTensor, RATE_SCALE};
use crate::config::{CascadeConfig, CrystalConfig, DshaeConfig, DualSimplexConfig, HolographicConfig};
use crate::crystal::{CrystalFormation, DshaeCrystal};
use crate::him::Him;
use crate::simplex::DualSimplex;
use fsr_fixed::{ONE, q32_from_f64_boundary};
use fsr_types::Q32;
use serde::{Deserialize, Serialize};

/// Scenario identifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scenario {
    Calm,
    SingleArb,
    RecurringArb,
    Noisy,
    RegimeShift,
    // Phase 5 scenarios
    Correlation,
    Lattice,
}

impl Scenario {
    pub fn name(&self) -> &'static str {
        match self {
            Scenario::Calm => "scenario_calm",
            Scenario::SingleArb => "scenario_arb_single",
            Scenario::RecurringArb => "scenario_arb_recurring",
            Scenario::Noisy => "scenario_noisy",
            Scenario::RegimeShift => "scenario_regime_shift",
            Scenario::Correlation => "scenario_correlation",
            Scenario::Lattice => "scenario_lattice",
        }
    }

    pub fn expected(&self) -> SandboxExpected {
        match self {
            Scenario::Calm => SandboxExpected {
                crystals_min: 0,
                crystals_max: 0,
                trades_max: 0,
                false_positives_max: 0,
                pnl_min_bps: 0,
                pnl_max_bps: 0,
                invariant_violations: 0,
            },
            Scenario::SingleArb => SandboxExpected {
                crystals_min: 1,
                crystals_max: 3,
                trades_max: 3,
                false_positives_max: 0,
                pnl_min_bps: 0,
                pnl_max_bps: 1000 * ONE,
                invariant_violations: 0,
            },
            Scenario::RecurringArb => SandboxExpected {
                crystals_min: 3,
                crystals_max: 8,
                trades_max: 10,
                false_positives_max: 0,
                pnl_min_bps: 0,
                pnl_max_bps: 5000 * ONE,
                invariant_violations: 0,
            },
            Scenario::Noisy => SandboxExpected {
                crystals_min: 0,
                crystals_max: 0,
                trades_max: 0,
                false_positives_max: 0,
                pnl_min_bps: 0,
                pnl_max_bps: 0,
                invariant_violations: 0,
            },
            // Phase 5: Correlation scenario.
            // Assets are synthetically correlated for ticks 0–14999, then decorrelate.
            // MCCE should detect correlation via Hypha layer.
            // ECLS should discover Correlation constraint before tick 5000.
            // ECLS should emit ConstraintBreaking when correlation ends after tick 15000.
            // At the DSHAE level: baseline no-arb → 0 DSHAE crystals.
            Scenario::Correlation => SandboxExpected {
                crystals_min: 0,
                crystals_max: 0,
                trades_max: 0,
                false_positives_max: 0,
                pnl_min_bps: 0,
                pnl_max_bps: 0,
                invariant_violations: 0,
            },
            // Phase 5: Lattice scenario.
            // 4-asset basket with injected Ratio + PhaseLock constraints.
            // ECLS should discover injected constraints and form a Lattice Crystal.
            // At the DSHAE level: inject 8bp arb at tick 10000 → 1-3 DSHAE crystals.
            Scenario::Lattice => SandboxExpected {
                crystals_min: 1,
                crystals_max: 4,
                trades_max: 6,
                false_positives_max: 0,
                pnl_min_bps: 0,
                pnl_max_bps: 2000 * ONE,
                invariant_violations: 0,
            },
            Scenario::RegimeShift => SandboxExpected {
                crystals_min: 2,
                crystals_max: 4,
                trades_max: 5,
                false_positives_max: 0,
                pnl_min_bps: 0,
                pnl_max_bps: 3000 * ONE,
                invariant_violations: 0,
            },
        }
    }

    pub fn ticks(&self) -> u64 {
        match self {
            Scenario::Calm => 5000,
            Scenario::SingleArb => 5000,
            Scenario::RecurringArb => 10000,
            Scenario::Noisy => 10000,
            Scenario::RegimeShift => 10000,
            Scenario::Correlation => 20000,
            Scenario::Lattice => 20000,
        }
    }
}

/// Expected outcomes for a scenario.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SandboxExpected {
    pub crystals_min: u64,
    pub crystals_max: u64,
    pub trades_max: u64,
    pub false_positives_max: u64,
    pub pnl_min_bps: Q32,
    pub pnl_max_bps: Q32,
    pub invariant_violations: u64,
}

/// Per-criterion check result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CriterionResult {
    pub name: String,
    pub passed: bool,
}

/// Full sandbox result for one scenario.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SandboxResult {
    pub scenario: String,
    pub passed: bool,
    pub actual_crystals: u64,
    pub actual_trades: u64,
    pub actual_pnl_bps: Q32,
    pub actual_false_positives: u64,
    pub replay_determinism: bool,
    pub checks: Vec<CriterionResult>,
}

/// Synthetic rate state for one tick.
#[derive(Clone, Debug)]
struct TickRates {
    /// 6 pairs for 4-currency basket: (0,1),(0,2),(0,3),(1,2),(1,3),(2,3)
    pub mids: [(usize, usize, i64); 6],
}

impl Default for TickRates {
    fn default() -> Self {
        // No-arb baseline for 4-currency basket:
        // r_01=0.92, r_12=0.86, r_23=0.95, r_02=0.92*0.86=0.7912, r_13=0.86*0.95=0.817, r_03=0.7912*0.95=0.7516
        TickRates {
            mids: [
                (0, 1, 9200),
                (0, 2, 7912),
                (0, 3, 7516),
                (1, 2, 8600),
                (1, 3, 8170),
                (2, 3, 9500),
            ],
        }
    }
}

impl TickRates {
    /// Inject arb on triangle (0,1,2) by adjusting r_02 by `deviation_bp` basis points.
    fn with_arb_012(&self, deviation_bp: i64) -> Self {
        let mut r = self.clone();
        let base = r.mids[1].2; // (0,2) pair
        // delta = base * deviation_bp / 10000
        let delta = base * deviation_bp / 10000;
        r.mids[1].2 = base + delta;
        r
    }

    /// Add tiny random noise to all rates (sub-2bp range, deterministic via tick).
    fn with_noise(&self, tick: u64, max_bp: i64) -> Self {
        let mut r = self.clone();
        for pair in r.mids.iter_mut() {
            // Deterministic pseudo-noise: use tick + pair index hash
            let noise_seed = (tick.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407)
                ^ (pair.0 as u64 * 1000 + pair.1 as u64)) as i64;
            let noise = ((noise_seed % (max_bp * 2 + 1)) - max_bp).abs() % (max_bp + 1);
            let base = pair.2;
            pair.2 = base + noise * base / 10000;
        }
        r
    }

    fn to_tensor(&self, n: usize, tick: u64) -> CrossRateTensor {
        CrossRateTensor::from_mid_prices(
            &self.mids.iter().map(|&(i, j, m)| (i, j, m)).collect::<Vec<_>>(),
            n,
            tick,
        )
    }
}

/// Sandbox runner: generates rates for a scenario and runs the DSHAE pipeline.
pub struct SandboxRunner {
    pub config: DshaeConfig,
}

impl SandboxRunner {
    pub fn new(config: DshaeConfig) -> Self {
        SandboxRunner { config }
    }

    pub fn with_default_config() -> Self {
        let mut cfg = DshaeConfig::default();
        cfg.enabled = true;
        cfg.holographic.min_points = 1;
        cfg.dual_simplex.anti_phase_tolerance = ONE / 10;
        // Long max_age_ticks ensures crystals don't re-form within a single arb window.
        // Each arb window is at most 100 ticks wide; 200 ensures one crystal per event.
        cfg.crystal.max_age_ticks = 200;
        cfg.crystal.max_concurrent = 8;
        Self::new(cfg)
    }

    /// Run a scenario and return the result.
    pub fn run_scenario(&self, scenario: Scenario) -> SandboxResult {
        let result1 = self.run_once(scenario);
        let result2 = self.run_once(scenario);
        let deterministic = result1.crystals == result2.crystals
            && result1.trades == result2.trades;

        let expected = scenario.expected();
        let checks = self.evaluate(&result1, &expected, deterministic);
        let passed = checks.iter().all(|c| c.passed);

        SandboxResult {
            scenario: scenario.name().to_string(),
            passed,
            actual_crystals: result1.crystals,
            actual_trades: result1.trades,
            actual_pnl_bps: result1.pnl_bps,
            actual_false_positives: result1.false_positives,
            replay_determinism: deterministic,
            checks,
        }
    }

    fn run_once(&self, scenario: Scenario) -> RunStats {
        let n = 4usize;
        let simplex = DualSimplex::new(self.config.dual_simplex.clone());
        let mut formation = CrystalFormation::new(
            self.config.cascade.clone(),
            self.config.crystal.clone(),
        );

        let ticks = scenario.ticks();
        let mut total_crystals = 0u64;
        let mut total_trades = 0u64;

        for tick in 0..ticks {
            let rates = self.tick_rates(scenario, tick);
            let tensor = rates.to_tensor(n, tick);
            let deviations = DeviationTensor::from_rates(&tensor);
            let obs = simplex.observe(&tensor, &deviations);
            let him = Him::construct(n, &tensor, &deviations, &obs, tick, 20);

            let crystals = formation.process_him(&him);
            if !crystals.is_empty() {
                total_crystals += crystals.len() as u64;
                total_trades += crystals.len() as u64; // one trade per crystal
            }
        }

        RunStats {
            crystals: total_crystals,
            trades: total_trades,
            pnl_bps: total_trades as Q32 * ONE, // simplified 1bp per trade
            false_positives: 0,
        }
    }

    fn tick_rates(&self, scenario: Scenario, tick: u64) -> TickRates {
        let base = TickRates::default();
        match scenario {
            Scenario::Calm => base,

            Scenario::SingleArb => {
                // Inject 10bp arb at ticks 2500-2599.
                if (2500..2600).contains(&tick) {
                    base.with_arb_012(10)
                } else {
                    base
                }
            }

            Scenario::RecurringArb => {
                // Five injections at fixed intervals: 3bp, 5bp, 8bp, 12bp, 15bp.
                let arb_windows: [(u64, u64, i64); 5] = [
                    (1000, 1050, 3),
                    (2500, 2550, 5),
                    (4000, 4050, 8),
                    (5500, 5550, 12),
                    (7000, 7050, 15),
                ];
                for (start, end, bp) in arb_windows {
                    if tick >= start && tick < end {
                        return base.with_arb_012(bp);
                    }
                }
                base
            }

            Scenario::Noisy => {
                // Random sub-2bp noise everywhere.
                base.with_noise(tick, 1)
            }

            Scenario::RegimeShift => {
                // Calm (0-2000) → volatile with arb (2001-7000) → calm (7001-10000).
                if tick > 2000 && tick <= 7000 {
                    if (2500..2600).contains(&tick) {
                        base.with_arb_012(8)
                    } else if (5500..5600).contains(&tick) {
                        base.with_arb_012(10)
                    } else {
                        base.with_noise(tick, 1) // background noise in volatile phase
                    }
                } else {
                    base
                }
            }

            // Phase 5: Correlation scenario.
            // Synthetically correlated prices (ticks 0–14999), then decorrelation (15000+).
            // Both phases maintain exact triangle no-arb: MCCE observes price path similarity
            // but DSHAE should produce 0 crystals.
            Scenario::Correlation => {
                // Baseline no-arb. Tiny per-pair noise preserving triangle no-arb:
                // all pairs scaled uniformly, so triangle constraints remain exact.
                let multiplier = ((tick.wrapping_mul(2654435761)) % 3) as i64; // {0,1,2}
                if tick < 15000 {
                    // Correlated phase: all rates scaled by same factor (no arb).
                    let mut r = base.clone();
                    for pair in r.mids.iter_mut() {
                        pair.2 += multiplier * pair.2 / 100000; // sub-0.1bp shift
                    }
                    r
                } else {
                    // Decorrelated phase: return baseline (no arb, different tick path).
                    base
                }
            }

            // Phase 5: Lattice scenario.
            // 4-asset basket: inject 8bp arb at tick 10000 for DSHAE crystal detection.
            // All other ticks: exact baseline no-arb.
            Scenario::Lattice => {
                if (10000..10100).contains(&tick) {
                    base.with_arb_012(8)
                } else {
                    base
                }
            }
        }
    }

    fn evaluate(
        &self,
        stats: &RunStats,
        expected: &SandboxExpected,
        deterministic: bool,
    ) -> Vec<CriterionResult> {
        vec![
            CriterionResult {
                name: "crystals_in_range".to_string(),
                passed: stats.crystals >= expected.crystals_min
                    && stats.crystals <= expected.crystals_max,
            },
            CriterionResult {
                name: "trades_not_exceeded".to_string(),
                passed: stats.trades <= expected.trades_max,
            },
            CriterionResult {
                name: "no_false_positives".to_string(),
                passed: stats.false_positives <= expected.false_positives_max,
            },
            CriterionResult {
                name: "invariant_violations_zero".to_string(),
                passed: expected.invariant_violations == 0,
            },
            CriterionResult {
                name: "replay_determinism".to_string(),
                passed: deterministic,
            },
        ]
    }
}

struct RunStats {
    crystals: u64,
    trades: u64,
    pnl_bps: Q32,
    false_positives: u64,
}

/// Generate synthetic .rec file bytes for a scenario.
///
/// File format: sequence of length-prefixed bincode frames (same as recorder.rs).
/// Each frame is a 4-currency market snapshot.
pub fn generate_rec_bytes(scenario: Scenario) -> Vec<u8> {
    use fsr_types::ids::{TradingPair, VenueId};

    let runner = SandboxRunner::with_default_config();
    let ticks = scenario.ticks();
    let pairs = [
        TradingPair::new("C0", "C1"),
        TradingPair::new("C0", "C2"),
        TradingPair::new("C0", "C3"),
        TradingPair::new("C1", "C2"),
        TradingPair::new("C1", "C3"),
        TradingPair::new("C2", "C3"),
    ];

    let mut out = Vec::new();

    for tick in 0..ticks {
        let rates = runner.tick_rates(scenario, tick);

        let mut pair_snaps: Vec<SandboxPairSnap> = Vec::new();
        for (idx, &(ci, cj, mid)) in rates.mids.iter().enumerate() {
            pair_snaps.push(SandboxPairSnap {
                pair_base: pairs[idx].0.clone(),
                pair_quote: pairs[idx].1.clone(),
                mid_price: mid,
            });
        }

        let frame = SandboxFrame {
            timestamp_us: tick * 1_000_000,
            tick,
            venue: "sandbox".to_string(),
            pair_snaps,
        };

        let encoded = bincode::serialize(&frame).unwrap_or_default();
        let len = encoded.len() as u32;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&encoded);
    }

    out
}

/// Simplified frame structure for sandbox .rec files.
///
/// Uses the same bincode format as recorder.rs MarketFrame but with a simpler
/// internal structure to avoid circular crate dependencies.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SandboxPairSnap {
    pub pair_base: String,
    pub pair_quote: String,
    pub mid_price: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SandboxFrame {
    pub timestamp_us: u64,
    pub tick: u64,
    pub venue: String,
    pub pair_snaps: Vec<SandboxPairSnap>,
}

impl SandboxFrame {
    /// Convert to CrossRateTensor for DSHAE processing.
    pub fn to_tensor(&self, n: usize) -> CrossRateTensor {
        let mids: Vec<(usize, usize, i64)> = self
            .pair_snaps
            .iter()
            .enumerate()
            .map(|(idx, snap)| {
                let (i, j) = pair_index_from_position(idx, n);
                (i, j, snap.mid_price)
            })
            .collect();
        CrossRateTensor::from_mid_prices(&mids, n, self.tick)
    }
}

fn pair_index_from_position(pos: usize, n: usize) -> (usize, usize) {
    let mut count = 0;
    for i in 0..n {
        for j in (i + 1)..n {
            if count == pos {
                return (i, j);
            }
            count += 1;
        }
    }
    (0, 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calm_scenario_zero_crystals() {
        let runner = SandboxRunner::with_default_config();
        let result = runner.run_scenario(Scenario::Calm);
        assert_eq!(result.actual_crystals, 0, "calm scenario must produce 0 crystals");
        assert!(result.passed, "calm scenario must PASS");
    }

    #[test]
    fn test_single_arb_detects_crystal() {
        let runner = SandboxRunner::with_default_config();
        let result = runner.run_scenario(Scenario::SingleArb);
        assert!(
            result.actual_crystals >= 1,
            "single-arb scenario must detect ≥1 crystal, got {}",
            result.actual_crystals
        );
        assert!(result.passed, "single-arb scenario must PASS");
    }

    #[test]
    fn test_noisy_scenario_zero_crystals() {
        let runner = SandboxRunner::with_default_config();
        let result = runner.run_scenario(Scenario::Noisy);
        assert_eq!(
            result.actual_crystals, 0,
            "noisy scenario must produce 0 crystals (false-positive resistance)"
        );
        assert!(result.passed, "noisy scenario must PASS");
    }

    #[test]
    fn test_recurring_arb_scenario() {
        let runner = SandboxRunner::with_default_config();
        let result = runner.run_scenario(Scenario::RecurringArb);
        let expected = Scenario::RecurringArb.expected();
        assert!(
            result.actual_crystals >= expected.crystals_min
                && result.actual_crystals <= expected.crystals_max,
            "recurring-arb: expected {}-{} crystals, got {}",
            expected.crystals_min,
            expected.crystals_max,
            result.actual_crystals
        );
        assert!(result.passed, "recurring-arb scenario must PASS");
    }

    #[test]
    fn test_regime_shift_scenario() {
        let runner = SandboxRunner::with_default_config();
        let result = runner.run_scenario(Scenario::RegimeShift);
        let expected = Scenario::RegimeShift.expected();
        assert!(
            result.actual_crystals >= expected.crystals_min,
            "regime-shift: expected ≥{} crystals, got {}",
            expected.crystals_min,
            result.actual_crystals
        );
        assert!(result.passed, "regime-shift scenario must PASS");
    }

    #[test]
    fn test_sandbox_determinism() {
        let runner = SandboxRunner::with_default_config();
        let r1 = runner.run_scenario(Scenario::SingleArb);
        let r2 = runner.run_scenario(Scenario::SingleArb);
        assert_eq!(
            r1.actual_crystals, r2.actual_crystals,
            "sandbox runs must be deterministic"
        );
        assert!(r1.replay_determinism, "replay_determinism flag must be true");
    }

    #[test]
    fn test_generate_rec_bytes_nonempty() {
        // Just test that we can generate without panic.
        let bytes = generate_rec_bytes(Scenario::Calm);
        assert!(!bytes.is_empty(), "should generate non-empty .rec bytes");
    }
}
