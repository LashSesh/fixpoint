//! Spore layer: data ingestion and auto-discovery of vertices.
//!
//! The Spore layer reads from exchange adapters (simulated from OrderBooks)
//! and automatically creates vertices when new tokens/pairs are encountered.
//! Phase 5 reads from Binance/Kraken exchange adapters.

use crate::vertex::{McceVertex, VertexType};
use fsr_isls::types::EntityId;
use fsr_types::market::OrderBook;
use std::collections::HashMap;

/// Spore layer: auto-discovery of market entities from OrderBook stream.
pub struct SporeLayer {
    /// Already-discovered entity IDs (by label).
    pub discovered: HashMap<String, EntityId>,
    pub discovery_count: u64,
}

impl SporeLayer {
    pub fn new() -> Self {
        SporeLayer {
            discovered: HashMap::new(),
            discovery_count: 0,
        }
    }

    /// Process a batch of OrderBooks, returning newly discovered vertices.
    pub fn ingest_books<'a>(
        &mut self,
        books: &[OrderBook],
        tick: u64,
    ) -> Vec<McceVertex> {
        let mut new_vertices = Vec::new();
        for book in books {
            let base = &book.pair.0;
            let quote = &book.pair.1;
            let venue = &book.venue.0;

            // Create Token vertices for base and quote.
            for symbol in [base, quote] {
                let vt = VertexType::Token {
                    symbol: symbol.clone(),
                    chain: "multi".to_string(),
                };
                if self.register_if_new(&vt, tick, &mut new_vertices) {
                    self.discovery_count += 1;
                }
            }

            // Create Exchange vertex.
            let ex_vt = VertexType::Exchange { name: venue.clone() };
            self.register_if_new(&ex_vt, tick, &mut new_vertices);

            // Create Pool vertex for this pair on this exchange.
            let pool_vt = VertexType::Pool {
                base: base.clone(),
                quote: quote.clone(),
                exchange: venue.clone(),
            };
            self.register_if_new(&pool_vt, tick, &mut new_vertices);
        }
        new_vertices
    }

    /// Ingest raw mid prices (for paper/sandbox mode).
    pub fn ingest_mids(
        &mut self,
        mids: &[(usize, usize, i64)],
        tick: u64,
    ) -> Vec<McceVertex> {
        let mut new_vertices = Vec::new();
        for (i, j, _price) in mids {
            let base_vt = VertexType::Token {
                symbol: format!("TOKEN{}", i),
                chain: "sim".to_string(),
            };
            let quote_vt = VertexType::Token {
                symbol: format!("TOKEN{}", j),
                chain: "sim".to_string(),
            };
            for vt in [base_vt, quote_vt] {
                if self.register_if_new(&vt, tick, &mut new_vertices) {
                    self.discovery_count += 1;
                }
            }
        }
        new_vertices
    }

    fn register_if_new(&mut self, vt: &VertexType, tick: u64, out: &mut Vec<McceVertex>) -> bool {
        let label = vt.label();
        if !self.discovered.contains_key(&label) {
            let v = McceVertex::new(vt.clone(), tick);
            self.discovered.insert(label, v.id);
            out.push(v);
            true
        } else {
            false
        }
    }

    pub fn known_entity_count(&self) -> usize {
        self.discovered.len()
    }
}

impl Default for SporeLayer {
    fn default() -> Self {
        Self::new()
    }
}
