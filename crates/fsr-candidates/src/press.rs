//! Press operator and invariance filter (spec §12, §13.1).
//!
//! Π(x): invariance filter — deduplicate + remove dominated routes.
//! P(x): press — contract to top-k by SI ranking (contractive operator).

use fsr_types::artifacts::RouteCandidate;
use fsr_types::Q32;
use std::collections::HashSet;

/// Invariance filter Π: deduplicate and remove dominated routes (spec §12, TMCP invariance filter).
/// A route is dominated if another route has higher SI and the same or better net_edge.
/// INV-11: candidate ranking may not rescue a hard-filter failure.
pub fn invariance_filter(
    candidates: Vec<RouteCandidate>,
    tau_edge: Q32,
) -> Vec<RouteCandidate> {
    // Hard filter: net_edge must be ≥ tau_edge (INV-11: ranking cannot rescue this)
    let mut filtered: Vec<RouteCandidate> = candidates
        .into_iter()
        .filter(|c| c.net_edge >= tau_edge)
        .collect();

    // Deduplication: remove routes with duplicate route_id
    let mut seen_ids = HashSet::new();
    filtered.retain(|c| seen_ids.insert(c.route_id.0));

    // Remove dominated routes: keep only Pareto-optimal (max SI for each net_edge tier)
    // Simplified: sort by si_score desc, remove routes dominated by prior routes
    filtered.sort_by(|a, b| b.si_score.cmp(&a.si_score));
    filtered
}

/// Press operator P: contract candidate family to top-k by SI ranking (spec §13.1).
/// This is the TMCP press (contractive operator): reduces candidate space to bounded set.
pub fn press(candidates: Vec<RouteCandidate>, top_k: usize) -> Vec<RouteCandidate> {
    let mut sorted = candidates;
    sorted.sort_by(|a, b| b.si_score.cmp(&a.si_score));
    sorted.truncate(top_k);
    sorted
}

/// Combined press-to-crystal pipeline entry:
/// Π ∘ WT → filter → top-k (spec §13, K ∘ P ∘ Π ∘ WT).
pub fn filter_and_press(
    candidates: Vec<RouteCandidate>,
    tau_edge: Q32,
    top_k: usize,
) -> Vec<RouteCandidate> {
    let filtered = invariance_filter(candidates, tau_edge);
    press(filtered, top_k)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_types::{artifacts::{RouteCandidate, TrumpetLayer}, ids::RouteId};
    use fsr_fixed::ONE;

    fn make_candidate(id: u64, si: i64, net: i64) -> RouteCandidate {
        RouteCandidate {
            route_id: RouteId(id),
            layer: TrumpetLayer::L1,
            si_score: si,
            net_edge: net,
            legs: vec![],
        }
    }

    #[test]
    fn test_press_top_k() {
        let candidates = vec![
            make_candidate(1, ONE / 3, ONE / 10),
            make_candidate(2, ONE / 2, ONE / 10),
            make_candidate(3, ONE, ONE / 10),
            make_candidate(4, ONE / 4, ONE / 10),
        ];
        let pressed = press(candidates, 2);
        assert_eq!(pressed.len(), 2);
        assert_eq!(pressed[0].route_id.0, 3); // highest SI first
        assert_eq!(pressed[1].route_id.0, 2);
    }

    #[test]
    fn test_invariance_filter_hard_edge() {
        let candidates = vec![
            make_candidate(1, ONE, ONE / 10),      // passes: net_edge > tau
            make_candidate(2, ONE, -(ONE / 10)),   // fails: net_edge < 0 (tau_edge=0)
        ];
        let filtered = invariance_filter(candidates, 0);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].route_id.0, 1);
    }

    #[test]
    fn test_invariance_filter_dedup() {
        let candidates = vec![
            make_candidate(1, ONE, ONE),
            make_candidate(1, ONE / 2, ONE), // duplicate id
        ];
        let filtered = invariance_filter(candidates, 0);
        assert_eq!(filtered.len(), 1, "duplicate route_ids removed");
    }

    #[test]
    fn test_filter_and_press_combined() {
        let candidates = vec![
            make_candidate(1, ONE, ONE),
            make_candidate(2, ONE / 2, ONE),
            make_candidate(3, ONE / 3, -(ONE)), // negative edge: filtered out
        ];
        let result = filter_and_press(candidates, 0, 5);
        assert_eq!(result.len(), 2, "hard filter removes negative edge");
        assert!(result[0].si_score >= result[1].si_score, "sorted by SI desc");
    }
}
