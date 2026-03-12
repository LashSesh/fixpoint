//! Mycelium layer: HDAG construction, hypercube embedding, topological features.
//!
//! Runs TTCP-style triangulation on the vertex cloud.
//! Detects persistent topological features (loops, clusters).

use crate::edge::{CorrelationEdge, TriangulationEdge};
use crate::vertex::McceVertex;
use fsr_fixed::ONE;
use fsr_isls::types::EntityId;
use fsr_types::Q32;
use std::collections::HashMap;

/// A detected cluster of correlated entities.
#[derive(Clone, Debug)]
pub struct VertexCluster {
    pub members: Vec<EntityId>,
    pub cohesion: Q32,
    pub formed_tick: u64,
    pub persistence: u64,
}

/// Triangulation result.
#[derive(Clone, Debug)]
pub struct Triangle {
    pub a: EntityId,
    pub b: EntityId,
    pub c: EntityId,
    /// Sum of edge weights in triangle.
    pub score: Q32,
    pub tick: u64,
}

/// Mycelium layer: triangulation and cluster detection.
pub struct MyceliumLayer {
    pub triangulations: Vec<TriangulationEdge>,
    pub clusters: Vec<VertexCluster>,
}

impl MyceliumLayer {
    pub fn new() -> Self {
        MyceliumLayer {
            triangulations: Vec::new(),
            clusters: Vec::new(),
        }
    }

    /// Triangulate the vertex cloud using correlation edges.
    /// Finds triangles (A,B,C) where all three pairwise correlations are strong.
    pub fn triangulate(
        &mut self,
        vertices: &[EntityId],
        edges: &[CorrelationEdge],
        tick: u64,
        min_edge_rho: Q32,
    ) -> Vec<Triangle> {
        let mut triangles = Vec::new();

        // Build adjacency for quick lookup.
        let mut adj: HashMap<(EntityId, EntityId), Q32> = HashMap::new();
        for e in edges {
            if e.rho.abs() >= min_edge_rho {
                adj.insert((e.from, e.to), e.rho);
                adj.insert((e.to, e.from), e.rho);
            }
        }

        let n = vertices.len().min(50); // cap at 50 vertices to avoid O(n^3) explosion
        for i in 0..n {
            for j in (i + 1)..n {
                let a = vertices[i];
                let b = vertices[j];
                if !adj.contains_key(&(a, b)) { continue; }
                let rho_ab = adj[&(a, b)];
                for k in (j + 1)..n {
                    let c = vertices[k];
                    if let (Some(&rho_ac), Some(&rho_bc)) = (adj.get(&(a, c)), adj.get(&(b, c))) {
                        // All three edges strong → triangle.
                        let score = ((rho_ab.abs() as i128 + rho_ac.abs() as i128 + rho_bc.abs() as i128) / 3) as Q32;
                        triangles.push(Triangle { a, b, c, score, tick });
                    }
                }
            }
        }
        triangles
    }

    /// Detect stable clusters from triangles.
    pub fn detect_clusters(&mut self, triangles: &[Triangle], tick: u64) {
        // Simple: union-find based clustering of triangle vertices.
        let mut union: HashMap<EntityId, EntityId> = HashMap::new();

        fn find(union: &mut HashMap<EntityId, EntityId>, x: EntityId) -> EntityId {
            if !union.contains_key(&x) {
                union.insert(x, x);
                return x;
            }
            let root = *union.get(&x).unwrap();
            if root == x { return x; }
            let resolved = find(union, root);
            union.insert(x, resolved);
            resolved
        }

        for tri in triangles {
            let ra = find(&mut union, tri.a);
            let rb = find(&mut union, tri.b);
            let rc = find(&mut union, tri.c);
            union.insert(ra, rb);
            let rb2 = find(&mut union, rb);
            union.insert(rb2, rc);
        }

        // Collect clusters.
        let mut cluster_map: HashMap<EntityId, Vec<EntityId>> = HashMap::new();
        let keys: Vec<EntityId> = union.keys().copied().collect();
        for v in keys {
            let root = find(&mut union, v);
            cluster_map.entry(root).or_default().push(v);
        }

        self.clusters.clear();
        for (_, members) in cluster_map {
            if members.len() >= 3 {
                self.clusters.push(VertexCluster {
                    cohesion: ONE * 8 / 10,
                    members,
                    formed_tick: tick,
                    persistence: 1,
                });
            }
        }
    }

    /// Update embedding for a vertex based on resonance snapshot.
    pub fn update_vertex_embedding(
        vertex: &mut McceVertex,
        psi: Q32,
        rho: Q32,
        omega: Q32,
        momentum: Q32,
        entropy: Q32,
    ) {
        vertex.update_embedding(psi, rho, omega, momentum, entropy);
    }
}

impl Default for MyceliumLayer {
    fn default() -> Self {
        Self::new()
    }
}
