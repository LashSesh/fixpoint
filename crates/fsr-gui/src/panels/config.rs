//! Config viewer/editor panel.

use crate::state::GuiState;
use crate::theme::*;
use egui::{RichText, ScrollArea, Ui};

pub fn render(ui: &mut Ui, _state: &GuiState, config_yaml: &str) {
    ui.heading(RichText::new("Configuration").color(TEXT_PRIMARY));
    ui.separator();

    ui.label(
        RichText::new("Config is loaded at startup and can be hot-reloaded via the `fsr` CLI.")
            .color(TEXT_DIM),
    );
    ui.add_space(8.0);

    if config_yaml.is_empty() {
        ui.label(RichText::new("No config file loaded. Use --config <path>.").color(TEXT_SECONDARY));
    } else {
        ui.label(RichText::new("Active config:").color(TEXT_SECONDARY));
        ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
            ui.code(config_yaml);
        });
    }
}
