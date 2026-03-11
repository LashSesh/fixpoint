//! egui_plot helpers: line charts, scatter plots.

use egui::Ui;
use egui_plot::{Line, Plot, PlotPoints, Points};
use std::collections::VecDeque;

/// Draw a line chart from a time series of (tick, value) points.
pub fn line_chart(
    ui: &mut Ui,
    id: &str,
    label: &str,
    history: &VecDeque<(u64, f32)>,
    color: egui::Color32,
    height: f32,
) {
    let points: PlotPoints = history
        .iter()
        .map(|&(t, v)| [t as f64, v as f64])
        .collect();
    let line = Line::new(points).color(color).name(label);
    Plot::new(id)
        .height(height)
        .show_axes([true, true])
        .show(ui, |plot_ui| {
            plot_ui.line(line);
        });
}

/// Draw a scatter plot from (x, y) points (for HIM visualization).
pub fn scatter_plot(
    ui: &mut Ui,
    id: &str,
    points_xy: &[(f32, f32)],
    color: egui::Color32,
    height: f32,
    x_label: &str,
    y_label: &str,
) {
    let plot_points: PlotPoints = points_xy.iter().map(|&(x, y)| [x as f64, y as f64]).collect();
    let scatter = Points::new(plot_points).color(color).radius(4.0).name(format!("{}/{}", x_label, y_label));
    Plot::new(id)
        .height(height)
        .show_axes([true, true])
        .show(ui, |plot_ui| {
            plot_ui.points(scatter);
        });
}

/// Draw multiple lines on the same plot (e.g., delta1/delta2/delta3).
pub fn multi_line_chart(
    ui: &mut Ui,
    id: &str,
    series: &[(&str, egui::Color32, Vec<[f64; 2]>)],
    height: f32,
) {
    Plot::new(id)
        .height(height)
        .show_axes([true, true])
        .show(ui, |plot_ui| {
            for (label, color, pts) in series {
                let line = Line::new(PlotPoints::new(pts.clone()))
                    .color(*color)
                    .name(*label);
                plot_ui.line(line);
            }
        });
}
