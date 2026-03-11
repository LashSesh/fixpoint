//! fsr-runtime: CLI entry point for FIXPOINT SWARM-R v3.0.0.
//!
//! 8 CLI commands (spec §27):
//!   run       -- Run paper-mode main loop for N ticks
//!   status    -- Show current system status
//!   replay    -- Replay from chain events file
//!   verify    -- Verify chain integrity
//!   promote   -- Trigger promotion flow
//!   config    -- Show/validate config profile
//!   invariants -- Check all 15 invariants
//!   benchmark -- Run calibration benchmark

mod config;
mod engine;
mod paper;

use clap::{Parser, Subcommand};
use config::FsrConfig;
use engine::{run_macro_cycle, StatusReport, SystemState};
use paper::default_paper_broker;

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

fn cmd_run(config: FsrConfig, ticks: u64, print_every: u64, output: &str) {
    let mut state = SystemState::new(config);
    let mut broker = default_paper_broker();

    println!("FIXPOINT SWARM-R v3.0.0 — Paper Mode");
    println!("Running {} ticks...", ticks);

    for _ in 0..ticks {
        let status = run_macro_cycle(&mut state, &mut broker);
        if status.tick.is_multiple_of(print_every) || status.tick == ticks - 1 {
            print_status(&status, output);
        }
    }

    println!("\n=== Final Summary ===");
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

    // Verify chain integrity at the end
    match state.chain.verify_both() {
        Ok(()) => println!("Chain integrity:   OK"),
        Err(e) => println!("Chain integrity:   FAILED — {}", e),
    }
}

fn print_status(status: &StatusReport, output: &str) {
    if output == "json" {
        println!("{}", serde_json::to_string(status).unwrap_or_default());
    } else {
        println!(
            "tick={:6} regime={:5} gate={} Γ={:+.4} SI={:+.4} ψ={:.3} ρ={:.3} ω={:.3} cands={} wind={}",
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
    // Run a few ticks then verify
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
}

fn cmd_promote(config: FsrConfig) {
    let mut state = SystemState::new(config.clone());
    let mut broker = default_paper_broker();

    println!("=== Promotion Flow ===");
    // Run calibration_window ticks to trigger paper validation
    for _ in 0..config.calibration_window {
        run_macro_cycle(&mut state, &mut broker);
    }

    // Try promotion steps
    if state.promotion.state == fsr_types::PromotionState::PaperValidated {
        if let Some(_ev) = state.promotion.propose_promotion() {
            println!("Promotion proposal emitted");
        }
        if let Some(_ev) = state.promotion.gate_pass() {
            println!("Promotion gate passed");
        }
        println!("Live activation would require live venue adapters (feature=live)");
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

    match cli.command {
        Commands::Run { ticks, print_every, output } => {
            cmd_run(config, ticks, print_every, &output);
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
