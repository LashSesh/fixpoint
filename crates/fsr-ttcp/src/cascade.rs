//! 3-level TTCP cascade (simplified Phase 2 version per spec §TTCP-Phase2).
//!
//! Level 0 (β₀) — Connected components:
//!   Build a graph where nodes are snapshots in the window.
//!   Two nodes are connected if |psi_i - psi_j| < threshold AND same momentum sign.
//!   Count connected components using union-find.
//!
//! Level 1 — Fiber partitioning:
//!   Partition the window into temporal fibers: consecutive runs of snapshots that
//!   share the same regime direction (momentum > 0 = ascending, ≤ 0 = descending).
//!   Each fiber tracks its mean coherence.
//!
//! Level 2 — Meta-convergence:
//!   A crystal is detected when:
//!     • There is exactly one connected component (all signals coherent).
//!     • All fibers have mean coherence > level2_threshold.
//!     • The mean psi over the window exceeds crystal_psi_threshold.

use anyhow::Result;
use fsr_fixed::ONE;
use fsr_types::{ResonanceSnapshot, Q32};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs,
    path::{Path, PathBuf},
};

pub type ComponentId = u32;

/// A temporal fiber: consecutive snapshots sharing the same momentum sign.
#[derive(Clone, Debug)]
pub struct Fiber {
    pub id: u32,
    /// Indices into the current window VecDeque.
    pub snapshot_indices: Vec<usize>,
    /// Mean kappa coherence across fiber snapshots.
    pub mean_coherence: Q32,
}

/// A crystal artifact emitted when meta-convergence is detected.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TtcpCrystal {
    /// Tick at which the crystal was detected.
    pub tick: u64,
    /// Cascade level that triggered emission (2 = full meta-convergence).
    pub level: u8,
    /// Number of connected components at detection.
    pub components: usize,
    /// Number of temporal fibers.
    pub fibers: usize,
    /// Convergence score in Q32 (mean psi across the convergence window).
    pub convergence_score: Q32,
    /// Mean resonance ψ across the window.
    pub resonance_psi: Q32,
    /// Mean resonance ρ across the window.
    pub resonance_rho: Q32,
}

impl TtcpCrystal {
    /// Serialize to JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Write to `dir/ttcp_{tick:010}.json`. Creates dir if needed.
    pub fn write_to_dir(&self, dir: &Path) -> Result<PathBuf> {
        fs::create_dir_all(dir)?;
        let filename = format!("ttcp_{:010}.json", self.tick);
        let path = dir.join(&filename);
        fs::write(&path, self.to_json())?;
        Ok(path)
    }
}

/// Configuration for the TTCP engine.
#[derive(Clone, Debug)]
pub struct TtcpConfig {
    /// Size of the rolling snapshot window.
    pub window_size: usize,
    /// Q32 threshold for "connected" in level-0 graph (|Δpsi| < threshold).
    pub level0_psi_threshold: Q32,
    /// Q32 minimum mean coherence for a fiber to be "converging" (level 1).
    pub level1_coherence_threshold: Q32,
    /// Q32 minimum mean psi for meta-convergence (level 2).
    pub crystal_psi_threshold: Q32,
}

impl Default for TtcpConfig {
    fn default() -> Self {
        TtcpConfig {
            window_size: 20,
            // 0.15 in Q32
            level0_psi_threshold: (ONE as f64 * 0.15) as Q32,
            // 0.20 in Q32
            level1_coherence_threshold: (ONE as f64 * 0.20) as Q32,
            // 0.30 in Q32
            crystal_psi_threshold: (ONE as f64 * 0.30) as Q32,
        }
    }
}

/// The TTCP analysis engine. Feed it one snapshot per tick.
pub struct TtcpEngine {
    config: TtcpConfig,
    window: VecDeque<ResonanceSnapshot>,
    pub crystals_found: u64,
    pub last_crystal_tick: Option<u64>,
}

impl TtcpEngine {
    pub fn new(config: TtcpConfig) -> Self {
        TtcpEngine {
            config,
            window: VecDeque::new(),
            crystals_found: 0,
            last_crystal_tick: None,
        }
    }

    pub fn with_window(window_size: usize) -> Self {
        Self::new(TtcpConfig {
            window_size,
            ..TtcpConfig::default()
        })
    }

    /// Ingest a new snapshot and run the 3-level cascade.
    /// Returns `Some(TtcpCrystal)` if meta-convergence is detected this tick.
    pub fn push_snapshot(&mut self, snap: ResonanceSnapshot) -> Option<TtcpCrystal> {
        let tick = snap.tick;
        self.window.push_back(snap);
        if self.window.len() > self.config.window_size {
            self.window.pop_front();
        }
        // Need a full window before attempting cascade.
        if self.window.len() < self.config.window_size {
            return None;
        }

        // Level 0: connected components.
        let components = self.compute_level0();
        let n_components = components.iter().copied().max().map_or(0, |m| m as usize + 1);

        // Level 1: fiber partitioning.
        let fibers = self.compute_level1();

        // Level 2: meta-convergence check.
        self.compute_level2(tick, n_components, &fibers)
    }

    // ── Level 0: Union-Find connected components ───────────────────────────────

    fn compute_level0(&self) -> Vec<ComponentId> {
        let n = self.window.len();
        let mut parent: Vec<usize> = (0..n).collect();

        let threshold = self.config.level0_psi_threshold;
        let snaps: Vec<&ResonanceSnapshot> = self.window.iter().collect();

        for i in 0..n {
            for j in (i + 1)..n {
                let delta_psi = (snaps[i].psi - snaps[j].psi).unsigned_abs() as Q32;
                let same_momentum_sign =
                    (snaps[i].momentum >= 0) == (snaps[j].momentum >= 0);
                if delta_psi < threshold && same_momentum_sign {
                    union(&mut parent, i, j);
                }
            }
        }

        // Normalize to contiguous component IDs.
        let roots: Vec<usize> = (0..n).map(|i| find(&parent, i)).collect();
        let mut id_map = std::collections::HashMap::new();
        let mut next_id: ComponentId = 0;
        roots
            .iter()
            .map(|&r| {
                *id_map.entry(r).or_insert_with(|| {
                    let id = next_id;
                    next_id += 1;
                    id
                })
            })
            .collect()
    }

    // ── Level 1: Temporal fiber partitioning ──────────────────────────────────

    fn compute_level1(&self) -> Vec<Fiber> {
        let snaps: Vec<&ResonanceSnapshot> = self.window.iter().collect();
        let n = snaps.len();
        let mut fibers: Vec<Fiber> = Vec::new();
        let mut current_sign: Option<bool> = None;
        let mut current_indices: Vec<usize> = Vec::new();

        for i in 0..n {
            let positive = snaps[i].momentum >= 0;
            if current_sign == Some(positive) {
                current_indices.push(i);
            } else {
                if !current_indices.is_empty() {
                    fibers.push(make_fiber(fibers.len() as u32, &current_indices, &snaps));
                }
                current_sign = Some(positive);
                current_indices = vec![i];
            }
        }
        if !current_indices.is_empty() {
            fibers.push(make_fiber(fibers.len() as u32, &current_indices, &snaps));
        }
        fibers
    }

    // ── Level 2: Meta-convergence ──────────────────────────────────────────────

    fn compute_level2(
        &mut self,
        tick: u64,
        n_components: usize,
        fibers: &[Fiber],
    ) -> Option<TtcpCrystal> {
        // Convergence requires: single component + all fibers above coherence threshold.
        if n_components != 1 {
            return None;
        }
        let all_fibers_coherent = fibers.iter().all(|f| {
            f.mean_coherence >= self.config.level1_coherence_threshold
        });
        if !all_fibers_coherent {
            return None;
        }

        let snaps: Vec<&ResonanceSnapshot> = self.window.iter().collect();
        let n = snaps.len() as Q32;
        let mean_psi = snaps.iter().map(|s| s.psi).sum::<Q32>() / n;
        let mean_rho = snaps.iter().map(|s| s.rho).sum::<Q32>() / n;

        if mean_psi < self.config.crystal_psi_threshold {
            return None;
        }

        self.crystals_found += 1;
        self.last_crystal_tick = Some(tick);

        Some(TtcpCrystal {
            tick,
            level: 2,
            components: n_components,
            fibers: fibers.len(),
            convergence_score: mean_psi,
            resonance_psi: mean_psi,
            resonance_rho: mean_rho,
        })
    }
}

// ── Union-Find helpers ─────────────────────────────────────────────────────────

fn find(parent: &[usize], mut x: usize) -> usize {
    while parent[x] != x {
        x = parent[x];
    }
    x
}

fn union(parent: &mut [usize], a: usize, b: usize) {
    let ra = find(parent, a);
    let rb = find(parent, b);
    if ra != rb {
        parent[ra] = rb;
    }
}

fn make_fiber(id: u32, indices: &[usize], snaps: &[&ResonanceSnapshot]) -> Fiber {
    let mean_coherence = if indices.is_empty() {
        0
    } else {
        let sum: Q32 = indices.iter().map(|&i| snaps[i].kappa).sum();
        sum / indices.len() as Q32
    };
    Fiber {
        id,
        snapshot_indices: indices.to_vec(),
        mean_coherence,
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_fixed::ONE;

    fn make_snap(tick: u64, psi: Q32, rho: Q32, kappa: Q32, momentum: Q32) -> ResonanceSnapshot {
        ResonanceSnapshot {
            kappa,
            entropy: ONE / 2,
            sync: ONE / 2,
            momentum,
            si: psi / 2,
            psi,
            rho,
            omega: ONE / 2,
            tick,
        }
    }

    #[test]
    fn test_ttcp_engine_no_crash_on_sparse_input() {
        let mut engine = TtcpEngine::with_window(5);
        for i in 0..3u64 {
            // Fewer than window_size → never crystals
            let result = engine.push_snapshot(make_snap(i, ONE / 3, ONE / 2, ONE / 4, ONE / 4));
            assert!(result.is_none(), "no crystal before window full");
        }
    }

    #[test]
    fn test_ttcp_engine_full_window_no_diverge() {
        let mut engine = TtcpEngine::with_window(5);
        // Feed 5 highly coherent, same-sign momentum snapshots
        // psi = 0.5, rho = 0.6, kappa = 0.5, momentum = +0.1
        // Should satisfy level0, level1, level2 (crystal_psi_threshold = 0.3 default).
        let psi = (ONE as f64 * 0.5) as Q32;
        let rho = (ONE as f64 * 0.6) as Q32;
        let kappa = (ONE as f64 * 0.5) as Q32;
        let momentum = (ONE as f64 * 0.1) as Q32;

        let mut last = None;
        for i in 0..5u64 {
            last = engine.push_snapshot(make_snap(i, psi, rho, kappa, momentum));
        }
        // After 5 identical coherent snapshots: expect a crystal.
        assert!(
            last.is_some(),
            "expected crystal from fully coherent window"
        );
        let crystal = last.unwrap();
        assert_eq!(crystal.level, 2);
        assert_eq!(crystal.components, 1);
        assert!(crystal.convergence_score > 0);
    }

    #[test]
    fn test_ttcp_engine_divergent_signals_no_crystal() {
        let mut engine = TtcpEngine::with_window(5);
        // Alternating positive/negative momentum → many fibers, not converged.
        let psi_high = (ONE as f64 * 0.8) as Q32;
        let psi_low = (ONE as f64 * 0.05) as Q32;

        for i in 0..5u64 {
            let psi = if i % 2 == 0 { psi_high } else { psi_low };
            let momentum = if i % 2 == 0 { ONE / 4 } else { -(ONE / 4) };
            engine.push_snapshot(make_snap(i, psi, ONE / 2, ONE / 4, momentum));
        }
        // Divergent psi values → level0 components > 1 → no crystal.
        assert_eq!(engine.crystals_found, 0);
    }

    #[test]
    fn test_crystal_write_to_dir() {
        let dir = std::env::temp_dir().join("fsr_ttcp_crystal_test");
        let _ = std::fs::remove_dir_all(&dir);

        let crystal = TtcpCrystal {
            tick: 42,
            level: 2,
            components: 1,
            fibers: 2,
            convergence_score: ONE / 2,
            resonance_psi: ONE / 2,
            resonance_rho: ONE / 3,
        };
        let path = crystal.write_to_dir(&dir).unwrap();
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"tick\""));
        assert!(content.contains("42"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
