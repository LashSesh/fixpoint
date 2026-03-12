//! Mycelial HDAG: the top-level persistent graph structure for MCCE.
//!
//! MycelialHdag integrates all four MCCE layers (Spore, Hypha, Mycelium, Fruiting)
//! and the ISLS PersistentGraph as its backing storage.

use crate::config::McceConfig;
use crate::edge::CorrelationEdge;
use crate::fruiting::{FruitingLayer, McceSignal};
use crate::hypha::HyphaLayer;
use crate::mycelium::MyceliumLayer;
use crate::spore::SporeLayer;
use crate::vertex::{McceVertex, VertexType};
use fsr_isls::types::{EdgeType, EntityId, PersistentGraph};
use fsr_types::market::OrderBook;
use fsr_types::Q32;
use std::collections::HashMap;

/// The Mycelial HDAG: persistent topological learning graph.
pub struct MycelialHdag {
    pub graph: PersistentGraph,
    pub vertex_types: HashMap<EntityId, VertexType>,
    pub vertices: HashMap<EntityId, McceVertex>,
    pub hypha: HyphaLayer,
    pub mycelium: MyceliumLayer,
    pub fruiting: FruitingLayer,
    pub spore: SporeLayer,
    pub tick: u64,
    pub config: McceConfig,
    pub total_signals_emitted: u64,
}

impl MycelialHdag {
    pub fn new(config: McceConfig) -> Self {
        let fruiting = FruitingLayer::new(
            config.fruiting_interval,
            fsr_fixed::ONE / 2,
        );
        MycelialHdag {
            graph: PersistentGraph::new(),
            vertex_types: HashMap::new(),
            vertices: HashMap::new(),
            hypha: HyphaLayer::new(config.clone()),
            mycelium: MyceliumLayer::new(),
            fruiting,
            spore: SporeLayer::new(),
            tick: 0,
            config,
            total_signals_emitted: 0,
        }
    }

    /// Process OrderBooks: Spore + Hypha + Mycelium + Fruiting.
    pub fn tick_books(&mut self, books: &[OrderBook], tick: u64) -> Vec<McceSignal> {
        self.tick = tick;

        // Spore: discover new vertices.
        let new_verts = self.spore.ingest_books(books, tick);
        let new_vertex_count = new_verts.len() as u64;
        for v in new_verts {
            self.graph.upsert_vertex(v.id, &v.vertex_type.label(), tick);
            self.vertex_types.insert(v.id, v.vertex_type.clone());
            self.vertices.insert(v.id, v);
        }

        // Hypha: push mid prices and update correlations.
        for book in books {
            if let Some(mid) = book.mid_bp() {
                let base_id = VertexType::Token {
                    symbol: book.pair.0.clone(),
                    chain: "multi".to_string(),
                }.canonical_id();
                let quote_id = VertexType::Token {
                    symbol: book.pair.1.clone(),
                    chain: "multi".to_string(),
                }.canonical_id();
                self.hypha.push_price(base_id, mid);
                self.hypha.push_price(quote_id, mid);
            }
        }
        self.hypha.update_correlations(tick);

        // Sync Hypha edges into PersistentGraph.
        let new_edge_count = self.sync_hypha_edges(tick);

        // Mycelium: triangulate active vertices if fruiting interval reached.
        let signals = if self.fruiting.should_fruit(tick) {
            let vertex_ids: Vec<EntityId> = self.vertices.keys().copied().collect();
            let triangles = self.mycelium.triangulate(
                &vertex_ids,
                &self.hypha.edges,
                tick,
                self.config.hypha_min_rho,
            );
            self.mycelium.detect_clusters(&triangles, tick);

            // Fruiting: emit signals.
            let total_verts = self.vertices.len() as u64;
            let sigs = self.fruiting.fruit(
                &triangles,
                &self.mycelium.clusters,
                new_vertex_count,
                new_edge_count,
                total_verts,
                tick,
            );
            self.total_signals_emitted += sigs.len() as u64;
            sigs
        } else {
            vec![]
        };

        signals
    }

    /// Process raw mid prices (paper/sandbox mode).
    pub fn tick_mids(&mut self, mids: &[(usize, usize, i64)], tick: u64) -> Vec<McceSignal> {
        self.tick = tick;

        let new_verts = self.spore.ingest_mids(mids, tick);
        let new_vertex_count = new_verts.len() as u64;
        for v in new_verts {
            self.graph.upsert_vertex(v.id, &v.vertex_type.label(), tick);
            self.vertex_types.insert(v.id, v.vertex_type.clone());
            self.vertices.insert(v.id, v);
        }

        for (i, _j, price) in mids {
            let id = VertexType::Token {
                symbol: format!("TOKEN{}", i),
                chain: "sim".to_string(),
            }.canonical_id();
            self.hypha.push_price(id, *price);
        }

        let update_interval = self.config.embedding_update_interval;
        if tick % update_interval == 0 {
            self.hypha.update_correlations(tick);
        }

        let new_edge_count = self.sync_hypha_edges(tick);

        let signals = if self.fruiting.should_fruit(tick) {
            let vertex_ids: Vec<EntityId> = self.vertices.keys().copied().collect();
            let triangles = self.mycelium.triangulate(
                &vertex_ids,
                &self.hypha.edges,
                tick,
                self.config.hypha_min_rho,
            );
            self.mycelium.detect_clusters(&triangles, tick);

            let total_verts = self.vertices.len() as u64;
            let sigs = self.fruiting.fruit(
                &triangles,
                &self.mycelium.clusters,
                new_vertex_count,
                new_edge_count,
                total_verts,
                tick,
            );
            self.total_signals_emitted += sigs.len() as u64;
            sigs
        } else {
            vec![]
        };

        signals
    }

    /// Push correlation edges from Hypha into the PersistentGraph.
    fn sync_hypha_edges(&mut self, tick: u64) -> u64 {
        let mut new_count = 0u64;
        let edges_clone: Vec<CorrelationEdge> = self.hypha.edges.clone();
        for e in &edges_clone {
            let prev_count = self.graph.edges.len();
            self.graph.upsert_edge(e.from, e.to, EdgeType::Correlation, e.weight, tick);
            if self.graph.edges.len() > prev_count {
                new_count += 1;
            }
        }
        new_count
    }

    pub fn vertex_count(&self) -> u64 {
        self.vertices.len() as u64
    }

    pub fn edge_count(&self) -> u64 {
        self.hypha.edges.len() as u64
    }

    pub fn cluster_count(&self) -> usize {
        self.mycelium.clusters.len()
    }

    pub fn graph_density(&self) -> f32 {
        let n = self.vertices.len() as f32;
        let e = self.hypha.edges.len() as f32;
        if n < 2.0 { return 0.0; }
        e / (n * (n - 1.0) / 2.0)
    }
}

impl Default for MycelialHdag {
    fn default() -> Self {
        Self::new(McceConfig::default())
    }
}
