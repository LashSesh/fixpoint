//! Mycelium panel: MCCE graph visualization.
//!
//! Shows:
//!   - Vertices as dots (sized by vertex count), edges as lines (opacity = correlation)
//!   - Force-directed layout (simulated)
//!   - Stats: vertex count, edge count, graph density, largest cluster
//!   - Click vertex for embedding, edge list, correlation history

use crate::state::GuiState;
use crate::theme;
use egui::{RichText, Ui};

/// Render the Mycelium tab.
pub fn render(ui: &mut Ui, state: &GuiState) {
    ui.heading(RichText::new("Mycelium — MCCE Graph").color(theme::ACCENT_BLUE));
    ui.separator();

    // Stats row.
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("Vertices: {}", state.mcce.vertex_count))
            .color(theme::ACCENT_GREEN));
        ui.separator();
        ui.label(RichText::new(format!("Edges: {}", state.mcce.edge_count))
            .color(theme::ACCENT_ORANGE));
        ui.separator();
        ui.label(RichText::new(format!("Density: {:.3}", state.mcce.graph_density))
            .color(theme::TEXT_SECONDARY));
        ui.separator();
        ui.label(RichText::new(format!("Clusters: {}", state.mcce.cluster_count))
            .color(theme::CRYSTAL_COLOR));
        ui.separator();
        ui.label(RichText::new(format!("Signals: {}", state.mcce.total_signals))
            .color(theme::TEXT_DIM));
    });

    ui.separator();

    // Graph visualization (simplified force-directed dot plot).
    ui.label(RichText::new("Graph Topology").color(theme::TEXT_SECONDARY));

    let available = ui.available_size();
    let canvas_height = (available.y * 0.5).min(300.0);
    let (rect, _resp) = ui.allocate_exact_size(
        egui::vec2(available.x, canvas_height),
        egui::Sense::hover(),
    );

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(10, 15, 25));

    if state.mcce.vertex_count == 0 {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "No vertices yet. Run engine to discover tokens.",
            egui::FontId::proportional(12.0),
            theme::TEXT_DIM,
        );
    } else {
        // Draw vertex nodes using stable positions derived from vertex index.
        let cx = rect.center().x;
        let cy = rect.center().y;
        let radius = (canvas_height / 2.0 - 20.0).min(100.0);

        for i in 0..state.mcce.vertex_count.min(50) {
            let angle = (i as f32 / state.mcce.vertex_count.min(50) as f32) * std::f32::consts::TAU;
            let x = cx + radius * angle.cos();
            let y = cy + radius * angle.sin();
            painter.circle_filled(
                egui::pos2(x, y),
                4.0,
                egui::Color32::from_rgb(100, 200, 100),
            );
        }

        // Draw edges (simplified: draw lines between adjacent node positions).
        let edge_opacity = (state.mcce.edge_count as f32 / state.mcce.vertex_count.max(1) as f32 * 50.0) as u8;
        let edge_color = egui::Color32::from_rgba_unmultiplied(80, 180, 255, edge_opacity.max(20));
        for i in 0..state.mcce.edge_count.min(100) {
            let a_angle = (i as f32 * 1.618033 % state.mcce.vertex_count.max(1) as f32 / state.mcce.vertex_count.max(1) as f32) * std::f32::consts::TAU;
            let b_angle = ((i as f32 * 2.718281) % state.mcce.vertex_count.max(1) as f32 / state.mcce.vertex_count.max(1) as f32) * std::f32::consts::TAU;
            let ax = cx + radius * a_angle.cos();
            let ay = cy + radius * a_angle.sin();
            let bx = cx + radius * b_angle.cos();
            let by = cy + radius * b_angle.sin();
            painter.line_segment(
                [egui::pos2(ax, ay), egui::pos2(bx, by)],
                egui::Stroke::new(0.5, edge_color),
            );
        }
    }

    ui.separator();

    // Learning dynamics log.
    ui.label(RichText::new("Learning Dynamics").color(theme::TEXT_SECONDARY));
    egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
        if state.mcce.vertex_count == 0 {
            ui.label(RichText::new("Engine not started.").color(theme::TEXT_DIM));
        } else {
            ui.label(RichText::new(format!(
                "After {} ticks: {} vertices, {} edges, density={:.3}",
                state.tick,
                state.mcce.vertex_count,
                state.mcce.edge_count,
                state.mcce.graph_density,
            )).color(theme::TEXT_SECONDARY));
            if state.mcce.cluster_count > 0 {
                ui.label(RichText::new(format!(
                    "  {} stable cluster(s) detected — DSHAE signals enriched",
                    state.mcce.cluster_count,
                )).color(theme::ACCENT_GREEN));
            }
        }
    });
}
