//! fsr-candidates: Route discovery, trumpet WT multiplexing, press (contraction) (spec §12, §13).
//!
//! Trumpet layers L1..L4 discover route candidates. WT aggregates them.
//! Press contracts to top-k by SI ranking. Invariance filter deduplicates.

pub mod discovery;
pub mod press;
pub mod trumpet;

pub use discovery::*;
pub use press::*;
pub use trumpet::*;
