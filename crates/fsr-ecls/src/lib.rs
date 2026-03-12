//! fsr-ecls: Emergent Constraint Lattice Spectroscopy (Phase 5).
//!
//! ECLS reads constraint programs from the MCCE graph.
//! It is the analytical dome that identifies market constraints.
//!
//! ECLS Rule: ECLS is READ-ONLY. It NEVER modifies the HDAG.
//! It discovers and publishes; downstream systems decide what to do.

pub mod config;
pub mod constraint;
pub mod crystal;
pub mod inverse_weave;
pub mod lattice;
pub mod scanner;
pub mod signal;
pub mod templates;
pub mod thermodynamics;

pub use config::EclsConfig;
pub use constraint::{ConstraintCandidate, ConstraintId};
pub use crystal::LatticeCrystal;
pub use scanner::EclsScanner;
pub use signal::EclsSignal;
pub use templates::ConstraintTemplate;
