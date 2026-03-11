//! Risk panel.

use crate::state::GuiState;
use crate::theme::*;
use egui::{Grid, RichText, Ui};

pub fn render(ui: &mut Ui, state: &GuiState) {
    ui.heading(RichText::new("Risk").color(TEXT_PRIMARY));
    ui.separator();

    let pnl = &state.pnl;

    Grid::new("risk_grid").num_columns(2).spacing([16.0, 6.0]).show(ui, |ui| {
        ui.label(RichText::new("Net P&L:").color(TEXT_SECONDARY));
        let pnl_color = if pnl.net_pnl_bps >= 0.0 { ACCENT_GREEN } else { ACCENT_RED };
        ui.label(RichText::new(format!("{:+.2} bp", pnl.net_pnl_bps)).color(pnl_color));
        ui.end_row();

        ui.label(RichText::new("Drawdown:").color(TEXT_SECONDARY));
        let dd_color = if pnl.drawdown_bps > 10.0 { ACCENT_RED } else { ACCENT_ORANGE };
        ui.label(RichText::new(format!("{:.2} bp", pnl.drawdown_bps)).color(dd_color));
        ui.end_row();

        ui.label(RichText::new("Settled:").color(TEXT_SECONDARY));
        ui.label(RichText::new(format!("{}", pnl.settled)).color(ACCENT_GREEN));
        ui.end_row();

        ui.label(RichText::new("Aborted:").color(TEXT_SECONDARY));
        ui.label(RichText::new(format!("{}", pnl.aborted)).color(ACCENT_RED));
        ui.end_row();

        let win_rate = if pnl.settled + pnl.aborted > 0 {
            100.0 * pnl.settled as f32 / (pnl.settled + pnl.aborted) as f32
        } else {
            0.0
        };
        ui.label(RichText::new("Win Rate:").color(TEXT_SECONDARY));
        ui.label(RichText::new(format!("{:.1}%", win_rate)).color(TEXT_PRIMARY));
        ui.end_row();
    });

    ui.separator();
    ui.label(RichText::new("Chain Integrity").color(TEXT_SECONDARY));
    ui.horizontal(|ui| {
        let color = match state.integrity.as_str() {
            "Healthy" => ACCENT_GREEN,
            _ => ACCENT_RED,
        };
        ui.label(RichText::new(&state.integrity).color(color));
    });
}
