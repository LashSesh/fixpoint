//! fsr-mcce: Mycelial Crypto-Cartography Engine (Phase 5).
//!
//! Maintains a persistent, self-expanding graph of the crypto ecosystem.
//! This is the long-term memory of the platform.
//!
//! Layer architecture:
//!   Spore layer:    Data ingestion + auto-discovery (creates vertices on encounter)
//!   Hypha layer:    Pairwise correlations with decay (edges between vertices)
//!   Mycelium layer: TTCP triangulation on vertex cloud (detect topological loops)
//!   Fruiting layer: Signal emission to DSHAE/ECLS (alert patterns)
//!
//! MCCE Leverage Principle:
//!   The longer MCCE runs, the more structural knowledge it accumulates,
//!   making DSHAE's signals increasingly precise.

pub mod basket_advisor;
pub mod config;
pub mod edge;
pub mod fruiting;
pub mod hdag;
pub mod hypha;
pub mod mycelium;
pub mod spore;
pub mod vertex;

pub use basket_advisor::BasketAdvisor;
pub use config::McceConfig;
pub use fruiting::{FruitingLayer, McceSignal};
pub use hdag::MycelialHdag;
pub use hypha::HyphaLayer;
pub use mycelium::MyceliumLayer;
pub use spore::SporeLayer;
pub use vertex::{VertexType, McceVertex};
pub use edge::CorrelationEdge;
