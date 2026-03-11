//! fsr-temporal: Tri-carrier scheduler, Kairos windows, temporal keys (spec §17).
//!
//! Tri-carrier time: (t1, t2, φ) where:
//!   t1 = commit clock (microseconds since epoch)
//!   t2 = exploration tick (macro-cycle counter)
//!   φ  = cyclic phase carrier, drifts with rate ωD > 0

use fsr_fixed::{q32_from_ratio, ONE};
use fsr_types::{Freshness, Q32, TemporalKey};
use serde::{Deserialize, Serialize};

/// Number of phase window bins M (configurable, default 64).
pub const DEFAULT_PHASE_BINS: u16 = 64;
/// Default freshness TTL in macro-cycle ticks.
pub const DEFAULT_FRESHNESS_TTL: u64 = 10;

/// Tri-carrier state (spec §17.1).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TriCarrier {
    /// t1: commit clock in microseconds since epoch.
    pub t1: u64,
    /// t2: exploration tick (macro-cycle counter).
    pub t2: u64,
    /// φ: cyclic phase as Q32 angle in [0, 2π·ONE).
    pub phi: Q32,
    /// Phase drift rate ωD (Q32, must remain > 0; drift collapse is a protocol fault).
    pub omega_d: Q32,
    /// Number of phase window bins M.
    pub phase_bins: u16,
}

impl TriCarrier {
    /// Create a new TriCarrier with the given drift rate.
    /// omega_d must be > 0.
    pub fn new(t1_us: u64, omega_d: Q32, phase_bins: u16) -> Self {
        debug_assert!(omega_d > 0, "drift collapse is a protocol fault (TMCP Axiom 2.1)");
        TriCarrier {
            t1: t1_us,
            t2: 0,
            phi: 0,
            omega_d,
            phase_bins,
        }
    }

    /// Advance t2 (exploration tick) by one macro-cycle.
    /// Does NOT advance t1 (only fill/receipt commits do).
    /// Updates φ by omega_d.
    pub fn advance_tick(&mut self) {
        self.t2 += 1;
        // φ += ωD (mod 2π·ONE)
        let two_pi = fsr_fixed::q32_from_f64_boundary(2.0 * std::f64::consts::PI);
        self.phi = self.phi.wrapping_add(self.omega_d);
        if self.phi >= two_pi {
            self.phi -= two_pi;
        }
        if self.phi < 0 {
            self.phi += two_pi;
        }
    }

    /// Advance t1 (commit clock) on fill/receipt events.
    pub fn advance_commit(&mut self, new_t1_us: u64) {
        if new_t1_us > self.t1 {
            self.t1 = new_t1_us;
        }
    }

    /// Compute phase window index j(x) = floor(M * φ̃ / 2π) (spec §17.2).
    pub fn phase_bin(&self) -> u16 {
        let two_pi = fsr_fixed::q32_from_f64_boundary(2.0 * std::f64::consts::PI);
        let m = self.phase_bins as i64;
        // j = floor(M * phi / two_pi)
        let ratio = q32_from_ratio(
            fsr_fixed::q32_mul(m * ONE, self.phi) >> 32,
            two_pi,
        );
        (ratio >> 32).clamp(0, (self.phase_bins - 1) as i64) as u16
    }

    /// Compute schedule key k(x) = (j(x), w mod M) (spec §17.2).
    pub fn schedule_key(&self, wind_count: u64) -> (u16, u16) {
        let j = self.phase_bin();
        let w_mod = (wind_count % self.phase_bins as u64) as u16;
        (j, w_mod)
    }

    /// Build a TemporalKey with freshness assessment.
    /// wind_count: current windnarbe w(x).
    /// freshness_ttl: ticks until stale/expired.
    pub fn temporal_key(&self, wind_count: u64, _freshness_ttl: u64) -> TemporalKey {
        let freshness = Freshness::Fresh; // computed at current tick; TTL checked at use site
        TemporalKey {
            commit_tick: self.t1,
            intrinsic_tick: self.t2,
            phase_bin: self.phase_bin(),
            wind_count,
            freshness,
        }
    }

    /// Assess freshness of a previously issued TemporalKey relative to now.
    pub fn assess_freshness(&self, key: &TemporalKey, freshness_ttl: u64) -> Freshness {
        let age = self.t2.saturating_sub(key.intrinsic_tick);
        if age == 0 {
            Freshness::Fresh
        } else if age <= freshness_ttl {
            Freshness::Stale
        } else {
            Freshness::Expired
        }
    }
}

/// Kairos scheduler: selects active trumpet layers, press depth, exit params (spec §7.2 step 7).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KairosScheduler {
    pub phase_bins: u16,
    /// Per-layer enable mask (L1..L4 = indices 0..3)
    pub layer_enable: [bool; 4],
    /// Press depth per layer
    pub press_depth: [u8; 4],
}

impl KairosScheduler {
    pub fn new(phase_bins: u16) -> Self {
        KairosScheduler {
            phase_bins,
            layer_enable: [true, true, true, true],
            press_depth: [3, 2, 2, 1],
        }
    }

    /// Apply resource-governed layer pruning (spec §12.4).
    pub fn apply_resource_posture(&mut self, posture: fsr_types::ResourcePosture) {
        use fsr_types::ResourcePosture;
        match posture {
            ResourcePosture::Nominal => {
                self.layer_enable = [true, true, true, true];
            }
            ResourcePosture::Constrained => {
                // L4 disabled
                self.layer_enable = [true, true, true, false];
            }
            ResourcePosture::Scarce => {
                // L3 + L4 disabled
                self.layer_enable = [true, true, false, false];
            }
            ResourcePosture::Emergency => {
                // Only L1 active
                self.layer_enable = [true, false, false, false];
            }
        }
    }

    /// Returns list of active layer indices (0-based, 0=L1..3=L4).
    pub fn active_layers(&self) -> Vec<usize> {
        self.layer_enable
            .iter()
            .enumerate()
            .filter_map(|(i, &en)| if en { Some(i) } else { None })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_types::ResourcePosture;

    #[test]
    fn test_tri_carrier_advance() {
        let mut tc = TriCarrier::new(1_000_000, ONE, DEFAULT_PHASE_BINS);
        assert_eq!(tc.t2, 0);
        tc.advance_tick();
        assert_eq!(tc.t2, 1);
        tc.advance_commit(2_000_000);
        assert_eq!(tc.t1, 2_000_000);
    }

    #[test]
    fn test_phase_bin_bounded() {
        let mut tc = TriCarrier::new(0, ONE, DEFAULT_PHASE_BINS);
        for _ in 0..1000 {
            let bin = tc.phase_bin();
            assert!(bin < DEFAULT_PHASE_BINS, "phase_bin out of range: {}", bin);
            tc.advance_tick();
        }
    }

    #[test]
    fn test_schedule_key() {
        let tc = TriCarrier::new(0, ONE, DEFAULT_PHASE_BINS);
        let (j, w) = tc.schedule_key(10);
        assert!(j < DEFAULT_PHASE_BINS);
        assert!(w < DEFAULT_PHASE_BINS);
        assert_eq!(w, 10 % DEFAULT_PHASE_BINS as u64 as u16);
    }

    #[test]
    fn test_kairos_resource_pruning() {
        let mut sched = KairosScheduler::new(DEFAULT_PHASE_BINS);
        sched.apply_resource_posture(ResourcePosture::Emergency);
        let active = sched.active_layers();
        assert_eq!(active, vec![0], "emergency: only L1");

        sched.apply_resource_posture(ResourcePosture::Scarce);
        let active = sched.active_layers();
        assert_eq!(active, vec![0, 1], "scarce: L1+L2");

        sched.apply_resource_posture(ResourcePosture::Constrained);
        let active = sched.active_layers();
        assert_eq!(active, vec![0, 1, 2], "constrained: L1+L2+L3");

        sched.apply_resource_posture(ResourcePosture::Nominal);
        let active = sched.active_layers();
        assert_eq!(active, vec![0, 1, 2, 3], "nominal: all layers");
    }

    #[test]
    fn test_freshness_assessment() {
        let tc = TriCarrier::new(0, ONE, DEFAULT_PHASE_BINS);
        let key = tc.temporal_key(0, 5);
        assert_eq!(tc.assess_freshness(&key, 5), Freshness::Fresh);

        // Simulate time passing
        let mut tc2 = TriCarrier::new(0, ONE, DEFAULT_PHASE_BINS);
        for _ in 0..3 {
            tc2.advance_tick();
        }
        assert_eq!(tc2.assess_freshness(&key, 5), Freshness::Stale);

        let mut tc3 = TriCarrier::new(0, ONE, DEFAULT_PHASE_BINS);
        for _ in 0..20 {
            tc3.advance_tick();
        }
        assert_eq!(tc3.assess_freshness(&key, 5), Freshness::Expired);
    }
}
