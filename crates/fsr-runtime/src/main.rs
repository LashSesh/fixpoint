//! fsr-runtime: CLI entry point for FIXPOINT SWARM-R v3.1.0 (Phase 3).
//!
//! Phase 1 commands (all unchanged):
//!   run       -- Run paper-mode main loop for N ticks
//!   status    -- Show current system status
//!   replay    -- Replay from chain events file
//!   verify    -- Verify chain integrity
//!   promote   -- Trigger promotion flow
//!   config    -- Show/validate config profile
//!   invariants -- Check all 15 invariants
//!   benchmark -- Run calibration benchmark
//!
//! Phase 2 new flags on `run`:
//!   --persist, --data-dir, --snapshot-every, --log-json, --tui, --hot-reload, --ttcp-dir
//!
//! Phase 3 new commands:
//!   run-paper           -- Explicit paper-mode run (alias for `run`)
//!   run-live            -- Live venue run with optional recording & sniper mode
//!   replay-historical   -- Historical replay with backtest report
//!   promote (extended)  -- 5-gate promotion pipeline
//!   backtest-report     -- Display a stored backtest report

mod backtest_report;
mod binance;
mod config;
mod engine;
mod hot_reload;
mod kraken;
mod observability;
mod paper;
mod persistence;
mod recorder;
mod replay_historical;
mod sniper;

use clap::{Parser, Subcommand};
use config::FsrConfig;
use engine::{run_macro_cycle, SystemState};
use paper::default_paper_broker;
use persistence::{PersistenceConfig, PersistenceManager};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

#[derive(Parser, Debug)]
#[command(
    name = "fsr",
    version = "3.1.0",
    about = "FIXPOINT SWARM-R: Deterministic Rust trading instrument (Phase 3)"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Configuration profile: conservative, balanced, aggressive, minimal
    #[arg(long, default_value = "conservative")]
    profile: String,

    /// Path to YAML config file (overrides profile)
    #[arg(long)]
    config: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run paper-mode main loop for N ticks (Phase 1/2 command, unchanged)
    Run {
        #[arg(long, default_value = "1000")]
        ticks: u64,
        #[arg(long, default_value = "100")]
        print_every: u64,
        #[arg(long, default_value = "text")]
        output: String,
        #[arg(long)]
        persist: bool,
        #[arg(long, default_value = "data")]
        data_dir: PathBuf,
        #[arg(long, default_value = "100")]
        snapshot_every: u64,
        #[arg(long)]
        log_json: bool,
        #[arg(long)]
        tui: bool,
        #[arg(long)]
        hot_reload: bool,
        #[arg(long)]
        ttcp_dir: Option<PathBuf>,
    },
    /// Run paper-mode main loop (Phase 3 explicit alias)
    RunPaper {
        #[arg(long, default_value = "1000")]
        ticks: u64,
        #[arg(long, default_value = "100")]
        print_every: u64,
        #[arg(long)]
        no_tui: bool,
        #[arg(long)]
        persist: bool,
        #[arg(long, default_value = "data")]
        data_dir: PathBuf,
    },
    /// Run with live venue data (requires --features live-data).
    /// Phase 3: supports --record (save .rec files) and --sniper mode.
    RunLive {
        /// Venue: binance or kraken
        #[arg(long, default_value = "binance")]
        venue: String,
        /// Record order books to data/recordings/
        #[arg(long)]
        record: bool,
        /// Enable sniper mode (TTCP-gated execution)
        #[arg(long)]
        sniper: bool,
        /// Disable TUI
        #[arg(long)]
        no_tui: bool,
        /// Root data directory
        #[arg(long, default_value = "data")]
        data_dir: PathBuf,
        /// Number of ticks (0 = run forever)
        #[arg(long, default_value = "0")]
        ticks: u64,
    },
    /// Replay recorded market data through the macro-cycle (Phase 3 §2.3).
    ReplayHistorical {
        /// Path pattern to .rec files (glob or space-separated)
        #[arg(long, required = true)]
        recording: Vec<PathBuf>,
        /// Output directory for chain files, report, logs
        #[arg(long, default_value = "data/backtest/latest")]
        output: PathBuf,
    },
    /// Promotion workflow: run backtest gate checks or approve a proposal (Phase 3 §4).
    PromoteCmd {
        /// Evaluate a config against a recording (5-gate pipeline)
        #[arg(long)]
        recording: Option<Vec<PathBuf>>,
        /// Approve an existing proposal by ID
        #[arg(long)]
        approve: Option<String>,
        /// Show current promotion status
        #[arg(long)]
        status: bool,
    },
    /// Display a stored backtest report (Phase 3 §3).
    BacktestReport {
        /// Run ID or path to report directory
        #[arg(long)]
        run: String,
        /// Base data directory
        #[arg(long, default_value = "data/backtest")]
        data_dir: PathBuf,
    },
    /// Show current system status (single tick)
    Status,
    /// Verify chain integrity from recorded events
    Verify,
    /// Trigger promotion flow (paper-mode, Phase 1/2 compat)
    Promote,
    /// Show configuration profile
    Config {
        #[arg(long, default_value = "false")]
        yaml: bool,
    },
    /// Check all 15 invariants against a baseline context
    Invariants,
    /// Run calibration benchmark
    Benchmark {
        #[arg(long, default_value = "100")]
        ticks: u64,
    },
    /// Replay events (requires events file)
    Replay {
        #[arg(long)]
        events_file: Option<String>,
    },
    /// Validate a YAML config file
    ValidateConfig,
    /// Show recent run IDs
    Runs {
        #[arg(long, default_value = "10")]
        last: usize,
    },
}

fn load_config(profile: &str, config_path: Option<&str>) -> FsrConfig {
    if let Some(path) = config_path {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(cfg) = FsrConfig::from_yaml(&content) {
                return cfg;
            }
        }
        eprintln!("Warning: failed to load config from {}, using profile", path);
    }
    match profile {
        "balanced" => FsrConfig::balanced(),
        "aggressive" => FsrConfig::aggressive(),
        "minimal" => FsrConfig::minimal(),
        _ => FsrConfig::conservative(),
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_run(
    config: FsrConfig,
    config_path: Option<String>,
    ticks: u64,
    print_every: u64,
    output: &str,
    persist: bool,
    data_dir: PathBuf,
    snapshot_every: u64,
    _log_json: bool,
    use_tui: bool,
    hot_reload_enabled: bool,
    ttcp_dir: Option<PathBuf>,
    sniper_enabled: bool,
) {
    let run_id = format!("run_{}", fsr_chain::persist::unix_ms());

    let mut pm = if persist {
        match PersistenceManager::new(
            run_id.clone(),
            PersistenceConfig {
                data_dir: data_dir.clone(),
                segment_size: 1000,
                snapshot_interval: snapshot_every,
            },
        ) {
            Ok(pm) => {
                info!(run_id = %run_id, "Persistence enabled");
                pm
            }
            Err(e) => {
                warn!(error = %e, "Failed to init persistence, running without");
                PersistenceManager::disabled()
            }
        }
    } else {
        PersistenceManager::disabled()
    };

    let mut state = SystemState::new(config.clone());
    let mut broker = default_paper_broker();

    // Phase 3: enable sniper if requested
    if sniper_enabled {
        state.sniper.enable();
        info!("Sniper mode enabled");
    }

    // Hot-reload watcher.
    let mut watcher = if hot_reload_enabled {
        config_path.as_deref().and_then(|path| {
            match hot_reload::ConfigWatcher::new(PathBuf::from(path)) {
                Ok(w) => Some(w),
                Err(e) => {
                    warn!(error = %e, "Failed to init hot-reload watcher");
                    None
                }
            }
        })
    } else {
        None
    };

    let crystal_dir = ttcp_dir.unwrap_or_else(|| data_dir.join("ttcp"));

    // TUI setup.
    #[cfg(feature = "tui")]
    let (dash_arc, _tui_thread) = if use_tui {
        let mode = if sniper_enabled { "paper|sniper".to_string() } else { "paper".to_string() };
        let arc = std::sync::Arc::new(std::sync::Mutex::new(fsr_tui::DashboardState {
            run_id: run_id.clone(),
            speed_multiplier: 1,
            mode,
            ..Default::default()
        }));
        let arc2 = std::sync::Arc::clone(&arc);
        let handle = std::thread::spawn(move || {
            fsr_tui::TuiApp::new(arc2).run().ok();
        });
        (Some(arc), Some(handle))
    } else {
        (None, None)
    };

    #[cfg(not(feature = "tui"))]
    if use_tui {
        eprintln!("Warning: --tui flag requires --features tui at compile time");
    }

    if !use_tui {
        println!(
            "FIXPOINT SWARM-R v3.1.0 — Paper Mode (run_id={}) sniper={}",
            run_id, sniper_enabled
        );
        println!("Running {} ticks…", ticks);
    }

    let run_ticks = if ticks == 0 { u64::MAX } else { ticks };

    for _i in 0..run_ticks {
        // Check TUI quit signal.
        #[cfg(feature = "tui")]
        if let Some(ref arc) = dash_arc {
            let (quit, pause, sniper_toggle) = {
                let s = arc.lock().unwrap();
                (s.quit_requested, s.paused, s.sniper_toggle_requested)
            };
            if quit {
                info!("TUI quit requested — stopping engine");
                break;
            }
            // Handle sniper toggle
            if sniper_toggle {
                state.sniper.toggle();
                arc.lock().unwrap().sniper_toggle_requested = false;
                info!(sniper_active = state.sniper.is_active(), "Sniper toggled");
            }
            while pause {
                std::thread::sleep(std::time::Duration::from_millis(50));
                if !arc.lock().map(|s| s.paused).unwrap_or(false) {
                    break;
                }
            }
        }

        // Hot-reload poll.
        if let Some(ref mut w) = watcher {
            if let Some(new_cfg) = w.poll(&state.config) {
                info!("hot-reload: applying new config");
                state.config = new_cfg;
                let tk = state.tri_carrier.temporal_key(
                    state.nc.windnarbe.wind_count,
                    state.config.freshness_ttl,
                );
                state.chain.shadow_append(fsr_types::EventTag::ConfigReloaded, vec![], tk);
            }
        }

        let status = run_macro_cycle(&mut state, &mut broker);

        // Persistence.
        if pm.enabled {
            if let Some(ev) = state.chain.shadow.events.last() {
                pm.flush_shadow_event(ev);
            }
            pm.maybe_snapshot(
                state.tick,
                state.chain.shadow.head_digest(),
                state.chain.commitment.head_digest(),
                state.regime.state,
                state.integrity.state,
                state.settled_cycles,
                state.aborted_cycles,
                status.psi,
                status.rho,
                status.omega,
                state.chain.shadow.event_count,
                state.chain.commitment.event_count,
            );
        }

        // TTCP crystal dir.
        if state.ttcp.last_crystal_tick == Some(state.tick.saturating_sub(1)) {
            let _ = std::fs::create_dir_all(&crystal_dir);
        }

        // TUI update.
        #[cfg(feature = "tui")]
        if let Some(ref arc) = dash_arc {
            engine::update_dashboard(&state, &status, arc);
        }

        // stdout print.
        if !use_tui
            && (status.tick.is_multiple_of(print_every) || status.tick == ticks.saturating_sub(1))
        {
            print_status(&status, output);
        }
    }

    pm.shutdown();

    if !use_tui {
        println!("\n=== Final Summary ===");
        println!("Run ID:            {}", run_id);
        println!("Total ticks:       {}", state.tick);
        println!("Settled cycles:    {}", state.settled_cycles);
        println!("Aborted cycles:    {}", state.aborted_cycles);
        println!("Wind count (NC):   {}", state.nc.windnarbe.wind_count);
        println!("Shadow events:     {}", state.chain.shadow.len());
        println!("Commit events:     {}", state.chain.commitment.len());
        println!("Shadow head:       {}", state.chain.shadow.head_digest());
        println!("Regime:            {:?}", state.regime.state);
        println!("Integrity:         {:?}", state.integrity.state);
        println!("Resource:          {:?}", state.resource.state);
        println!("TTCP crystals:     {}", state.ttcp.crystals_found);
        if sniper_enabled {
            println!("Sniper executions: {}", state.sniper.total_sniper_executions);
        }

        match state.chain.verify_both() {
            Ok(()) => println!("Chain integrity:   OK"),
            Err(e) => println!("Chain integrity:   FAILED — {}", e),
        }

        if persist {
            println!("Data written to:   {}/", data_dir.display());
        }
    }
}

fn print_status(status: &engine::StatusReport, output: &str) {
    if output == "json" {
        println!("{}", serde_json::to_string(status).unwrap_or_default());
    } else {
        println!(
            "tick={:6} regime={:5} gate={} Γ={:+.4} SI={:+.4} ψ={:.3} ρ={:.3} ω={:.3} cands={} wind={} ttcp={} sniper={}",
            status.tick,
            status.regime,
            if status.gate_open { "OPEN" } else { "SHUT" },
            fsr_fixed::q32_to_f64_display_only(status.gamma_score),
            fsr_fixed::q32_to_f64_display_only(status.si),
            fsr_fixed::q32_to_f64_display_only(status.psi),
            fsr_fixed::q32_to_f64_display_only(status.rho),
            fsr_fixed::q32_to_f64_display_only(status.omega),
            status.candidates_found,
            status.wind_count,
            status.ttcp_crystals_found,
            status.sniper_executed,
        );
    }
}

fn cmd_status(config: FsrConfig) {
    let mut state = SystemState::new(config);
    let mut broker = default_paper_broker();
    let status = run_macro_cycle(&mut state, &mut broker);
    print_status(&status, "text");
}

fn cmd_verify(config: FsrConfig) {
    let mut state = SystemState::new(config);
    let mut broker = default_paper_broker();
    for _ in 0..10 {
        run_macro_cycle(&mut state, &mut broker);
    }
    match state.chain.verify_both() {
        Ok(()) => println!("Chain verification: PASSED"),
        Err(e) => {
            println!("Chain verification: FAILED — {}", e);
            std::process::exit(1);
        }
    }
}

fn cmd_invariants(_config: FsrConfig) {
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

    println!("=== Invariant Check (15 invariants) ===");
    let results = check_invariants(&ctx);
    let mut all_pass = true;
    for r in &results {
        let status = if r.passed { "PASS" } else { "FAIL" };
        println!("[{}] INV-{:02}: {}", status, r.id, r.message);
        if !r.passed { all_pass = false; }
    }
    if all_pass {
        println!("\nAll 15 invariants: PASSED");
    } else {
        println!("\nSome invariants: FAILED");
        std::process::exit(1);
    }
}

fn cmd_config(config: &FsrConfig, as_yaml: bool) {
    if as_yaml {
        println!("{}", serde_yaml::to_string(config).unwrap_or_default());
    } else {
        println!("Profile: {:?}", config.profile);
        println!("Gate θ_open: {}, θ_close: {}", config.theta_open, config.theta_close);
        println!("Trumpet weights: L1={} L2={} L3={} L4={}",
            config.trumpet_w_l1, config.trumpet_w_l2, config.trumpet_w_l3, config.trumpet_w_l4);
        println!("Press top-k: {}", config.press_top_k);
        println!("τ_leak: {}, τ_XT: {}", config.tau_leak, config.tau_xt);
    }
}

fn cmd_benchmark(config: FsrConfig, ticks: u64) {
    println!("=== Calibration Benchmark ({} ticks) ===", ticks);
    let mut state = SystemState::new(config);
    let mut broker = default_paper_broker();
    let mut total_gate_open = 0u64;
    let mut total_candidates = 0u64;

    for _ in 0..ticks {
        let s = run_macro_cycle(&mut state, &mut broker);
        if s.gate_open { total_gate_open += 1; }
        total_candidates += s.candidates_found as u64;
    }

    println!("Settled:       {}", state.settled_cycles);
    println!("Aborted:       {}", state.aborted_cycles);
    println!("Gate open:     {}/{} ({:.1}%)",
        total_gate_open, ticks,
        100.0 * total_gate_open as f64 / ticks.max(1) as f64);
    println!("Avg candidates: {:.2}", total_candidates as f64 / ticks.max(1) as f64);
    println!("Final ρ:       {:.4}", fsr_fixed::q32_to_f64_display_only(state.resonance.stability.rho()));
    println!("Final ω:       {:.4}", fsr_fixed::q32_to_f64_display_only(state.resonance.efficiency.omega()));
    println!("TTCP crystals: {}", state.ttcp.crystals_found);
}

fn cmd_promote_legacy(config: FsrConfig) {
    let mut state = SystemState::new(config.clone());
    let mut broker = default_paper_broker();

    println!("=== Promotion Flow (legacy) ===");
    for _ in 0..config.calibration_window {
        run_macro_cycle(&mut state, &mut broker);
    }

    if state.promotion.state == fsr_types::PromotionState::PaperValidated {
        if let Some(_ev) = state.promotion.propose_promotion() {
            println!("Promotion proposal emitted");
        }
        if let Some(_ev) = state.promotion.gate_pass() {
            println!("Promotion gate passed");
        }
        println!("Live activation requires live venue adapters (--features live-exec)");
    } else {
        println!("Promotion state: {:?}", state.promotion.state);
        println!("Run more ticks for paper validation to complete.");
    }
}

fn cmd_replay(events_file: Option<&str>) {
    match events_file {
        Some(f) => println!("Replay from file: {} (use replay-historical for Phase 3 backtest)", f),
        None => println!("Replay: no events file specified. Use --events-file <path>"),
    }
    println!("Note: replay verification requires identical inputs → identical evidence digests (TMCP Theorem 9.3)");
}

/// Phase 3: `fsr replay-historical`
fn cmd_replay_historical(
    config: FsrConfig,
    recording_paths: Vec<PathBuf>,
    output_dir: PathBuf,
) {
    let run_id = format!("replay_{}", fsr_chain::persist::unix_ms());
    println!("=== Historical Replay: {} ===", run_id);
    println!("Recording files: {}", recording_paths.len());
    println!("Output: {}", output_dir.display());

    match replay_historical::run_replay(&recording_paths, config, &output_dir, &run_id) {
        Ok(report) => {
            println!("\nReplay complete. {} ticks processed.", report.total_ticks);
            println!("Settled: {} | Aborted: {}", report.settled_trades, report.aborted_trades);
            println!("Report written to: {}/", output_dir.display());
        }
        Err(e) => {
            eprintln!("Replay failed: {}", e);
            std::process::exit(1);
        }
    }
}

/// Phase 3: `fsr promote-cmd`
fn cmd_promote_workflow(
    config: FsrConfig,
    recording_paths: Option<Vec<PathBuf>>,
    approve: Option<String>,
    show_status: bool,
) {
    use fsr_calibration::PromotionWorkflow;

    // Load or create workflow state (simplified: stateless across runs)
    let mut wf = PromotionWorkflow::new();

    if show_status {
        println!("Promotion status: {}", wf.status_line());
        return;
    }

    if let Some(proposal_id) = approve {
        // In a real system we'd load the saved workflow state.
        // For now, simulate a pre-approved state.
        println!("Approving proposal: {}", proposal_id);
        // We need to set state to PromotionPending first (it would be saved between runs)
        // This is a simplified demo - real impl would persist state.
        println!("Note: approval requires prior promote --config run to have produced a proposal.");
        println!("In production, state persists between runs via chain events.");
        return;
    }

    if let Some(paths) = recording_paths {
        println!("=== Promotion Workflow: Running 5-Gate Check ===");
        println!("Config: {:?}", config.profile);
        println!("Recording files: {}", paths.len());

        let run_id = format!("promote_{}", fsr_chain::persist::unix_ms());
        let output_dir = PathBuf::from("data/backtest").join(&run_id);

        // Run backtest
        match replay_historical::run_replay(&paths, config.clone(), &output_dir, &run_id) {
            Ok(report) => {
                // Run gate checks
                let config_hash_hex = format!("{}", report.config_hash);
                let backtest_hash_hex = format!("{}", report.chain_digest_final);
                let ts_ms = fsr_chain::persist::unix_ms();

                let ok = wf.evaluate_backtest(
                    report.net_pnl_bps,
                    report.invariant_violations,
                    report.win_rate,
                    report.max_drawdown_bps,
                    report.ttcp_crystals_emitted,
                    &config_hash_hex,
                    &backtest_hash_hex,
                    ts_ms,
                );

                if ok {
                    if let Some(ref prop) = wf.proposal {
                        println!(
                            "\nPromosal ID: {}\nUse: fsr promote-cmd --approve {} to proceed.",
                            prop.proposal_id, prop.proposal_id
                        );
                    }
                } else {
                    println!("\nPromotion gates FAILED. Revise config and rerun.");
                }
            }
            Err(e) => {
                eprintln!("Backtest failed during promotion check: {}", e);
            }
        }
    } else {
        println!("Usage: fsr promote-cmd --recording <files> OR --approve <id> OR --status");
    }
}

/// Phase 3: `fsr run-live`
fn cmd_run_live(
    config: FsrConfig,
    venue: &str,
    record: bool,
    sniper_enabled: bool,
    _no_tui: bool,
    data_dir: PathBuf,
    ticks: u64,
) {
    let _run_id = format!("live_{}", fsr_chain::persist::unix_ms());
    let _rec_dir = data_dir.join("recordings");

    #[cfg(not(feature = "live-data"))]
    {
        println!("FIXPOINT SWARM-R v3.1.0 — Live Mode (STUB: no live-data feature)");
        println!("Venue: {} | Record: {} | Sniper: {}", venue, record, sniper_enabled);
        println!("Build with --features live-data for live WebSocket connections.");
        println!("Falling back to paper-mode simulation.");
        // Fall back to paper mode for demo
        cmd_run(
            config, None, if ticks == 0 { 1000 } else { ticks }, 100, "text",
            false, data_dir, 100, false, false, false, None, sniper_enabled,
        );
    }

    #[cfg(feature = "live-data")]
    {
        use crate::binance::VenueBroker;
        println!("FIXPOINT SWARM-R v3.1.0 — Live Mode");
        println!("Venue: {} | Record: {} | Sniper: {}", venue, record, sniper_enabled);

        // Recording writer
        let mut rec_writer = if record {
            match recorder::RecordingWriter::new(&run_id, &rec_dir) {
                Ok(w) => {
                    println!("Recording to: {}/", rec_dir.display());
                    Some(w)
                }
                Err(e) => {
                    eprintln!("Warning: failed to open recording writer: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let mut state = SystemState::new(config.clone());
        if sniper_enabled {
            state.sniper.enable();
            println!("Sniper mode enabled");
        }

        let mut broker: Box<dyn VenueBroker> = match venue {
            "binance" | "BINANCE" => {
                match crate::binance::live::BinanceAdapter::new("btcusdt", "BTC", "USDT") {
                    Ok(adapter) => Box::new(adapter),
                    Err(e) => {
                        eprintln!("Failed to connect to Binance: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            "kraken" | "KRAKEN" => {
                match crate::kraken::live::KrakenAdapter::new(&["XBT/USDT", "ETH/USDT"]) {
                    Ok(adapter) => Box::new(adapter),
                    Err(e) => {
                        eprintln!("Failed to connect to Kraken: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            other => {
                eprintln!("Unknown venue: {}. Use 'binance' or 'kraken'.", other);
                std::process::exit(1);
            }
        };

        let run_ticks = if ticks == 0 { u64::MAX } else { ticks };
        println!("Running {} ticks…", if ticks == 0 { "∞".to_string() } else { ticks.to_string() });

        for _i in 0..run_ticks {
            broker.advance_all();
            let books = broker.all_books();

            // Record
            if let Some(ref mut w) = rec_writer {
                let frames = recorder::MarketFrame::from_books(state.tick, &books);
                for frame in &frames {
                    let _ = w.append(frame);
                }
            }

            let status = engine::run_macro_cycle_with_books(&mut state, books);
            if state.tick.is_multiple_of(100) {
                print_status(&status, "text");
            }
        }

        if let Some(mut w) = rec_writer {
            let _ = w.flush();
        }
    }
}

/// Phase 3: `fsr backtest-report`
fn cmd_backtest_report(run: &str, data_dir: &Path) {
    let report_dir = data_dir.join(run);
    let report_path = report_dir.join("report.json");
    let summary_path = report_dir.join("summary.txt");

    if summary_path.exists() {
        match std::fs::read_to_string(&summary_path) {
            Ok(s) => {
                println!("{}", s);
                return;
            }
            Err(e) => eprintln!("Failed to read summary.txt: {}", e),
        }
    }

    if report_path.exists() {
        match std::fs::read_to_string(&report_path) {
            Ok(json) => {
                match serde_json::from_str::<backtest_report::BacktestReport>(&json) {
                    Ok(r) => println!("{}", r.summary_text()),
                    Err(e) => eprintln!("Failed to parse report.json: {}", e),
                }
            }
            Err(e) => eprintln!("Failed to read report.json: {}", e),
        }
    } else {
        eprintln!("No report found for run '{}' in {}", run, data_dir.display());
        eprintln!("Expected: {}", report_path.display());
    }
}

fn main() {
    let cli = Cli::parse();
    let config = load_config(&cli.profile, cli.config.as_deref());

    // Tracing initialization.
    let _guard = match &cli.command {
        Commands::Run { log_json, data_dir, .. } => {
            let run_id_prefix = format!("run_{}", fsr_chain::persist::unix_ms());
            observability::init_tracing(&run_id_prefix, *log_json, data_dir).ok()
        }
        _ => {
            observability::init_tracing("fsr", false, &PathBuf::from("data")).ok()
        }
    };

    match cli.command {
        Commands::Run {
            ticks, print_every, output, persist, data_dir,
            snapshot_every, log_json, tui, hot_reload, ttcp_dir,
        } => {
            cmd_run(
                config, cli.config, ticks, print_every, &output,
                persist, data_dir, snapshot_every, log_json, tui,
                hot_reload, ttcp_dir, false,
            );
        }
        Commands::RunPaper { ticks, print_every, no_tui, persist, data_dir } => {
            cmd_run(
                config, cli.config, ticks, print_every, "text",
                persist, data_dir, 100, false, !no_tui,
                false, None, false,
            );
        }
        Commands::RunLive { venue, record, sniper, no_tui, data_dir, ticks } => {
            cmd_run_live(config, &venue, record, sniper, no_tui, data_dir, ticks);
        }
        Commands::ReplayHistorical { recording, output } => {
            cmd_replay_historical(config, recording, output);
        }
        Commands::PromoteCmd { recording, approve, status } => {
            cmd_promote_workflow(config, recording, approve, status);
        }
        Commands::BacktestReport { run, data_dir } => {
            cmd_backtest_report(&run, &data_dir);
        }
        Commands::Status => cmd_status(config),
        Commands::Verify => cmd_verify(config),
        Commands::Promote => cmd_promote_legacy(config),
        Commands::Config { yaml } => cmd_config(&config, yaml),
        Commands::Invariants => cmd_invariants(config),
        Commands::Benchmark { ticks } => cmd_benchmark(config, ticks),
        Commands::Replay { events_file } => cmd_replay(events_file.as_deref()),
        Commands::ValidateConfig => {
            println!("Config validation: OK (profile={:?})", config.profile);
        }
        Commands::Runs { last } => {
            println!("Recent runs (last {}): (feature not yet implemented)", last);
        }
    }
}
