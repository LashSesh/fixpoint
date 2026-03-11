//! fsr-ttcp: Temporal Triangulation Cascade Protocol — Phase 2 analysis engine.
//!
//! Implements a simplified 3-level cascade over ResonanceSnapshot streams:
//!   Level 0 (β₀): Connected-component analysis of the resonance signal graph.
//!   Level 1:      Temporal fiber partitioning — groups snapshots by coherence windows.
//!   Level 2:      Meta-convergence — detects if fibers align toward a crystal point.
//!
//! When convergence is detected, a TtcpCrystal artifact is emitted.
//! Crystals are written to data/ttcp/{tick:010}.json.

pub mod cascade;

pub use cascade::{ComponentId, Fiber, TtcpConfig, TtcpCrystal, TtcpEngine};
