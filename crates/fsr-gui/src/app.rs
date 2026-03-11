//! Main eframe App struct: tab-based GUI with engine thread.

use crate::panels::sandbox::SandboxPanel;
use crate::state::GuiState;
use crate::theme;
use egui::{CentralPanel, Context, RichText, TopBottomPanel};
use std::sync::{Arc, Mutex};

/// Active tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Dashboard,
    PnlChart,
    Crystals,
    Ttcp,
    Risk,
    Config,
    Sandbox,
}

impl Tab {
    fn label(&self) -> &'static str {
        match self {
            Tab::Dashboard => "Dashboard",
            Tab::PnlChart => "P&L",
            Tab::Crystals => "Crystals",
            Tab::Ttcp => "TTCP",
            Tab::Risk => "Risk",
            Tab::Config => "Config",
            Tab::Sandbox => "Sandbox",
        }
    }
}

/// The main App struct passed to eframe::run_native.
pub struct FsrGuiApp {
    /// Shared GUI state from engine thread.
    pub state: Arc<Mutex<GuiState>>,
    /// Current tab.
    active_tab: Tab,
    /// Sandbox panel (has its own state).
    sandbox: SandboxPanel,
    /// Loaded config YAML for display.
    config_yaml: String,
    /// Whether to start on the sandbox tab.
    start_on_sandbox: bool,
}

impl FsrGuiApp {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        state: Arc<Mutex<GuiState>>,
        config_yaml: String,
        start_on_sandbox: bool,
    ) -> Self {
        FsrGuiApp {
            state,
            active_tab: if start_on_sandbox { Tab::Sandbox } else { Tab::Dashboard },
            sandbox: SandboxPanel::default(),
            config_yaml,
            start_on_sandbox,
        }
    }
}

impl eframe::App for FsrGuiApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Apply dark theme on every frame.
        ctx.set_visuals(theme::dark_theme());

        // Request repaint continuously (for live updates).
        ctx.request_repaint();

        // Snapshot current state.
        let state = {
            let lock = self.state.lock().unwrap_or_else(|p| p.into_inner());
            lock.clone()
        };

        if state.quit_requested {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // ── Top bar: tabs + status line ───────────────────────────────────────
        TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("FSR v4").color(theme::ACCENT_BLUE).size(13.0));
                ui.separator();

                for tab in [
                    Tab::Dashboard, Tab::PnlChart, Tab::Crystals, Tab::Ttcp,
                    Tab::Risk, Tab::Config, Tab::Sandbox,
                ] {
                    let selected = self.active_tab == tab;
                    let color = if selected { theme::ACCENT_BLUE } else { theme::TEXT_SECONDARY };
                    if ui.selectable_label(selected, RichText::new(tab.label()).color(color)).clicked() {
                        self.active_tab = tab;
                    }
                }
            });
        });

        // ── Bottom status bar ─────────────────────────────────────────────────
        TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let pnl_color = if state.pnl.net_pnl_bps >= 0.0 {
                    theme::ACCENT_GREEN
                } else {
                    theme::ACCENT_RED
                };
                ui.label(
                    RichText::new(format!("P&L {:+.1}bp", state.pnl.net_pnl_bps))
                        .color(pnl_color),
                );
                ui.separator();
                ui.label(
                    RichText::new(format!("DD {:.1}bp", state.pnl.drawdown_bps))
                        .color(theme::ACCENT_ORANGE),
                );
                ui.separator();
                ui.label(
                    RichText::new(format!("Crystals: {}", state.dshae.crystals_found))
                        .color(theme::CRYSTAL_COLOR),
                );
                ui.separator();
                let (gate_label, gate_color) = if state.gate_open {
                    ("gate:open", theme::ACCENT_GREEN)
                } else {
                    ("gate:shut", theme::TEXT_DIM)
                };
                ui.label(RichText::new(gate_label).color(gate_color));
                ui.separator();
                ui.label(
                    RichText::new(format!("chain:ok  tick:{}", state.tick))
                        .color(theme::TEXT_DIM),
                );
            });
        });

        // ── Main content area ─────────────────────────────────────────────────
        CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                match self.active_tab {
                    Tab::Dashboard => crate::panels::overview::render(ui, &state),
                    Tab::PnlChart => crate::panels::pnl_chart::render(ui, &state),
                    Tab::Crystals => crate::panels::crystals::render(ui, &state),
                    Tab::Ttcp => crate::panels::ttcp::render(ui, &state),
                    Tab::Risk => crate::panels::risk::render(ui, &state),
                    Tab::Config => crate::panels::config::render(ui, &state, &self.config_yaml),
                    Tab::Sandbox => self.sandbox.render(ui),
                }
            });
        });
    }
}
