//! Promotion Workflow (Phase 3 §4): 5-gate pipeline from Candidate → LiveActive.
//!
//! Steps:
//!   1. Candidate      → paper validate via backtest replay
//!   2. PaperValidated → emit PromotionProposal
//!   3. PromotionPending → operator approval
//!   4. LiveEligible   → operator starts live mode
//!   5. LiveActive     → live trading with sniper
//!
//! Gate Checks (spec §4.2):
//!   G1: net_pnl_bps >= 0 (no net loss in backtest)
//!   G2: invariant_violations == 0
//!   G3: win_rate >= 0.50 (50% of settled trades are profitable)
//!   G4: max_drawdown_bps <= max_allowed_drawdown_bps
//!   G5: ttcp_crystals_emitted >= 1 (at least one crystal in replay)

use fsr_types::{PromotionState, Q32};
use serde::{Deserialize, Serialize};

/// Result of one gate check.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GateCheckResult {
    pub gate_id: u8,
    pub name: String,
    pub passed: bool,
    pub value: String,
    pub threshold: String,
}

/// Proposal artifact emitted after gate checks pass (spec §4.1 step 3).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromotionProposal {
    pub proposal_id: String,
    pub config_hash_hex: String,
    pub backtest_report_hash_hex: String,
    /// Net P&L from backtest in basis points (Q32)
    pub net_pnl_bps: Q32,
    /// Total TTCP crystals in replay
    pub ttcp_crystal_count: u64,
    /// Win rate (Q32 fraction)
    pub win_rate: Q32,
    /// Max drawdown (Q32 basis points absolute value)
    pub max_drawdown_bps: Q32,
    /// Timestamp as Unix ms
    pub created_at_ms: u64,
    pub gate_results: Vec<GateCheckResult>,
}

impl PromotionProposal {
    /// Unique proposal ID built from config hash + timestamp.
    pub fn new_id(config_hash_hex: &str, timestamp_ms: u64) -> String {
        format!("prop_{}_{}",
            &config_hash_hex[..8.min(config_hash_hex.len())],
            timestamp_ms)
    }

    /// Human-readable summary of the proposal.
    pub fn summary(&self) -> String {
        use fsr_fixed::q32_to_f64_display_only;
        let win_pct = q32_to_f64_display_only(self.win_rate) * 100.0;
        let net = q32_to_f64_display_only(self.net_pnl_bps);
        let dd = q32_to_f64_display_only(self.max_drawdown_bps);
        format!(
            "=== PROMOTION PROPOSAL: {} ===\n\
             Config: {}...\n\
             Net P&L: {:+.2} bp | DD: {:.2} bp | Win: {:.1}% | Crystals: {}\n\
             Gates:   {}/{} passed\n\
             ===",
            self.proposal_id,
            &self.config_hash_hex[..8.min(self.config_hash_hex.len())],
            net, dd, win_pct,
            self.ttcp_crystal_count,
            self.gate_results.iter().filter(|g| g.passed).count(),
            self.gate_results.len(),
        )
    }

    /// True iff all 5 gates passed.
    pub fn all_gates_passed(&self) -> bool {
        self.gate_results.len() == 5 && self.gate_results.iter().all(|g| g.passed)
    }
}

/// Configuration for promotion gate thresholds.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromotionGateConfig {
    /// G4: maximum allowed drawdown (Q32 bps, absolute)
    pub max_drawdown_bps: Q32,
    /// G3: minimum win rate (Q32 fraction, e.g. ONE/2 = 50%)
    pub min_win_rate: Q32,
    /// G5: minimum TTCP crystals required
    pub min_ttcp_crystals: u64,
    /// Whether human approval is required before live activation
    pub live_promotion_requires_human_approval: bool,
}

impl Default for PromotionGateConfig {
    fn default() -> Self {
        use fsr_fixed::ONE;
        PromotionGateConfig {
            max_drawdown_bps: 3000i64 * ONE, // 30 bp in Q32
            min_win_rate: ONE / 2,            // 50%
            min_ttcp_crystals: 1,
            live_promotion_requires_human_approval: true,
        }
    }
}

/// Run all 5 gate checks against backtest results.
/// Returns (all_passed, Vec<GateCheckResult>).
pub fn run_gate_checks(
    net_pnl_bps: Q32,
    invariant_violations: u64,
    win_rate: Q32,
    max_drawdown_bps: Q32,
    ttcp_crystals: u64,
    gate_cfg: &PromotionGateConfig,
) -> (bool, Vec<GateCheckResult>) {
    use fsr_fixed::q32_to_f64_display_only;

    let g1 = GateCheckResult {
        gate_id: 1,
        name: "Net P&L non-negative".to_string(),
        passed: net_pnl_bps >= 0,
        value: format!("{:.4} bp", q32_to_f64_display_only(net_pnl_bps)),
        threshold: ">= 0".to_string(),
    };
    let g2 = GateCheckResult {
        gate_id: 2,
        name: "No invariant violations".to_string(),
        passed: invariant_violations == 0,
        value: invariant_violations.to_string(),
        threshold: "== 0".to_string(),
    };
    let g3 = GateCheckResult {
        gate_id: 3,
        name: "Win rate >= 50%".to_string(),
        passed: win_rate >= gate_cfg.min_win_rate,
        value: format!("{:.1}%", q32_to_f64_display_only(win_rate) * 100.0),
        threshold: format!("{:.1}%", q32_to_f64_display_only(gate_cfg.min_win_rate) * 100.0),
    };
    let g4 = GateCheckResult {
        gate_id: 4,
        name: "Max drawdown within limit".to_string(),
        passed: max_drawdown_bps <= gate_cfg.max_drawdown_bps,
        value: format!("{:.4} bp", q32_to_f64_display_only(max_drawdown_bps)),
        threshold: format!("<= {:.4} bp", q32_to_f64_display_only(gate_cfg.max_drawdown_bps)),
    };
    let g5 = GateCheckResult {
        gate_id: 5,
        name: "Minimum TTCP crystals".to_string(),
        passed: ttcp_crystals >= gate_cfg.min_ttcp_crystals,
        value: ttcp_crystals.to_string(),
        threshold: format!(">= {}", gate_cfg.min_ttcp_crystals),
    };

    let all_passed = g1.passed && g2.passed && g3.passed && g4.passed && g5.passed;
    (all_passed, vec![g1, g2, g3, g4, g5])
}

/// Summary of a backtest run passed into the promotion workflow (spec §4.2).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BacktestSummary {
    pub net_pnl_bps: Q32,
    pub invariant_violations: u64,
    pub win_rate: Q32,
    pub max_drawdown_bps: Q32,
    pub ttcp_crystals: u64,
    pub config_hash_hex: String,
    pub backtest_hash_hex: String,
    pub timestamp_ms: u64,
}

/// The operational promotion workflow controller (spec §4).
pub struct PromotionWorkflow {
    pub state: PromotionState,
    pub proposal: Option<PromotionProposal>,
    pub gate_config: PromotionGateConfig,
}

impl PromotionWorkflow {
    pub fn new() -> Self {
        PromotionWorkflow {
            state: PromotionState::Candidate,
            proposal: None,
            gate_config: PromotionGateConfig::default(),
        }
    }

    pub fn with_gate_config(mut self, cfg: PromotionGateConfig) -> Self {
        self.gate_config = cfg;
        self
    }

    /// Step 1→2: run backtest gate checks; if passing, transition to PaperValidated
    /// and create a PromotionProposal.
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_backtest(
        &mut self,
        net_pnl_bps: Q32,
        invariant_violations: u64,
        win_rate: Q32,
        max_drawdown_bps: Q32,
        ttcp_crystals: u64,
        config_hash_hex: &str,
        backtest_hash_hex: &str,
        timestamp_ms: u64,
    ) -> bool {
        let summary = BacktestSummary {
            net_pnl_bps,
            invariant_violations,
            win_rate,
            max_drawdown_bps,
            ttcp_crystals,
            config_hash_hex: config_hash_hex.to_string(),
            backtest_hash_hex: backtest_hash_hex.to_string(),
            timestamp_ms,
        };
        self.evaluate_backtest_summary(&summary)
    }

    /// Step 1→2: run backtest gate checks using a `BacktestSummary`.
    pub fn evaluate_backtest_summary(&mut self, summary: &BacktestSummary) -> bool {
        if self.state != PromotionState::Candidate {
            return false;
        }

        let (all_passed, gate_results) = run_gate_checks(
            summary.net_pnl_bps,
            summary.invariant_violations,
            summary.win_rate,
            summary.max_drawdown_bps,
            summary.ttcp_crystals,
            &self.gate_config,
        );

        println!("=== Promotion Gate Checks ===");
        for gr in &gate_results {
            println!("[{}] G{}: {} ({} / {})",
                if gr.passed { "PASS" } else { "FAIL" },
                gr.gate_id, gr.name, gr.value, gr.threshold);
        }

        if all_passed {
            let proposal_id = PromotionProposal::new_id(&summary.config_hash_hex, summary.timestamp_ms);
            let proposal = PromotionProposal {
                proposal_id,
                config_hash_hex: summary.config_hash_hex.clone(),
                backtest_report_hash_hex: summary.backtest_hash_hex.clone(),
                net_pnl_bps: summary.net_pnl_bps,
                ttcp_crystal_count: summary.ttcp_crystals,
                win_rate: summary.win_rate,
                max_drawdown_bps: summary.max_drawdown_bps,
                created_at_ms: summary.timestamp_ms,
                gate_results,
            };
            println!("{}", proposal.summary());
            self.proposal = Some(proposal);
            self.state = PromotionState::PaperValidated;
            // Automatically move to PromotionPending
            self.state = PromotionState::PromotionPending;
        } else {
            println!("Promotion gates FAILED. Config must be revised.");
        }

        all_passed
    }

    /// Step 3→4: operator approval. Returns true if approved.
    pub fn approve(&mut self, proposal_id: &str) -> bool {
        if self.state != PromotionState::PromotionPending {
            println!("Cannot approve: not in PromotionPending state (current: {:?})", self.state);
            return false;
        }
        let matches = self.proposal.as_ref()
            .map(|p| p.proposal_id == proposal_id)
            .unwrap_or(false);
        if !matches {
            println!("Proposal ID mismatch. Expected: {:?}",
                self.proposal.as_ref().map(|p| &p.proposal_id));
            return false;
        }
        self.state = PromotionState::LiveEligible;
        println!("Proposal {} approved. State: LiveEligible", proposal_id);
        true
    }

    /// Step 4→5: activate live mode.
    pub fn activate_live(&mut self) -> bool {
        if self.state == PromotionState::LiveEligible {
            self.state = PromotionState::LiveActive;
            true
        } else {
            false
        }
    }

    pub fn can_activate(&self) -> bool {
        self.state == PromotionState::LiveEligible
    }

    pub fn status_line(&self) -> String {
        match &self.proposal {
            Some(p) => format!("state={:?} proposal={}", self.state, p.proposal_id),
            None => format!("state={:?} proposal=none", self.state),
        }
    }

    /// Demote back to Candidate (e.g. after live regression).
    pub fn demote(&mut self) {
        self.state = PromotionState::Demoted;
        self.proposal = None;
    }
}

impl Default for PromotionWorkflow {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_fixed::ONE;

    #[test]
    fn test_gate_checks_all_pass() {
        let cfg = PromotionGateConfig::default();
        let (passed, results) = run_gate_checks(
            1000,           // positive pnl
            0,              // no violations
            ONE * 3 / 5,    // 60% win rate
            0,              // zero drawdown
            2,              // 2 crystals
            &cfg,
        );
        assert!(passed, "all gates should pass");
        assert_eq!(results.len(), 5);
        for r in &results {
            assert!(r.passed, "G{} must pass", r.gate_id);
        }
    }

    #[test]
    fn test_gate_checks_pnl_fail() {
        let cfg = PromotionGateConfig::default();
        let (passed, results) = run_gate_checks(
            -100,       // negative pnl
            0, ONE * 3/5, 0, 2, &cfg,
        );
        assert!(!passed, "should fail due to negative pnl");
        assert!(!results[0].passed, "G1 must fail");
        assert!(results[1].passed, "G2 must pass");
    }

    #[test]
    fn test_gate_checks_violation_fail() {
        let cfg = PromotionGateConfig::default();
        let (passed, _) = run_gate_checks(100, 1, ONE * 3/5, 0, 2, &cfg);
        assert!(!passed, "invariant violation should fail G2");
    }

    #[test]
    fn test_promotion_workflow_happy_path() {
        let mut wf = PromotionWorkflow::new();
        assert_eq!(wf.state, PromotionState::Candidate);

        let ok = wf.evaluate_backtest(
            1000, 0, ONE * 3/5, 0, 2,
            "abc123hash", "def456hash",
            1_000_000,
        );
        assert!(ok, "all gates should pass");
        assert_eq!(wf.state, PromotionState::PromotionPending);
        assert!(wf.proposal.is_some());

        let proposal_id = wf.proposal.as_ref().unwrap().proposal_id.clone();
        let approved = wf.approve(&proposal_id);
        assert!(approved);
        assert_eq!(wf.state, PromotionState::LiveEligible);
        assert!(wf.can_activate());

        let activated = wf.activate_live();
        assert!(activated);
        assert_eq!(wf.state, PromotionState::LiveActive);
    }

    #[test]
    fn test_promotion_workflow_gate_fail() {
        let mut wf = PromotionWorkflow::new();
        let ok = wf.evaluate_backtest(
            -500, 0, ONE / 3, 100_000, 0, // pnl < 0, win_rate < 50%, no crystals
            "abc123", "def456", 0,
        );
        assert!(!ok);
        assert_eq!(wf.state, PromotionState::Candidate, "must stay Candidate on gate fail");
    }

    #[test]
    fn test_promotion_approve_wrong_id() {
        let mut wf = PromotionWorkflow::new();
        wf.evaluate_backtest(1000, 0, ONE * 3/5, 0, 2, "abc", "def", 0);
        let approved = wf.approve("wrong_id");
        assert!(!approved, "wrong proposal_id must be rejected");
    }
}
