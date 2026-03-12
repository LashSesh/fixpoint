# FIXPOINT SWARM:

---

### "The Topology of Greed"

---

Deterministisches Rust-Handelssystem mit Phasensynchronisation, DSHAE-Arbitrage-Engine zur holographischen Phasenraumprojektion und Desktop-GUI

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
8. [Validierungs-Sandbox](#validierungs-sandbox)
9. [Entwicklung](#entwicklung)
10. [Invarianten](#invarianten)
11. [Lizenz](#lizenz)

---

## Überblick

FIXPOINT SWARM ist ein vollständig deterministisches, kettengesichertes Handelssystem, das in Rust implementiert ist. Es kombiniert:

- **Resonanz-Engine** (ψ/ρ/ω-Metriken) für Marktzustandsbewertung
- **TTCP-Kristallisierung** (Tri-Carrier-Phase-Konvergenz) für Handelssignale
- **DSHAE-Engine** (Dual-Simplex Holographic Arbitrage Engine) für Dreiecksarbitrage
- **Dual-Chain-Integrität** (Shadow + Commitment Chain, SHA-256-verkettete Events)
- **Desktop-GUI** (egui/eframe) mit Live-Dashboard und Sandbox-Validierung

Das System ist vollständig auditierbar: jede Zustandsänderung wird als unveränderliches Event in der Chain gespeichert. Ein deterministischer Replay mit identischen Inputs produziert Bit-für-Bit identische Outputs.

---

## Architektur

```
┌─────────────────────────────────────────────────────┐
│                  fsr-runtime (CLI: fsr)              │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────┐ │
│  │ Macro-   │  │ DSHAE-   │  │  TTCP-Engine       │ │
│  │ Cycle    │  │ Bridge   │  │  (Kristalle)       │ │
│  └──────────┘  └──────────┘  └────────────────────┘ │
└───────────────────────┬─────────────────────────────┘
                        │ Arc<Mutex<GuiState>>
┌───────────────────────▼─────────────────────────────┐
│                fsr-gui (Desktop-GUI)                  │
│  Dashboard │ P&L │ Kristalle │ TTCP │ Risiko │ Sandbox│
└─────────────────────────────────────────────────────┘

Kern-Crates:
  fsr-types    ←─ gemeinsame Typen (Q32, EventTag, OrderBook …)
  fsr-fixed    ←─ Q32-Festkomma-Arithmetik (32.32, ONE = 1<<32)
  fsr-chain    ←─ Dual-Chain (Shadow + Commitment, SHA-256)
  fsr-resonance ← Resonanz-Engine (SI, ψ, ρ, ω, κ, Entropie)
  fsr-gate     ←─ Kairos-Gate (Regime-FSM, Gamma-Score)
  fsr-ttcp     ←─ TTCP-Kristallisierung (3-Ebenen-Kaskade)
  fsr-dshae    ←─ DSHAE-Engine (Phase 4, Dreiecksarbitrage)
  fsr-tui      ←─ Terminal-UI (ratatui)
```

### 20-Schritte-Makrozyklus

Jeder Tick durchläuft exakt 20 Schritte (Spec §7.2):

```
1. OBSERVE      – OrderBooks einlesen
2. NORMALIZE    – Preise normalisieren
3. EXTRACT      – Q32-Signale extrahieren
4. TEMPORAL     – TriCarrier / TemporalKey
5. RESOURCE     – Ressourcenbudget prüfen
6. REGIME       – RegimeFSM fortschalten (Alpha/Beta/Gamma)
7. INTEGRITY    – IntegritätsFSM (Healthy/Degraded/SafeHold)
8. NULLCENTER   – Windnarbe-Gate
9. CANDIDATES   – Kandidaten filtern + Press top-k
10. CSP         – Lockstep-Zulässigkeit
11. EXECUTE     – Handelsausführung (Paper / Live)
12. PRESS       – (bereits in 9)
13. CRYSTAL     – TTCP-Kristallerkennung
14. HEDGE       – HedgeFSM
15. EVIDENCE    – MacroCycleEnd in Shadow-Chain
16. CALIBRATE   – Förderungs-FSM
17. INVARIANTS  – 15 Invarianten prüfen
18. SAFE-HOLD   – Bei Verletzung einfrieren
19. ROTATE      – Chain-Rotation
20. STATUS      – StatusReport ausgeben
    + Phase 4: DSHAE-Tick nach Schritt 15
```

---

## Crate-Verzeichnis

| Crate | Beschreibung |
|-------|-------------|
| `fsr-types` | Gemeinsame Typen: `Q32`, `EventTag`, `OrderBook`, FSM-States |
| `fsr-fixed` | Q32-Festkomma (32.32-Bit): `q32_mul`, `q32_ln`, `q32_from_ratio` |
| `fsr-temporal` | `TriCarrier`, `TemporalKey`, `KairosScheduler` |
| `fsr-nullcenter` | Windnarbe-Gate, NC-Zertifikat |
| `fsr-resonance` | Resonanz-Engine: SI, ψ, ρ, ω, κ, Entropie, Impuls |
| `fsr-gate` | `KairosGate`, `RegimeFsm` (Alpha/Beta/Gamma) |
| `fsr-mirror` | POR-FSM, MCI-Berechnung |
| `fsr-candidates` | Kandidatenfilterung, `wt_multiplex`, `filter_and_press` |
| `fsr-csp` | CSP-FSM, Lockstep-Zulässigkeit, Quorum |
| `fsr-hedge` | Hedge-FSM (Safe/Hedging/Unwind/Recovery) |
| `fsr-calibration` | Förderungs-Workflow, 5-Gate-Pipeline |
| `fsr-chain` | Dual-Chain (SHA-256), Persistenz, Replay |
| `fsr-governance` | 15 Invarianten, Integrity-FSM, Resource-FSM |
| `fsr-ttcp` | TTCP-Engine (Tri-Carrier-Phase-Konvergenz, 3 Ebenen) |
| `fsr-tui` | Terminal-UI mit ratatui (Feature: `tui`) |
| `fsr-runtime` | CLI-Binary `fsr`: alle Befehle, Makrozyklus-Engine |
| `fsr-dshae` | **Phase 4**: DSHAE-Engine, HIM, Kristallkaskade, Sandbox |
| `fsr-gui` | **Phase 4**: Desktop-GUI (egui/eframe), alle Panels |

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

# GUI-Binary
cargo build --bin fsr-gui

# Mit TUI-Feature
cargo build --bin fsr --features fsr-runtime/tui

# Release-Build
cargo build --release --workspace --exclude fsr-gui
```

### Tests ausführen

```bash
# Alle Tests (ohne GUI)
cargo test --workspace --exclude fsr-gui

# Nur DSHAE-Tests
cargo test -p fsr-dshae

# Phase-4-Integrationstests
cargo test -p fsr-chain --test phase4_integration

# Einzelnen Test ausführen
cargo test -p fsr-dshae test_all_5_scenarios_pass
```

Aktueller Teststand: **211 Tests, 0 Fehler** (176 Phase 1–3 + 35 Phase 4).

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

### `fsr gui` — Desktop-GUI (Phase 4)

```bash
fsr gui --sandbox                    # Sandbox-Validierung (5 Szenarien)
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

#### Achsen-Konfiguration (`DshaeConfig`)

```rust
DshaeConfig {
    enabled: false,          // Im Shadow-Modus standardmäßig deaktiviert
    mode: DshaeMode::Shadow,
    basket: BasketConfig {
        min_basket_size: 3,
        max_basket_size: 8,
    },
    dual_simplex: DualSimplexConfig {
        cycle_length: 3,
        notional_per_leg: 1000 * ONE,    // Q32
        anti_phase_tolerance: ONE/100,    // 1%
        rebalance_window: 5,
    },
    holographic: HolographicConfig {
        q32_scale: 1_000_000,
        min_points: 4,
    },
    cascade: CascadeConfig {
        max_depth: 3,
        dk_contraction_rate: 0.85,        // Q32-Faktor
        press_top_k: 16,
    },
    crystal: CrystalConfig {
        tau_edge_bp: 5,                   // 5 Basispunkte Mindestkante
        max_age_ticks: 3,
        max_concurrent: 2,
    },
}
```

### Desktop-GUI (`fsr-gui`)

Die GUI kommuniziert über `Arc<Mutex<GuiState>>` mit dem Engine-Thread.

#### Tabs

| Tab | Inhalt |
|-----|--------|
| **Dashboard** | FSM-Zustände, Resonanzmetriken, Gate-Status, Event-Log |
| **P&L-Chart** | Kumulativer Gewinn/Verlust, TTCP-Delta-Verläufe |
| **Kristalle** | DSHAE-Kristall-Tabelle, HIM-Streudiagramm |
| **TTCP** | Konvergenz-Score, 3-Ebenen-Kaskade |
| **Risiko** | Drawdown, Win-Rate, Chain-Integrität |
| **Config** | Geladene YAML-Konfiguration |
| **Sandbox** | 5-Szenario-Validierung mit PASS/FAIL |

#### GUI starten

```bash
# Sandbox-Validierung
fsr-gui --sandbox

# Live-Dashboard (Paper-Modus)
fsr-gui --config balanced.yaml

# Über fsr-Wrapper
fsr gui --sandbox
fsr gui --config balanced.yaml
```

---

## Validierungs-Sandbox

Die Sandbox enthält 5 vorberechnete Szenarien zur DSHAE-Validierung. Alle Szenarien sind vollständig deterministisch (kein Netzwerkzugriff, kein Paper-Modus erforderlich).

### Szenarien

| Nr. | Name | Beschreibung | Erwartetes Ergebnis |
|-----|------|-------------|---------------------|
| 1 | **Calm** | Keine Arbitrage, ruhige Preise | 0 Kristalle |
| 2 | **SingleArb** | Einmalige 10-bp-Arbitrage bei Tick 2500 | 1–4 Kristalle |
| 3 | **RecurringArb** | 5 Arb-Fenster (3,5,8,12,15 bp) | 3–8 Kristalle |
| 4 | **Noisy** | Sub-2-bp-Rauschen, kein echter Arb | 0 Kristalle (Falsch-Positiv-Test) |
| 5 | **RegimeShift** | Volatilitätswechsel + 2 Arb-Events | 2–6 Kristalle |

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
let result = runner.run_scenario(Scenario::SingleArb);
assert!(result.passed);
println!("Kristalle: {}", result.actual_crystals);

// Alle 5 Szenarien
for scenario in [Calm, SingleArb, RecurringArb, Noisy, RegimeShift] {
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
```

---

## Entwicklung

### Workspace-Struktur

```
fixpoint/
├── Cargo.toml               # Workspace-Root
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
    ├── fsr-chain/           # Dual-Chain
    ├── fsr-governance/      # 15 Invarianten
    ├── fsr-ttcp/            # TTCP-Engine
    ├── fsr-tui/             # Terminal-UI
    ├── fsr-runtime/         # CLI-Binary (fsr)
    ├── fsr-dshae/           # DSHAE-Engine (Phase 4)
    └── fsr-gui/             # Desktop-GUI (Phase 4)
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

---

## Phasen-Übersicht

| Phase | Version | Inhalt |
|-------|---------|--------|
| Phase 1 | v1.0 | Kern-Engine, Chain, Resonanz, 15 Invarianten |
| Phase 2 | v2.0 | Persistenz, TUI, TTCP, Hot-Reload, JSON-Logging |
| Phase 3 | v3.1 | Live-Venues, Sniper-Modus, Backtest, Förderungs-Workflow |
| Phase 4 | v4.0 | DSHAE-Engine, HIM, Sandbox, Desktop-GUI |

---

## Lizenz

MIT — siehe [LICENSE](LICENSE)

---

*FIXPOINT SWARM-R ist ein Forschungs- und Bildungssystem. Es dient nicht als Finanzberatung. Der Einsatz im Live-Handel erfolgt auf eigenes Risiko.*
