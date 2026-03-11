//! Sandbox launcher + results panel (spec §4.3).
//!
//! Displays 5 scenario cards with expected outcomes.
//! Clicking a scenario runs it via SandboxRunner and shows PASS/FAIL with criteria.

use crate::theme::*;
use egui::{Color32, Grid, RichText, Ui};
use fsr_dshae::sandbox_gen::{Scenario, SandboxResult, SandboxRunner};
use std::sync::{Arc, Mutex};
use std::thread;

/// Sandbox panel state (persisted across frames).
pub struct SandboxPanel {
    runner: SandboxRunner,
    results: [Option<SandboxResult>; 5],
    running: [bool; 5],
}

impl Default for SandboxPanel {
    fn default() -> Self {
        SandboxPanel {
            runner: SandboxRunner::with_default_config(),
            results: [None, None, None, None, None],
            running: [false; 5],
        }
    }
}

const SCENARIOS: [Scenario; 5] = [
    Scenario::Calm,
    Scenario::SingleArb,
    Scenario::RecurringArb,
    Scenario::Noisy,
    Scenario::RegimeShift,
];

const SCENARIO_LABELS: [&str; 5] = [
    "Calm Market (no arb)",
    "Single Arbitrage",
    "Recurring Arbitrage",
    "Noisy Market (no arb)",
    "Regime Shift",
];

const EXPECTED_LABELS: [&str; 5] = [
    "Expected: 0 trades, 0 crystals",
    "Expected: 1 crystal near injection tick",
    "Expected: 3-5 crystals at varying intensities",
    "Expected: 0 trades (false-positive resistance)",
    "Expected: 2 crystals during volatile phase",
];

impl SandboxPanel {
    /// Render the sandbox tab.
    pub fn render(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Validation Sandbox").color(TEXT_PRIMARY));
        ui.separator();

        ui.label(
            RichText::new(
                "One-click offline validation. All scenarios run entirely on synthetic bundled data.",
            )
            .color(TEXT_DIM),
        );
        ui.add_space(8.0);

        // Render each scenario.
        for (idx, scenario) in SCENARIOS.iter().enumerate() {
            self.render_scenario_card(ui, idx, *scenario);
            ui.add_space(4.0);
        }

        ui.separator();

        // Run All button
        if ui
            .button(RichText::new("▶ Run All 5 Scenarios").color(TEXT_PRIMARY))
            .clicked()
        {
            for idx in 0..5 {
                if !self.running[idx] {
                    self.run_scenario(idx);
                }
            }
        }

        // Summary
        let total = self.results.iter().filter(|r| r.is_some()).count();
        let passed = self.results.iter().filter(|r| r.as_ref().map_or(false, |s| s.passed)).count();
        if total > 0 {
            ui.add_space(8.0);
            ui.separator();
            let summary_color = if passed == total { ACCENT_GREEN } else { ACCENT_RED };
            ui.label(
                RichText::new(format!("SUMMARY: {}/{} scenarios PASS", passed, total))
                    .color(summary_color)
                    .size(16.0),
            );
        }
    }

    fn render_scenario_card(&mut self, ui: &mut Ui, idx: usize, scenario: Scenario) {
        ui.group(|ui| {
            ui.horizontal(|ui| {
                // Run button
                let btn_label = if self.running[idx] {
                    "⏳ Running..."
                } else {
                    "▶ Run"
                };
                if ui.button(btn_label).clicked() && !self.running[idx] {
                    self.run_scenario(idx);
                }

                ui.label(RichText::new(SCENARIO_LABELS[idx]).color(TEXT_PRIMARY));
                ui.separator();
                ui.label(RichText::new(EXPECTED_LABELS[idx]).color(TEXT_DIM).size(11.0));
            });

            if let Some(result) = &self.results[idx] {
                ui.add_space(4.0);
                render_result(ui, result);
            }
        });
    }

    fn run_scenario(&mut self, idx: usize) {
        let scenario = SCENARIOS[idx];
        let runner = SandboxRunner::with_default_config();
        // Run synchronously for simplicity (scenarios are fast: <1s each).
        self.results[idx] = Some(runner.run_scenario(scenario));
        self.running[idx] = false;
    }
}

fn render_result(ui: &mut Ui, result: &SandboxResult) {
    let (status_label, status_color) = if result.passed {
        ("✓ PASS", ACCENT_GREEN)
    } else {
        ("✗ FAIL", ACCENT_RED)
    };

    ui.horizontal(|ui| {
        ui.label(RichText::new(status_label).color(status_color).size(14.0));
        ui.separator();
        ui.label(
            RichText::new(format!(
                "crystals={} trades={} det={}",
                result.actual_crystals,
                result.actual_trades,
                if result.replay_determinism { "✓" } else { "✗" }
            ))
            .color(TEXT_SECONDARY),
        );
    });

    // Per-criterion checks.
    Grid::new(format!("checks_{}", result.scenario))
        .num_columns(2)
        .spacing([12.0, 2.0])
        .show(ui, |ui| {
            for check in &result.checks {
                let (mark, col) = if check.passed { ("✓", ACCENT_GREEN) } else { ("✗", ACCENT_RED) };
                ui.label(RichText::new(format!("{} {}", mark, check.name)).color(col).size(11.0));
                ui.end_row();
            }
        });
}
