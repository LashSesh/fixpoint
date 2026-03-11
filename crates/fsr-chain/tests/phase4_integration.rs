//! Phase 4 integration tests (spec §4).
//!
//! Covers Phase 4 subsystems:
//!   - DSHAE sandbox: all 5 scenarios PASS
//!   - Phase 4 EventTag bincode stability
//!   - DshaeEngine basic pipeline (no-arb → no crystal, arb → crystal)
//!   - Axle invariant holds for all sandbox scenarios
//!   - HIM point count matches C(n,3)

// ── Phase 4 EventTag bincode stability ───────────────────────────────────────

#[test]
fn test_phase4_event_tags_additive_only() {
    use fsr_types::EventTag;

    // Phase 4 tags must serialize to higher discriminants than Phase 3 tags.
    let encode = |tag: EventTag| -> u32 {
        let bytes = bincode::serialize(&tag).unwrap();
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    };

    let replay_completed = encode(EventTag::ReplayCompleted);
    let dshae_crystal = encode(EventTag::DshaeCrystalFormed);

    assert!(
        dshae_crystal > replay_completed,
        "DshaeCrystalFormed ({}) must be after ReplayCompleted ({})",
        dshae_crystal,
        replay_completed
    );

    // All Phase 4 tags must be in order.
    let phase4_tags = [
        encode(EventTag::DshaeCrystalFormed),
        encode(EventTag::DshaeHimUpdated),
        encode(EventTag::DshaeSandboxPass),
        encode(EventTag::DshaeSandboxFail),
        encode(EventTag::DshaeBasketRebalanced),
        encode(EventTag::GuiSessionStarted),
        encode(EventTag::GuiSessionEnded),
    ];
    for i in 1..phase4_tags.len() {
        assert!(
            phase4_tags[i] > phase4_tags[i - 1],
            "Phase 4 EventTag at position {} ({}) must be greater than position {} ({})",
            i, phase4_tags[i], i - 1, phase4_tags[i - 1]
        );
    }
}

// ── DSHAE sandbox: all 5 scenarios PASS ──────────────────────────────────────

#[test]
fn test_sandbox_calm_scenario_passes() {
    use fsr_dshae::{SandboxRunner, Scenario};
    let runner = SandboxRunner::with_default_config();
    let result = runner.run_scenario(Scenario::Calm);
    assert!(result.passed, "Calm scenario must PASS: {:?}", result.checks);
    assert_eq!(result.actual_crystals, 0, "Calm must produce 0 crystals");
}

#[test]
fn test_sandbox_single_arb_passes() {
    use fsr_dshae::{SandboxRunner, Scenario};
    let runner = SandboxRunner::with_default_config();
    let result = runner.run_scenario(Scenario::SingleArb);
    assert!(result.passed, "SingleArb scenario must PASS: {:?}", result.checks);
    assert!(
        result.actual_crystals >= 1,
        "SingleArb must detect ≥1 crystal, got {}",
        result.actual_crystals
    );
}

#[test]
fn test_sandbox_recurring_arb_passes() {
    use fsr_dshae::{SandboxRunner, Scenario};
    let runner = SandboxRunner::with_default_config();
    let result = runner.run_scenario(Scenario::RecurringArb);
    assert!(result.passed, "RecurringArb scenario must PASS: {:?}", result.checks);
    assert!(
        result.actual_crystals >= 3,
        "RecurringArb must detect ≥3 crystals, got {}",
        result.actual_crystals
    );
}

#[test]
fn test_sandbox_noisy_scenario_passes() {
    use fsr_dshae::{SandboxRunner, Scenario};
    let runner = SandboxRunner::with_default_config();
    let result = runner.run_scenario(Scenario::Noisy);
    assert!(result.passed, "Noisy scenario must PASS: {:?}", result.checks);
    assert_eq!(
        result.actual_crystals, 0,
        "Noisy must produce 0 crystals (false-positive resistance), got {}",
        result.actual_crystals
    );
}

#[test]
fn test_sandbox_regime_shift_passes() {
    use fsr_dshae::{SandboxRunner, Scenario};
    let runner = SandboxRunner::with_default_config();
    let result = runner.run_scenario(Scenario::RegimeShift);
    assert!(result.passed, "RegimeShift scenario must PASS: {:?}", result.checks);
    assert!(
        result.actual_crystals >= 2,
        "RegimeShift must detect ≥2 crystals, got {}",
        result.actual_crystals
    );
}

#[test]
fn test_all_5_scenarios_pass() {
    use fsr_dshae::{SandboxRunner, Scenario};
    let runner = SandboxRunner::with_default_config();
    let scenarios = [
        Scenario::Calm,
        Scenario::SingleArb,
        Scenario::RecurringArb,
        Scenario::Noisy,
        Scenario::RegimeShift,
    ];
    let mut pass_count = 0;
    for scenario in &scenarios {
        let result = runner.run_scenario(*scenario);
        if result.passed {
            pass_count += 1;
        } else {
            eprintln!("FAIL: {:?} — {:?}", scenario, result.checks);
        }
    }
    assert_eq!(pass_count, 5, "All 5 sandbox scenarios must PASS, got {}/5", pass_count);
}

// ── DSHAE engine pipeline ─────────────────────────────────────────────────────

#[test]
fn test_dshae_engine_no_arb_no_crystal() {
    use fsr_dshae::{DshaeConfig, DshaeEngine};
    use fsr_fixed::ONE;

    let mut cfg = DshaeConfig::default();
    cfg.enabled = true;
    cfg.holographic.min_points = 1;
    cfg.dual_simplex.anti_phase_tolerance = ONE / 10;

    let mut engine = DshaeEngine::new(cfg);

    // No-arb baseline rates (r_02 = r_01 * r_12 exactly).
    let mids = vec![
        (0usize, 1usize, 9200i64),
        (0, 2, 7912),
        (0, 3, 7516),
        (1, 2, 8600),
        (1, 3, 8170),
        (2, 3, 9500),
    ];

    for tick in 0..100 {
        let crystals = engine.push_mids(&mids, tick);
        assert!(crystals.is_empty(), "no-arb must not produce crystals at tick {}", tick);
    }
    assert_eq!(engine.crystals_found, 0);
}

#[test]
fn test_dshae_engine_arb_detects_crystal() {
    use fsr_dshae::{DshaeConfig, DshaeEngine};
    use fsr_fixed::ONE;

    let mut cfg = DshaeConfig::default();
    cfg.enabled = true;
    cfg.holographic.min_points = 1;
    cfg.dual_simplex.anti_phase_tolerance = ONE / 10;

    let mut engine = DshaeEngine::new(cfg);

    // Inject 10bp arb on triangle 0-1-2 (r_02 += 10bp).
    let base_02 = 7912i64;
    let arb_02 = base_02 + base_02 * 10 / 10000; // +10bp
    let arb_mids = vec![
        (0usize, 1usize, 9200i64),
        (0, 2, arb_02),
        (0, 3, 7516),
        (1, 2, 8600),
        (1, 3, 8170),
        (2, 3, 9500),
    ];

    let mut found = 0u64;
    for tick in 0..200 {
        let crystals = engine.push_mids(&arb_mids, tick);
        found += crystals.len() as u64;
    }
    assert!(found > 0, "10bp arb must produce ≥1 crystal over 200 ticks");
    assert_eq!(engine.crystals_found, found);
}

// ── HIM point count ───────────────────────────────────────────────────────────

#[test]
fn test_him_n4_produces_4_points() {
    use fsr_dshae::expected_him_points;
    assert_eq!(expected_him_points(4), 4, "C(4,3) = 4 HIM points for n=4");
    assert_eq!(expected_him_points(5), 10, "C(5,3) = 10 HIM points for n=5");
    assert_eq!(expected_him_points(6), 20, "C(6,3) = 20 HIM points for n=6");
}

// ── Sandbox determinism ───────────────────────────────────────────────────────

#[test]
fn test_sandbox_fully_deterministic() {
    use fsr_dshae::{SandboxRunner, Scenario};
    let runner = SandboxRunner::with_default_config();
    let r1 = runner.run_scenario(Scenario::SingleArb);
    let r2 = runner.run_scenario(Scenario::SingleArb);
    assert_eq!(
        r1.actual_crystals, r2.actual_crystals,
        "sandbox must be fully deterministic: run1={} run2={}",
        r1.actual_crystals, r2.actual_crystals
    );
    assert!(r1.replay_determinism, "replay_determinism flag must be true");
}
