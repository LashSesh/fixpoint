//! Constraints panel: ECLS constraint list, lattice crystal log, breaking alerts.
//!
//! Shows:
//!   - Active constraints sorted by stability
//!   - Template type, bound entities, satisfaction rate, window size
//!   - Lattice crystal log
//!   - Constraint-breaking alerts (red)

use crate::state::GuiState;
use crate::theme;
use egui::{RichText, Ui};

/// Render the Constraints tab.
pub fn render(ui: &mut Ui, state: &GuiState) {
    ui.heading(RichText::new("Constraints — ECLS Lattice Spectroscopy").color(theme::ACCENT_BLUE));
    ui.separator();

    // Stats row.
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("Active Constraints: {}", state.ecls.active_constraints))
            .color(theme::ACCENT_GREEN));
        ui.separator();
        ui.label(RichText::new(format!("Lattice Crystals: {}", state.ecls.lattice_crystals))
            .color(theme::CRYSTAL_COLOR));
        ui.separator();
        ui.label(RichText::new(format!("Breaking Events: {}", state.ecls.breaking_events))
            .color(theme::ACCENT_RED));
    });

    ui.separator();

    // Constraint-breaking alerts.
    if state.ecls.breaking_events > 0 {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!(
                "⚠ {} constraint-breaking event(s) — regime change likely",
                state.ecls.breaking_events,
            )).color(theme::ACCENT_RED));
        });
        ui.separator();
    }

    // Active constraints list.
    ui.label(RichText::new("Active Constraint Candidates").color(theme::TEXT_SECONDARY));
    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
        if state.ecls.active_constraints == 0 {
            ui.label(RichText::new("No constraints discovered yet. Run engine for 100+ ticks.")
                .color(theme::TEXT_DIM));
        } else {
            // Display recent constraint events.
            for msg in state.ecls.recent_events.iter().rev().take(20) {
                ui.label(RichText::new(msg).color(theme::TEXT_SECONDARY).size(11.0));
            }
        }
    });

    ui.separator();

    // Lattice crystal log.
    ui.label(RichText::new("Lattice Crystal Log").color(theme::TEXT_SECONDARY));
    egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
        if state.ecls.lattice_crystals == 0 {
            ui.label(RichText::new("No lattice crystals formed yet.")
                .color(theme::TEXT_DIM));
        } else {
            ui.label(RichText::new(format!(
                "{} lattice crystal(s) committed. Stable arbitrage structures identified.",
                state.ecls.lattice_crystals,
            )).color(theme::CRYSTAL_COLOR));
        }
    });
}
