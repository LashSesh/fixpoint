//! Phase 5 integration tests (spec §5).
//!
//! Covers Phase 5 subsystems:
//!   - Phase 5 EventTag bincode stability (additive after Phase 4 tags)
//!   - ISLS EvidenceChain: genesis, append, verify, determinism
//!   - fsr-chain thin-wrapper: HashChain and EvidenceChain have identical digests
//!   - DSHAE sandbox: all 7 scenarios PASS (Phase 1–4 scenarios + 2 Phase 5 scenarios)
//!   - MCCE basic: vertex discovery, edge creation
//!   - ECLS basic: scanner runs without panic on empty HDAG

// ── Phase 5 EventTag bincode stability ───────────────────────────────────────

#[test]
fn test_phase5_event_tags_additive_after_phase4() {
    use fsr_types::EventTag;

    let encode = |tag: EventTag| -> u32 {
        let bytes = bincode::serialize(&tag).unwrap();
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    };

    // Phase 5 tags must come after Phase 4 GuiSessionEnded.
    let gui_ended = encode(EventTag::GuiSessionEnded);
    let isls_obs = encode(EventTag::IslsObservationWritten);
    let ecls_scan = encode(EventTag::EclsScanCompleted);

    assert!(
        isls_obs > gui_ended,
        "IslsObservationWritten ({}) must be after GuiSessionEnded ({})",
        isls_obs, gui_ended
    );
    assert!(
        ecls_scan > isls_obs,
        "EclsScanCompleted ({}) must be after IslsObservationWritten ({})",
        ecls_scan, isls_obs
    );

    // All Phase 5 tags must be in strictly increasing order.
    let phase5_tags = [
        encode(EventTag::IslsObservationWritten),
        encode(EventTag::IslsConsensusCommit),
        encode(EventTag::IslsConsensusDefer),
        encode(EventTag::IslsSemanticCrystalFormed),
        encode(EventTag::IslsStorageCompacted),
        encode(EventTag::McceVertexDiscovered),
        encode(EventTag::McceEdgeCreated),
        encode(EventTag::McceTriangleDetected),
        encode(EventTag::McceClusterFormed),
        encode(EventTag::McCeFruitingSignal),
        encode(EventTag::EclsConstraintDiscovered),
        encode(EventTag::EclsConstraintBreaking),
        encode(EventTag::EclsLatticeCrystalFormed),
        encode(EventTag::EclsScanCompleted),
    ];
    for i in 1..phase5_tags.len() {
        assert!(
            phase5_tags[i] > phase5_tags[i - 1],
            "Phase 5 EventTag at position {} ({}) must be > position {} ({})",
            i, phase5_tags[i], i - 1, phase5_tags[i - 1]
        );
    }
}

// ── ISLS EvidenceChain ────────────────────────────────────────────────────────

#[test]
fn test_isls_evidence_chain_genesis_nonzero() {
    use fsr_isls::evidence::EvidenceChain;
    use fsr_types::Hash256;

    let chain = EvidenceChain::new("test");
    assert_ne!(chain.head, Hash256::ZERO, "genesis hash must not be zero");
    assert!(chain.is_empty());
}

#[test]
fn test_isls_evidence_chain_append_and_verify() {
    use fsr_isls::evidence::EvidenceChain;
    use fsr_types::{EventTag, Freshness, TemporalKey};

    let mut chain = EvidenceChain::new("test");
    let tk = TemporalKey {
        commit_tick: 0,
        intrinsic_tick: 1,
        phase_bin: 0,
        wind_count: 0,
        freshness: Freshness::Fresh,
    };
    chain.append(EventTag::IslsObservationWritten, vec![1, 2, 3], tk);
    chain.append(EventTag::IslsConsensusCommit, vec![4, 5], tk);
    assert_eq!(chain.len(), 2);
    assert!(chain.verify().is_ok(), "chain must be consistent after 2 appends");
}

#[test]
fn test_isls_evidence_chain_determinism() {
    use fsr_isls::evidence::EvidenceChain;
    use fsr_types::{EventTag, Freshness, TemporalKey};

    let tk = TemporalKey {
        commit_tick: 0,
        intrinsic_tick: 42,
        phase_bin: 1,
        wind_count: 3,
        freshness: Freshness::Fresh,
    };
    let mut c1 = EvidenceChain::new("chain1");
    let mut c2 = EvidenceChain::new("chain2");
    c1.append(EventTag::IslsObservationWritten, vec![10, 20, 30], tk);
    c2.append(EventTag::IslsObservationWritten, vec![10, 20, 30], tk);
    assert_eq!(c1.head, c2.head, "deterministic hashing: same inputs → same head");
}

#[test]
fn test_isls_evidence_chain_detects_tampering() {
    use fsr_isls::evidence::EvidenceChain;
    use fsr_types::{EventTag, Freshness, TemporalKey};

    let tk = TemporalKey {
        commit_tick: 0,
        intrinsic_tick: 1,
        phase_bin: 0,
        wind_count: 0,
        freshness: Freshness::Fresh,
    };
    let mut chain = EvidenceChain::new("test");
    chain.append(EventTag::IslsObservationWritten, vec![1], tk);
    chain.append(EventTag::IslsConsensusCommit, vec![2], tk);
    // Tamper with the first event payload.
    chain.events[0].payload = vec![99];
    assert!(chain.verify().is_err(), "tampered chain must fail verify");
}

// ── fsr-chain thin-wrapper: hash consistency ──────────────────────────────────

#[test]
fn test_hash_chain_and_evidence_chain_same_genesis() {
    use fsr_chain::{HashChain, compute_genesis};

    let hash_chain = HashChain::new("test");
    let genesis = compute_genesis();
    assert_eq!(
        hash_chain.head, genesis,
        "HashChain genesis must match EvidenceChain genesis"
    );
}

#[test]
fn test_hash_chain_and_evidence_chain_same_digest() {
    use fsr_chain::HashChain;
    use fsr_isls::evidence::EvidenceChain;
    use fsr_types::{EventTag, Freshness, TemporalKey};

    let tk = TemporalKey {
        commit_tick: 0,
        intrinsic_tick: 7,
        phase_bin: 2,
        wind_count: 1,
        freshness: Freshness::Fresh,
    };
    let payload = vec![42u8, 43u8, 44u8];

    let mut hc = HashChain::new("hc");
    let mut ec = EvidenceChain::new("ec");
    hc.append(EventTag::MacroCycleStart, payload.clone(), tk);
    ec.append(EventTag::MacroCycleStart, payload.clone(), tk);

    assert_eq!(
        hc.head, ec.head,
        "HashChain and EvidenceChain must produce identical digests for same inputs"
    );
}

// ── DSHAE sandbox: all 7 scenarios PASS ──────────────────────────────────────

#[test]
fn test_sandbox_correlation_scenario_passes() {
    use fsr_dshae::{SandboxRunner, Scenario};
    let runner = SandboxRunner::with_default_config();
    let result = runner.run_scenario(Scenario::Correlation);
    assert!(
        result.passed,
        "Correlation scenario must PASS: {:?}",
        result.checks
    );
}

#[test]
fn test_sandbox_lattice_scenario_passes() {
    use fsr_dshae::{SandboxRunner, Scenario};
    let runner = SandboxRunner::with_default_config();
    let result = runner.run_scenario(Scenario::Lattice);
    assert!(
        result.passed,
        "Lattice scenario must PASS: {:?}",
        result.checks
    );
}

#[test]
fn test_all_7_scenarios_pass() {
    use fsr_dshae::{SandboxRunner, Scenario};
    let runner = SandboxRunner::with_default_config();
    let scenarios = [
        Scenario::Calm,
        Scenario::SingleArb,
        Scenario::RecurringArb,
        Scenario::Noisy,
        Scenario::RegimeShift,
        Scenario::Correlation,
        Scenario::Lattice,
    ];
    let mut pass_count = 0;
    let mut fail_names = Vec::new();
    for scenario in &scenarios {
        let result = runner.run_scenario(*scenario);
        if result.passed {
            pass_count += 1;
        } else {
            fail_names.push(format!("{:?}: {:?}", scenario, result.checks));
        }
    }
    assert_eq!(
        pass_count, 7,
        "All 7 sandbox scenarios must PASS, got {}/7. Failures: {:?}",
        pass_count, fail_names
    );
}

// ── MCCE basic smoke tests ────────────────────────────────────────────────────

#[test]
fn test_mcce_hdag_creates_without_panic() {
    use fsr_mcce::{McceConfig, MycelialHdag};
    let config = McceConfig::default();
    let hdag = MycelialHdag::new(config);
    assert_eq!(hdag.vertex_count(), 0);
    assert_eq!(hdag.edge_count(), 0);
}

#[test]
fn test_mcce_tick_mids_discovers_vertices() {
    use fsr_mcce::{McceConfig, MycelialHdag};

    let mut config = McceConfig::default();
    config.enabled = true;
    let mut hdag = MycelialHdag::new(config);

    // Feed 10 ticks of mid prices (pair indices).
    for tick in 0u64..10 {
        let mids = vec![(0usize, 1usize, 9200i64), (0, 2, 7912), (1, 2, 8600)];
        hdag.tick_mids(&mids, tick);
    }
    assert!(hdag.vertex_count() >= 2, "should discover vertices from pairs");
}

#[test]
fn test_mcce_multiple_ticks_no_panic() {
    use fsr_mcce::{McceConfig, MycelialHdag};

    let mut config = McceConfig::default();
    config.enabled = true;
    config.hypha_window_ticks = 5;
    let mut hdag = MycelialHdag::new(config);

    for tick in 0u64..50 {
        let mids = vec![(0usize, 1usize, 9200i64 + (tick as i64 % 10)), (1, 2, 8600)];
        hdag.tick_mids(&mids, tick);
    }
    // Just verify it doesn't panic and produces some state.
    let _ = hdag.vertex_count();
    let _ = hdag.edge_count();
    let _ = hdag.cluster_count();
}

// ── ECLS basic smoke tests ────────────────────────────────────────────────────

#[test]
fn test_ecls_scanner_empty_hdag_no_panic() {
    use fsr_mcce::{McceConfig, MycelialHdag};
    use fsr_ecls::{EclsConfig, EclsScanner};

    let config = EclsConfig::default();
    let hdag = MycelialHdag::new(McceConfig::default());
    let templates = config.active_templates();
    let mut scanner = EclsScanner::new();
    let signals = scanner.scan(&hdag, &templates, &config, 0);
    assert!(signals.is_empty(), "no signals from empty HDAG");
}

#[test]
fn test_ecls_config_active_templates() {
    use fsr_ecls::{ConstraintTemplate, EclsConfig};

    let config = EclsConfig::default();
    let templates = config.active_templates();
    // Granger and Spectral should be deferred (inactive).
    assert!(
        !templates.iter().any(|t| matches!(t, ConstraintTemplate::Granger { .. })),
        "Granger should be deferred (inactive) in Phase 5"
    );
    assert!(
        !templates.iter().any(|t| matches!(t, ConstraintTemplate::Spectral { .. })),
        "Spectral should be deferred (inactive) in Phase 5"
    );
}

// ── ISLS persistence smoke test ───────────────────────────────────────────────

#[test]
fn test_isls_persistence_write_observation() {
    use fsr_isls::{IslsConfig, IslsPersistence};
    use fsr_isls::observation::{Observation, ObsPayload, SourceId};
    use fsr_fixed::ONE;

    let config = IslsConfig::default();
    let mut isls = IslsPersistence::new(config);

    let obs = Observation::new(
        1_000_000u64,
        SourceId("test".to_string()),
        ObsPayload::Price(0, ONE),
    );
    isls.write_observation(1, obs);
    assert_eq!(isls.observation_count, 1, "observation count must be 1 after write");
}

#[test]
fn test_isls_verify_chains_ok_after_append() {
    use fsr_isls::{IslsConfig, IslsPersistence};
    use fsr_types::{EventTag, Freshness, TemporalKey};

    let config = IslsConfig::default();
    let mut isls = IslsPersistence::new(config);
    let tk = TemporalKey {
        commit_tick: 0,
        intrinsic_tick: 1,
        phase_bin: 0,
        wind_count: 0,
        freshness: Freshness::Fresh,
    };
    isls.shadow_append(EventTag::IslsObservationWritten, vec![1, 2], tk);
    assert!(isls.verify_both().is_ok(), "chains must verify after append");
}
