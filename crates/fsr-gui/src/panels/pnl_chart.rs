//! P&L time series chart panel.

use crate::state::GuiState;
use crate::theme::*;
use crate::charts::line_chart;
use egui::{RichText, Ui};

pub fn render(ui: &mut Ui, state: &GuiState) {
    ui.heading(RichText::new("P&L Chart").color(TEXT_PRIMARY));
    ui.separator();

    ui.horizontal(|ui| {
        ui.label(RichText::new("Net P&L:").color(TEXT_SECONDARY));
        let pnl = state.pnl.net_pnl_bps;
        let color = if pnl >= 0.0 { ACCENT_GREEN } else { ACCENT_RED };
        ui.label(RichText::new(format!("{:+.2} bp", pnl)).color(color));
        ui.separator();
        ui.label(RichText::new("Settled:").color(TEXT_SECONDARY));
        ui.label(RichText::new(format!("{}", state.pnl.settled)).color(TEXT_PRIMARY));
        ui.separator();
        ui.label(RichText::new("Aborted:").color(TEXT_SECONDARY));
        ui.label(RichText::new(format!("{}", state.pnl.aborted)).color(TEXT_PRIMARY));
        ui.separator();
        ui.label(RichText::new("Drawdown:").color(TEXT_SECONDARY));
        ui.label(
            RichText::new(format!("{:.2} bp", state.pnl.drawdown_bps)).color(ACCENT_ORANGE),
        );
    });

    ui.add_space(8.0);

    if state.pnl_history.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(RichText::new("No P&L data yet — run paper mode or sandbox.").color(TEXT_DIM));
        });
        return;
    }

    line_chart(ui, "pnl_chart", "Cumulative P&L (bp)", &state.pnl_history, ACCENT_GREEN, 280.0);

    ui.add_space(8.0);
    ui.label(RichText::new("TTCP Delta History").color(TEXT_SECONDARY));

    if !state.delta_history.is_empty() {
        let d1: std::collections::VecDeque<(u64, f32)> =
            state.delta_history.iter().map(|&(t, d, _, _)| (t, d)).collect();
        let d2: std::collections::VecDeque<(u64, f32)> =
            state.delta_history.iter().map(|&(t, _, d, _)| (t, d)).collect();
        let d3: std::collections::VecDeque<(u64, f32)> =
            state.delta_history.iter().map(|&(t, _, _, d)| (t, d)).collect();

        let s1: Vec<[f64; 2]> = d1.iter().map(|&(t, v)| [t as f64, v as f64]).collect();
        let s2: Vec<[f64; 2]> = d2.iter().map(|&(t, v)| [t as f64, v as f64]).collect();
        let s3: Vec<[f64; 2]> = d3.iter().map(|&(t, v)| [t as f64, v as f64]).collect();

        crate::charts::multi_line_chart(
            ui,
            "delta_chart",
            &[
                ("δ1", ACCENT_BLUE, s1),
                ("δ2", ACCENT_GREEN, s2),
                ("δ3", ACCENT_ORANGE, s3),
            ],
            160.0,
        );
    }
}
