# FIXPOINT SWARM-R (AinSoft-R v5)

### "The Topology of Greed"

---

Deterministic Rust trading system with phase synchronization, DSHAE arbitrage engine for holographic phase-space projection, Intelligence Substrate (ISLS/MCCE/ECLS), and desktop GUI

---

"No one can serve two masters. Either he will hate the one and love the other, or he will be devoted to the one and despise the other. You cannot serve both God and money.
Therefore I tell you, do not worry about your life, what you will eat or drink; or about your body, what you will wear. Is not life more than food, and the body more than clothes?"
- Matthew 6:24–25

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Crate Directory](#crate-directory)
4. [Quick Start](#quick-start)
5. [Configuration](#configuration)
6. [CLI Reference](#cli-reference)
7. [Phase 4 Components](#phase-4-components)
8. [Phase 5 Components](#phase-5-components)
9. [Validation Sandbox](#validation-sandbox)
10. [Development](#development)
11. [Invariants](#invariants)
12. [Phase Overview](#phase-overview)
13. [License](#license)

---

## Overview

FIXPOINT SWARM-R is a fully deterministic, chain-secured trading system implemented in Rust. It combines:

- **Resonance Engine** (ψ/ρ/ω metrics) for market state assessment
- **TTCP crystallization** (Tri-Carrier Phase Convergence) for trade signals
- **DSHAE engine** (Dual-Simplex Holographic Arbitrage Engine) for triangular arbitrage
- **Dual-chain integrity** (Shadow + Commitment Chain, SHA-256 chained events)
- **Intelligence Substrate** (ISLS + MCCE + ECLS) — persistent topological memory and constraint detection (Phase 5)
- **Desktop GUI** (egui/eframe) with live dashboard, sandbox validation, and mycelium visualization

The system is fully auditable: every state change is stored in the chain as an immutable event. A deterministic replay with identical inputs produces bit-for-bit identical outputs.

---

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│                  fsr-runtime (CLI: fsr)                     │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────────────┐ │
│  │ Macro-   │  │ DSHAE    │  │  TTCP Engine              │ │
│  │ Cycle    │  │ Bridge   │  │  (Crystals)               │ │
│  │ (24 steps)│ └──────────┘  └────────────────────────────┘ │
│  └────┬─────┘                                               │
│       │ Phase 5: ISLS-PERSIST / MCCE-UPDATE / ECLS-SCAN    │
│  ┌────▼───────────────────────────────────────────────────┐ │
│  │  Intelligence Substrate                               │ │
│  │  fsr-isls (Ledger) │ fsr-mcce (Graph) │ fsr-ecls (Scan)│ │
│  └────────────────────────────────────────────────────────┘ │
└───────────────────────┬─────────────────────────────────────┘
                        │ Arc<Mutex<GuiState>>
┌───────────────────────▼─────────────────────────────────────┐
│                fsr-gui (Desktop GUI)                        │
│  Dashboard │ P&L │ Crystals │ TTCP │ Risk │ Config │       │
│  Sandbox   │ Mycelium (new) │ Constraints (new) │          │
│  Knowledge (new)                                            │
└─────────────────────────────────────────────────────────────┘

Core crates:
  fsr-types     ←─ shared types (Q32, EventTag, OrderBook …)
  fsr-fixed     ←─ Q32 fixed-point arithmetic (32.32, ONE = 1<<32)
  fsr-chain     ←─ dual-chain (Shadow + Commitment, SHA-256) + ISLS wrapper
  fsr-resonance ←─ resonance engine (SI, ψ, ρ, ω, κ, entropy)
  fsr-gate      ←─ Kairos gate (Regime FSM, Gamma score)
  fsr-ttcp      ←─ TTCP crystallization (3-level cascade)
  fsr-dshae     ←─ DSHAE engine (Phase 4, triangular arbitrage)
  fsr-isls      ←─ Intelligent Semantic Ledger Substrate (Phase 5)
  fsr-mcce      ←─ Mycelial Crypto-Cartography Engine (Phase 5)
  fsr-ecls      ←─ Emergent Constraint Lattice Spectroscopy (Phase 5)
  fsr-tui       ←─ terminal UI (ratatui)

# 24-Step Macro Cycle

Each tick goes through exactly 24 steps (Spec §7.2 + Phase 5 extension):

1.  OBSERVE         – Read order books
2.  NORMALIZE       – Normalize prices
3.  ISLS-PERSIST    – Write observation to ISLS hot tier          ← NEW Phase 5
4.  MCCE-UPDATE     – Update mycelial HDAG                        ← NEW Phase 5
5.  EXTRACT         – Extract Q32 signals
6.  TEMPORAL        – TriCarrier / TemporalKey
7.  RESOURCE        – Check resource budget
8.  REGIME          – Advance RegimeFSM (Alpha/Beta/Gamma)
9.  INTEGRITY       – IntegrityFSM (Healthy/Degraded/SafeHold)
9b. ECLS-SCAN       – Constraint scan (every scan_interval ticks) ← NEW Phase 5
10. NULLCENTER      – Windscar gate
11. CANDIDATES      – Filter candidates + Press top-k
12. CSP             – Lockstep admissibility
13. EXECUTE         – Trade execution (Paper / Live)
14. CRYSTAL         – TTCP crystal detection
15. HEDGE           – HedgeFSM
16. EVIDENCE        – MacroCycleEnd in shadow chain
17. MCCE-FRUITING   – Emit mycelium signals                       ← NEW Phase 5
18. ISLS-CONSENSUS  – Resonant consensus + storage compaction     ← NEW Phase 5
19. CALIBRATE       – Promotion FSM
20. INVARIANTS      – Check 15 invariants
21. SAFE-HOLD       – Freeze on violation
22. ROTATE          – Chain rotation
23. STATUS          – Output status report
    + Phase 4: DSHAE tick after step 16

Crate Directory
Crate	Phase	Description
fsr-types	1	Shared types: Q32, EventTag, OrderBook, FSM states
fsr-fixed	1	Q32 fixed-point (32.32-bit): q32_mul, q32_ln, q32_from_ratio
fsr-temporal	1	TriCarrier, TemporalKey, KairosScheduler
fsr-nullcenter	1	Windscar gate, NC certificate
fsr-resonance	1	Resonance engine: SI, ψ, ρ, ω, κ, entropy, impulse
fsr-gate	1	KairosGate, RegimeFsm (Alpha/Beta/Gamma)
fsr-mirror	1	POR FSM, MCI calculation
fsr-candidates	1	Candidate filtering, wt_multiplex, filter_and_press
fsr-csp	1	CSP FSM, lockstep admissibility, quorum
fsr-hedge	1	Hedge FSM (Safe/Hedging/Unwind/Recovery)
fsr-calibration	2	Promotion workflow, 5-gate pipeline
fsr-chain	1	Dual-chain (SHA-256), persistence, replay; re-exports ISLS EvidenceChain
fsr-governance	1	15 invariants, Integrity FSM, Resource FSM
fsr-ttcp	2	TTCP engine (Tri-Carrier Phase Convergence, 3 levels)
fsr-tui	2	Terminal UI with ratatui (feature: tui)
fsr-runtime	1	CLI binary fsr: all commands, 24-step macro-cycle engine
fsr-dshae	4	DSHAE engine, HIM, crystal cascade, 7-scenario sandbox
fsr-gui	4	Desktop GUI (egui/eframe 0.29), all panels including Phase 5 tabs
fsr-isls	5	Intelligent Semantic Ledger Substrate: tiered storage, EvidenceChain, SemanticCrystal, PersistentGraph, resonant consensus
fsr-mcce	5	Mycelial Crypto-Cartography Engine: 4-layer HDAG (Spore/Hypha/Mycelium/Fruiting), Pearson correlation, petgraph
fsr-ecls	5	Emergent Constraint Lattice Spectroscopy: read-only scanner, 7 constraint templates, inverse weaving, thermodynamics
Quick Start
Requirements

Rust 1.78+ (stable)

Linux / macOS / Windows

For GUI: OpenGL-capable display (X11 or Wayland on Linux)

Build

# Entire workspace (without GUI — headless, no GUI deps required)
cargo build --workspace --exclude fsr-gui

# CLI binary only
cargo build --bin fsr

# GUI binary (Phase 4 + Phase 5 panels)
cargo build --bin fsr-gui

# With TUI feature
cargo build --bin fsr --features fsr-runtime/tui

# Release build
cargo build --release --workspace --exclude fsr-gui

Run tests

# All tests
cargo test --workspace

# DSHAE tests only (Phase 4)
cargo test -p fsr-dshae

# Phase 4 integration tests
cargo test -p fsr-chain --test phase4_integration

# Phase 5 integration tests (17 tests: ISLS, MCCE, ECLS, all 7 scenarios)
cargo test -p fsr-chain --test phase5_integration

# All 7 sandbox scenarios
cargo test -p fsr-chain --test phase5_integration test_all_7_scenarios_pass
