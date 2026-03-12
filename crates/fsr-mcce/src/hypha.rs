//! Hypha layer: pairwise correlation engine with exponential decay.
//!
//! Computes rolling Pearson correlations (log-returns, 500-tick windows).
//! Stores edge annotations in the PersistentGraph.
//! Prunes edges that decay below the minimum weight threshold.

use crate::config::McceConfig;
use crate::edge::CorrelationEdge;
use fsr_fixed::ONE;
use fsr_isls::types::EntityId;
use fsr_types::Q32;
use std::collections::HashMap;

/// Price history buffer for a single entity.
#[derive(Clone, Debug, Default)]
pub struct PriceBuffer {
    pub prices: Vec<Q32>,
    pub max_len: usize,
}

impl PriceBuffer {
    pub fn new(max_len: usize) -> Self {
        PriceBuffer { prices: Vec::new(), max_len }
    }

    pub fn push(&mut self, price: Q32) {
        if self.prices.len() >= self.max_len {
            self.prices.remove(0);
        }
        self.prices.push(price);
    }

    /// Compute log-returns: return vec of (p[i+1] - p[i]) / p[i] as Q32.
    pub fn log_returns(&self) -> Vec<Q32> {
        if self.prices.len() < 2 {
            return vec![];
        }
        self.prices.windows(2).map(|w| {
            if w[0] == 0 { 0 }
            else { ((w[1] - w[0]) as i128 * ONE as i128 / w[0] as i128) as Q32 }
        }).collect()
    }
}

/// Pearson correlation between two equal-length slices of Q32 values.
/// Returns Q32 ∈ [-ONE, ONE].
pub fn pearson_q32(xs: &[Q32], ys: &[Q32]) -> Q32 {
    let n = xs.len().min(ys.len());
    if n < 2 {
        return 0;
    }
    let n64 = n as i128;

    let sum_x: i128 = xs[..n].iter().map(|&v| v as i128).sum();
    let sum_y: i128 = ys[..n].iter().map(|&v| v as i128).sum();
    let mean_x = sum_x / n64;
    let mean_y = sum_y / n64;

    let mut cov: i128 = 0;
    let mut var_x: i128 = 0;
    let mut var_y: i128 = 0;
    for i in 0..n {
        let dx = xs[i] as i128 - mean_x;
        let dy = ys[i] as i128 - mean_y;
        cov += dx * dy / ONE as i128;
        var_x += dx * dx / ONE as i128;
        var_y += dy * dy / ONE as i128;
    }

    let denom_sq = var_x * var_y;
    if denom_sq <= 0 {
        return 0;
    }
    // sqrt approximation: integer Newton's method
    let denom = isqrt(denom_sq);
    if denom == 0 {
        return 0;
    }
    ((cov * ONE as i128) / denom) as Q32
}

fn isqrt(n: i128) -> i128 {
    if n <= 0 { return 0; }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Hypha layer state.
pub struct HyphaLayer {
    /// Price history per entity.
    pub price_buffers: HashMap<EntityId, PriceBuffer>,
    /// Correlation edges.
    pub edges: Vec<CorrelationEdge>,
    pub config: McceConfig,
}

impl HyphaLayer {
    pub fn new(config: McceConfig) -> Self {
        HyphaLayer {
            price_buffers: HashMap::new(),
            edges: Vec::new(),
            config,
        }
    }

    /// Push a price observation for an entity.
    pub fn push_price(&mut self, entity_id: EntityId, price: Q32) {
        let buf = self.price_buffers
            .entry(entity_id)
            .or_insert_with(|| PriceBuffer::new(self.config.hypha_window_ticks));
        buf.push(price);
    }

    /// Update all pairwise correlations and decay edges.
    pub fn update_correlations(&mut self, tick: u64) {
        let ids: Vec<EntityId> = self.price_buffers.keys().copied().collect();
        let decay_rate = self.config.hypha_decay_rate;
        let min_rho = self.config.hypha_min_rho;

        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let a = ids[i];
                let b = ids[j];
                let rho = {
                    let ra = self.price_buffers[&a].log_returns();
                    let rb = self.price_buffers[&b].log_returns();
                    if ra.len() >= 10 && rb.len() >= 10 {
                        pearson_q32(&ra, &rb)
                    } else {
                        continue;
                    }
                };
                let abs_rho = rho.abs();
                if abs_rho < min_rho {
                    // Below minimum: decay existing edge if present.
                    if let Some(e) = self.edges.iter_mut().find(|e| e.from == a && e.to == b) {
                        e.decay_and_update(rho, tick, decay_rate);
                    }
                    continue;
                }

                if let Some(e) = self.edges.iter_mut().find(|e| e.from == a && e.to == b) {
                    e.decay_and_update(rho, tick, decay_rate);
                } else {
                    self.edges.push(CorrelationEdge::new(a, b, rho, tick));
                }
            }
        }

        // Prune insignificant edges.
        let min_weight = ONE / 100; // 1% minimum weight
        self.edges.retain(|e| e.is_significant(min_weight));
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn get_correlation(&self, a: EntityId, b: EntityId) -> Option<Q32> {
        self.edges.iter().find(|e| (e.from == a && e.to == b) || (e.from == b && e.to == a))
            .map(|e| e.rho)
    }
}
