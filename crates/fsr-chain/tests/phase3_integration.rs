//! Phase 3 integration tests (spec §3.8).
//!
//! Covers Phase 3 subsystems accessible via library crates:
//!   - Promotion workflow 5-gate pipeline (fsr-calibration)
//!   - EventTag bincode stability with Phase 3 additions (fsr-types)
//!   - DashboardState Phase 3 fields (fsr-tui)
//!   - CandidateLeg / Side types (fsr-types)

// ── Phase 3 EventTag bincode stability ───────────────────────────────────────

#[test]
fn test_phase3_event_tags_bincode_stable() {
    use fsr_types::EventTag;

    let encode = |tag: EventTag| -> u32 {
        let bytes = bincode::serialize(&tag).unwrap();
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    };

    let phase2_macro_cycle_end = encode(EventTag::MacroCycleEnd);
    let recording_started = encode(EventTag::RecordingStarted);
    assert!(
        recording_started > phase2_macro_cycle_end,
        "RecordingStarted ({}) must be after MacroCycleEnd ({})",
        recording_started,
        phase2_macro_cycle_end
    );

    let tag_order = [
        encode(EventTag::RecordingStarted),
        encode(EventTag::RecordingFrame),
        encode(EventTag::BacktestCompleted),
        encode(EventTag::PromotionGateChecked),
        encode(EventTag::PromotionApproved),
        encode(EventTag::SniperArmed),
        encode(EventTag::SniperExecuted),
        encode(EventTag::SniperCooldown),
        encode(EventTag::SniperDisarmed),
        encode(EventTag::DailyLossLimitHit),
        encode(EventTag::RiskLimitBreached),
        encode(EventTag::ReplayStarted),
        encode(EventTag::ReplayCompleted),
    ];
    for i in 1..tag_order.len() {
        assert_eq!(
            tag_order[i],
            tag_order[i - 1] + 1,
            "Phase 3 tags must be contiguous (index {} = {})",
            i,
            tag_order[i]
        );
    }
}

// ── Promotion Gate Checks ─────────────────────────────────────────────────────

#[test]
fn test_promotion_gates_all_pass() {
    use fsr_calibration::{run_gate_checks, PromotionGateConfig};
    use fsr_fixed::ONE;

    let cfg = PromotionGateConfig::default();
    let (all_passed, gates) = run_gate_checks(
        100,
        0,
        ONE * 6 / 10,
        100,
        3,
        &cfg,
    );
    assert!(all_passed, "All 5 gates should pass");
    assert_eq!(gates.len(), 5);
    for g in &gates {
        assert!(g.passed, "Gate {} ({}) should pass", g.gate_id, g.name);
    }
}

#[test]
fn test_promotion_gates_g1_fail_negative_pnl() {
    use fsr_calibration::{run_gate_checks, PromotionGateConfig};
    use fsr_fixed::ONE;

    let cfg = PromotionGateConfig::default();
    let (all_passed, gates) = run_gate_checks(-500, 0, ONE * 6 / 10, 100, 3, &cfg);
    assert!(!all_passed, "G1 should fail with negative PnL");
    assert!(!gates[0].passed, "Gate 1 (no net loss) should fail");
}

#[test]
fn test_promotion_gates_g3_fail_low_win_rate() {
    use fsr_calibration::{run_gate_checks, PromotionGateConfig};
    use fsr_fixed::ONE;

    let cfg = PromotionGateConfig::default();
    let (all_passed, gates) = run_gate_checks(500, 0, ONE * 3 / 10, 100, 3, &cfg);
    assert!(!all_passed);
    assert!(!gates[2].passed, "Gate 3 (win rate) should fail");
}

#[test]
fn test_promotion_gates_g4_fail_excess_drawdown() {
    use fsr_calibration::{run_gate_checks, PromotionGateConfig};
    use fsr_fixed::ONE;

    let cfg = PromotionGateConfig::default();
    // Default max_drawdown_bps = 3000*ONE; supply 4000*ONE → G4 fails
    let (all_passed, gates) = run_gate_checks(500, 0, ONE * 6 / 10, 4000i64 * ONE, 3, &cfg);
    assert!(!all_passed);
    assert!(!gates[3].passed, "Gate 4 (drawdown) should fail");
}

#[test]
fn test_promotion_gates_g5_fail_no_crystals() {
    use fsr_calibration::{run_gate_checks, PromotionGateConfig};
    use fsr_fixed::ONE;

    let cfg = PromotionGateConfig::default();
    let (all_passed, gates) = run_gate_checks(500, 0, ONE * 6 / 10, 100, 0, &cfg);
    assert!(!all_passed);
    assert!(!gates[4].passed, "Gate 5 (TTCP crystal) should fail");
}

// ── PromotionWorkflow FSM ─────────────────────────────────────────────────────

#[test]
fn test_promotion_workflow_full_path() {
    use fsr_calibration::PromotionWorkflow;
    use fsr_fixed::ONE;
    use fsr_types::PromotionState;

    let mut wf = PromotionWorkflow::new();
    assert_eq!(wf.state, PromotionState::Candidate);

    let passed = wf.evaluate_backtest(
        500, 0, ONE * 6 / 10, 200, 2,
        "aabbccddee", "11223344ff", 1_000_000u64,
    );
    assert!(passed, "evaluate_backtest should return true when all gates pass");
    assert_eq!(wf.state, PromotionState::PromotionPending);
    assert!(wf.proposal.is_some(), "Proposal should be created");
    assert!(wf.proposal.as_ref().unwrap().all_gates_passed());

    let proposal_id = wf.proposal.as_ref().unwrap().proposal_id.clone();

    let approved = wf.approve(&proposal_id);
    assert!(approved, "Approval should succeed");
    assert_eq!(wf.state, PromotionState::LiveEligible);

    assert!(wf.can_activate());
    let activated = wf.activate_live();
    assert!(activated);
    assert_eq!(wf.state, PromotionState::LiveActive);
}

#[test]
fn test_promotion_workflow_fails_gates() {
    use fsr_calibration::PromotionWorkflow;
    use fsr_fixed::ONE;
    use fsr_types::PromotionState;

    let mut wf = PromotionWorkflow::new();
    let passed = wf.evaluate_backtest(
        -1000, 0, ONE * 6 / 10, 100, 2,
        "failconfig00", "failhash0000", 2_000_000u64,
    );
    assert!(!passed, "evaluate_backtest should return false when gates fail");
    assert_eq!(wf.state, PromotionState::Candidate);
    assert!(wf.proposal.is_none(), "No proposal when gates fail");
}

#[test]
fn test_promotion_workflow_approve_wrong_id() {
    use fsr_calibration::PromotionWorkflow;
    use fsr_fixed::ONE;

    let mut wf = PromotionWorkflow::new();
    let passed = wf.evaluate_backtest(
        100, 0, ONE * 6 / 10, 50, 1,
        "goodcfg00", "goodhash0", 3_000_000u64,
    );
    assert!(passed);
    let approved = wf.approve("wrong_id_xyz");
    assert!(!approved, "Approval with wrong ID must fail");
}

// ── DashboardState Phase 3 fields ─────────────────────────────────────────────

#[test]
fn test_dashboard_state_phase3_defaults() {
    use fsr_tui::state::DashboardState;

    let state = DashboardState::default();
    assert!(!state.sniper.enabled, "sniper disabled by default");
    assert_eq!(state.sniper.total_executions, 0);
    assert_eq!(state.risk.net_pnl_bps, 0);
    assert!(!state.risk.daily_limit_hit);
    assert_eq!(state.binance_l1_count, 0);
    assert_eq!(state.binance_l2_count, 0);
    assert_eq!(state.kraken_l1_count, 0);
    assert_eq!(state.cross_venue_count, 0);
    assert!(!state.sniper_toggle_requested);
}

#[test]
fn test_dashboard_state_event_log_bounded() {
    use fsr_tui::state::{DashboardState, MAX_EVENT_LOG};

    let mut state = DashboardState::default();
    for i in 0..MAX_EVENT_LOG + 10 {
        state.push_event(format!("event_{}", i));
    }
    assert_eq!(state.event_log.len(), MAX_EVENT_LOG, "event log must be capped");
    assert_eq!(
        state.event_log.back().unwrap(),
        &format!("event_{}", MAX_EVENT_LOG + 9)
    );
}

// ── Phase 3 CandidateLeg / Side types ────────────────────────────────────────

#[test]
fn test_candidate_leg_fields() {
    use fsr_types::{
        ids::{TradingPair, VenueId},
        market::{CandidateLeg, Side},
    };

    let leg = CandidateLeg {
        venue: VenueId("BINANCE".to_string()),
        pair: TradingPair::new("BTC", "USDT"),
        side: Side::Buy,
        price: 100_000i64,
        available_qty: 500i64,
    };
    assert!(matches!(leg.side, Side::Buy));
    assert_eq!(leg.venue.0, "BINANCE");
    assert_eq!(leg.pair.0, "BTC");
}

#[test]
fn test_side_serde_roundtrip() {
    use fsr_types::market::Side;

    let buy_bytes = bincode::serialize(&Side::Buy).unwrap();
    let sell_bytes = bincode::serialize(&Side::Sell).unwrap();
    let buy_rt: Side = bincode::deserialize(&buy_bytes).unwrap();
    let sell_rt: Side = bincode::deserialize(&sell_bytes).unwrap();
    assert!(matches!(buy_rt, Side::Buy));
    assert!(matches!(sell_rt, Side::Sell));
    let buy_disc = u32::from_le_bytes([buy_bytes[0], buy_bytes[1], buy_bytes[2], buy_bytes[3]]);
    let sell_disc = u32::from_le_bytes([sell_bytes[0], sell_bytes[1], sell_bytes[2], sell_bytes[3]]);
    assert_eq!(buy_disc, 0);
    assert_eq!(sell_disc, 1);
}
