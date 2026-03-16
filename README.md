# FIXPOINT SWARM-R (AinSoft-R v5)

### "Topology of Klemm" (Greed)

---

Deterministisches Rust-Handelssystem mit Phasensynchronisation, DSHAE-Arbitrage-Engine zur holographischen Phasenraumprojektion, Intelligence Substrate (ISLS/MCCE/ECLS) und Desktop-GUI

---

"Niemand kann zwei Herren dienen: Entweder er wird den einen hassen und den andern lieben, oder er wird an dem einen hängen und den andern verachten. Ihr könnt nicht Gott dienen und dem Mammon.
Darum sage ich euch: Sorgt euch nicht um euer Leben, was ihr essen und trinken werdet; auch nicht um euren Leib, was ihr anziehen werdet. Ist nicht das Leben mehr als die Nahrung und der Leib mehr als die Kleidung?"
- Mt 6,24-25

---

## Inhaltsverzeichnis

1. [Überblick](#überblick)
2. [Architektur](#architektur)
3. [Crate-Verzeichnis](#crate-verzeichnis)
4. [Schnellstart](#schnellstart)
5. [Konfiguration](#konfiguration)
6. [CLI-Referenz](#cli-referenz)
7. [Phase-4-Komponenten](#phase-4-komponenten)
8. [Phase-5-Komponenten](#phase-5-komponenten)
9. [Validierungs-Sandbox](#validierungs-sandbox)
10. [Entwicklung](#entwicklung)
11. [Invarianten](#invarianten)
12. [Phasen-Übersicht](#phasen-übersicht)
13. [Lizenz](#lizenz)

---

## Überblick

FIXPOINT SWARM-R ist ein vollständig deterministisches, kettengesichertes Handelssystem, das in Rust implementiert ist. Es kombiniert:

- **Resonanz-Engine** (ψ/ρ/ω-Metriken) für Marktzustandsbewertung
- **TTCP-Kristallisierung** (Tri-Carrier-Phase-Konvergenz) für Handelssignale
- **DSHAE-Engine** (Dual-Simplex Holographic Arbitrage Engine) für Dreiecksarbitrage
- **Dual-Chain-Integrität** (Shadow + Commitment Chain, SHA-256-verkettete Events)
- **Intelligence Substrate** (ISLS + MCCE + ECLS) — persistentes topologisches Gedächtnis und Constraint-Erkennung (Phase 5)
- **Desktop-GUI** (egui/eframe) mit Live-Dashboard, Sandbox-Validierung und Mycelium-Visualisierung

Das System ist vollständig auditierbar: jede Zustandsänderung wird als unveränderliches Event in der Chain gespeichert. Ein deterministischer Replay mit identischen Inputs produziert Bit-für-Bit identische Outputs.

---

## Architektur

```
┌──────────────────────────────────────────────────────────────┐
│                  fsr-runtime (CLI: fsr)                       │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────────────┐  │
│  │ Macro-   │  │ DSHAE-   │  │  TTCP-Engine               │  │
│  │ Cycle    │  │ Bridge   │  │  (Kristalle)               │  │
│  │ (24 Schr)│  └──────────┘  └────────────────────────────┘  │
│  └────┬─────┘                                                 │
│       │ Phase 5: ISLS-PERSIST / MCCE-UPDATE / ECLS-SCAN      │
│  ┌────▼─────────────────────────────────────────────────────┐ │
│  │  Intelligence Substrate                                   │ │
│  │  fsr-isls (Ledger) │ fsr-mcce (Graph) │ fsr-ecls (Scan) │ │
│  └──────────────────────────────────────────────────────────┘ │
└───────────────────────┬──────────────────────────────────────┘
                        │ Arc<Mutex<GuiState>>
┌───────────────────────▼──────────────────────────────────────┐
│                fsr-gui (Desktop-GUI)                           │
│  Dashboard │ P&L │ Kristalle │ TTCP │ Risiko │ Config │      │
│  Sandbox   │ Mycelium (neu) │ Constraints (neu) │            │
│  Knowledge (neu)                                              │
└──────────────────────────────────────────────────────────────┘

Kern-Crates:
  fsr-types    ←─ gemeinsame Typen (Q32, EventTag, OrderBook …)
  fsr-fixed    ←─ Q32-Festkomma-Arithmetik (32.32, ONE = 1<<32)
  fsr-chain    ←─ Dual-Chain (Shadow + Commitment, SHA-256) + ISLS-Wrapper
  fsr-resonance ← Resonanz-Engine (SI, ψ, ρ, ω, κ, Entropie)
  fsr-gate     ←─ Kairos-Gate (Regime-FSM, Gamma-Score)
  fsr-ttcp     ←─ TTCP-Kristallisierung (3-Ebenen-Kaskade)
  fsr-dshae    ←─ DSHAE-Engine (Phase 4, Dreiecksarbitrage)
  fsr-isls     ←─ Intelligent Semantic Ledger Substrate (Phase 5)
  fsr-mcce     ←─ Mycelial Crypto-Cartography Engine (Phase 5)
  fsr-ecls     ←─ Emergent Constraint Lattice Spectroscopy (Phase 5)
  fsr-tui      ←─ Terminal-UI (ratatui)
```

### 24-Schritte-Makrozyklus

Jeder Tick durchläuft exakt 24 Schritte (Spec §7.2 + Phase-5-Erweiterung):

```
1.  OBSERVE       – OrderBooks einlesen
2.  NORMALIZE     – Preise normalisieren
3.  ISLS-PERSIST  – Observation in ISLS Hot-Tier schreiben          ← NEU Phase 5
4.  MCCE-UPDATE   – Mycelial HDAG aktualisieren                     ← NEU Phase 5
5.  EXTRACT       – Q32-Signale extrahieren
6.  TEMPORAL      – TriCarrier / TemporalKey
7.  RESOURCE      – Ressourcenbudget prüfen
8.  REGIME        – RegimeFSM fortschalten (Alpha/Beta/Gamma)
9.  INTEGRITY     – IntegritätsFSM (Healthy/Degraded/SafeHold)
9b. ECLS-SCAN     – Constraint-Scan (alle scan_interval Ticks)      ← NEU Phase 5
10. NULLCENTER    – Windnarbe-Gate
11. CANDIDATES    – Kandidaten filtern + Press top-k
12. CSP           – Lockstep-Zulässigkeit
13. EXECUTE       – Handelsausführung (Paper / Live)
14. CRYSTAL       – TTCP-Kristallerkennung
15. HEDGE         – HedgeFSM
16. EVIDENCE      – MacroCycleEnd in Shadow-Chain
17. MCCE-FRUITING – Mycelium-Signale emittieren                     ← NEU Phase 5
18. ISLS-CONSENSUS– Resonant-Consensus + Storage-Compaction         ← NEU Phase 5
19. CALIBRATE     – Förderungs-FSM
20. INVARIANTS    – 15 Invarianten prüfen
21. SAFE-HOLD     – Bei Verletzung einfrieren
22. ROTATE        – Chain-Rotation
23. STATUS        – StatusReport ausgeben
    + Phase 4: DSHAE-Tick nach Schritt 16
```

---

## Crate-Verzeichnis

| Crate | Phase | Beschreibung |
|-------|-------|-------------|
| `fsr-types` | 1 | Gemeinsame Typen: `Q32`, `EventTag`, `OrderBook`, FSM-States |
| `fsr-fixed` | 1 | Q32-Festkomma (32.32-Bit): `q32_mul`, `q32_ln`, `q32_from_ratio` |
| `fsr-temporal` | 1 | `TriCarrier`, `TemporalKey`, `KairosScheduler` |
| `fsr-nullcenter` | 1 | Windnarbe-Gate, NC-Zertifikat |
| `fsr-resonance` | 1 | Resonanz-Engine: SI, ψ, ρ, ω, κ, Entropie, Impuls |
| `fsr-gate` | 1 | `KairosGate`, `RegimeFsm` (Alpha/Beta/Gamma) |
| `fsr-mirror` | 1 | POR-FSM, MCI-Berechnung |
| `fsr-candidates` | 1 | Kandidatenfilterung, `wt_multiplex`, `filter_and_press` |
| `fsr-csp` | 1 | CSP-FSM, Lockstep-Zulässigkeit, Quorum |
| `fsr-hedge` | 1 | Hedge-FSM (Safe/Hedging/Unwind/Recovery) |
| `fsr-calibration` | 2 | Förderungs-Workflow, 5-Gate-Pipeline |
| `fsr-chain` | 1 | Dual-Chain (SHA-256), Persistenz, Replay; re-exportiert ISLS-EvidenceChain |
| `fsr-governance` | 1 | 15 Invarianten, Integrity-FSM, Resource-FSM |
| `fsr-ttcp` | 2 | TTCP-Engine (Tri-Carrier-Phase-Konvergenz, 3 Ebenen) |
| `fsr-tui` | 2 | Terminal-UI mit ratatui (Feature: `tui`) |
| `fsr-runtime` | 1 | CLI-Binary `fsr`: alle Befehle, 24-Schritte-Makrozyklus-Engine |
| `fsr-dshae` | 4 | DSHAE-Engine, HIM, Kristallkaskade, 7-Szenario-Sandbox |
| `fsr-gui` | 4 | Desktop-GUI (egui/eframe 0.29), alle Panels inkl. Phase-5-Tabs |
| `fsr-isls` | **5** | **Intelligent Semantic Ledger Substrate**: Tiered Storage, EvidenceChain, SemanticCrystal, PersistentGraph, Resonant Consensus |
| `fsr-mcce` | **5** | **Mycelial Crypto-Cartography Engine**: 4-Layer HDAG (Spore/Hypha/Mycelium/Fruiting), Pearson-Korrelation, petgraph |
| `fsr-ecls` | **5** | **Emergent Constraint Lattice Spectroscopy**: READ-ONLY Scanner, 7 Constraint-Templates, Inverse Weaving, Thermodynamik |

---

## Schnellstart

### Voraussetzungen

- Rust 1.78+ (Stable)
- Linux / macOS / Windows
- Für GUI: OpenGL-fähige Anzeige (X11 oder Wayland unter Linux)

### Bauen

```bash
# Gesamtes Workspace (ohne GUI — headless, keine GUI-Deps erforderlich)
cargo build --workspace --exclude fsr-gui

# Nur CLI-Binary
cargo build --bin fsr

# GUI-Binary (Phase 4 + Phase 5 Panels)
cargo build --bin fsr-gui

# Mit TUI-Feature
cargo build --bin fsr --features fsr-runtime/tui

# Release-Build
cargo build --release --workspace --exclude fsr-gui
```

### Tests ausführen

```bash
# Alle Tests
cargo test --workspace

# Nur DSHAE-Tests (Phase 4)
cargo test -p fsr-dshae

# Phase-4-Integrationstests
cargo test -p fsr-chain --test phase4_integration

# Phase-5-Integrationstests (17 Tests: ISLS, MCCE, ECLS, alle 7 Szenarien)
cargo test -p fsr-chain --test phase5_integration

# Alle 7 Sandbox-Szenarien
cargo test -p fsr-chain --test phase5_integration test_all_7_scenarios_pass
```

Aktueller Teststand: **228+ Tests, 0 Fehler** (Phase 1–4: 211 + Phase 5: 17).

---

## Konfiguration

Das System unterstützt vier eingebaute Profile sowie benutzerdefinierte YAML-Konfigurationen.

### Eingebaute Profile

| Profil | Beschreibung |
|--------|-------------|
| `conservative` | Niedrige Risikobereitschaft, enge Gates (Standard) |
| `balanced` | Ausgewogenes Verhältnis Rendite/Risiko |
| `aggressive` | Weite Gates, höhere Positionsgrößen |
| `minimal` | Minimale Konfiguration für Tests |

### YAML-Konfiguration

```yaml
# balanced.yaml – Beispielkonfiguration
profile: balanced
theta_open: 0.65
theta_close: 0.35
press_top_k: 8
tau_leak: 0.01
tau_xt: 0.02
regime_si_weaken: 0.3
regime_gamma_trigger: 0.7
calibration_window: 500
freshness_ttl: 100
```

Konfiguration laden:

```bash
fsr run --config balanced.yaml --ticks 10000
fsr gui --config balanced.yaml
```

---

## CLI-Referenz

### `fsr run` — Paper-Modus

```bash
fsr run [OPTIONEN]

Optionen:
  --ticks <N>            Anzahl der Ticks [Standard: 1000]
  --print-every <N>      Ausgabeintervall [Standard: 100]
  --output <FORMAT>      text | json [Standard: text]
  --persist              Persistenz aktivieren
  --data-dir <PFAD>      Datenpfad [Standard: data/]
  --snapshot-every <N>   Snapshot-Intervall [Standard: 100]
  --log-json             JSON-Logging
  --tui                  Terminal-UI aktivieren (benötigt --features tui)
  --hot-reload           Konfigurations-Hot-Reload
  --ttcp-dir <PFAD>      TTCP-Kristallverzeichnis

Beispiele:
  fsr run --ticks 5000 --persist --tui
  fsr run --config balanced.yaml --output json --ticks 1000
```

### `fsr run-live` — Live-Modus

```bash
fsr run-live --venue <binance|kraken> [--record] [--sniper] [--ticks <N>]

# Benötigt: cargo build --features live-data
# Ohne Feature: Fallback auf Paper-Modus
```

### `fsr replay-historical` — Historischer Replay

```bash
fsr replay-historical --recording data/rec_001.rec [--output data/backtest/run1]
```

### `fsr promote-cmd` — Förderungs-Workflow (5 Gates)

```bash
fsr promote-cmd --recording data/recordings/*.rec   # Gates prüfen
fsr promote-cmd --approve <proposal-id>              # Genehmigen
fsr promote-cmd --status                             # Status anzeigen
```

### `fsr gui` — Desktop-GUI (Phase 4 + Phase 5)

```bash
fsr gui --sandbox                    # Sandbox-Validierung (7 Szenarien)
fsr gui --config balanced.yaml       # Live-Dashboard im Paper-Modus
fsr gui --sandbox --width 1920 --height 1080

# Direkt (ohne fsr-Wrapper):
fsr-gui --sandbox
fsr-gui --config balanced.yaml --profile balanced
```

### Weitere Befehle

```bash
fsr status                    # Einzelner Tick, Statusausgabe
fsr verify                    # Chain-Integrität prüfen
fsr invariants                # Alle 15 Invarianten prüfen
fsr benchmark --ticks 10000   # Kalibrierungs-Benchmark
fsr config --yaml             # Aktive Konfiguration als YAML ausgeben
fsr validate-config           # YAML-Konfiguration validieren
fsr backtest-report --run <run-id>  # Backtest-Bericht anzeigen
```

---

## Phase-4-Komponenten

### DSHAE-Engine (`fsr-dshae`)

Die **Dual-Simplex Holographic Arbitrage Engine** erkennt Dreiecksarbitrage-Gelegenheiten in einem n-Währungs-Korb.

#### Mathematische Grundlagen

**Kreuzraten-Tensor** `r_ij`:
```
r_ij = mid_bp / RATE_SCALE     (RATE_SCALE = 10_000)
```

**Deviations-Tensor** `D_ijk`:
```
D_ijk = ln(r_ij) + ln(r_jk) − ln(r_ik)
```
Im Gleichgewicht (kein Arbitrage): `D_ijk = 0`.

**Alpha/Beta-Zyklen** (Dual-Simplex):
```
p_α = ln(r_ij) + ln(r_jk) + ln(r_ki)    (Vorwärts-Dreieck)
p_β = ln(r_ik) + ln(r_kj) + ln(r_ji)    (Rückwärts-Dreieck)
Anti-Phasen-Filter: |p_α + p_β| ≤ τ_anti
```

**Holographisches Interferenz-Manifold (HIM)**: Für jedes ungeordnete Dreieck `{i,j,k}` mit `i < j < k` wird ein 5D-Punkt erzeugt:
```
x1 = ln(r_ij),  x2 = ln(r_jk),  x3 = D_ijk,
ψ  = Phasendifferential,  ω = Korb-Phasenwinkel
```
Anzahl der Punkte: `C(n,3) = n·(n−1)·(n−2)/6`

**DK/WT/Pi/Press-Kaskade**:
1. **DK** (Kontraktion): `net_edge · dk_rate > 0`
2. **WT** (Wavelet-Gewichtung): Score += `ψ/4`
3. **Π** (Schwellwert-Filter): `net_edge ≥ τ_edge`  (Standard: 5 bp)
4. **Press** (Top-k): Behalte die k besten nach Score

**Axle-Invariante**: Für jede Dreiecksposition gilt: Nettoexposition jeder Währung = 0 (geschlossene Schleife).

### Desktop-GUI (`fsr-gui`)

Die GUI kommuniziert über `Arc<Mutex<GuiState>>` mit dem Engine-Thread.

#### Tabs

| Tab | Phase | Inhalt |
|-----|-------|--------|
| **Dashboard** | 1–4 | FSM-Zustände, Resonanzmetriken, Gate-Status, Event-Log |
| **P&L-Chart** | 2 | Kumulativer Gewinn/Verlust, TTCP-Delta-Verläufe |
| **Kristalle** | 4 | DSHAE-Kristall-Tabelle, HIM-Streudiagramm |
| **TTCP** | 2 | Konvergenz-Score, 3-Ebenen-Kaskade |
| **Risiko** | 1 | Drawdown, Win-Rate, Chain-Integrität |
| **Config** | 1 | Geladene YAML-Konfiguration |
| **Sandbox** | 4 | 7-Szenario-Validierung mit PASS/FAIL |
| **Mycelium** | **5** | HDAG-Dot-Plot (Vertices/Edges), Cluster-Stats, Lernverlauf |
| **Constraints** | **5** | Aktive Constraints (ECLS), Lattice-Crystal-Log, Breaking-Alerts |
| **Knowledge** | **5** | ISLS Tier-Stats, Beobachtungen, Semantic Crystals, Graph-Wachstum |

---

## Phase-5-Komponenten

### ISLS — Intelligent Semantic Ledger Substrate (`fsr-isls`)

ISLS ist die persistente Wissensgrundlage des Systems. Es speichert alle Beobachtungen append-only in drei Speicher-Tiers (ISLS Axiom 3.3):

```
Hot  → Warm  → Cold
(letzte 3600 Ticks) → (90 Tage) → (Archiv)
```

**Kern-Typen:**

| Typ | Beschreibung |
|-----|-------------|
| `EntityId = u64` | Kanonische Entitäts-ID (DefaultHasher) |
| `EvidenceChain` | SHA-256-verkettete Ereigniskette (identische Digest-Funktion wie HashChain) |
| `PersistentGraph` | Vertex/Edge-Store für MCCE-HDAG |
| `SemanticCrystal` | Kondensiertes topologisches Wissens-Fragment |
| `TieredStorage` | Hot/Warm/Cold-Verwaltung mit `compact()` |
| `IslsPersistence` | Top-Level-Koordinator (Graph + Storage + Crystals + 2 Chains) |

**Resonant Consensus:**
```rust
score = stability × coherence × evidence   // Q32-Dreifach-Multiplikation
if score >= commit_threshold (¾):          // Commit → SemanticCrystal
    Commit(proof)
else:
    Defer
```

**fsr-chain Thin-Wrapper**: `fsr-chain` re-exportiert `EvidenceChain`, `compute_genesis` und `compute_digest` aus `fsr-isls`. `HashChain` und `EvidenceChain` produzieren für gleiche Inputs identische Digests.

---

### MCCE — Mycelial Crypto-Cartography Engine (`fsr-mcce`)

MCCE ist das Langzeitgedächtnis der Plattform. Je länger es läuft, desto präziser werden DSHAE-Signale (MCCE-Leverage-Prinzip).

#### 4-Schichten-Architektur

```
Eingabe (OrderBooks / Preise)
        │
        ▼
┌───────────────────┐
│  Spore Layer      │  Auto-Discovery: erzeugt Vertices (Token/Exchange/Pool)
└────────┬──────────┘
         │
         ▼
┌───────────────────┐
│  Hypha Layer      │  Pearson-Korrelation mit Exponential-Decay
│                   │  ρ_new = α·ρ_alt + (1-α)·ρ_aktuell
│                   │  weight *= (1 - decay_rate) pro Tick
└────────┬──────────┘
         │
         ▼
┌───────────────────┐
│  Mycelium Layer   │  TTCP-Triangulation (O(n³), max 50 Vertices)
│                   │  Union-Find Cluster-Erkennung
└────────┬──────────┘
         │
         ▼
┌───────────────────┐
│  Fruiting Layer   │  Signal-Emission: StableCluster / PersistentTriangle
│                   │  CorrelationDecay / GraphGrowth
└───────────────────┘
```

**5D-Embedding** pro Vertex: `[ψ, ρ, ω, momentum, entropy]`

**Konfiguration (`McceConfig`):**
```rust
McceConfig {
    enabled: true,
    hypha_window_ticks: 500,       // Korrelations-Fenster
    hypha_min_rho: ONE * 3 / 10,  // Mindeskorrelation (0.3)
    hypha_decay_rate: ONE / 200,   // Decay pro Tick (0.5%)
    embedding_update_interval: 10,
    fruiting_interval: 100,        // Signal-Emission alle 100 Ticks
}
```

**Hinweis**: `petgraph = "0.6"` ist ausschließlich in `fsr-mcce` als Abhängigkeit vorhanden.

---

### ECLS — Emergent Constraint Lattice Spectroscopy (`fsr-ecls`)

ECLS scannt das MCCE-HDAG **READ-ONLY** auf emergente Marktconstraints.

#### Constraint-Templates

| Template | Phase 5 aktiv | Beschreibung |
|----------|:---:|-------------|
| `Band` | ✓ | Preisband um k·σ |
| `Ratio` | ✓ | Stabiles Preisverhältnis zwischen zwei Assets |
| `Correlation` | ✓ | Pearson-Korrelation ≥ ρ_target ± δ |
| `Topological` | ✓ | Betti-Zahl einer Cluster-Komponente |
| `PhaseLock` | ✓ | Synchrone Bewegungsmuster |
| `Granger` | ✗ (Phase 6) | Granger-Kausalität (zurückgestellt) |
| `Spectral` | ✗ (Phase 6) | Spektralkohärenz (zurückgestellt) |

**Inverse Weaving**: Konsistente Constraint-Kandidaten werden zu `LatticeCrystal`-Gruppen zusammengeführt. Die thermodynamische freie Energie bewertet die Stabilität:

```
F = (1 − mean_stability) × ONE
```

**ECLS-Signale**: `StableLattice` | `ConstraintBreaking` | `NewConstraintCandidate`

---

### Phase-5 EventTags

14 neue Tags wurden additiv an die bestehende `EventTag`-Enum angehängt (niemals umgeordnet):

```rust
IslsObservationWritten, IslsConsensusCommit, IslsConsensusDefer,
IslsSemanticCrystalFormed, IslsStorageCompacted,
McceVertexDiscovered, McceEdgeCreated, McceTriangleDetected,
McceClusterFormed, McCeFruitingSignal,
EclsConstraintDiscovered, EclsConstraintBreaking,
EclsLatticeCrystalFormed, EclsScanCompleted
```

---

## Validierungs-Sandbox

Die Sandbox enthält **7 vorberechnete Szenarien** zur DSHAE-Validierung. Alle Szenarien sind vollständig deterministisch (kein Netzwerkzugriff, kein Paper-Modus erforderlich).

### Szenarien

| Nr. | Name | Beschreibung | Erwartetes Ergebnis |
|-----|------|-------------|---------------------|
| 1 | **Calm** | Keine Arbitrage, ruhige Preise | 0 Kristalle |
| 2 | **SingleArb** | Einmalige 10-bp-Arbitrage bei Tick 2500 | 1–3 Kristalle |
| 3 | **RecurringArb** | 5 Arb-Fenster (3,5,8,12,15 bp) | 3–8 Kristalle |
| 4 | **Noisy** | Sub-2-bp-Rauschen, kein echter Arb | 0 Kristalle (Falsch-Positiv-Test) |
| 5 | **RegimeShift** | Volatilitätswechsel + 2 Arb-Events | ≥2 Kristalle |
| 6 | **Correlation** | Phase 5: Korrelation → Dekorrelation (20.000 Ticks) | 0 DSHAE-Kristalle |
| 7 | **Lattice** | Phase 5: 4-Asset-Korb mit 8-bp-Arb bei Tick 10000 | 1–4 Kristalle |

### Bewertungskriterien (je Szenario)

1. `crystals_in_range` — Kristallzahl im erwarteten Bereich
2. `trades_not_exceeded` — Handelsanzahl unter Maximum
3. `no_false_positives` — Keine Falsch-Positive
4. `invariant_violations_zero` — Keine Axle-Invarianten-Verletzungen
5. `replay_determinism` — Zwei Durchläufe liefern identische Ergebnisse

### Sandbox programmatisch ausführen

```rust
use fsr_dshae::{SandboxRunner, Scenario};

let runner = SandboxRunner::with_default_config();

// Einzelnes Szenario
let result = runner.run_scenario(Scenario::Lattice);
assert!(result.passed);
println!("Kristalle: {}", result.actual_crystals);

// Alle 7 Szenarien
for scenario in [Calm, SingleArb, RecurringArb, Noisy, RegimeShift, Correlation, Lattice] {
    let r = runner.run_scenario(scenario);
    println!("{}: {}", r.scenario, if r.passed { "PASS" } else { "FAIL" });
}
```

### Fixture-Dateien

Die `.rec`-Dateien unter `fixtures/sandbox/` werden beim ersten Build automatisch durch `fsr-dshae/build.rs` erzeugt:

```
fixtures/sandbox/
  scenario_calm_000.rec
  scenario_arb_single_000.rec
  scenario_arb_recurring_000.rec
  scenario_noisy_000.rec
  scenario_regime_shift_000.rec
  scenario_correlation_000.rec      ← Phase 5 (20.000 Ticks)
  scenario_lattice_000.rec          ← Phase 5 (20.000 Ticks)
```

---

## Entwicklung

### Workspace-Struktur

```
fixpoint/
├── Cargo.toml               # Workspace-Root (21 Crates)
├── Cargo.lock
├── README.md
├── fixtures/
│   └── sandbox/             # Sandbox-.rec-Dateien (auto-generiert)
└── crates/
    ├── fsr-types/           # Gemeinsame Typen
    ├── fsr-fixed/           # Q32-Arithmetik
    ├── fsr-temporal/        # TriCarrier, TemporalKey
    ├── fsr-nullcenter/      # Windnarbe-Gate
    ├── fsr-resonance/       # Resonanz-Engine
    ├── fsr-gate/            # KairosGate, RegimeFSM
    ├── fsr-mirror/          # POR-FSM, MCI
    ├── fsr-candidates/      # Kandidatenfilter
    ├── fsr-csp/             # CSP-FSM
    ├── fsr-hedge/           # Hedge-FSM
    ├── fsr-calibration/     # Förderungs-Workflow
    ├── fsr-chain/           # Dual-Chain + ISLS-Thin-Wrapper
    ├── fsr-governance/      # 15 Invarianten
    ├── fsr-ttcp/            # TTCP-Engine
    ├── fsr-tui/             # Terminal-UI
    ├── fsr-runtime/         # CLI-Binary (fsr), 24-Schritte-Engine
    ├── fsr-dshae/           # DSHAE-Engine (Phase 4)
    ├── fsr-gui/             # Desktop-GUI (Phase 4 + Phase 5 Panels)
    ├── fsr-isls/            # Intelligence Substrate: Ledger (Phase 5)
    ├── fsr-mcce/            # Intelligence Substrate: Graph (Phase 5)
    └── fsr-ecls/            # Intelligence Substrate: Scanner (Phase 5)
```

### Q32-Festkomma-Arithmetik

Das gesamte System verwendet Q32-Festkomma (32.32-Bit, vorzeichenbehaftet 64-Bit):

```rust
// ONE = 1 << 32 ≈ 4_294_967_296
use fsr_fixed::{ONE, q32_mul, q32_ln, q32_from_ratio, q32_to_f64_display_only};

let half = ONE / 2;                          // 0.5 in Q32
let x = q32_from_ratio(9200, 10000);        // 0.92 in Q32
let y = q32_mul(x, ONE / 2);               // 0.46 in Q32
let l = q32_ln(x);                          // ln(0.92) in Q32
let f = q32_to_f64_display_only(x);        // nur zur Anzeige!
```

**Wichtig**: `q32_to_f64_display_only` darf **nie** für Berechnungen verwendet werden — nur zur Ausgabe. Alle internen Berechnungen laufen in Q32.

### Neue Crate hinzufügen

Das Workspace verwendet `resolver = "2"`. Neue Crates:

1. `crates/<name>/Cargo.toml` erstellen mit `version.workspace = true`
2. In `Cargo.toml` (Root) unter `members` eintragen
3. Abhängigkeiten nach Möglichkeit als `workspace.dependencies` deklarieren

### Features

| Feature | Crate | Beschreibung |
|---------|-------|-------------|
| `tui` | `fsr-runtime` | Terminal-UI (ratatui + crossterm) |
| `live-data` | `fsr-runtime` | Live WebSocket (Binance/Kraken) |
| `live-exec` | `fsr-runtime` | Live-Orderausführung (impliziert live-data) |

```bash
# Mit TUI
cargo build --bin fsr --features fsr-runtime/tui

# Mit Live-Daten
cargo build --bin fsr --features fsr-runtime/live-data
```

### Headless-Build (CI/Server)

Der headless Build schließt `fsr-gui` aus und zieht keine GUI-Abhängigkeiten (egui, eframe) heran:

```bash
cargo build --workspace --exclude fsr-gui
cargo test --workspace --exclude fsr-gui
```

---

## Invarianten

Das System prüft bei jedem Tick 15 Invarianten (Spec §8):

| Nr. | Invariante |
|-----|-----------|
| INV-01 | NC-Zertifikat vorhanden vor Aktivierung |
| INV-02 | Event bei jedem FSM-Übergang emittiert |
| INV-03 | TemporalKey gültig |
| INV-04 | Quorum-Prüfungen abgeschlossen |
| INV-05 | Paper-Validierung vor Live-Betrieb |
| INV-06 | Promotions-Gate bestanden |
| INV-07 | Replay-Divergenz offengelegt |
| INV-08 | Kein versteckter mutabler Zustand |
| INV-09 | Shadow-Chain konsistent |
| INV-10 | Commitment-Chain konsistent |
| INV-11 | Schema-Migration erhält Abstammung |
| INV-12 | Hedge-Leverage innerhalb Grenzwert |
| INV-13 | Harter Filter nicht durch Ranking gerettet |
| INV-14 | Safe-Hold friert Ausführung ein |
| INV-15 | Ressourcen schwächen Gates nicht |

```bash
# Invarianten manuell prüfen
fsr invariants
```

### DSHAE-Axle-Invariante

Zusätzlich zu den 15 System-Invarianten erzwingt die DSHAE-Engine die **Axle-Invariante** für jede Dreiecksposition:

```
Σ Nettoexposition[Währung i] = 0  ∀ i ∈ {0..n-1}
```

Toleranz: `ε = ONE / 1000` (0.1% in Q32).

### ISLS Axiom 3.3 (Phase 5)

Alle drei Speicher-Tiers (Hot/Warm/Cold) sind **append-only**. Kein Datensatz wird jemals gelöscht oder überschrieben — nur von Hot nach Warm nach Cold befördert.

---

## Phasen-Übersicht

| Phase | Version | Crates | Inhalt |
|-------|---------|--------|--------|
| Phase 1 | v1.0 | 13 | Kern-Engine, Chain, Resonanz, 15 Invarianten |
| Phase 2 | v2.0 | +2 | Persistenz, TUI, TTCP, Hot-Reload, JSON-Logging |
| Phase 3 | v3.1 | — | Live-Venues, Sniper-Modus, Backtest, Förderungs-Workflow |
| Phase 4 | v4.0 | +2 | DSHAE-Engine, HIM, Sandbox (5 Szenarien), Desktop-GUI |
| Phase 5 | v5.0 | **+3** | **Intelligence Substrate: ISLS + MCCE + ECLS, 24-Schritte-Zyklus, 7-Szenario-Sandbox, 3 neue GUI-Tabs** |

---

## Lizenz

MIT — siehe [LICENSE](LICENSE)

---

*FIXPOINT SWARM-R ist ein Forschungs- und Bildungssystem. Es dient nicht als Finanzberatung. Der Einsatz im Live-Handel erfolgt auf eigenes Risiko.*
