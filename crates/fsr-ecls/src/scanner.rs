//! Constraint scanner: evaluates templates against the MCCE HDAG.
//!
//! ECLS Rule: Scanner is READ-ONLY. Does not modify the HDAG.

use crate::config::EclsConfig;
use crate::constraint::{ConstraintCandidate, ConstraintId};
use crate::templates::ConstraintTemplate;
use fsr_fixed::ONE;
use fsr_isls::types::{EntityId, EdgeType};
use fsr_mcce::hdag::MycelialHdag;
use fsr_types::Q32;
use std::collections::HashMap;

/// Evaluates whether a pair of entities satisfies a given constraint template
/// over the rolling observation window.
pub fn evaluate_template(
    template: &ConstraintTemplate,
    a: EntityId,
    b: EntityId,
    hdag: &MycelialHdag,
    _window_size: u64,
) -> Q32 {
    match template {
        ConstraintTemplate::Band { k_sigma, .. } => {
            // Band: check if price variance within k_sigma bands.
            // Proxy: use edge weight (correlation strength) as a band proxy.
            if let Some(rho) = hdag.hypha.get_correlation(a, b) {
                let within_band = rho.abs() >= ONE / 2; // >50% corr → in-band
                if within_band { ONE * *k_sigma / ONE } else { 0 }
            } else { 0 }
        }
        ConstraintTemplate::Ratio { ratio, tolerance, .. } => {
            // Ratio: check if the correlation ratio is within tolerance.
            if let Some(rho) = hdag.hypha.get_correlation(a, b) {
                let diff = (rho - ratio).abs();
                if diff <= *tolerance { ONE } else { 0 }
            } else { 0 }
        }
        ConstraintTemplate::Correlation { rho_target, delta } => {
            if let Some(rho) = hdag.hypha.get_correlation(a, b) {
                let diff = (rho - rho_target).abs();
                if diff <= *delta { ONE } else { 0 }
            } else { 0 }
        }
        ConstraintTemplate::Granger { .. } | ConstraintTemplate::Spectral { .. } => {
            // Deferred to Phase 6.
            0
        }
        ConstraintTemplate::Topological { betti_dim, expected_value } => {
            // Check if the cluster containing these vertices has the expected Betti number.
            let cluster = hdag.mycelium.clusters.iter().find(|c| {
                c.members.contains(&a) && c.members.contains(&b)
            });
            if let Some(_c) = cluster {
                // Simplified: β₀ (connected components) = 1 for connected cluster.
                if *betti_dim == 0 && *expected_value == 1 { ONE }
                else { 0 }
            } else { 0 }
        }
        ConstraintTemplate::PhaseLock { tolerance, .. } => {
            // Phase lock: check if two vertices have similar Pearson correlation patterns.
            if let Some(rho) = hdag.hypha.get_correlation(a, b) {
                if rho.abs() >= (ONE - tolerance) { ONE } else { 0 }
            } else { 0 }
        }
    }
}

/// ECLS constraint scanner.
pub struct EclsScanner {
    pub candidates: Vec<ConstraintCandidate>,
    pub next_id: ConstraintId,
    pub last_scan_tick: u64,
}

impl EclsScanner {
    pub fn new() -> Self {
        EclsScanner {
            candidates: Vec::new(),
            next_id: 0,
            last_scan_tick: 0,
        }
    }

    /// Scan the HDAG for constraint instantiations.
    /// ECLS Rule: READ-ONLY — does not modify hdag.
    pub fn scan(
        &mut self,
        hdag: &MycelialHdag,
        templates: &[ConstraintTemplate],
        config: &EclsConfig,
        tick: u64,
    ) -> Vec<ConstraintCandidate> {
        self.last_scan_tick = tick;
        let mut new_candidates = Vec::new();

        // Iterate over edges in the HDAG.
        let edges: Vec<(EntityId, EntityId)> = hdag.hypha.edges.iter()
            .map(|e| (e.from, e.to))
            .collect();

        for (u, v) in &edges {
            for template in templates {
                if !template.is_active_phase5() {
                    continue;
                }
                let satisfaction = evaluate_template(template, *u, *v, hdag, config.window_size);
                if satisfaction >= config.alpha_min {
                    // Check if we already track this constraint.
                    let existing = self.candidates.iter_mut().find(|c| {
                        c.entities == vec![*u, *v] && &c.template == template
                    });
                    if let Some(c) = existing {
                        c.update(satisfaction, tick);
                        new_candidates.push(c.clone());
                    } else {
                        let id = self.next_id;
                        self.next_id += 1;
                        let c = ConstraintCandidate::new(
                            id,
                            template.clone(),
                            vec![*u, *v],
                            satisfaction,
                            config.window_size,
                            tick,
                        );
                        self.candidates.push(c.clone());
                        new_candidates.push(c);
                    }
                }
            }
        }

        new_candidates
    }

    pub fn active_count(&self) -> usize {
        self.candidates.len()
    }
}

impl Default for EclsScanner {
    fn default() -> Self {
        Self::new()
    }
}
