//! fsr-gui: Desktop GUI entry point for FIXPOINT SWARM-R Phase 4.
//!
//! Usage:
//!   fsr-gui --sandbox                       Opens window in sandbox validation mode
//!   fsr-gui --config <path.yaml>            Opens window in paper-mode with live dashboard
//!   fsr-gui --profile balanced              Opens window using built-in profile

mod app;
mod charts;
mod panels;
mod state;
mod theme;

use app::FsrGuiApp;
use clap::Parser;
use state::{GuiState, RunMode};
use std::sync::{Arc, Mutex};

#[derive(Parser, Debug)]
#[command(
    name = "fsr-gui",
    version = "4.0.0",
    about = "FIXPOINT SWARM-R: Desktop GUI (Phase 4)"
)]
struct Cli {
    /// Open in sandbox validation mode (5 pre-bundled scenarios)
    #[arg(long)]
    sandbox: bool,

    /// Path to YAML config file for paper-mode dashboard
    #[arg(long)]
    config: Option<String>,

    /// Config profile: conservative, balanced, aggressive, minimal
    #[arg(long, default_value = "conservative")]
    profile: String,

    /// Initial window width
    #[arg(long, default_value = "1280")]
    width: u32,

    /// Initial window height
    #[arg(long, default_value = "800")]
    height: u32,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Load config YAML for display in the Config tab.
    let config_yaml = if let Some(ref path) = cli.config {
        std::fs::read_to_string(path).unwrap_or_else(|e| {
            format!("# Error loading {}: {}\n", path, e)
        })
    } else {
        format!("# Profile: {}\n# Use --config <path.yaml> to load a custom config.\n", cli.profile)
    };

    // Shared GUI state: engine thread writes, render thread reads.
    let shared_state: Arc<Mutex<GuiState>> = Arc::new(Mutex::new({
        let mut s = GuiState::default();
        s.mode = if cli.sandbox { RunMode::Sandbox } else { RunMode::Paper };
        s
    }));

    // If not in sandbox mode, start a paper-mode engine thread.
    if !cli.sandbox {
        let state_clone = Arc::clone(&shared_state);
        let profile = cli.profile.clone();
        let config_path = cli.config.clone();

        std::thread::spawn(move || {
            run_paper_engine(state_clone, &profile, config_path.as_deref());
        });
    }

    // Launch eframe window.
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("FIXPOINT SWARM-R v4.0 — GUI")
            .with_inner_size([cli.width as f32, cli.height as f32])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    let start_on_sandbox = cli.sandbox;
    let state_for_app = Arc::clone(&shared_state);
    let config_yaml_for_app = config_yaml;

    eframe::run_native(
        "FIXPOINT SWARM-R v4",
        native_options,
        Box::new(move |cc| {
            Ok(Box::new(FsrGuiApp::new(
                cc,
                state_for_app,
                config_yaml_for_app,
                start_on_sandbox,
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))
}

/// Paper-mode engine loop: runs DSHAE + resonance engine and pushes state updates.
///
/// This runs in a background thread; the GUI reads from `Arc<Mutex<GuiState>>`.
fn run_paper_engine(
    shared: Arc<Mutex<GuiState>>,
    profile: &str,
    config_path: Option<&str>,
) {
    use fsr_dshae::{DshaeConfig, DshaeEngine, DshaeSummary};

    // Build DSHAE config.
    let mut dshae_cfg = DshaeConfig::default();
    dshae_cfg.enabled = true;
    dshae_cfg.holographic.min_points = 1;
    dshae_cfg.dual_simplex.anti_phase_tolerance = fsr_fixed::ONE / 10;

    let mut dshae = DshaeEngine::new(dshae_cfg);

    // Baseline mid prices (no-arb 4-currency basket).
    let base_mids: [(usize, usize, i64); 6] = [
        (0, 1, 9200),
        (0, 2, 7912),
        (0, 3, 7516),
        (1, 2, 8600),
        (1, 3, 8170),
        (2, 3, 9500),
    ];

    let mut cumulative_pnl_bps: f32 = 0.0;
    let mut settled: u64 = 0;
    let mut aborted: u64 = 0;

    for tick in 0u64.. {
        // Check if quit has been requested.
        {
            let s = shared.lock().unwrap_or_else(|p| p.into_inner());
            if s.quit_requested {
                break;
            }
        }

        // Inject a small synthetic arb every 500 ticks for demo purposes.
        let mids: Vec<(usize, usize, i64)> = if tick % 500 == 250 {
            // 8bp arb on triangle 0-1-2.
            let mut m = base_mids.to_vec();
            let base_02 = m[1].2;
            m[1].2 = base_02 + base_02 * 8 / 10000;
            m
        } else {
            base_mids.to_vec()
        };

        let crystals = dshae.push_mids(&mids, tick);

        // Update shared state.
        {
            let mut s = shared.lock().unwrap_or_else(|p| p.into_inner());
            s.tick = tick;

            // Simulated gate: open when crystals seen recently.
            let gate_open = dshae.last_crystal_tick
                .map(|lt| tick.saturating_sub(lt) < 50)
                .unwrap_or(false);
            s.gate_open = gate_open;

            // P&L: each crystal adds ~1bp simulated.
            for c in &crystals {
                settled += 1;
                let edge_bps = fsr_fixed::q32_to_f64_display_only(c.net_edge) as f32 * 10000.0;
                cumulative_pnl_bps += edge_bps.max(0.0);
                s.push_pnl(tick, cumulative_pnl_bps);
                s.push_crystal(state::CrystalDisplay::from(c));
                s.push_event(format!(
                    "tick={} crystal triangle=({},{},{}) edge={:.1}bp",
                    tick, c.triangle.0, c.triangle.1, c.triangle.2,
                    edge_bps,
                ));
            }
            if crystals.is_empty() && tick % 100 == 0 {
                // Occasional null tick events.
                aborted += 1;
            }

            // Update DSHAE summary.
            s.dshae = DshaeSummary {
                enabled: true,
                mode: "Shadow".to_string(),
                crystals_found: dshae.crystals_found,
                last_crystal_tick: dshae.last_crystal_tick,
                active_crystals: crystals.len(),
                basket_size: dshae.basket.n,
            };

            // Simulated resonance metrics (sinusoidal for demo).
            let t = tick as f32 * 0.01;
            s.resonance.si = (t.sin() * 0.3 + 0.5).clamp(0.0, 1.0);
            s.resonance.psi = (t.cos() * 0.2 + 0.6).clamp(0.0, 1.0);
            s.resonance.rho = 0.7 + 0.1 * (t * 0.5).sin();
            s.resonance.omega = (t * 0.3).sin().abs();
            s.resonance.kappa = 0.5;
            s.resonance.entropy = 0.3 + 0.1 * (t * 2.0).sin().abs();

            s.regime = "Alpha".into();
            s.integrity = "Healthy".into();
            s.resource = "Nominal".into();

            s.pnl.net_pnl_bps = cumulative_pnl_bps;
            s.pnl.drawdown_bps = (cumulative_pnl_bps * 0.05).max(0.0);
            s.pnl.settled = settled;
            s.pnl.aborted = aborted;
        }

        // ~100 ticks/second target (10ms per tick).
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}
