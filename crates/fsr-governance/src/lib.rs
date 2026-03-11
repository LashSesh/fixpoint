//! fsr-governance: Integrity + Resource FSMs, recovery routing, rollback (spec §20, §21, §22, Appendix B.6-B.8).
//!
//! 15 invariants enforced here (Appendix A).
//! Recovery routing table drives typed failure → recovery action.

pub mod integrity;
pub mod invariants;
pub mod recovery;
pub mod resource;

pub use integrity::*;
pub use invariants::*;
pub use recovery::*;
pub use resource::*;
