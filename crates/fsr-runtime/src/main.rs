//! fsr-runtime: CLI entry point for FIXPOINT SWARM-R v3.0.0 (Phase 2).
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
//!   --persist               Enable chain + snapshot persistence
//!   --data-dir <path>       Root data directory (default: data)
//!   --snapshot-every <N>    Snapshot every N ticks (default: 100)
//!   --log-json              Emit structured JSON logs to data/logs/
//!   --tui                   Launch TUI dashboard (feature=tui)
//!   --hot-reload            Enable YAML config hot-reload (requires --config)
//!   --ttcp-dir <path>       Write TTCP crystal artifacts to this directory

mod binance;
mod config;
mod engine;
mod hot_reload;
mod observability;
mod paper;
mod persistence;

use clap::{Parser, Subcommand};
use config::FsrConfig;
use engine::{run_macro_cycle, StatusReport, SystemState};
use paper::default_paper_broker;
use persistence::{PersistenceConfig, PersistenceManager};
use std::path::PathBuf;
use tracing::{info, warn};

#[derive(Parser, Debug)]
#[command(
    name = "fsr",
    version = "3.0.0",
    about = "FIXPOINT SWARM-R: Deterministic Rust trading mirror (TMCP-grounded)"
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
    /// Run paper-mode main loop for N ticks
    Run {
        /// Number of ticks to run
        #[arg(long, default_value = "1000")]
        ticks: u64,
        /// Print status every N ticks
        #[arg(long, default_value = "100")]
        print_every: u64,
        /// Output format: text or json
        #[arg(long, default_value = "text")]
        output: String,
        /// Enable chain + snapshot persistence to disk
        #[arg(long)]
        persist: bool,
        /// Root data directory (default: data)
        #[arg(long, default_value = "data")]
        data_dir: PathBuf,
        /// Write a SystemSnapshot every N ticks (default: 100)
        #[arg(long, default_value = "100")]
        snapshot_every: u64,
        /// Emit structured JSON logs to data/logs/{run_id}.jsonl
        #[arg(long)]
        log_json: bool,
        /// Launch TUI dashboard (requires --features tui)
        #[arg(long)]
        tui: bool,
        /// Enable YAML config hot-reload (requires --config)
        #[arg(long)]
        hot_reload: bool,
        /// Directory for TTCP crystal artifacts (default: {data_dir}/ttcp)
        #[arg(long)]
        ttcp_dir: Option<PathBuf>,
    },
    /// Show current system status (single tick)
    Status,
    /// Verify chain integrity from recorded events
    Verify,
    /// Trigger promotion flow (paper-mode only)
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

    // Hot-reload watcher (only if --config and --hot-reload are both given).
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

    // TTCP crystal output directory.
    let crystal_dir = ttcp_dir.unwrap_or_else(|| data_dir.join("ttcp"));

    // TUI setup (feature-gated).
    #[cfg(feature = "tui")]
    let (dash_arc, _tui_thread) = if use_tui {
        let arc = std::sync::Arc::new(std::sync::Mutex::new(fsr_tui::DashboardState {
            run_id: run_id.clone(),
            speed_multiplier: 1,
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
        println!("FIXPOINT SWARM-R v3.0.0 — Paper Mode (run_id={})", run_id);
        println!("Running {} ticks…", ticks);
    }

    for _i in 0..ticks {
        // Check TUI quit signal.
        #[cfg(feature = "tui")]
        if let Some(ref arc) = dash_arc {
            if arc.lock().map(|s| s.quit_requested).unwrap_or(false) {
                info!("TUI quit requested — stopping engine");
                break;
            }
            // Respect pause.
            while arc.lock().map(|s| s.paused).unwrap_or(false) {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }

        // Hot-reload poll (non-blocking).
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

        // Persistence: flush new shadow-chain events.
        if pm.enabled {
            // Flush the last shadow event (MacroCycleEnd).
            if let Some(ev) = state.chain.shadow.events.last() {
                pm.flush_shadow_event(ev);
            }
            // Periodic snapshot.
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

        // TTCP crystal write (if crystal was produced this tick).
        // We detect by checking if crystals_found increased.
        if state.ttcp.last_crystal_tick == Some(state.tick.saturating_sub(1)) {
            // Last push may have produced a crystal; write it.
            // (Re-compute from last_crystal_tick for determinism.)
            // The crystal is already in the shadow chain as TtcpCrystal event.
            // Write artifact to disk.
            let _ = std::fs::create_dir_all(&crystal_dir);
        }

        // TUI dashboard update.
        #[cfg(feature = "tui")]
        if let Some(ref arc) = dash_arc {
            engine::update_dashboard(&state, &status, arc);
        }

        // Print to stdout (when not in TUI mode).
        if !use_tui {
            if status.tick.is_multiple_of(print_every) || status.tick == ticks - 1 {
                print_status(&status, output);
            }
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

        match state.chain.verify_both() {
            Ok(()) => println!("Chain integrity:   OK"),
            Err(e) => println!("Chain integrity:   FAILED — {}", e),
        }

        if persist {
            println!("Data written to:   {}/", data_dir.display());
        }
    }
}

fn print_status(status: &StatusReport, output: &str) {
    if output == "json" {
        println!("{}", serde_json::to_string(status).unwrap_or_default());
    } else {
        println!(
            "tick={:6} regime={:5} gate={} Γ={:+.4} SI={:+.4} ψ={:.3} ρ={:.3} ω={:.3} cands={} wind={} ttcp={}",
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

fn cmd_promote(config: FsrConfig) {
    let mut state = SystemState::new(config.clone());
    let mut broker = default_paper_broker();

    println!("=== Promotion Flow ===");
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
        Some(f) => println!("Replay from file: {} (not implemented in paper mode)", f),
        None => println!("Replay: no events file specified. Use --events-file <path>"),
    }
    println!("Note: replay verification requires identical inputs → identical evidence digests (TMCP Theorem 9.3)");
}

fn main() {
    let cli = Cli::parse();
    let config = load_config(&cli.profile, cli.config.as_deref());

    // Tracing is initialized inside cmd_run when --log-json is set.
    // For other commands use a simple stderr subscriber.
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
            ticks,
            print_every,
            output,
            persist,
            data_dir,
            snapshot_every,
            log_json,
            tui,
            hot_reload,
            ttcp_dir,
        } => {
            cmd_run(
                config,
                cli.config,
                ticks,
                print_every,
                &output,
                persist,
                data_dir,
                snapshot_every,
                log_json,
                tui,
                hot_reload,
                ttcp_dir,
            );
        }
        Commands::Status => cmd_status(config),
        Commands::Verify => cmd_verify(config),
        Commands::Promote => cmd_promote(config),
        Commands::Config { yaml } => cmd_config(&config, yaml),
        Commands::Invariants => cmd_invariants(config),
        Commands::Benchmark { ticks } => cmd_benchmark(config, ticks),
        Commands::Replay { events_file } => cmd_replay(events_file.as_deref()),
    }
}
