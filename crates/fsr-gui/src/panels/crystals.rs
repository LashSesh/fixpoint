//! Crystals log + HIM scatter plot panel.

use crate::state::GuiState;
use crate::theme::*;
use egui::{Grid, RichText, Ui};
use egui_plot::{Plot, PlotPoints, Points};

pub fn render(ui: &mut Ui, state: &GuiState) {
    ui.heading(RichText::new("Crystals  +  HIM Manifold").color(TEXT_PRIMARY));
    ui.separator();

    ui.columns(2, |cols| {
        // Left: Crystal log
        let ui = &mut cols[0];
        ui.label(RichText::new("Crystal Log").color(TEXT_SECONDARY));

        if state.crystal_log.is_empty() {
            ui.label(RichText::new("No crystals yet.").color(TEXT_DIM));
        } else {
            egui::ScrollArea::vertical().max_height(350.0).show(ui, |ui| {
                Grid::new("crystal_grid")
                    .num_columns(4)
                    .spacing([8.0, 3.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Tick").color(TEXT_SECONDARY));
                        ui.label(RichText::new("Triangle").color(TEXT_SECONDARY));
                        ui.label(RichText::new("Edge (bp)").color(TEXT_SECONDARY));
                        ui.label(RichText::new("Age").color(TEXT_SECONDARY));
                        ui.end_row();

                        for c in state.crystal_log.iter().rev().take(50) {
                            ui.label(RichText::new(format!("{}", c.tick)).color(TEXT_DIM));
                            ui.label(
                                RichText::new(format!(
                                    "{},{},{}",
                                    c.triangle.0, c.triangle.1, c.triangle.2
                                ))
                                .color(CRYSTAL_COLOR),
                            );
                            ui.label(
                                RichText::new(format!("{:.2}", c.net_edge_bps))
                                    .color(ACCENT_GREEN),
                            );
                            ui.label(RichText::new(format!("{}", c.age)).color(TEXT_DIM));
                            ui.end_row();
                        }
                    });
            });
        }

        // Right: HIM scatter (x3 vs psi for all recent crystals)
        let ui = &mut cols[1];
        ui.label(RichText::new("HIM Scatter  (net-edge vs ψ)").color(TEXT_SECONDARY));

        let scatter_pts: Vec<[f64; 2]> = state
            .crystal_log
            .iter()
            .map(|c| [c.net_edge_bps as f64, c.tick as f64 % 100.0])
            .collect();

        if scatter_pts.is_empty() {
            ui.label(RichText::new("No crystal data for scatter.").color(TEXT_DIM));
        } else {
            let pts = Points::new(PlotPoints::new(scatter_pts))
                .color(CRYSTAL_COLOR)
                .radius(5.0)
                .name("crystals");
            Plot::new("him_scatter")
                .height(350.0)
                .show_axes([true, true])
                .show(ui, |plot_ui| {
                    plot_ui.points(pts);
                });
        }
    });

    ui.separator();
    ui.horizontal(|ui| {
        ui.label(RichText::new("Total DSHAE Crystals:").color(TEXT_SECONDARY));
        ui.label(
            RichText::new(format!("{}", state.dshae.crystals_found)).color(CRYSTAL_COLOR),
        );
        if let Some(t) = state.dshae.last_crystal_tick {
            ui.separator();
            ui.label(RichText::new(format!("Last: tick {}", t)).color(TEXT_DIM));
        }
    });
}
