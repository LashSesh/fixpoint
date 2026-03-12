//! Fruiting layer: signal emission to DSHAE and ECLS.
//!
//! Emits signals when the Mycelium layer detects:
//!   - Vertices forming a stable cluster
//!   - Persistent triangles
//!   - Non-transitive correlation patterns

use crate::mycelium::{Triangle, VertexCluster};
use fsr_isls::types::EntityId;
use fsr_types::Q32;
use serde::{Deserialize, Serialize};

/// Signals emitted by MCCE to downstream systems.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum McceSignal {
    /// A stable cluster of correlated entities has formed.
    StableCluster {
        members: Vec<EntityId>,
        cohesion: Q32,
        tick: u64,
    },
    /// A persistent triangle (non-transitive arbitrage opportunity).
    PersistentTriangle {
        a: EntityId,
        b: EntityId,
        c: EntityId,
        score: Q32,
        tick: u64,
    },
    /// A correlation pattern has weakened (potential regime change).
    CorrelationDecay {
        from: EntityId,
        to: EntityId,
        old_rho: Q32,
        new_rho: Q32,
        tick: u64,
    },
    /// Graph has grown significantly (new vertices/edges added).
    GraphGrowth {
        new_vertices: u64,
        new_edges: u64,
        total_vertices: u64,
        tick: u64,
    },
}

/// Fruiting layer: evaluates triangles and clusters, emits McceSignals.
pub struct FruitingLayer {
    pub signals: Vec<McceSignal>,
    pub fruiting_interval: u64,
    pub last_fruiting_tick: u64,
    pub min_cluster_size: usize,
    pub min_triangle_score: Q32,
}

impl FruitingLayer {
    pub fn new(fruiting_interval: u64, min_triangle_score: Q32) -> Self {
        FruitingLayer {
            signals: Vec::new(),
            fruiting_interval,
            last_fruiting_tick: 0,
            min_cluster_size: 3,
            min_triangle_score,
        }
    }

    /// Check if fruiting should run this tick.
    pub fn should_fruit(&self, tick: u64) -> bool {
        tick >= self.last_fruiting_tick + self.fruiting_interval
    }

    /// Process triangles and clusters, emit signals.
    pub fn fruit(
        &mut self,
        triangles: &[Triangle],
        clusters: &[VertexCluster],
        new_vertices: u64,
        new_edges: u64,
        total_vertices: u64,
        tick: u64,
    ) -> Vec<McceSignal> {
        self.last_fruiting_tick = tick;
        let mut emitted = Vec::new();

        for tri in triangles {
            if tri.score >= self.min_triangle_score {
                emitted.push(McceSignal::PersistentTriangle {
                    a: tri.a,
                    b: tri.b,
                    c: tri.c,
                    score: tri.score,
                    tick,
                });
            }
        }

        for cluster in clusters {
            if cluster.members.len() >= self.min_cluster_size {
                emitted.push(McceSignal::StableCluster {
                    members: cluster.members.clone(),
                    cohesion: cluster.cohesion,
                    tick,
                });
            }
        }

        if new_vertices > 0 || new_edges > 0 {
            emitted.push(McceSignal::GraphGrowth {
                new_vertices,
                new_edges,
                total_vertices,
                tick,
            });
        }

        self.signals.extend(emitted.clone());
        emitted
    }

    /// Drain the signal queue.
    pub fn drain_signals(&mut self) -> Vec<McceSignal> {
        std::mem::take(&mut self.signals)
    }
}

impl Default for FruitingLayer {
    fn default() -> Self {
        use fsr_fixed::ONE;
        Self::new(100, ONE / 2)
    }
}
