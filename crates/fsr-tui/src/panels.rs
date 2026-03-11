//! Six-panel TUI layout for FIXPOINT SWARM-R Phase 3 dashboard.
//!
//! Layout (2 columns × 3 rows):
//!   ┌──────────────────┬──────────────────┐
//!   │ [1] Regime+Sniper│ [2] Resonance    │
//!   ├──────────────────┼──────────────────┤
//!   │ [3] Candidates   │ [4] Risk & P&L   │
//!   ├──────────────────┼──────────────────┤
//!   │ [5] Event Log    │ [6] TTCP Status  │
//!   └──────────────────┴──────────────────┘
//!
//! Phase 3 changes (spec §8):
//!   Panel 1: Regime & Gates → Regime & Sniper (adds sniper status)
//!   Panel 3: Candidates → Candidates & Venues (per-venue counts)
//!   Panel 4: P&L → Risk & P&L (risk dashboard, spec §6)
//!   Panel 6: TTCP Status → TTCP & Crystals (delta1/2/3)

/// Risk dashboard panel (Phase 3 §6, spec placement: src/panels/risk.rs).
pub mod risk {
    use crate::state::DashboardState;
    use fsr_fixed::q32_to_f64_display_only;
    use ratatui::{
        layout::Rect,
        style::{Color, Modifier, Style},
        text::{Line, Span},
        widgets::{Block, Borders, Paragraph},
        Frame,
    };

    pub fn render(f: &mut Frame, area: Rect, state: &DashboardState) {
        let r = &state.risk;
        let net = q32_to_f64_display_only(r.net_pnl_bps);
        let dd = q32_to_f64_display_only(r.drawdown_bps);
        let dd_lim = q32_to_f64_display_only(r.max_drawdown_limit_bps);
        let lev = q32_to_f64_display_only(r.leverage);
        let lev_max = q32_to_f64_display_only(r.max_leverage);
        let risk_pct = q32_to_f64_display_only(r.risk_budget_fraction) * 100.0;

        let total = state.settled_cycles + state.aborted_cycles;
        let win_pct = if total > 0 {
            100.0 * state.settled_cycles as f64 / total as f64
        } else {
            0.0
        };

        let net_color = if net >= 0.0 { Color::Green } else { Color::Red };
        let dd_color = if dd_lim > 0.0 && dd.abs() < dd_lim * 0.5 {
            Color::Green
        } else if dd_lim > 0.0 && dd.abs() < dd_lim * 0.8 {
            Color::Yellow
        } else {
            Color::Red
        };
        let daily_color = if r.daily_limit_hit { Color::Red } else { Color::Green };

        let text = vec![
            Line::from(vec![
                Span::raw("  Net: "),
                Span::styled(
                    format!("{:+.2} bp", net),
                    Style::default().fg(net_color).add_modifier(Modifier::BOLD),
                ),
                Span::raw("  DD: "),
                Span::styled(
                    format!("{:.2}/{:.0} bp", dd, dd_lim),
                    Style::default().fg(dd_color),
                ),
            ]),
            Line::from(vec![
                Span::raw("  Exp: "),
                Span::styled(
                    format!("{:.1}", q32_to_f64_display_only(r.exposure_bps)),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw("  Lev: "),
                Span::styled(
                    format!("{:.2}x/{:.1}x", lev, lev_max),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(vec![
                Span::raw("  Trades: "),
                Span::styled(
                    format!("{} ({:.1}%)", total, win_pct),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw("  Risk: "),
                Span::styled(
                    format!("{:.1}%", risk_pct),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
            Line::from(vec![
                Span::raw("  Daily: "),
                Span::styled(
                    if r.daily_limit_hit {
                        "LIMIT HIT".to_string()
                    } else {
                        format!(
                            "{:.2}/{:.0} bp",
                            q32_to_f64_display_only(r.daily_loss_bps),
                            q32_to_f64_display_only(r.daily_loss_limit_bps)
                        )
                    },
                    Style::default().fg(daily_color),
                ),
            ]),
            Line::from(vec![
                Span::raw("  Inventory: "),
                Span::styled(
                    if r.inventory_summary.is_empty() {
                        "none".to_string()
                    } else {
                        r.inventory_summary.clone()
                    },
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
        ];

        let block = Block::default()
            .title(" [4] Risk & P&L ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray));
        f.render_widget(Paragraph::new(text).block(block), area);
    }
}

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
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(area);

    let top = split_row(rows[0]);
    let mid = split_row(rows[1]);
    let bot = split_row(rows[2]);

    render_regime_sniper_panel(f, top[0], state);
    render_resonance_panel(f, top[1], state);
    render_candidates_venues_panel(f, mid[0], state);
    risk::render(f, mid[1], state);
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

// ── Panel 1: Regime & Sniper ───────────────────────────────────────────────────

/// Legacy alias kept for backward compat with any external callers.
pub fn render_regime_panel(f: &mut Frame, area: Rect, state: &DashboardState) {
    render_regime_sniper_panel(f, area, state);
}

pub fn render_regime_sniper_panel(f: &mut Frame, area: Rect, state: &DashboardState) {
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
    let sniper_color = match state.sniper.state_name.as_str() {
        "ARMED" => Color::Green,
        "EXECUTING" => Color::LightGreen,
        "COOLDOWN" => Color::Yellow,
        "OBSERVING" => Color::Cyan,
        _ => Color::DarkGray,
    };

    let paused_line = if state.paused {
        Line::from(Span::styled(
            "  ⏸ PAUSED",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ))
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
            Span::raw("  Gate:      "),
            Span::styled(
                if state.gate_open { "OPEN ▲" } else { "SHUT ▼" },
                Style::default().fg(gate_color).add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(" (G={:.3})", q32_to_f64_display_only(state.gamma_score))),
        ]),
        Line::from(vec![
            Span::raw("  Integrity: "),
            Span::styled(
                state.integrity.clone(),
                Style::default().fg(integrity_color),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Sniper:    "),
            Span::styled(
                if state.sniper.status_line.is_empty() {
                    "DISABLED".to_string()
                } else {
                    state.sniper.status_line.clone()
                },
                Style::default().fg(sniper_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        paused_line,
    ];

    let block = Block::default()
        .title(" [1] Regime & Sniper ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    f.render_widget(Paragraph::new(text).block(block), area);
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

// ── Panel 3: Candidates & Venues ─────────────────────────────────────────────

/// Legacy alias.
pub fn render_candidates_panel(f: &mut Frame, area: Rect, state: &DashboardState) {
    render_candidates_venues_panel(f, area, state);
}

pub fn render_candidates_venues_panel(f: &mut Frame, area: Rect, state: &DashboardState) {
    let cand_color = if state.candidates_found > 0 { Color::Green } else { Color::DarkGray };
    let xvenue_color = if state.cross_venue_count > 0 { Color::Cyan } else { Color::DarkGray };

    let text = vec![
        Line::from(vec![
            Span::raw("  BIN L1: "),
            Span::styled(format!("{}", state.binance_l1_count), Style::default().fg(cand_color)),
            Span::raw("  L2: "),
            Span::styled(format!("{}", state.binance_l2_count), Style::default().fg(cand_color)),
        ]),
        Line::from(vec![
            Span::raw("  KRK L1: "),
            Span::styled(format!("{}", state.kraken_l1_count), Style::default().fg(Color::Cyan)),
            Span::raw("  L2: "),
            Span::styled(format!("{}", state.kraken_l2_count), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw("  X-venue: "),
            Span::styled(
                format!("{}", state.cross_venue_count),
                Style::default().fg(xvenue_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Wind:     "),
            Span::styled(format!("{}", state.wind_count), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::raw("  Shadow:   "),
            Span::styled(
                format!("{} events", state.shadow_event_count),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::raw("  Head:     "),
            Span::styled(
                state.shadow_head.chars().take(16).collect::<String>() + "…",
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ];

    let block = Block::default()
        .title(" [3] Candidates & Venues ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    f.render_widget(Paragraph::new(text).block(block), area);
}

// ── Panel 4: Risk & P&L (delegated to risk submodule) ─────────────────────────

pub fn render_pnl_panel(f: &mut Frame, area: Rect, state: &DashboardState) {
    risk::render(f, area, state);
}

// ── Panel 5: Event Log ────────────────────────────────────────────────────────

pub fn render_event_log_panel(f: &mut Frame, area: Rect, state: &DashboardState) {
    let items: Vec<ListItem> = state
        .event_log
        .iter()
        .rev()
        .take(area.height.saturating_sub(2) as usize)
        .map(|s| {
            ListItem::new(Line::from(Span::styled(
                s.as_str(),
                Style::default().fg(Color::Gray),
            )))
        })
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

// ── Panel 6: TTCP & Crystals ──────────────────────────────────────────────────

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
            "  SI={:.3} ψ={:.3} ρ={:.3}",
            q32_to_f64_display_only(c.convergence_score),
            q32_to_f64_display_only(c.resonance_psi),
            q32_to_f64_display_only(c.resonance_rho),
        ),
        None => "  (none yet)".to_string(),
    };

    let last_ago = t.last_crystal_tick
        .map_or(0u64, |lt| state.tick.saturating_sub(lt));

    let text = vec![
        Line::from(vec![
            Span::raw("  Cascade: "),
            Span::styled(
                if t.level == 0 { "inactive".to_string() } else { format!("{}/4", t.level) },
                Style::default().fg(level_color).add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                " d1={:.3} d2={:.3} d3={:.3}",
                q32_to_f64_display_only(t.avg_delta1),
                q32_to_f64_display_only(t.avg_delta2),
                q32_to_f64_display_only(t.avg_delta3),
            )),
        ]),
        Line::from(vec![
            Span::raw("  Crystals: "),
            Span::styled(
                format!("{} total", t.crystals_found),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw(format!("  Last: {} ticks ago", last_ago)),
        ]),
        Line::from(vec![
            Span::raw("  Validity: "),
            Span::styled(
                format!("{} ticks remaining", t.crystal_validity_remaining),
                Style::default().fg(if t.crystal_validity_remaining > 0 {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
        ]),
        Line::from(Span::styled("  Last crystal:", Style::default().fg(Color::DarkGray))),
        Line::from(Span::styled(last_crystal_line, Style::default().fg(Color::Green))),
        Line::from(Span::raw("")),
        Line::from(Span::styled(
            "  [q]uit [p]ause [r]esume [s]niper",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .title(" [6] TTCP & Crystals ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    f.render_widget(Paragraph::new(text).block(block), area);
}
