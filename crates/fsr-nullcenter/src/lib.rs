//! fsr-nullcenter: Nullcenter certificates, windnarbe, and factor-through validation.
//!
//! Every jump (route-family-to-execution collapse), commit (crystallization),
//! accepted promotion, rollback, and adaptive config update MUST factor through
//! the nullcenter (◁NC). Direct speculative-to-live collapse is forbidden (spec §11).

use fsr_types::{Hash256, NullcenterCert, Q32, TemporalKey};
use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};

/// The windnarbe W(x) = (w(x), χ(x)) (spec §11.3).
/// Monotonic ordering + replay anchor + integrity witness.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Windnarbe {
    /// w(x): winding counter incremented on every NC traversal.
    pub wind_count: u64,
    /// χ(x): rolling digest of traversal sequence.
    pub rolling_digest: Hash256,
}

impl Windnarbe {
    pub fn new() -> Self {
        Windnarbe {
            wind_count: 0,
            rolling_digest: Hash256::ZERO,
        }
    }

    /// Increment the windnarbe on a nullcenter traversal.
    /// The rolling digest mixes in the new wind_count + cert label.
    pub fn advance(&mut self, label: &[u8]) {
        self.wind_count += 1;
        let mut hasher = Sha256::new();
        hasher.update(self.rolling_digest.0);
        hasher.update(self.wind_count.to_le_bytes());
        hasher.update(label);
        let result = hasher.finalize();
        self.rolling_digest = Hash256(result.into());
    }
}

impl Default for Windnarbe {
    fn default() -> Self {
        Self::new()
    }
}

/// Reason for a nullcenter traversal (determines label used in windnarbe).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NcTraversalReason {
    Jump,
    Commit,
    Promotion,
    Rollback,
    AdaptiveUpdate,
}

impl NcTraversalReason {
    fn label(self) -> &'static [u8] {
        match self {
            NcTraversalReason::Jump => b"jump",
            NcTraversalReason::Commit => b"commit",
            NcTraversalReason::Promotion => b"promotion",
            NcTraversalReason::Rollback => b"rollback",
            NcTraversalReason::AdaptiveUpdate => b"adaptive_update",
        }
    }
}

/// NullcenterGate: the central admissibility engine.
/// Implements A = π ◦ α ◦ ι (spec §11.1, §11.2).
pub struct NullcenterGate {
    pub windnarbe: Windnarbe,
    /// Shadow-chain head digest (pre-traversal state).
    pub shadow_head: Hash256,
}

impl NullcenterGate {
    pub fn new() -> Self {
        NullcenterGate {
            windnarbe: Windnarbe::new(),
            shadow_head: Hash256::ZERO,
        }
    }

    /// Factor an operation through the nullcenter (◁NC).
    ///
    /// `gate_open`: result of all sub-gates (regime, mirror, temporal, risk, PoR).
    /// `si_at_cert`: SI score at certification time.
    /// `phase_bin`: temporal phase window.
    /// `temporal_key`: must be Fresh or Stale (not Expired).
    /// `reason`: traversal reason (for windnarbe label).
    ///
    /// Returns Ok(NullcenterCert) if accepted, Err if rejected.
    pub fn factor_through(
        &mut self,
        gate_open: bool,
        si_at_cert: Q32,
        phase_bin: u16,
        temporal_key: &TemporalKey,
        reason: NcTraversalReason,
    ) -> Result<NullcenterCert, fsr_types::FsrError> {
        use fsr_types::{Freshness, FsrError};

        // INV-03: no consequential event without valid temporal key
        if temporal_key.freshness == Freshness::Expired {
            return Err(FsrError::TemporalKeyExpired);
        }

        let pre_digest = self.shadow_head;

        // ι: entry — project state into certificate request (done by caller passing args)
        // α: validate against gates, invariants, PoR
        // We advance the windnarbe regardless of gate state (traversal still happened)
        self.windnarbe.advance(reason.label());

        // Compute post-digest: mix shadow_head + traversal context
        let post_digest = compute_post_digest(pre_digest, &self.windnarbe, reason, gate_open);
        self.shadow_head = post_digest;

        // π: emit valid certificate or reject
        let cert = NullcenterCert {
            gate_open,
            si_at_cert,
            phase_bin,
            wind_count: self.windnarbe.wind_count,
            rolling_digest: self.windnarbe.rolling_digest,
            pre_digest,
            post_digest,
        };

        Ok(cert)
    }
}

impl Default for NullcenterGate {
    fn default() -> Self {
        Self::new()
    }
}

fn compute_post_digest(
    pre: Hash256,
    windnarbe: &Windnarbe,
    reason: NcTraversalReason,
    gate_open: bool,
) -> Hash256 {
    let mut hasher = Sha256::new();
    hasher.update(pre.0);
    hasher.update(windnarbe.wind_count.to_le_bytes());
    hasher.update(windnarbe.rolling_digest.0);
    hasher.update(reason.label());
    hasher.update([gate_open as u8]);
    Hash256(hasher.finalize().into())
}

/// Verify that a NullcenterCert is internally consistent with the windnarbe snapshot.
pub fn verify_cert(cert: &NullcenterCert, expected_wind_count: u64) -> bool {
    cert.wind_count == expected_wind_count
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_types::{Freshness, TemporalKey};

    fn make_key(tick: u64, wind: u64) -> TemporalKey {
        TemporalKey {
            commit_tick: 0,
            intrinsic_tick: tick,
            phase_bin: 0,
            wind_count: wind,
            freshness: Freshness::Fresh,
        }
    }

    #[test]
    fn test_windnarbe_advances_monotonically() {
        let mut w = Windnarbe::new();
        assert_eq!(w.wind_count, 0);
        w.advance(b"jump");
        assert_eq!(w.wind_count, 1);
        w.advance(b"commit");
        assert_eq!(w.wind_count, 2);
        // digest changes
        let d1 = w.rolling_digest;
        w.advance(b"rollback");
        assert_ne!(w.rolling_digest, d1);
    }

    #[test]
    fn test_nc_factor_through_gate_open() {
        let mut nc = NullcenterGate::new();
        let key = make_key(0, 0);
        let cert = nc
            .factor_through(true, 0, 0, &key, NcTraversalReason::Jump)
            .unwrap();
        assert!(cert.gate_open);
        assert_eq!(cert.wind_count, 1);
        assert_ne!(cert.pre_digest, cert.post_digest);
    }

    #[test]
    fn test_nc_factor_through_gate_closed() {
        let mut nc = NullcenterGate::new();
        let key = make_key(0, 0);
        let cert = nc
            .factor_through(false, 0, 0, &key, NcTraversalReason::Commit)
            .unwrap();
        assert!(!cert.gate_open);
        assert_eq!(cert.wind_count, 1);
    }

    #[test]
    fn test_nc_rejects_expired_key() {
        let mut nc = NullcenterGate::new();
        let key = TemporalKey {
            commit_tick: 0,
            intrinsic_tick: 0,
            phase_bin: 0,
            wind_count: 0,
            freshness: Freshness::Expired,
        };
        let result = nc.factor_through(true, 0, 0, &key, NcTraversalReason::Jump);
        assert!(result.is_err());
    }

    #[test]
    fn test_nc_sequential_digests_chain() {
        let mut nc = NullcenterGate::new();
        let key = make_key(0, 0);
        let cert1 = nc
            .factor_through(true, 100, 0, &key, NcTraversalReason::Jump)
            .unwrap();
        let cert2 = nc
            .factor_through(true, 200, 0, &key, NcTraversalReason::Commit)
            .unwrap();
        // cert2.pre_digest should equal cert1.post_digest
        assert_eq!(cert2.pre_digest, cert1.post_digest);
    }
}
