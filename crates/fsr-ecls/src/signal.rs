//! ECLS signal emission to DSHAE.
//!
//! ECLS signals feed DSHAE:
//!   StableLattice     → safe to trade within the lattice
//!   ConstraintBreaking → regime change likely, DSHAE should pause
//!   NewConstraintCandidate → expand DSHAE basket awareness

use crate::constraint::ConstraintCandidate;
use crate::crystal::LatticeCrystal;
use fsr_types::{TradingPair, Q32};
use serde::{Deserialize, Serialize};

/// Severity of a constraint breaking event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BreakingSeverity {
    Minor,
    Moderate,
    Severe,
}

/// Signals emitted by ECLS to downstream systems.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EclsSignal {
    /// A stable constraint lattice exists — safe to trade within it.
    StableLattice {
        crystal_id: u64,
        affected_pairs: Vec<String>,
        stability: Q32,
    },
    /// A constraint is breaking — regime change likely.
    ConstraintBreaking {
        constraint_id: u64,
        severity: BreakingSeverity,
        template_name: String,
    },
    /// A new constraint candidate has been discovered.
    NewConstraintCandidate {
        candidate_id: u64,
        template_name: String,
        satisfaction_rate: Q32,
    },
}

impl EclsSignal {
    pub fn from_crystal(crystal: &LatticeCrystal) -> Self {
        let pairs: Vec<String> = crystal.constraints.iter()
            .flat_map(|c| c.entities.iter().map(|e| format!("ent:{}", e)))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        EclsSignal::StableLattice {
            crystal_id: crystal.id,
            affected_pairs: pairs,
            stability: crystal.free_energy,
        }
    }

    pub fn breaking(candidate: &ConstraintCandidate, severity: BreakingSeverity) -> Self {
        EclsSignal::ConstraintBreaking {
            constraint_id: candidate.id,
            severity,
            template_name: candidate.template.name().to_string(),
        }
    }

    pub fn new_candidate(candidate: &ConstraintCandidate) -> Self {
        EclsSignal::NewConstraintCandidate {
            candidate_id: candidate.id,
            template_name: candidate.template.name().to_string(),
            satisfaction_rate: candidate.satisfaction_rate,
        }
    }
}
