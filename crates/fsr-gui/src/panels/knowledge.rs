//! Knowledge panel: ISLS substrate overview.
//!
//! Shows:
//!   - Storage tier usage (hot/warm/cold)
//!   - Total observations stored
//!   - Total semantic crystals
//!   - Graph growth rate chart over time
//!   - Replay verification status

use crate::state::GuiState;
use crate::theme;
use egui::{RichText, Ui};
use egui_plot::{Line, Plot, PlotPoints};

/// Render the Knowledge tab.
pub fn render(ui: &mut Ui, state: &GuiState) {
    ui.heading(RichText::new("Knowledge — ISLS Semantic Ledger").color(theme::ACCENT_BLUE));
    ui.separator();

    // Storage tier stats.
    ui.label(RichText::new("Storage Tiers").color(theme::TEXT_SECONDARY));
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("Hot: {} obs", state.isls.hot_count))
            .color(theme::ACCENT_GREEN));
        ui.separator();
        ui.label(RichText::new(format!("Warm: {} obs", state.isls.warm_count))
            .color(theme::ACCENT_ORANGE));
        ui.separator();
        ui.label(RichText::new(format!("Cold: {} obs", state.isls.cold_count))
            .color(theme::TEXT_DIM));
    });

    ui.separator();

    // Summary stats.
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("Total Observations: {}", state.isls.total_observations))
            .color(theme::ACCENT_GREEN));
        ui.separator();
        ui.label(RichText::new(format!("Semantic Crystals: {}", state.isls.crystal_count))
            .color(theme::CRYSTAL_COLOR));
        ui.separator();
        let verify_color = if state.isls.replay_verified {
            theme::ACCENT_GREEN
        } else {
            theme::ACCENT_RED
        };
        ui.label(RichText::new(if state.isls.replay_verified {
            "Replay: verified"
        } else {
            "Replay: pending"
        }).color(verify_color));
    });

    ui.separator();

    // Graph growth rate chart.
    ui.label(RichText::new("Knowledge Graph Growth").color(theme::TEXT_SECONDARY));

    let vertex_history: Vec<[f64; 2]> = state.isls.vertex_history.iter()
        .enumerate()
        .map(|(i, &v)| [i as f64, v as f64])
        .collect();

    if !vertex_history.is_empty() {
        let points = PlotPoints::new(vertex_history);
        let line = Line::new(points)
            .color(theme::ACCENT_GREEN)
            .name("Vertex Count");

        Plot::new("knowledge_growth_plot")
            .height(150.0)
            .show(ui, |plot_ui| {
                plot_ui.line(line);
            });
    } else {
        ui.label(RichText::new("No data yet. Run engine to accumulate knowledge.")
            .color(theme::TEXT_DIM));
    }

    ui.separator();

    // Evidence chain status.
    ui.label(RichText::new("Evidence Chain Status").color(theme::TEXT_SECONDARY));
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("Shadow events: {}", state.isls.shadow_event_count))
            .color(theme::TEXT_SECONDARY));
        ui.separator();
        ui.label(RichText::new(format!("Commit events: {}", state.isls.commit_event_count))
            .color(theme::TEXT_SECONDARY));
        ui.separator();
        ui.label(RichText::new(format!("Shadow head: {}", &state.isls.shadow_head[..8.min(state.isls.shadow_head.len())]))
            .color(theme::TEXT_DIM)
            .monospace());
    });
}
