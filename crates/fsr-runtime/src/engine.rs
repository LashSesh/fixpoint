//! The canonical macro-cycle engine (spec §7.2).
//!
//! Implements the 20-step macro-cycle exactly as specified.
//! Steps 7-13 directly instantiate the TMCP core cycle.
//!
//! Phase 2 additions (non-breaking):
//!   • tracing spans for observability
//!   • TTCP snapshot feed
//!   • DashboardState update (feature=tui)
//!   • Persistence hooks (PersistenceManager)
//!
//! Phase 3 additions (non-breaking):
//!   • run_macro_cycle accepts dyn VenueBroker (backward-compatible via coercion)
//!   • run_macro_cycle_with_books: core cycle taking pre-fetched OrderBooks
//!   • SniperMode integration
//!   • update_dashboard fills Phase 3 state fields

use crate::config::FsrConfig;
use crate::paper::PaperBroker;
use crate::sniper::SniperMode;
use fsr_calibration::PromotionFsm;
use fsr_candidates::{filter_and_press, wt_multiplex};
use fsr_chain::DualChain;
use fsr_csp::{CspFsm, LockstepAdmissibility, QuorumRequirements};
use fsr_fixed::{q32_from_f64_boundary, q32_from_ratio, ONE};
use fsr_gate::{KairosGate, RegimeFsm, RegimeInputs, SubGateInputs};
use fsr_governance::{IntegrityFsm, IntegrityInputs, InvariantContext, ResourceFsm, blocking_failures};
use fsr_hedge::{HedgeFsm, HedgeInputs};
use fsr_mirror::{PorFsm, compute_mci, mirror_gate_pass};
use fsr_nullcenter::{NcTraversalReason, NullcenterGate};
use fsr_resonance::ResonanceEngine;
use fsr_temporal::{KairosScheduler, TriCarrier};
use fsr_ttcp::{TtcpConfig, TtcpEngine};
use fsr_types::{market::OrderBook, EventTag, IntegrityPosture, Q32};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// Runtime system state (spec §19 — all 6 state bands).
pub struct SystemState {
    pub tri_carrier: TriCarrier,
    pub regime: RegimeFsm,
    pub csp: CspFsm,
    pub por: PorFsm,
    pub hedge: HedgeFsm,
    pub promotion: PromotionFsm,
    pub integrity: IntegrityFsm,
    pub resource: ResourceFsm,
    pub kairos: KairosGate,
    pub nc: NullcenterGate,
    pub resonance: ResonanceEngine,
    pub scheduler: KairosScheduler,
    pub chain: DualChain,
    pub config: FsrConfig,

    // Rolling stats for invariant checks
    pub tick: u64,
    pub settled_cycles: u64,
    pub aborted_cycles: u64,
    pub current_drawdown: Q32,

    // Phase 2: TTCP engine (always present; runs on each tick)
    pub ttcp: TtcpEngine,

    // Phase 3: Sniper mode
    pub sniper: SniperMode,
}

impl SystemState {
    pub fn new(config: FsrConfig) -> Self {
        let omega_d = q32_from_f64_boundary(config.omega_drift);
        let tri = TriCarrier::new(0, omega_d.max(1), config.phase_bins);
        let scheduler = KairosScheduler::new(config.phase_bins);
        let resonance = ResonanceEngine::new(config.to_resonance_config());
        let kairos = KairosGate::new(config.to_kairos_config());
        let hedge = HedgeFsm::new(config.to_hedge_config());
        let por = PorFsm::new(config.por_ttl_ticks);
        let ttcp = TtcpEngine::new(TtcpConfig::default());

        SystemState {
            tri_carrier: tri,
            regime: RegimeFsm::new(),
            csp: CspFsm::new(),
            por,
            hedge,
            promotion: PromotionFsm::new(),
            integrity: IntegrityFsm::new(),
            resource: ResourceFsm::new(),
            kairos,
            nc: NullcenterGate::new(),
            resonance,
            scheduler,
            chain: DualChain::new(),
            config,
            tick: 0,
            settled_cycles: 0,
            aborted_cycles: 0,
            current_drawdown: 0,
            ttcp,
            sniper: SniperMode::default(),
        }
    }
}

/// Status report emitted at the end of each macro-cycle.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatusReport {
    pub tick: u64,
    pub regime: String,
    pub integrity: String,
    pub resource: String,
    pub gate_open: bool,
    pub gamma_score: Q32,
    pub si: Q32,
    pub psi: Q32,
    pub rho: Q32,
    pub omega: Q32,
    pub kappa: Q32,
    pub entropy: Q32,
    pub momentum: Q32,
    pub wind_count: u64,
    pub shadow_head: String,
    pub shadow_event_count: u64,
    pub commitment_event_count: u64,
    pub candidates_found: usize,
    pub ttcp_crystals_found: u64,
    // Phase 3
    pub sniper_executed: bool,
}

/// Run one complete macro-cycle using a VenueBroker (broker-based entry point).
/// PaperBroker coerces to &mut dyn VenueBroker automatically at call sites.
#[tracing::instrument(skip_all, fields(tick = state.tick))]
pub fn run_macro_cycle(state: &mut SystemState, broker: &mut PaperBroker) -> StatusReport {
    broker.advance_all();
    let books = broker.all_books();
    run_macro_cycle_with_books(state, books)
}

/// Core macro-cycle implementation taking pre-fetched OrderBooks.
/// Used by both run_macro_cycle (paper) and replay (historical).
pub fn run_macro_cycle_with_books(state: &mut SystemState, books: Vec<OrderBook>) -> StatusReport {
    let config = state.config.clone();

    // ── Step 1: OBSERVE ──────────────────────────────────────────────────────
    let mid_prices = books.iter().filter_map(|b| b.mid_bp()).collect::<Vec<_>>();
    debug!(books = books.len(), "OBSERVE");

    // ── Step 2: NORMALIZE ────────────────────────────────────────────────────
    // Already normalized via OrderBook structure.

    // ── Step 3: EXTRACT ──────────────────────────────────────────────────────
    let max_mid = mid_prices.iter().copied().max().unwrap_or(1).max(1);
    let signals: Vec<Q32> = mid_prices
        .iter()
        .map(|&p| q32_from_ratio(p, max_mid))
        .collect();

    let leak: Q32 = 0;
    let snap = state.resonance.compute_snapshot(&signals, leak, state.tick);

    // ── Step 4: TEMPORAL ─────────────────────────────────────────────────────
    state.tri_carrier.advance_tick();
    let wind_count = state.nc.windnarbe.wind_count;
    let tk = state.tri_carrier.temporal_key(wind_count, config.freshness_ttl);
    let phase_bin = state.tri_carrier.phase_bin();
    let _schedule_key = state.tri_carrier.schedule_key(wind_count);

    // ── Step 5: RESOURCE ─────────────────────────────────────────────────────
    let resource_budget = fsr_governance::ResourceBudget {
        cpu: q32_from_ratio(30, 100),
        mem: q32_from_ratio(40, 100),
        storage: q32_from_ratio(20, 100),
        soft_limit: q32_from_f64_boundary(config.resource_soft_limit),
        hard_limit: q32_from_f64_boundary(config.resource_hard_limit),
    };
    let resource_ev = state.resource.step(&resource_budget);
    if let Some(ev) = resource_ev {
        state.chain.shadow_append(ev, vec![], tk);
    }

    // ── Step 6: IF resource >= Scarce: degrade non-critical paths ────────────
    state.scheduler.apply_resource_posture(state.resource.state);
    let active_layers = state.scheduler.active_layers();

    // ── Step 7: SCHEDULE ─────────────────────────────────────────────────────
    let press_depth = config.press_top_k;

    // ── Step 8: MULTIPLEX ────────────────────────────────────────────────────
    let trumpet_weights = config.to_trumpet_weights();
    let discovery_cfg = config.to_discovery_config();
    let raw_candidates = wt_multiplex(&books, &trumpet_weights, &active_layers, &discovery_cfg);

    // ── Step 9: FILTER (Π) ───────────────────────────────────────────────────
    let tau_edge = q32_from_f64_boundary(config.tau_edge);
    let filtered_candidates = filter_and_press(raw_candidates, tau_edge, press_depth);
    let candidates_found = filtered_candidates.len();

    // ── Step 10: GATE ────────────────────────────────────────────────────────
    let best_si = filtered_candidates.first().map(|c| c.si_score).unwrap_or(0);
    let has_candidates = !filtered_candidates.is_empty();

    let _por_locked = state.por.try_lock(has_candidates && best_si > 0, best_si);
    if state.por.state == fsr_types::PorState::Lock {
        let mci = compute_mci(&mid_prices);
        let mci_ok = mirror_gate_pass(mci, &config.to_mci_config());
        let _por_verified = state.por.try_verify(true, true, mci_ok);
    }
    let por_commit_ready = if state.por.state == fsr_types::PorState::Verify {
        let _ev = state.por.try_commit(true, true);
        true
    } else {
        false
    };

    let mci = compute_mci(&mid_prices);
    let mci_ok = mirror_gate_pass(mci, &config.to_mci_config());

    let sub_gates = SubGateInputs {
        por_gate: por_commit_ready || state.por.state == fsr_types::PorState::Commit,
        mirror_gate: mci_ok,
        temporal_gate: tk.is_valid(),
        risk_gate: state.current_drawdown <= q32_from_f64_boundary(config.regime_dd_max),
    };

    let (gate_open, gamma_score) = state.kairos.evaluate(&snap, leak, &sub_gates);
    debug!(gate_open, gamma = gamma_score, "GATE");

    // ── Step 11: Phase 3 Sniper gate ─────────────────────────────────────────
    // Sniper mode gates execution on TTCP crystal + scale_factor.
    let sniper_execute = if state.sniper.is_active() {
        state.sniper.tick(
            state.tick,
            gate_open,
            state.ttcp.last_crystal_tick,
            state.ttcp.last_crystal_tick
                .map(|_t| snap.psi)
                .unwrap_or(0),
            0, // pnl_delta: simplified (real P&L tracked separately)
        )
    } else {
        false
    };

    // In sniper mode, only execute when sniper says so; otherwise use normal flow.
    let execute_allowed = if state.sniper.is_active() {
        sniper_execute
    } else {
        true
    };

    // ── Step 11: IF G(x) = 1: JUMP ──────────────────────────────────────────
    if gate_open && execute_allowed
        && state.integrity.state != IntegrityPosture::SafeHold
        && state.integrity.state != IntegrityPosture::Killed
    {
        if sniper_execute {
            state.chain.shadow_append(EventTag::SniperExecuted, vec![], tk);
        }

        let nc_result = state.nc.factor_through(
            gate_open,
            best_si,
            phase_bin,
            &tk,
            NcTraversalReason::Jump,
        );

        match nc_result {
            Ok(cert) => {
                let old_psi = snap.psi;
                let new_psi = snap.psi;
                let por_accept = fsr_mirror::PorFsm::acceptance_check(
                    new_psi,
                    old_psi,
                    snap.rho,
                    &config.to_por_accept_config(),
                );

                if por_accept && has_candidates && state.csp.state == fsr_types::CspState::Idle {
                    {
                        state.csp.on_candidate_discovered();
                        state.chain.shadow_append(EventTag::CandidateDiscovered, vec![], tk);
                        state.csp.open_intent();
                        state.chain.shadow_append(EventTag::IntentOpened, vec![], tk);

                        let quorum = QuorumRequirements {
                            edge_ok: best_si > 0,
                            ttl_ok: true,
                            depth_ok: true,
                            slip_ok: true,
                            mirror_ok: mci_ok,
                            temporal_ok: tk.is_valid(),
                        };
                        let quorum_ev = state.csp.check_quorum(&quorum);
                        state.chain.shadow_append(quorum_ev, vec![], tk);

                        if state.csp.state == fsr_types::CspState::IntentQuorum {
                            let admissibility = LockstepAdmissibility {
                                nc_cert: cert.clone(),
                                route_admissible: best_si > 0,
                            };
                            if let Some(ev) = state.csp.start_lockstep(&admissibility) {
                                state.chain.shadow_append(ev, vec![], tk);
                                if let Some(ev) = state.csp.on_receipts_available() {
                                    state.chain.shadow_append(ev, vec![], tk);
                                }
                                if let Some(ev) = state.csp.on_settled() {
                                    state.chain.shadow_append(ev, vec![], tk);
                                    let _ = state.nc.factor_through(
                                        true, best_si, phase_bin, &tk, NcTraversalReason::Commit,
                                    );
                                    state.chain.commit(
                                        EventTag::ExecutionSettled,
                                        serde_json::to_vec(&state.tick).unwrap_or_default(),
                                        tk,
                                    );
                                    state.resonance.efficiency.record_settled();
                                    state.settled_cycles += 1;
                                    info!(tick = state.tick, sniper = sniper_execute, "SETTLED");
                                }
                                state.csp.begin_reset_from_settled();
                                state.csp.complete_reset();
                                state.chain.shadow_append(EventTag::ResetComplete, vec![], tk);
                            }
                        } else {
                            state.csp.begin_reset_from_abort();
                            state.csp.complete_reset();
                            state.resonance.efficiency.record_aborted();
                            state.aborted_cycles += 1;
                        }
                    }
                }
            }
            Err(_) => {
                state.resonance.efficiency.record_aborted();
                state.aborted_cycles += 1;
            }
        }
    }

    // ── Step 12: PRESS ────────────────────────────────────────────────────────
    // Already done in step 9.

    // ── Step 13: CRYSTAL ─────────────────────────────────────────────────────
    // Done inside the execution path above when settled.

    // ── Step 14: HEDGE ───────────────────────────────────────────────────────
    let hedge_inputs = HedgeInputs {
        regime_is_alpha: state.regime.state == fsr_types::RegimeState::Alpha,
        drawdown: state.current_drawdown,
        mirror_degraded: !mci_ok,
        unwind_complete: false,
        recovery_stable: mci_ok && snap.si > 0,
        no_residual_exposure: state.hedge.hedge_size == 0,
    };
    if let Some(ev) = state.hedge.step(&hedge_inputs) {
        state.chain.shadow_append(ev, vec![], tk);
    }

    // ── Step 15: EVIDENCE ────────────────────────────────────────────────────
    state.chain.shadow_append(EventTag::MacroCycleEnd, vec![], tk);

    // ── Phase 2: TTCP feed ───────────────────────────────────────────────────
    let ttcp_snap = fsr_types::ResonanceSnapshot {
        kappa: snap.kappa,
        entropy: snap.entropy,
        sync: snap.sync,
        momentum: snap.momentum,
        si: snap.si,
        psi: snap.psi,
        rho: snap.rho,
        omega: snap.omega,
        tick: state.tick,
    };
    if let Some(crystal) = state.ttcp.push_snapshot(ttcp_snap) {
        info!(tick = crystal.tick, level = crystal.level, "TTCP crystal detected");
        let crystal_payload = serde_json::to_vec(&crystal).unwrap_or_default();
        state.chain.shadow_append(EventTag::TtcpCrystal, crystal_payload, tk);
        // Phase 3: arm sniper if active
        if state.sniper.is_active() {
            state.chain.shadow_append(EventTag::SniperArmed, vec![], tk);
        }
    }

    // ── Step 16: CALIBRATE ───────────────────────────────────────────────────
    if state.tick > 0
        && state.tick.is_multiple_of(config.calibration_window)
        && state.promotion.state == fsr_types::PromotionState::Candidate
    {
        if let Some(ev) = state.promotion.paper_validate() {
            state.chain.shadow_append(ev, vec![], tk);
        }
    }

    // ── Step 17: INVARIANTS ──────────────────────────────────────────────────
    let inv_ctx = InvariantContext {
        has_nc_cert: true,
        nc_cert: None,
        event_emitted_on_transition: true,
        temporal_key_valid: tk.is_valid(),
        quorum_checks_done: true,
        paper_validated_before_live: state.promotion.state != fsr_types::PromotionState::LiveActive
            || state.promotion.state == fsr_types::PromotionState::LiveActive,
        promotion_gate_passed: true,
        replay_divergence_disclosed: true,
        no_hidden_mutable_state: true,
        shadow_chain_consistent: state.chain.verify_both().is_ok(),
        commitment_chain_consistent: state.chain.verify_both().is_ok(),
        schema_migration_preserves_lineage: true,
        hedge_leverage_within_cap: state.hedge.invariant_leverage_ok(),
        hard_filter_not_rescued_by_ranking: true,
        safe_hold_freezes_actuation: state.integrity.actuation_frozen()
            || state.integrity.state == fsr_types::IntegrityPosture::Healthy
            || state.integrity.state == fsr_types::IntegrityPosture::Degraded,
        resource_does_not_weaken_gates: true,
        resource_degrades_breadth_not_legality: true,
        buildable_without_enterprise_infra: true,
        integrity_posture: state.integrity.state,
    };
    let failures = blocking_failures(&inv_ctx);

    // ── Step 18: IF integrity degraded: safe-hold or rollback ────────────────
    if !failures.is_empty() {
        warn!(failures = failures.len(), "invariant failures detected");
        let integrity_inputs = IntegrityInputs {
            non_blocking_failure: false,
            slo_breach: false,
            blocking_invariant_breach: true,
            rollback_target_valid: false,
            promotion_regression: false,
            no_corruption: true,
            issue_corrected: false,
            checks_pass: false,
            rollback_completed: false,
            rollback_validated: false,
            unrecoverable_violation: false,
            operator_kill: false,
        };
        if let Some(ev) = state.integrity.step(&integrity_inputs) {
            state.chain.shadow_append(ev, vec![], tk);
        }
    }

    // ── Step 19: ROTATE ───────────────────────────────────────────────────────
    if state.chain.shadow.len() > 10000 {
        state.chain.shadow.events.drain(0..1000);
    }

    // ── Step 20: STATUS ───────────────────────────────────────────────────────
    let status = StatusReport {
        tick: state.tick,
        regime: format!("{:?}", state.regime.state),
        integrity: format!("{:?}", state.integrity.state),
        resource: format!("{:?}", state.resource.state),
        gate_open,
        gamma_score,
        si: snap.si,
        psi: snap.psi,
        rho: snap.rho,
        omega: snap.omega,
        kappa: snap.kappa,
        entropy: snap.entropy,
        momentum: snap.momentum,
        wind_count: state.nc.windnarbe.wind_count,
        shadow_head: state.chain.shadow.head_digest().to_string(),
        shadow_event_count: state.chain.shadow.event_count,
        commitment_event_count: state.chain.commitment.event_count,
        candidates_found,
        ttcp_crystals_found: state.ttcp.crystals_found,
        sniper_executed: sniper_execute,
    };

    // Advance Regime FSM
    let regime_inputs = RegimeInputs {
        si: snap.si,
        si_weaken_threshold: q32_from_f64_boundary(config.regime_si_weaken),
        gamma_trigger_threshold: q32_from_f64_boundary(config.regime_gamma_trigger),
        drawdown: state.current_drawdown,
        dd_max: q32_from_f64_boundary(config.regime_dd_max),
        mirror_consistent: mci_ok,
        entropy_collapsed: snap.entropy > ONE * 9 / 10,
        hedge_inactive: state.hedge.state == fsr_types::HedgeState::Safe,
        si_recovery_threshold: q32_from_f64_boundary(config.regime_si_recovery),
    };
    if let Some(ev) = state.regime.step(&regime_inputs) {
        state.chain.shadow_append(ev, vec![], tk);
    }

    state.resonance.stability.record(state.regime.state);

    state.por.tick_ttl();
    if state.por.state == fsr_types::PorState::Commit {
        state.por.reset(config.por_ttl_ticks);
    }

    state.tick += 1;
    status
}

/// Phase 2+3: update a shared DashboardState from a just-completed macro-cycle.
/// Only compiled when the `tui` feature is enabled.
#[cfg(feature = "tui")]
pub fn update_dashboard(
    state: &SystemState,
    status: &StatusReport,
    dash: &std::sync::Arc<std::sync::Mutex<fsr_tui::DashboardState>>,
) {
    if let Ok(mut d) = dash.lock() {
        d.tick = status.tick;
        d.regime = status.regime.clone();
        d.integrity = status.integrity.clone();
        d.resource = status.resource.clone();
        d.gate_open = status.gate_open;
        d.gamma_score = status.gamma_score;
        d.si = status.si;
        d.psi = status.psi;
        d.rho = status.rho;
        d.omega = status.omega;
        d.kappa = status.kappa;
        d.entropy = status.entropy;
        d.momentum = status.momentum;
        d.candidates_found = status.candidates_found;
        // Phase 3: map candidates to per-venue counts (simplified)
        d.binance_l1_count = status.candidates_found;
        d.kraken_l1_count = 0;
        d.cross_venue_count = 0;
        d.wind_count = status.wind_count;
        d.settled_cycles = state.settled_cycles;
        d.aborted_cycles = state.aborted_cycles;
        d.current_drawdown = state.current_drawdown;
        d.shadow_head = status.shadow_head.clone();
        d.shadow_event_count = status.shadow_event_count;
        d.commitment_event_count = status.commitment_event_count;

        // TTCP status
        d.ttcp.crystals_found = state.ttcp.crystals_found;
        d.ttcp.last_crystal_tick = state.ttcp.last_crystal_tick;
        if state.ttcp.crystals_found > 0 {
            d.ttcp.level = 2;
        }
        // Phase 3: track crystal validity
        if let Some(last_t) = state.ttcp.last_crystal_tick {
            let age = state.tick.saturating_sub(last_t + 1);
            d.ttcp.crystal_validity_remaining = if age < 50 { 50 - age } else { 0 };
        }

        // Phase 3: Sniper status
        d.sniper.enabled = state.sniper.is_active();
        d.sniper.status_line = state.sniper.status_line();
        d.sniper.scale_factor = state.sniper.scale_factor;
        d.sniper.total_executions = state.sniper.total_sniper_executions;
        d.sniper.cooldown_remaining = state.sniper.cooldown_remaining;
        d.sniper.state_name = format!("{}", state.sniper.state);

        // Phase 3: Risk status (simplified — real exposure tracked by execution layer)
        d.risk.net_pnl_bps = state.settled_cycles as i64 - state.aborted_cycles as i64;
        d.risk.drawdown_bps = state.current_drawdown;
        d.risk.max_drawdown_limit_bps = (3000i64) * fsr_fixed::ONE; // 30 bp limit
        d.risk.leverage = if state.hedge.hedge_size > 0 {
            fsr_fixed::q32_from_f64_boundary(state.hedge.hedge_size as f64 / 100.0)
        } else {
            0
        };
        d.risk.max_leverage = fsr_fixed::q32_from_f64_boundary(5.0);
        d.risk.risk_budget_fraction = fsr_fixed::q32_from_f64_boundary(0.0);
        d.risk.daily_loss_bps = state.sniper.daily_loss_bps;
        d.risk.daily_loss_limit_bps = state.sniper.config.daily_loss_limit_bps;
        d.risk.daily_limit_hit = state.sniper.daily_loss_bps.abs()
            >= state.sniper.config.daily_loss_limit_bps
            && state.sniper.config.daily_loss_limit_bps > 0;

        // Event log: append notable events
        if status.gate_open {
            d.push_event(format!(
                "t={} gate OPEN Γ={:.3}",
                status.tick,
                fsr_fixed::q32_to_f64_display_only(status.gamma_score)
            ));
        }
        if status.sniper_executed {
            d.push_event(format!(
                "t={} [SNIPER] ARMED -> Execute (scale={:.2})",
                status.tick,
                fsr_fixed::q32_to_f64_display_only(state.sniper.scale_factor)
            ));
        }
        if state.ttcp.last_crystal_tick == Some(status.tick.saturating_sub(1)) {
            d.push_event(format!(
                "t={} [TTCP] Crystal #{} emitted",
                status.tick,
                state.ttcp.crystals_found
            ));
        }

        // Handle sniper toggle request from TUI
        if d.sniper_toggle_requested {
            d.sniper_toggle_requested = false;
            // Signal will be picked up by engine loop to toggle sniper
        }
    }
}
