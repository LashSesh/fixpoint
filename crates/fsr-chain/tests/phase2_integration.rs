//! Phase 2 integration tests (spec §2.8).
//!
//! Covers all Phase 2 subsystems accessible via library crates:
//!   - Persistence (fsr-chain::persist)
//!   - TTCP engine (fsr-ttcp)
//!   - TUI dashboard state (fsr-tui)
//!   - Phase 1 regression: chain integrity + 15 invariants

// ── Persistence ───────────────────────────────────────────────────────────────

#[test]
fn test_persistence_chain_writer_roundtrip() {
    use fsr_chain::persist::{ChainFileWriter, ChainReader};
    use fsr_types::{ChainEvent, EventTag, Freshness, Hash256, TemporalKey};
    use std::fs;

    let dir = std::env::temp_dir().join("fsr_integ_persist");
    let _ = fs::remove_dir_all(&dir);

    let mut writer = ChainFileWriter::new("integ_run", &dir, "shadow", 10).unwrap();
    for i in 0..25u64 {
        let event = ChainEvent {
            tag: EventTag::MacroCycleEnd,
            payload: vec![i as u8],
            temporal_key: TemporalKey {
                commit_tick: 0,
                intrinsic_tick: i,
                phase_bin: 0,
                wind_count: 0,
                freshness: Freshness::Fresh,
            },
            prev_digest: Hash256::ZERO,
            digest: Hash256::ZERO,
        };
        writer.append_event(&event).unwrap();
    }
    writer.flush().unwrap();

    let events = ChainReader::read_all_events(&dir, "shadow").unwrap();
    assert_eq!(events.len(), 25, "all 25 events must round-trip");
    assert_eq!(events[0].temporal_key.intrinsic_tick, 0);
    assert_eq!(events[24].temporal_key.intrinsic_tick, 24);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_persistence_snapshot_write_and_reload() {
    use fsr_chain::persist::SystemSnapshot;
    use fsr_types::{Hash256, IntegrityPosture, RegimeState};
    use std::fs;

    let dir = std::env::temp_dir().join("fsr_integ_snap");
    let _ = fs::remove_dir_all(&dir);

    let snap = SystemSnapshot {
        run_id: "integ_snap_001".to_string(),
        tick: 500,
        timestamp_unix_ms: 0,
        chain_shadow_head: Hash256::ZERO,
        chain_commitment_head: Hash256::ZERO,
        regime_state: RegimeState::Alpha,
        integrity_state: IntegrityPosture::Healthy,
        settled_cycles: 12,
        aborted_cycles: 3,
        resonance_psi: 1_000_000,
        resonance_rho: 2_000_000,
        resonance_omega: 3_000_000,
        shadow_event_count: 200,
        commitment_event_count: 12,
    };
    snap.write(&dir).unwrap();

    let loaded = SystemSnapshot::read_latest(&dir).unwrap().unwrap();
    assert_eq!(loaded.tick, 500);
    assert_eq!(loaded.settled_cycles, 12);
    assert_eq!(loaded.run_id, "integ_snap_001");
    assert!(matches!(loaded.regime_state, RegimeState::Alpha));
    assert!(matches!(loaded.integrity_state, IntegrityPosture::Healthy));

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn test_persistence_latest_snapshot_among_multiple() {
    use fsr_chain::persist::SystemSnapshot;
    use fsr_types::{Hash256, IntegrityPosture, RegimeState};
    use std::fs;

    let dir = std::env::temp_dir().join("fsr_integ_snap_multi");
    let _ = fs::remove_dir_all(&dir);

    for tick in [100u64, 200, 300] {
        let snap = SystemSnapshot {
            run_id: "multi_snap".to_string(),
            tick,
            timestamp_unix_ms: tick,
            chain_shadow_head: Hash256::ZERO,
            chain_commitment_head: Hash256::ZERO,
            regime_state: RegimeState::Beta,
            integrity_state: IntegrityPosture::Healthy,
            settled_cycles: tick / 10,
            aborted_cycles: 0,
            resonance_psi: 0,
            resonance_rho: 0,
            resonance_omega: 0,
            shadow_event_count: tick,
            commitment_event_count: 0,
        };
        snap.write(&dir).unwrap();
    }

    let latest = SystemSnapshot::read_latest(&dir).unwrap().unwrap();
    assert_eq!(latest.tick, 300, "read_latest must return highest tick");

    let _ = fs::remove_dir_all(&dir);
}

// ── TTCP Engine ───────────────────────────────────────────────────────────────

#[test]
fn test_ttcp_no_crystal_before_window_full() {
    use fsr_fixed::ONE;
    use fsr_ttcp::{TtcpConfig, TtcpEngine};
    use fsr_types::ResonanceSnapshot;

    let mut engine = TtcpEngine::new(TtcpConfig {
        window_size: 10,
        ..TtcpConfig::default()
    });
    for i in 0..9u64 {
        let crystal = engine.push_snapshot(ResonanceSnapshot {
            kappa: ONE / 2,
            entropy: ONE / 4,
            sync: ONE / 2,
            momentum: ONE / 4,
            si: ONE / 3,
            psi: (ONE as f64 * 0.5) as i64,
            rho: (ONE as f64 * 0.6) as i64,
            omega: ONE / 2,
            tick: i,
        });
        assert!(crystal.is_none(), "no crystal before window full (i={})", i);
    }
}

#[test]
fn test_ttcp_crystal_on_coherent_window() {
    use fsr_fixed::ONE;
    use fsr_ttcp::{TtcpConfig, TtcpEngine};
    use fsr_types::ResonanceSnapshot;

    let window_size = 5;
    let mut engine = TtcpEngine::new(TtcpConfig {
        window_size,
        level0_psi_threshold: (ONE as f64 * 0.3) as i64,
        level1_coherence_threshold: (ONE as f64 * 0.1) as i64,
        crystal_psi_threshold: (ONE as f64 * 0.2) as i64,
    });

    let psi = (ONE as f64 * 0.5) as i64;
    let kappa = (ONE as f64 * 0.5) as i64;
    let momentum = ONE / 4;

    let mut last = None;
    for i in 0..window_size as u64 {
        last = engine.push_snapshot(ResonanceSnapshot {
            kappa,
            entropy: ONE / 4,
            sync: ONE / 2,
            momentum,
            si: ONE / 3,
            psi,
            rho: ONE / 2,
            omega: ONE / 2,
            tick: i,
        });
    }
    assert!(last.is_some(), "expected crystal from fully coherent window");
    let crystal = last.unwrap();
    assert_eq!(crystal.level, 2);
    assert_eq!(crystal.components, 1);
    assert_eq!(engine.crystals_found, 1);
}

#[test]
fn test_ttcp_crystal_writes_valid_json() {
    use fsr_fixed::ONE;
    use fsr_ttcp::TtcpCrystal;
    use std::fs;

    let dir = std::env::temp_dir().join("fsr_integ_ttcp_json");
    let _ = fs::remove_dir_all(&dir);

    let crystal = TtcpCrystal {
        tick: 999,
        level: 2,
        components: 1,
        fibers: 1,
        convergence_score: ONE / 2,
        resonance_psi: ONE / 2,
        resonance_rho: ONE / 3,
    };

    let path = crystal.write_to_dir(&dir).unwrap();
    assert!(path.exists(), "crystal file must exist on disk");

    let json = fs::read_to_string(&path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["tick"], 999);
    assert_eq!(parsed["level"], 2);
    assert_eq!(parsed["components"], 1);

    let _ = fs::remove_dir_all(&dir);
}

// ── TUI State ─────────────────────────────────────────────────────────────────

#[test]
fn test_dashboard_state_updated_by_engine_thread() {
    use fsr_tui::DashboardState;
    use std::sync::{Arc, Mutex};

    let state = Arc::new(Mutex::new(DashboardState::default()));

    for i in 1u64..=10 {
        let mut s = state.lock().unwrap();
        s.tick = i;
        s.regime = "Alpha".to_string();
        s.gate_open = i % 2 == 0;
        s.push_event(format!("tick {}", i));
    }

    let s = state.lock().unwrap();
    assert_eq!(s.tick, 10);
    assert_eq!(s.regime, "Alpha");
    assert_eq!(s.event_log.len(), 10);
    assert_eq!(s.event_log.back().unwrap(), "tick 10");
}

#[test]
fn test_dashboard_event_log_bounded() {
    use fsr_tui::{state::MAX_EVENT_LOG, DashboardState};

    let mut s = DashboardState::default();
    for i in 0..(MAX_EVENT_LOG + 50) {
        s.push_event(format!("event {}", i));
    }
    assert_eq!(s.event_log.len(), MAX_EVENT_LOG);
    // Oldest events are dropped; newest survives.
    assert!(
        s.event_log.back().unwrap().contains(&(MAX_EVENT_LOG + 49).to_string()),
        "most recent event must be last in log"
    );
}

#[test]
fn test_dashboard_quit_flag_across_threads() {
    use fsr_tui::DashboardState;
    use std::sync::{Arc, Mutex};

    let state = Arc::new(Mutex::new(DashboardState::default()));
    let state2 = Arc::clone(&state);

    std::thread::spawn(move || {
        state2.lock().unwrap().quit_requested = true;
    })
    .join()
    .unwrap();

    assert!(state.lock().unwrap().quit_requested);
}

// ── Phase 1 Regression ───────────────────────────────────────────────────────

#[test]
fn test_phase1_chain_integrity_preserved() {
    use fsr_chain::{DualChain, HashChain};
    use fsr_types::{EventTag, Freshness, TemporalKey};

    let make_tk = |tick: u64| TemporalKey {
        commit_tick: 0,
        intrinsic_tick: tick,
        phase_bin: 0,
        wind_count: 0,
        freshness: Freshness::Fresh,
    };

    let mut chain = HashChain::new("phase1-regression");
    for i in 0..50u64 {
        chain.append(EventTag::MacroCycleEnd, vec![i as u8], make_tk(i));
    }
    assert!(chain.verify().is_ok(), "Phase 1 hash chain integrity must hold after Phase 2 additions");
    assert_eq!(chain.len(), 50);

    let mut dual = DualChain::new();
    dual.shadow_append(EventTag::MacroCycleStart, vec![], make_tk(0));
    dual.commit(EventTag::ExecutionSettled, vec![1], make_tk(0));
    assert!(dual.verify_both().is_ok(), "dual chain must verify");
}

#[test]
fn test_phase1_chain_determinism_preserved() {
    use fsr_chain::HashChain;
    use fsr_types::{EventTag, Freshness, TemporalKey};

    let tk = TemporalKey {
        commit_tick: 0,
        intrinsic_tick: 42,
        phase_bin: 7,
        wind_count: 3,
        freshness: Freshness::Fresh,
    };

    let mut c1 = HashChain::new("chain-a");
    let mut c2 = HashChain::new("chain-b");
    c1.append(EventTag::RegimeChanged, vec![1, 2, 3], tk);
    c2.append(EventTag::RegimeChanged, vec![1, 2, 3], tk);

    assert_eq!(c1.head, c2.head, "same inputs must produce same digest (TMCP Theorem 9.3)");
}

#[test]
fn test_phase1_event_tags_bincode_stable() {
    // Phase 1 EventTag variants must still serialize correctly after Phase 2 additions.
    // Adding new variants to an enum does NOT change bincode encoding of existing variants
    // (bincode uses variant index, which is preserved by appending-only additions).
    let phase1_tags = [
        fsr_types::EventTag::RegimeChanged,
        fsr_types::EventTag::CandidateDiscovered,
        fsr_types::EventTag::ExecutionSettled,
        fsr_types::EventTag::MacroCycleEnd,
        fsr_types::EventTag::IntentOpened,
        fsr_types::EventTag::QuorumPassed,
    ];
    for tag in phase1_tags {
        let encoded = bincode::serialize(&tag).unwrap();
        let decoded: fsr_types::EventTag = bincode::deserialize(&encoded).unwrap();
        assert_eq!(tag, decoded, "EventTag {:?} must round-trip via bincode", tag);
    }
    // Phase 2 additions.
    for tag in [
        fsr_types::EventTag::SnapshotWritten,
        fsr_types::EventTag::ConfigReloaded,
        fsr_types::EventTag::TtcpCrystal,
    ] {
        let encoded = bincode::serialize(&tag).unwrap();
        let decoded: fsr_types::EventTag = bincode::deserialize(&encoded).unwrap();
        assert_eq!(tag, decoded, "Phase 2 EventTag {:?} must round-trip", tag);
    }
}

#[test]
fn test_phase1_15_invariants_still_pass() {
    use fsr_governance::{InvariantContext, check_invariants};
    use fsr_types::IntegrityPosture;

    let ctx = InvariantContext {
        has_nc_cert: true,
        nc_cert: None,
        event_emitted_on_transition: true,
        temporal_key_valid: true,
        quorum_checks_done: true,
        paper_validated_before_live: true,
        promotion_gate_passed: true,
        replay_divergence_disclosed: true,
        no_hidden_mutable_state: true,
        shadow_chain_consistent: true,
        commitment_chain_consistent: true,
        schema_migration_preserves_lineage: true,
        hedge_leverage_within_cap: true,
        hard_filter_not_rescued_by_ranking: true,
        safe_hold_freezes_actuation: true,
        resource_does_not_weaken_gates: true,
        resource_degrades_breadth_not_legality: true,
        buildable_without_enterprise_infra: true,
        integrity_posture: IntegrityPosture::Healthy,
    };

    let results = check_invariants(&ctx);
    assert_eq!(results.len(), 15, "Phase 2 must not break the 15-invariant count");
    for r in &results {
        assert!(r.passed, "INV-{:02} must still pass after Phase 2: {}", r.id, r.message);
    }
}
