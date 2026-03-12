//! VertexTypes and embedding for MCCE.

use fsr_isls::types::EntityId;
use fsr_types::Q32;
use serde::{Deserialize, Serialize};

/// The type of a vertex in the Mycelial HDAG.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VertexType {
    /// A blockchain protocol (Bitcoin, Ethereum, Solana, …).
    Protocol { chain: String },
    /// A token on a specific chain.
    Token { symbol: String, chain: String },
    /// A centralised or decentralised exchange.
    Exchange { name: String },
    /// A liquidity pool (e.g. AMM pair).
    Pool { base: String, quote: String, exchange: String },
    /// An aggregate synthetic (basket, index).
    Aggregate { name: String },
}

impl VertexType {
    pub fn label(&self) -> String {
        match self {
            VertexType::Protocol { chain } => format!("protocol:{}", chain),
            VertexType::Token { symbol, chain } => format!("token:{}:{}", chain, symbol),
            VertexType::Exchange { name } => format!("exchange:{}", name),
            VertexType::Pool { base, quote, exchange } => {
                format!("pool:{}:{}/{}", exchange, base, quote)
            }
            VertexType::Aggregate { name } => format!("agg:{}", name),
        }
    }

    pub fn canonical_id(&self) -> EntityId {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        self.label().hash(&mut h);
        h.finish()
    }
}

/// A vertex in the MCCE graph, with metadata and 5D embedding.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McceVertex {
    pub id: EntityId,
    pub vertex_type: VertexType,
    pub first_seen_tick: u64,
    pub last_seen_tick: u64,
    pub observation_count: u64,
    /// 5D embedding [ψ, ρ, ω, momentum, entropy].
    pub embedding: [Q32; 5],
    /// Estimated market cap (log-scale, Q32).
    pub log_market_cap: Q32,
    /// Rolling return variance (Q32).
    pub return_variance: Q32,
}

impl McceVertex {
    pub fn new(vertex_type: VertexType, tick: u64) -> Self {
        let id = vertex_type.canonical_id();
        McceVertex {
            id,
            vertex_type,
            first_seen_tick: tick,
            last_seen_tick: tick,
            observation_count: 1,
            embedding: [0i64; 5],
            log_market_cap: 0,
            return_variance: 0,
        }
    }

    pub fn update_embedding(&mut self, psi: Q32, rho: Q32, omega: Q32, momentum: Q32, entropy: Q32) {
        self.embedding = [psi, rho, omega, momentum, entropy];
    }
}
