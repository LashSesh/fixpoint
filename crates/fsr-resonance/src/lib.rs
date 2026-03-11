//! fsr-resonance: Resonance metrics, Spiral Index, Evaluation Triplet (spec §8, §16).
//!
//! Computes κ, H, sync, M, SI in Q32 fixed-point.
//! The evaluation triplet Φ(x) = (ψ, ρ, ω) feeds the Kairos gate score.

use fsr_fixed::{
    q32_clamp, q32_coherence, q32_div, q32_entropy, q32_from_ratio, q32_mul, q32_tanh, ONE,
};
use fsr_types::{Q32, ResonanceSnapshot};
use serde::{Deserialize, Serialize};

/// Configuration for resonance computation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResonanceConfig {
    /// Ψ: regime-dependent amplification factor (Q32).
    pub psi_amp: Q32,
    /// SI normalization constant SImax (Q32, basis points).
    pub si_max: Q32,
    /// Fee penalty (Q32, basis points).
    pub fees_bp: Q32,
    /// Slippage estimate (Q32, basis points).
    pub slip_bp: Q32,
    /// Gas/latency decay penalty (Q32, basis points).
    pub latency_decay_bp: Q32,
    /// Window size for ρ (regime stability rolling window).
    pub stability_window: u64,
    /// Window size for ω (execution efficiency rolling window).
    pub efficiency_window: u64,
}

impl Default for ResonanceConfig {
    fn default() -> Self {
        ResonanceConfig {
            psi_amp: ONE,                          // Ψ = 1.0
            si_max: 100 * ONE,                     // SImax = 100.0 in Q32
            fees_bp: q32_from_ratio(10, 1),        // 10 bp
            slip_bp: q32_from_ratio(5, 1),         // 5 bp
            latency_decay_bp: q32_from_ratio(2, 1),// 2 bp
            stability_window: 100,
            efficiency_window: 100,
        }
    }
}

/// Rolling window accumulator for regime stability (ρ).
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct StabilityAccumulator {
    pub alpha_ticks: u64,
    pub beta_ticks: u64,
    pub total_ticks: u64,
    pub window: u64,
}

impl StabilityAccumulator {
    pub fn new(window: u64) -> Self {
        StabilityAccumulator {
            alpha_ticks: 0,
            beta_ticks: 0,
            total_ticks: 0,
            window,
        }
    }

    /// Record a tick in the given regime.
    pub fn record(&mut self, regime: fsr_types::RegimeState) {
        use fsr_types::RegimeState;
        self.total_ticks += 1;
        match regime {
            RegimeState::Alpha => self.alpha_ticks += 1,
            RegimeState::Beta => self.beta_ticks += 1,
            RegimeState::Gamma => {}
        }
        // Simple sliding: reset if window exceeded
        if self.total_ticks > self.window {
            // decay oldest by proportional reduction
            let excess = self.total_ticks - self.window;
            self.total_ticks = self.window;
            let alpha_decay = self.alpha_ticks * excess / (self.window + excess);
            let beta_decay = self.beta_ticks * excess / (self.window + excess);
            self.alpha_ticks = self.alpha_ticks.saturating_sub(alpha_decay);
            self.beta_ticks = self.beta_ticks.saturating_sub(beta_decay);
        }
    }

    /// ρ(x) = (alpha_ticks + beta_ticks) / window_size (spec §8).
    pub fn rho(&self) -> Q32 {
        let stable = self.alpha_ticks + self.beta_ticks;
        let denom = self.window.max(1);
        q32_clamp(q32_from_ratio(stable as i64, denom as i64), 0, ONE)
    }
}

/// Rolling window accumulator for execution efficiency (ω).
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct EfficiencyAccumulator {
    pub settled: u64,
    pub aborted: u64,
    pub window: u64,
}

impl EfficiencyAccumulator {
    pub fn new(window: u64) -> Self {
        EfficiencyAccumulator {
            settled: 0,
            aborted: 0,
            window,
        }
    }

    pub fn record_settled(&mut self) {
        self.settled += 1;
        self.decay_if_needed();
    }

    pub fn record_aborted(&mut self) {
        self.aborted += 1;
        self.decay_if_needed();
    }

    fn decay_if_needed(&mut self) {
        let total = self.settled + self.aborted;
        if total > self.window {
            // proportional decay
            let excess = total - self.window;
            let s_decay = self.settled * excess / total;
            let a_decay = self.aborted * excess / total;
            self.settled = self.settled.saturating_sub(s_decay);
            self.aborted = self.aborted.saturating_sub(a_decay);
        }
    }

    /// ω(x) = settled / (settled + aborted + ε) (spec §8).
    pub fn omega(&self) -> Q32 {
        // ε term: use 1 lot as epsilon to avoid division by zero
        let denom = self.settled + self.aborted + 1;
        q32_clamp(q32_from_ratio(self.settled as i64, denom as i64), 0, ONE)
    }
}

/// Resonance engine: computes all metrics from route signal observations.
pub struct ResonanceEngine {
    pub config: ResonanceConfig,
    pub stability: StabilityAccumulator,
    pub efficiency: EfficiencyAccumulator,
}

impl ResonanceEngine {
    pub fn new(config: ResonanceConfig) -> Self {
        let stability = StabilityAccumulator::new(config.stability_window);
        let efficiency = EfficiencyAccumulator::new(config.efficiency_window);
        ResonanceEngine {
            config,
            stability,
            efficiency,
        }
    }

    /// Compute a full ResonanceSnapshot from normalized route signals r_i ∈ [0,1] (as Q32).
    /// leak: leakage penalty from the WT reconstruction residual (Q32, basis points).
    /// tick: current macro-cycle counter.
    pub fn compute_snapshot(
        &self,
        signals: &[Q32],
        leak: Q32,
        tick: u64,
    ) -> ResonanceSnapshot {
        if signals.is_empty() {
            return ResonanceSnapshot {
                kappa: 0,
                entropy: 0,
                sync: 0,
                momentum: 0,
                si: 0,
                psi: 0,
                rho: self.stability.rho(),
                omega: self.efficiency.omega(),
                tick,
            };
        }

        // κ = coherence(r1..rn) (spec §16.1)
        let kappa = q32_coherence(signals);

        // H = entropy(r1..rn) (spec §16.1)
        let entropy = q32_entropy(signals);

        // sync = (1/n) Σ ri (spec §16.1)
        let n = signals.len() as i64;
        let sum: i64 = signals.iter().copied().fold(0i64, |acc, v| acc.saturating_add(v));
        let sync = q32_from_ratio(sum, n * ONE);

        // M = tanh(2·sync - 1) (spec §16.1)
        let arg = q32_mul(2 * ONE, sync) - ONE;
        let momentum = q32_tanh(arg);

        // SI = Ψ · κ · max(0, M) - (H + fees + slip + gas + latency_decay + leak) (spec §16.2)
        let max_m = q32_clamp(momentum, 0, ONE);
        let positive_term = q32_mul(self.config.psi_amp, q32_mul(kappa, max_m));
        let penalty = entropy
            .saturating_add(self.config.fees_bp)
            .saturating_add(self.config.slip_bp)
            .saturating_add(self.config.latency_decay_bp)
            .saturating_add(leak);
        let si = positive_term.saturating_sub(penalty);

        // ψ(x) = clamp(SI / SImax, 0, 1) (spec §8)
        let psi = if self.config.si_max > 0 {
            q32_clamp(q32_div(si, self.config.si_max), 0, ONE)
        } else {
            0
        };

        // ρ and ω from accumulators
        let rho = self.stability.rho();
        let omega = self.efficiency.omega();

        ResonanceSnapshot {
            kappa,
            entropy,
            sync,
            momentum,
            si,
            psi,
            rho,
            omega,
            tick,
        }
    }
}

/// Compute net edge for a route (spec §16.3).
/// ΠZ = Π p_i, NetEdgeZ = ΠZ - 1 - Σ(fi + si(qi))
/// All values in Q32 (as basis point fractions).
pub fn compute_net_edge(leg_prices: &[Q32], leg_costs: &[Q32]) -> Q32 {
    if leg_prices.is_empty() {
        return -ONE;
    }
    // ΠZ = product of normalized price ratios
    let mut product = ONE;
    for &p in leg_prices {
        product = q32_mul(product, p);
    }
    // Σ costs
    let total_cost: i64 = leg_costs.iter().copied().fold(0i64, |acc, c| acc.saturating_add(c));
    // NetEdge = ΠZ - 1 - Σcosts
    product.saturating_sub(ONE).saturating_sub(total_cost)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_fixed::{HALF, ONE};
    use fsr_types::RegimeState;

    #[test]
    fn test_snapshot_empty_signals() {
        let engine = ResonanceEngine::new(ResonanceConfig::default());
        let snap = engine.compute_snapshot(&[], 0, 0);
        assert_eq!(snap.kappa, 0);
        assert_eq!(snap.entropy, 0);
        assert_eq!(snap.si, 0);
    }

    #[test]
    fn test_snapshot_uniform_signals() {
        let engine = ResonanceEngine::new(ResonanceConfig::default());
        let signals = vec![HALF, HALF, HALF, HALF];
        let snap = engine.compute_snapshot(&signals, 0, 1);
        // sync = 0.5, M = tanh(0) = 0
        assert_eq!(snap.sync, HALF);
        assert_eq!(snap.momentum, 0);
        // All identical → coherence = 1
        assert_eq!(snap.kappa, ONE);
    }

    #[test]
    fn test_stability_accumulator() {
        let mut acc = StabilityAccumulator::new(10);
        for _ in 0..7 {
            acc.record(RegimeState::Alpha);
        }
        for _ in 0..3 {
            acc.record(RegimeState::Gamma);
        }
        let rho = acc.rho();
        // 7/10 = 0.7
        let expected = q32_from_ratio(7, 10);
        let diff = (rho - expected).abs();
        assert!(diff < ONE / 100, "rho ≈ 0.7, got {:?}", rho);
    }

    #[test]
    fn test_efficiency_accumulator() {
        let mut acc = EfficiencyAccumulator::new(100);
        for _ in 0..8 {
            acc.record_settled();
        }
        for _ in 0..2 {
            acc.record_aborted();
        }
        let omega = acc.omega();
        // 8 / (8+2+1) ≈ 0.727
        assert!(omega > HALF, "omega should be > 0.5");
    }

    #[test]
    fn test_net_edge_positive() {
        // All leg prices = 1.0, no costs => product = 1, net = 0
        let prices = vec![ONE, ONE, ONE];
        let costs = vec![0i64, 0i64, 0i64];
        let ne = compute_net_edge(&prices, &costs);
        assert_eq!(ne, 0);
    }

    #[test]
    fn test_net_edge_negative_when_costly() {
        let prices = vec![ONE, ONE, ONE];
        // cost = 10bp each
        let c = fsr_fixed::BP_ONE * 10;
        let costs = vec![c, c, c];
        let ne = compute_net_edge(&prices, &costs);
        assert!(ne < 0, "net edge should be negative with costs");
    }

    #[test]
    fn test_psi_bounded() {
        let engine = ResonanceEngine::new(ResonanceConfig::default());
        let signals = vec![ONE, ONE, HALF, ONE];
        let snap = engine.compute_snapshot(&signals, 0, 0);
        assert!(snap.psi >= 0, "ψ must be ≥ 0");
        assert!(snap.psi <= ONE, "ψ must be ≤ 1");
    }
}
