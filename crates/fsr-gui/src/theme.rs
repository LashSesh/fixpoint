//! Color scheme and fonts for the FSR GUI.

use egui::{Color32, Visuals};

pub const BG_DARK: Color32 = Color32::from_rgb(18, 20, 28);
pub const BG_PANEL: Color32 = Color32::from_rgb(26, 30, 40);
pub const BG_CARD: Color32 = Color32::from_rgb(36, 40, 55);
pub const ACCENT_BLUE: Color32 = Color32::from_rgb(64, 128, 255);
pub const ACCENT_GREEN: Color32 = Color32::from_rgb(64, 220, 140);
pub const ACCENT_RED: Color32 = Color32::from_rgb(255, 80, 80);
pub const ACCENT_ORANGE: Color32 = Color32::from_rgb(255, 160, 50);
pub const ACCENT_YELLOW: Color32 = Color32::from_rgb(255, 220, 50);
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(230, 235, 245);
pub const TEXT_SECONDARY: Color32 = Color32::from_rgb(150, 160, 185);
pub const TEXT_DIM: Color32 = Color32::from_rgb(90, 100, 120);

pub const CRYSTAL_COLOR: Color32 = Color32::from_rgb(180, 100, 255);
pub const GATE_OPEN_COLOR: Color32 = ACCENT_GREEN;
pub const GATE_SHUT_COLOR: Color32 = ACCENT_RED;

/// Configure the dark visual theme for the app.
pub fn dark_theme() -> Visuals {
    let mut v = Visuals::dark();
    v.panel_fill = BG_PANEL;
    v.window_fill = BG_DARK;
    v.extreme_bg_color = BG_DARK;
    v
}
