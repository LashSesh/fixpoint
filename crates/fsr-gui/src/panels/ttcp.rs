//! TTCP convergence chart panel.

use crate::state::GuiState;
use crate::theme::*;
use egui::{RichText, Ui};

pub fn render(ui: &mut Ui, state: &GuiState) {
    ui.heading(RichText::new("TTCP Convergence").color(TEXT_PRIMARY));
    ui.separator();

    let ttcp = &state.ttcp;

    ui.horizontal(|ui| {
        ui.label(RichText::new("Level:").color(TEXT_SECONDARY));
        let level_color = if ttcp.level >= 2 { ACCENT_GREEN } else if ttcp.level == 1 { ACCENT_ORANGE } else { TEXT_DIM };
        ui.label(RichText::new(format!("{}", ttcp.level)).color(level_color));
        ui.separator();
        ui.label(RichText::new("Crystals:").color(TEXT_SECONDARY));
        ui.label(RichText::new(format!("{}", ttcp.crystals_found)).color(ACCENT_YELLOW));
        ui.separator();
        if let Some(t) = ttcp.last_crystal_tick {
            ui.label(RichText::new(format!("Last: tick {}", t)).color(TEXT_DIM));
        }
    });

    ui.add_space(8.0);

    ui.label(RichText::new("Convergence Score History").color(TEXT_SECONDARY));

    if state.delta_history.is_empty() {
        ui.label(RichText::new("No data yet.").color(TEXT_DIM));
        return;
    }

    // Show TTCP convergence from delta_history (d1 represents ψ convergence).
    let conv_pts: Vec<[f64; 2]> = state
        .delta_history
        .iter()
        .map(|&(t, d1, _, _)| [t as f64, d1 as f64])
        .collect();

    use egui_plot::{Line, Plot, PlotPoints};
    let line = Line::new(PlotPoints::new(conv_pts))
        .color(ACCENT_YELLOW)
        .name("TTCP ψ");
    Plot::new("ttcp_chart")
        .height(200.0)
        .show_axes([true, true])
        .show(ui, |plot_ui| {
            plot_ui.line(line);
        });

    ui.separator();
    ui.label(RichText::new("3-Level Cascade").color(TEXT_SECONDARY));
    ui.horizontal(|ui| {
        for (level, label) in [(0u8, "β₀ Components"), (1u8, "Fiber"), (2u8, "Meta-Conv")] {
            let active = ttcp.level >= level;
            let color = if active { ACCENT_GREEN } else { TEXT_DIM };
            ui.group(|ui| {
                ui.label(RichText::new(format!("L{}", level)).color(color));
                ui.label(RichText::new(label).color(if active { TEXT_PRIMARY } else { TEXT_DIM }));
            });
        }
    });
}
