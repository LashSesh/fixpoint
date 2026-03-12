//! Core shared types for ISLS (ISLS Definitions 4.1–4.6).

use fsr_types::{Hash256, Q32};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Stable identifier for a graph entity (token, exchange, pool, protocol, etc.).
pub type EntityId = u64;

/// Stable identifier for an edge in the PersistentGraph.
pub type EdgeId = u64;

/// Tripolar state: coherence (ψ), density (ρ), phase frequency (ω).
/// (ISLS Definition 4.3)
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct TripolarState {
    /// Coherence amplitude ψ ∈ [0, ONE].
    pub psi: Q32,
    /// Density ρ ∈ [0, ONE].
    pub rho: Q32,
    /// Phase frequency ω.
    pub omega: Q32,
}

/// A vertex record stored in the PersistentGraph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VertexRecord {
    pub id: EntityId,
    pub label: String,
    pub first_seen: u64,
    pub last_seen: u64,
    pub observation_count: u64,
    /// 5D embedding coordinates [Q32; 5].
    pub embedding: [Q32; 5],
    pub tripolar: TripolarState,
}

impl VertexRecord {
    pub fn new(id: EntityId, label: String, tick: u64) -> Self {
        VertexRecord {
            id,
            label,
            first_seen: tick,
            last_seen: tick,
            observation_count: 1,
            embedding: [0i64; 5],
            tripolar: TripolarState::default(),
        }
    }
}

/// A typed directed edge in the PersistentGraph.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TypedEdge {
    pub id: EdgeId,
    pub from: EntityId,
    pub to: EntityId,
    pub edge_type: EdgeType,
    pub weight: Q32,
    pub created_tick: u64,
    pub last_updated_tick: u64,
    pub annotations: Vec<u8>,
}

/// Edge classification.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeType {
    /// Correlation edge (Hypha layer).
    Correlation,
    /// Triangulation edge (Mycelium layer).
    Triangulation,
    /// Membership (token belongs to exchange/pool).
    Membership,
    /// Causal (Granger-like, future extension).
    Causal,
}

/// Topological signature of a crystal or subgraph region.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TopoSignature {
    /// Betti numbers [β₀, β₁, β₂].
    pub betti: [u32; 3],
    /// Stability score τ(C) ∈ [0, ONE].
    pub stability: Q32,
    /// Digest of the topological description.
    pub digest: Hash256,
}

/// A subgraph region (set of vertex IDs that form the condensed region).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SubGraph {
    pub vertices: Vec<EntityId>,
    pub edges: Vec<EdgeId>,
}

/// Persistent graph: the long-term knowledge structure.
/// (ISLS Definition 4.2)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PersistentGraph {
    /// Φ: vertices indexed by EntityId.
    pub vertices: BTreeMap<EntityId, VertexRecord>,
    /// Λ: edges.
    pub edges: Vec<TypedEdge>,
    /// Σ: 5D embeddings per entity.
    pub embedding: BTreeMap<EntityId, [Q32; 5]>,
    /// Historical embedding archive (compressed snapshots).
    pub tensor_archive: Vec<u8>,
    /// Edge annotations (feature vectors) per EdgeId.
    pub edge_annotations: BTreeMap<EdgeId, Vec<Q32>>,
    /// Next available edge ID.
    pub next_edge_id: EdgeId,
}

impl PersistentGraph {
    pub fn new() -> Self {
        PersistentGraph::default()
    }

    /// Upsert a vertex, returning the EntityId.
    pub fn upsert_vertex(&mut self, id: EntityId, label: &str, tick: u64) {
        if let Some(v) = self.vertices.get_mut(&id) {
            v.last_seen = tick;
            v.observation_count += 1;
        } else {
            self.vertices.insert(id, VertexRecord::new(id, label.to_string(), tick));
            self.embedding.insert(id, [0i64; 5]);
        }
    }

    /// Add or update an edge between two vertices.
    pub fn upsert_edge(
        &mut self,
        from: EntityId,
        to: EntityId,
        edge_type: EdgeType,
        weight: Q32,
        tick: u64,
    ) -> EdgeId {
        // Check if edge already exists.
        if let Some(e) = self.edges.iter_mut().find(|e| {
            e.from == from && e.to == to && e.edge_type == edge_type
        }) {
            e.weight = weight;
            e.last_updated_tick = tick;
            return e.id;
        }
        let id = self.next_edge_id;
        self.next_edge_id += 1;
        self.edges.push(TypedEdge {
            id,
            from,
            to,
            edge_type,
            weight,
            created_tick: tick,
            last_updated_tick: tick,
            annotations: vec![],
        });
        id
    }

    /// Remove edges below a weight threshold (pruning).
    pub fn prune_edges(&mut self, min_weight: Q32) {
        self.edges.retain(|e| e.weight >= min_weight);
    }

    /// Get edges for a specific vertex.
    pub fn edges_for(&self, vertex_id: EntityId) -> Vec<&TypedEdge> {
        self.edges.iter().filter(|e| e.from == vertex_id || e.to == vertex_id).collect()
    }

    /// Get N-hop neighborhood (simplified BFS to depth).
    pub fn neighborhood(&self, start: EntityId, depth: u8) -> Vec<EntityId> {
        let mut visited = std::collections::HashSet::new();
        let mut queue = vec![(start, 0u8)];
        let mut result = vec![];
        while let Some((vid, d)) = queue.pop() {
            if visited.contains(&vid) { continue; }
            visited.insert(vid);
            result.push(vid);
            if d < depth {
                for e in &self.edges {
                    if e.from == vid && !visited.contains(&e.to) {
                        queue.push((e.to, d + 1));
                    } else if e.to == vid && !visited.contains(&e.from) {
                        queue.push((e.from, d + 1));
                    }
                }
            }
        }
        result
    }
}
