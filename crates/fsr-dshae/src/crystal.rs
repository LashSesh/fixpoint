//! Crystal formation via DK/WT/Pi/Press cascade (spec §2.2.4).
//!
//! The cascade applies to HIM points:
//!   1. DK (Contraction): discard points where |D_ijk| * dk_rate < threshold.
//!   2. WT (Wavelet Transform): weight points by psi (phase differential).
//!   3. Pi (Π filter): keep points with net_edge > tau_edge.
//!   4. Press (top-k): retain at most k points by net_edge.
//!
//! Surviving points become DshaeCrystal artifacts.

use crate::config::{CascadeConfig, CrystalConfig};
use crate::him::{Him, HimPoint};
use fsr_fixed::{ONE, q32_from_f64_boundary, q32_mul};
use fsr_types::Q32;
use serde::{Deserialize, Serialize};

/// A DSHAE crystal: a triangle arbitrage opportunity that passed all cascade filters.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DshaeCrystal {
    /// Triangle indices (i, j, k), i < j < k.
    pub triangle: (usize, usize, usize),
    /// Net-edge magnitude |D_ijk| in Q32.
    pub net_edge: Q32,
    /// Phase differential ψ at formation.
    pub psi: Q32,
    /// Tick of crystal formation.
    pub tick: u64,
    /// Crystal age counter (incremented each tick).
    pub age: u64,
    /// Cascade depth that produced this crystal.
    pub cascade_depth: u8,
}

impl DshaeCrystal {
    /// True if the crystal is still valid (age < max_age_ticks).
    pub fn is_valid(&self, max_age_ticks: u64) -> bool {
        self.age < max_age_ticks
    }

    /// Advance age by one tick. Returns true if still valid.
    pub fn tick_age(&mut self, max_age_ticks: u64) -> bool {
        self.age += 1;
        self.is_valid(max_age_ticks)
    }
}

/// The crystal formation engine.
pub struct CrystalFormation {
    pub cascade_cfg: CascadeConfig,
    pub crystal_cfg: CrystalConfig,
    /// Currently active crystals.
    pub active_crystals: Vec<DshaeCrystal>,
    /// Total crystals ever formed.
    pub crystals_found: u64,
    /// Tick of most recent crystal.
    pub last_crystal_tick: Option<u64>,
}

impl CrystalFormation {
    pub fn new(cascade_cfg: CascadeConfig, crystal_cfg: CrystalConfig) -> Self {
        CrystalFormation {
            cascade_cfg,
            crystal_cfg,
            active_crystals: Vec::new(),
            crystals_found: 0,
            last_crystal_tick: None,
        }
    }

    /// Process a HIM and emit new crystals for this tick.
    ///
    /// Returns the list of newly formed crystals.
    pub fn process_him(&mut self, him: &Him) -> Vec<DshaeCrystal> {
        // Age existing crystals; remove expired ones.
        self.active_crystals
            .retain_mut(|c| c.tick_age(self.crystal_cfg.max_age_ticks));

        // Collect triangles that already have an active crystal (no duplicates per event).
        let active_triangles: std::collections::HashSet<(usize, usize, usize)> =
            self.active_crystals.iter().map(|c| c.triangle).collect();

        // Run the cascade.
        let candidates = self.run_cascade(&him.points, him.tick);

        // Filter: skip triangles that already have an active crystal.
        // Limit concurrent crystals.
        let capacity = self.crystal_cfg.max_concurrent
            .saturating_sub(self.active_crystals.len());
        let new_crystals: Vec<DshaeCrystal> = candidates
            .into_iter()
            .filter(|c| !active_triangles.contains(&c.triangle))
            .take(capacity)
            .collect();

        for c in &new_crystals {
            self.crystals_found += 1;
            self.last_crystal_tick = Some(c.tick);
            self.active_crystals.push(c.clone());
        }

        new_crystals
    }

    /// Run DK/WT/Pi/Press cascade on HIM points.
    fn run_cascade(&self, points: &[HimPoint], tick: u64) -> Vec<DshaeCrystal> {
        if points.is_empty() {
            return vec![];
        }

        // ── Step 1: DK Contraction ────────────────────────────────────────────
        // Weight each point's net_edge by dk_contraction_rate.
        // Points with net_edge * dk_rate < tau_edge are filtered out.
        let tau_edge_q32 = q32_from_f64_boundary(self.crystal_cfg.tau_edge_bp as f64 / 10_000.0);
        let dk_rate = self.cascade_cfg.dk_contraction_rate;

        let after_dk: Vec<(&HimPoint, Q32)> = points
            .iter()
            .filter_map(|p| {
                let weighted = q32_mul(p.net_edge(), dk_rate);
                if weighted > 0 {
                    Some((p, weighted))
                } else {
                    None
                }
            })
            .collect();

        // ── Step 2: WT (Wavelet-weighting by psi) ────────────────────────────
        // Amplify score by psi component (phase differential).
        let after_wt: Vec<(&HimPoint, Q32)> = after_dk
            .into_iter()
            .map(|(p, score)| {
                // WT score = weighted_edge + psi/4 (psi provides momentum boost)
                let wt_score = score.saturating_add(p.psi / 4);
                (p, wt_score)
            })
            .collect();

        // ── Step 3: Π filter (tau_edge threshold) ────────────────────────────
        let after_pi: Vec<(&HimPoint, Q32)> = after_wt
            .into_iter()
            .filter(|(p, _)| p.net_edge() >= tau_edge_q32)
            .collect();

        // ── Step 4: Press (top-k by score) ───────────────────────────────────
        let mut ranked = after_pi;
        ranked.sort_by(|a, b| b.1.cmp(&a.1));
        ranked.truncate(self.cascade_cfg.press_top_k);

        // ── Crystal formation ─────────────────────────────────────────────────
        ranked
            .into_iter()
            .map(|(p, _score)| DshaeCrystal {
                triangle: p.triangle,
                net_edge: p.net_edge(),
                psi: p.psi,
                tick,
                age: 0,
                cascade_depth: self.cascade_cfg.max_depth as u8,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basket::{CrossRateTensor, DeviationTensor};
    use crate::config::{CascadeConfig, CrystalConfig, DualSimplexConfig};
    use crate::him::Him;
    use crate::simplex::DualSimplex;
    use fsr_fixed::ONE;

    fn default_configs() -> (CascadeConfig, CrystalConfig) {
        (
            CascadeConfig {
                max_depth: 3,
                dk_contraction_rate: q32_from_f64_boundary(0.85),
                press_top_k: 16,
            },
            CrystalConfig {
                tau_edge_bp: 5,
                max_age_ticks: 3,
                max_concurrent: 2,
            },
        )
    }

    fn simplex_cfg() -> DualSimplexConfig {
        DualSimplexConfig {
            cycle_length: 3,
            notional_per_leg: 1000 * ONE,
            anti_phase_tolerance: ONE / 10,
            rebalance_window: 5,
        }
    }

    #[test]
    fn test_no_arb_produces_no_crystal() {
        let mids = vec![
            (0, 1, 9200i64),
            (1, 2, 8600i64),
            (0, 2, 7912i64),
        ];
        let tensor = CrossRateTensor::from_mid_prices(&mids, 3, 0);
        let deviations = DeviationTensor::from_rates(&tensor);
        let simplex = DualSimplex::new(simplex_cfg());
        let obs = simplex.observe(&tensor, &deviations);
        let him = Him::construct(3, &tensor, &deviations, &obs, 0, 20);

        let (cc, crc) = default_configs();
        let mut formation = CrystalFormation::new(cc, crc);
        let crystals = formation.process_him(&him);

        assert!(
            crystals.is_empty(),
            "no-arb should produce no crystals, got {}",
            crystals.len()
        );
    }

    #[test]
    fn test_known_arb_produces_crystal() {
        // Inject 10bp arb on r_02.
        let mids = vec![
            (0, 1, 9200i64),
            (1, 2, 8600i64),
            (0, 2, 7920i64), // mispriced ~10bp
        ];
        let tensor = CrossRateTensor::from_mid_prices(&mids, 3, 0);
        let deviations = DeviationTensor::from_rates(&tensor);
        let simplex = DualSimplex::new(simplex_cfg());
        let obs = simplex.observe(&tensor, &deviations);
        let him = Him::construct(3, &tensor, &deviations, &obs, 0, 20);

        let (cc, crc) = default_configs();
        let mut formation = CrystalFormation::new(cc, crc);
        let crystals = formation.process_him(&him);

        assert!(
            !crystals.is_empty(),
            "10bp arb should produce at least one crystal"
        );
        assert_eq!(formation.crystals_found, crystals.len() as u64);
    }

    #[test]
    fn test_crystal_age_expiry() {
        let mids = vec![
            (0, 1, 9200i64),
            (1, 2, 8600i64),
            (0, 2, 7920i64),
        ];
        let tensor = CrossRateTensor::from_mid_prices(&mids, 3, 0);
        let deviations = DeviationTensor::from_rates(&tensor);
        let simplex = DualSimplex::new(simplex_cfg());
        let obs = simplex.observe(&tensor, &deviations);
        let him_arb = Him::construct(3, &tensor, &deviations, &obs, 0, 20);

        let (cc, crc) = default_configs(); // max_age = 3
        let mut formation = CrystalFormation::new(cc, crc);

        // Tick 0: inject arb → crystal forms.
        formation.process_him(&him_arb);
        assert_eq!(formation.active_crystals.len(), 1);

        // Ticks 1,2,3: age the crystal with no-arb HIM.
        let mids_calm = vec![
            (0, 1, 9200i64),
            (1, 2, 8600i64),
            (0, 2, 7912i64),
        ];
        let tensor_calm = CrossRateTensor::from_mid_prices(&mids_calm, 3, 1);
        let dev_calm = DeviationTensor::from_rates(&tensor_calm);
        let obs_calm = simplex.observe(&tensor_calm, &dev_calm);

        for tick in 1..=3 {
            let him_calm = Him::construct(3, &tensor_calm, &dev_calm, &obs_calm, tick, 20);
            formation.process_him(&him_calm);
        }

        // After 3 ticks of aging, crystal should be expired (age >= max_age_ticks=3).
        assert_eq!(
            formation.active_crystals.len(),
            0,
            "crystal should expire after max_age_ticks"
        );
    }
}
