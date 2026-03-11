//! Sniper status panel (stub for Phase 4 — sniper is Phase 3 feature).

use crate::state::GuiState;
use crate::theme::*;
use egui::{RichText, Ui};

pub fn render(ui: &mut Ui, _state: &GuiState) {
    ui.heading(RichText::new("Sniper Mode").color(TEXT_PRIMARY));
    ui.separator();
    ui.label(
        RichText::new("Sniper mode is a Phase 3 feature accessible via the `fsr` CLI.")
            .color(TEXT_DIM),
    );
    ui.label(RichText::new("Use: fsr run --sniper --tui").color(TEXT_SECONDARY));
}
