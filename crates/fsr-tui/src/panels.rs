//! Six-panel TUI layout for FIXPOINT SWARM-R Phase 2 dashboard.
//!
//! Layout (2 columns × 3 rows):
//!   ┌──────────────────┬──────────────────┐
//!   │ [1] Regime/Gates │ [2] Resonance    │
//!   ├──────────────────┼──────────────────┤
//!   │ [3] Candidates   │ [4] P&L          │
//!   ├──────────────────┼──────────────────┤
//!   │ [5] Event Log    │ [6] TTCP Status  │
//!   └──────────────────┴──────────────────┘

use crate::state::DashboardState;
use fsr_fixed::q32_to_f64_display_only;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

/// Render all 6 panels into `area`.
pub fn render_all(f: &mut Frame, area: Rect, state: &DashboardState) {
    // Split into 3 rows.
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(area);

    // Each row split into 2 columns.
    let top = split_row(rows[0]);
    let mid = split_row(rows[1]);
    let bot = split_row(rows[2]);

    render_regime_panel(f, top[0], state);
    render_resonance_panel(f, top[1], state);
    render_candidates_panel(f, mid[0], state);
    render_pnl_panel(f, mid[1], state);
    render_event_log_panel(f, bot[0], state);
    render_ttcp_panel(f, bot[1], state);
}

fn split_row(row: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(row)
        .to_vec()
}

// ── Panel 1: Regime & Gates ────────────────────────────────────────────────────

pub fn render_regime_panel(f: &mut Frame, area: Rect, state: &DashboardState) {
    let gate_color = if state.gate_open { Color::Green } else { Color::Red };
    let regime_color = match state.regime.as_str() {
        "Alpha" => Color::Green,
        "Beta" => Color::Yellow,
        "Gamma" => Color::Red,
        _ => Color::White,
    };
    let integrity_color = match state.integrity.as_str() {
        "Healthy" => Color::Green,
        "Degraded" => Color::Yellow,
        "SafeHold" | "RollbackPending" | "Killed" => Color::Red,
        _ => Color::White,
    };

    let paused_line = if state.paused {
        Line::from(Span::styled("  ⏸ PAUSED", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)))
    } else {
        Line::from(Span::raw(""))
    };

    let text = vec![
        Line::from(vec![
            Span::raw("  Tick:      "),
            Span::styled(
                format!("{}", state.tick),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Regime:    "),
            Span::styled(
                state.regime.clone(),
                Style::default().fg(regime_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Integrity: "),
            Span::styled(
                state.integrity.clone(),
                Style::default().fg(integrity_color),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Resource:  "),
            Span::styled(state.resource.clone(), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::raw("  Gate:      "),
            Span::styled(
                if state.gate_open { "OPEN ▲" } else { "SHUT ▼" },
                Style::default().fg(gate_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Γ score:   "),
            Span::styled(
                format!("{:+.4}", q32_to_f64_display_only(state.gamma_score)),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        paused_line,
    ];

    let block = Block::default()
        .title(" [1] Regime & Gates ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    let paragraph = Paragraph::new(text).block(block);
    f.render_widget(paragraph, area);
}

// ── Panel 2: Resonance Metrics ────────────────────────────────────────────────

pub fn render_resonance_panel(f: &mut Frame, area: Rect, state: &DashboardState) {
    let fmt = |v: fsr_types::Q32| format!("{:+.4}", q32_to_f64_display_only(v));

    let text = vec![
        Line::from(vec![
            Span::raw("  ψ (psi):      "),
            Span::styled(fmt(state.psi), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw("  ρ (rho):      "),
            Span::styled(fmt(state.rho), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw("  ω (omega):    "),
            Span::styled(fmt(state.omega), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw("  κ (kappa):    "),
            Span::styled(fmt(state.kappa), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::raw("  H (entropy):  "),
            Span::styled(fmt(state.entropy), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::raw("  M (momentum): "),
            Span::styled(fmt(state.momentum), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::raw("  SI:           "),
            Span::styled(fmt(state.si), Style::default().fg(Color::Green)),
        ]),
    ];

    let block = Block::default()
        .title(" [2] Resonance Metrics ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    f.render_widget(Paragraph::new(text).block(block), area);
}

// ── Panel 3: Candidates ───────────────────────────────────────────────────────

pub fn render_candidates_panel(f: &mut Frame, area: Rect, state: &DashboardState) {
    let cand_color = if state.candidates_found > 0 {
        Color::Green
    } else {
        Color::DarkGray
    };

    let text = vec![
        Line::from(vec![
            Span::raw("  Found:    "),
            Span::styled(
                format!("{}", state.candidates_found),
                Style::default().fg(cand_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Wind:     "),
            Span::styled(
                format!("{}", state.wind_count),
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Shadow:   "),
            Span::styled(
                format!("{} events", state.shadow_event_count),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Commit:   "),
            Span::styled(
                format!("{} events", state.commitment_event_count),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Head:     "),
            Span::styled(
                // Show only first 16 chars of hex digest
                state.shadow_head.chars().take(16).collect::<String>() + "…",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ];

    let block = Block::default()
        .title(" [3] Candidates ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    f.render_widget(Paragraph::new(text).block(block), area);
}

// ── Panel 4: P&L ──────────────────────────────────────────────────────────────

pub fn render_pnl_panel(f: &mut Frame, area: Rect, state: &DashboardState) {
    let total = state.settled_cycles + state.aborted_cycles;
    let win_rate = if total > 0 {
        100.0 * state.settled_cycles as f64 / total as f64
    } else {
        0.0
    };

    let text = vec![
        Line::from(vec![
            Span::raw("  Settled:  "),
            Span::styled(
                format!("{}", state.settled_cycles),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Aborted:  "),
            Span::styled(
                format!("{}", state.aborted_cycles),
                Style::default().fg(Color::Red),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Win rate: "),
            Span::styled(
                format!("{:.1}%", win_rate),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Drawdown: "),
            Span::styled(
                format!("{:.4}", q32_to_f64_display_only(state.current_drawdown)),
                Style::default().fg(Color::Yellow),
            ),
        ]),
    ];

    let block = Block::default()
        .title(" [4] P&L ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    f.render_widget(Paragraph::new(text).block(block), area);
}

// ── Panel 5: Event Log ────────────────────────────────────────────────────────

pub fn render_event_log_panel(f: &mut Frame, area: Rect, state: &DashboardState) {
    // Show most recent events at the bottom of the list.
    let items: Vec<ListItem> = state
        .event_log
        .iter()
        .rev()
        .take(area.height.saturating_sub(2) as usize)
        .map(|s| ListItem::new(Line::from(Span::styled(s.as_str(), Style::default().fg(Color::Gray)))))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    let block = Block::default()
        .title(" [5] Event Log ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    f.render_widget(List::new(items).block(block), area);
}

// ── Panel 6: TTCP Status ──────────────────────────────────────────────────────

pub fn render_ttcp_panel(f: &mut Frame, area: Rect, state: &DashboardState) {
    let t = &state.ttcp;
    let level_color = match t.level {
        0 => Color::DarkGray,
        1 => Color::Yellow,
        2 | 3 => Color::Green,
        _ => Color::White,
    };

    let last_crystal_line = match &t.last_crystal {
        Some(c) => format!(
            "  SI={:.3} ψ={:.3}",
            q32_to_f64_display_only(c.convergence_score),
            q32_to_f64_display_only(c.resonance_psi)
        ),
        None => "  (none yet)".to_string(),
    };

    let text = vec![
        Line::from(vec![
            Span::raw("  Level:    "),
            Span::styled(
                if t.level == 0 {
                    "inactive".to_string()
                } else {
                    format!("L{}", t.level)
                },
                Style::default().fg(level_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Crystals: "),
            Span::styled(
                format!("{}", t.crystals_found),
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Last tick: "),
            Span::styled(
                t.last_crystal_tick
                    .map_or("-".to_string(), |t| t.to_string()),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(Span::styled(
            "  Last crystal:",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            last_crystal_line,
            Style::default().fg(Color::Green),
        )),
        Line::from(Span::raw("")),
        Line::from(Span::styled(
            "  [q]uit [p]ause [r]esume",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .title(" [6] TTCP Status ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    f.render_widget(Paragraph::new(text).block(block), area);
}
