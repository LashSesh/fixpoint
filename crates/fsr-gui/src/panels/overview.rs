//! Dashboard overview panel: FSM states, gate score, resonance snapshot.

use crate::state::GuiState;
use crate::theme::*;
use egui::{Color32, Grid, RichText, Ui};

pub fn render(ui: &mut Ui, state: &GuiState) {
    ui.heading(RichText::new("FIXPOINT SWARM-R  ·  Dashboard").color(TEXT_PRIMARY));
    ui.separator();

    ui.columns(2, |cols| {
        // Left column: FSM states + gate
        let ui = &mut cols[0];
        Grid::new("fsm_grid").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            ui.label(RichText::new("Tick:").color(TEXT_SECONDARY));
            ui.label(RichText::new(format!("{}", state.tick)).color(TEXT_PRIMARY));
            ui.end_row();

            ui.label(RichText::new("Mode:").color(TEXT_SECONDARY));
            ui.label(RichText::new(format!("{}", state.mode)).color(ACCENT_BLUE));
            ui.end_row();

            ui.label(RichText::new("Regime:").color(TEXT_SECONDARY));
            let regime_color = match state.regime.as_str() {
                "Alpha" => ACCENT_GREEN,
                "Beta" => ACCENT_ORANGE,
                "Gamma" => ACCENT_RED,
                _ => TEXT_PRIMARY,
            };
            ui.label(RichText::new(&state.regime).color(regime_color));
            ui.end_row();

            ui.label(RichText::new("Integrity:").color(TEXT_SECONDARY));
            let integ_color = match state.integrity.as_str() {
                "Healthy" => ACCENT_GREEN,
                "Degraded" => ACCENT_ORANGE,
                "SafeHold" | "Killed" => ACCENT_RED,
                _ => TEXT_PRIMARY,
            };
            ui.label(RichText::new(&state.integrity).color(integ_color));
            ui.end_row();

            ui.label(RichText::new("Resource:").color(TEXT_SECONDARY));
            ui.label(RichText::new(&state.resource).color(TEXT_PRIMARY));
            ui.end_row();

            ui.label(RichText::new("Gate:").color(TEXT_SECONDARY));
            let (gate_label, gate_color) = if state.gate_open {
                ("OPEN", GATE_OPEN_COLOR)
            } else {
                ("SHUT", GATE_SHUT_COLOR)
            };
            ui.label(RichText::new(format!("{} (Γ={:.3})", gate_label, state.gamma_score)).color(gate_color));
            ui.end_row();

            ui.label(RichText::new("Candidates:").color(TEXT_SECONDARY));
            ui.label(RichText::new(format!("{}", state.candidates_found)).color(TEXT_PRIMARY));
            ui.end_row();
        });

        // Right column: resonance metrics
        let ui = &mut cols[1];
        ui.label(RichText::new("Resonance").color(TEXT_SECONDARY).size(12.0));
        Grid::new("resonance_grid").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            let r = &state.resonance;
            for (label, val) in [
                ("SI:", r.si),
                ("ψ:", r.psi),
                ("ρ:", r.rho),
                ("ω:", r.omega),
                ("κ:", r.kappa),
                ("H:", r.entropy),
            ] {
                ui.label(RichText::new(label).color(TEXT_SECONDARY));
                ui.label(RichText::new(format!("{:.4}", val)).color(ACCENT_BLUE));
                ui.end_row();
            }
        });
    });

    ui.separator();

    // DSHAE summary
    ui.horizontal(|ui| {
        ui.label(RichText::new("DSHAE:").color(TEXT_SECONDARY));
        let dshae_color = if state.dshae.enabled { ACCENT_GREEN } else { TEXT_DIM };
        ui.label(
            RichText::new(if state.dshae.enabled { "ENABLED" } else { "shadow" })
                .color(dshae_color),
        );
        ui.separator();
        ui.label(RichText::new("Crystals:").color(TEXT_SECONDARY));
        ui.label(
            RichText::new(format!("{}", state.dshae.crystals_found)).color(CRYSTAL_COLOR),
        );
        ui.separator();
        ui.label(RichText::new("TTCP:").color(TEXT_SECONDARY));
        ui.label(
            RichText::new(format!("{}", state.ttcp.crystals_found)).color(ACCENT_YELLOW),
        );
    });

    ui.separator();

    // Recent events
    ui.label(RichText::new("Recent Events").color(TEXT_SECONDARY));
    egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
        let events: Vec<&String> = state.event_log.iter().rev().take(20).collect();
        for ev in events {
            ui.label(RichText::new(ev).color(TEXT_DIM).size(11.0));
        }
    });
}
