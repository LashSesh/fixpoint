# Architektur: Deterministisches Rohstoff-Handelssystem auf IBKR

**Auf Basis von FIXPOINT SWARM-R v3.0.0 ("The Topology of Greed")**

Forschungslauf — Machbarkeitsanalyse & Zielarchitektur
Stand: 2026-06-17 · Branch: `claude/custom-system-architecture-pqc49t`

---

## 0. Executive Verdict

**fixpoint ist eine außergewöhnlich starke Grundlage — aber nicht aus den Gründen, die auf den ersten Blick einleuchten.**

Der Wert liegt **nicht** in der DSHAE-Arbitrage-Engine (die ist krypto-triangel-spezifisch und größtenteils Wegwerf für Rohstoffe). Der Wert liegt in der **asset-agnostischen Intelligenz-, Sicherheits- und Determinismus-Infrastruktur**: Resonanz-Bewertung, Kairos-Gating, Nullcenter-Admissibility, die Commitment-FSM, Hedge-/Governance-Invarianten, die Hash-Chain-Auditierbarkeit und das persistente Lernsubstrat (ISLS/MCCE/ECLS). Das ist genau das, was jemand, der "von Grund auf" baut, **nie sauber hinbekommt** — und was hier bereits formal verifizierbar existiert.

Ehrliche, code-verifizierte Bilanz:

| Kategorie | Anteil (grob) | Status |
|---|---|---|
| Wiederverwendbar (Brain + Safety + Memory + Determinismus) | ~14 von 21 Crates | **Direkt nutzbar**, asset-agnostisch |
| Adaptierbar (Markt-Datentypen, Engine-Zyklus, Candidates) | ~3 Crates | Erweitern, nicht neu schreiben |
| Ersetzen (DSHAE-Triangel-Engine) | 1 Crate | Neue Spread-Engine; Crack/Crush erben 3-Bein-Maschinerie |
| **Komplett neu bauen** | — | **IBKR-Adapter + Order-Management-System (OMS)** |

**Die wichtigste ehrliche Warnung:** Das System hat **heute keinen funktionierenden Execution-Pfad** — auch nicht für Krypto. `BinanceExecutor` ist ein Stub ohne Signierung, nicht in den Makrozyklus verdrahtet. Der gesamte Order-Lebenszyklus (Submission, Fill-Tracking, Teilausführungen, Rejects, Reconnect, Positions-/PnL-Abgleich) muss neu und sicherheitskritisch gebaut werden. Das ist der größte Einzelbrocken und steht zwischen "elegante Forschungsmaschine" und "Programm, das echtes Geld auf Rohstoffe setzt".

**Fazit:** Machbar und lohnend — vorausgesetzt, man versteht, dass man die *Gehirn-, Gedächtnis- und Sicherheitsschicht erntet* und die *Hände (Execution) sowie die domänenspezifischen Augen (Spread-Signale)* neu baut.

---

## 1. Was fixpoint wirklich ist

FIXPOINT SWARM-R ist ein vollständig deterministisches Rust-Handelssystem (21 Crates, 5 Phasen), das jeden Zustandsübergang in zwei Hash-Ketten (Shadow + Commitment) unveränderlich festschreibt. Gleiche Inputs → bit-für-bit gleiche Outputs (deterministischer Replay). Kein Float in Entscheidungspfaden — alles in Q32 (32.32 Fixed-Point).

**Der konzeptionelle Kern** ist ein 24-Schritt-Makrozyklus, der pro Tick durchläuft:

```
OBSERVE → NORMALIZE → PERSIST → LEARN(MCCE) → EXTRACT(ψ/ρ/ω) → TEMPORAL →
RESOURCE → REGIME → INTEGRITY → ECLS-SCAN → NULLCENTER → CANDIDATES →
CSP → EXECUTE → CRYSTAL(TTCP) → HEDGE → EVIDENCE → DSHAE-TICK →
MCCE-FRUITING → ISLS-CONSENSUS → CALIBRATE → INVARIANTS → SAFE-HOLD → STATUS
```

Die entscheidende Eigenschaft: **Die meisten dieser Schritte wissen nichts über Krypto.** Sie operieren auf abstrakten Größen — Kohärenz (κ), Entropie (H), Spiral-Index (SI), dem Bewertungstripel Φ = (ψ Qualität, ρ Stabilität, ω Effizienz). Das ist eine generische Regelungstheorie-Maschine, die zufällig auf Krypto-Orderbüchern läuft.

---

## 2. Code-verifizierte Machbarkeitskarte

Diese Tabelle basiert auf direkter Quellcode-Inspektion, nicht auf Annahmen.

### 2.1 Direkt wiederverwendbar (asset-agnostisch)

| Crate | Was es liefert | Verifizierter Befund |
|---|---|---|
| `fsr-fixed` | Q32-Arithmetik (mul/div/ln/tanh) | Keine Asset-Annahme. Übernehmen. |
| `fsr-types` | `TradingPair(String, String)`, Preise als `i64`, FSM-States | **`TradingPair` ist ein generisches String-Tupel** — keine Hardcoded-Krypto-Enums. `("CLN26","CLQ26")` funktioniert sofort. |
| `fsr-temporal` | TriCarrier-Scheduler (t1, t2, φ), TemporalKey | Agnostisch. Übernehmen. |
| `fsr-resonance` | κ, H, sync, M, SI, Φ=(ψ,ρ,ω) | Operiert auf abstrakten Signalen. Füttern mit Spread-Returns statt Token-Preisen. |
| `fsr-gate` | Kairos-Gate, RegimeFSM (Alpha/Beta/Gamma) | Schwellwerte konfigurierbar. Übernehmen. |
| `fsr-nullcenter` | Admissibility-Barrier, Windnarbe | Sicherheitsschicht, agnostisch. Übernehmen. |
| `fsr-csp` | 9-State Commitment-FSM | Passt auf Futures-Combo-Orders. Übernehmen. |
| `fsr-mirror` | Proof-of-Route, MCI | Agnostisch. Übernehmen. |
| `fsr-hedge` | Drawdown/Leverage-Guard | Margin-Modell ergänzen. |
| `fsr-chain` | Dual-SHA-256-Ketten, Audit-Trail | **Compliance-Goldwert** für Futures (s. §9). Übernehmen. |
| `fsr-governance` | 15 Invarianten, Integrity/Resource-FSMs | Invarianten ergänzen (FND, Margin, Roll). |
| `fsr-isls` | Persistentes Wissenssubstrat, Evidence-Chains | Agnostisch. Übernehmen. |
| `fsr-mcce` | Myzelium-Korrelationsgraph (HDAG) | **Stärker für Rohstoffe** (stabile fundamentale Links). Vertices = Kontrakte. |
| `fsr-ecls` | Constraint-Spektroskopie (Band/Ratio/Correlation/Granger/Spectral/...) | **Granger passt exzellent** auf Feedstock-Ketten. Erweitern um saisonale Templates. |
| `fsr-calibration` | 5-Gate-Promotion (Backtest→Paper→Live) | Metriken anpassen. Übernehmen. |

### 2.2 Adaptieren (erweitern, nicht ersetzen)

| Crate | Problem | Adaption |
|---|---|---|
| Markt-Datentypen (`fsr-types::market`) | Preise in "basis points × i64", keine Tick/Multiplier-Semantik | **`ContractSpec` einführen** (tick_size, multiplier, FND/LTD) statt globaler bp-Annahme. |
| `fsr-runtime::engine` | 24-Schritt-Zyklus, `run_macro_cycle_with_books(state, books)` | Voll entkoppelt — nimmt vorgefertigte `OrderBook`s. **Neuer Bridge-Schritt** für Spread-Engine, analog `DshaeBridge` (engine.rs:468). |
| `fsr-candidates` | Trumpet L1 ist hart 3-Bein-Triangel (discovery.rs:31) | Parallele Spread-Discovery-Schicht ergänzen. |

### 2.3 Ersetzen

| Crate | Verifizierter Befund | Konsequenz |
|---|---|---|
| `fsr-dshae` | `C(n,3)`-Enumeration, `D_ijk = ln(r_ij)+ln(r_jk)−ln(r_ik)`, Alpha/Beta-Zyklen — **die Zahl 3 ist durchgängig hart verdrahtet** (him.rs, basket.rs, simplex.rs, axle.rs) | Calendar-Spreads (2-Bein) brauchen neue Mathematik. **Aber: Crack/Crush sind ebenfalls 3-Bein-Zyklen** (1 Input → 2 Outputs) und können die DSHAE-Tensor-/Simplex-Maschinerie mit angepasster Deviationsformel erben. Neue Crate `fsr-spread`, DSHAE bleibt optional für Krypto-Triangeln. |

### 2.4 Komplett neu bauen (existiert nicht)

| Komponente | Warum neu | Risiko |
|---|---|---|
| **IBKR-Marktdaten-Adapter** | `VenueBroker`-Trait ist sauber (2 Methoden), aber IBKR spricht TWS-Socket-Protokoll, nicht WebSocket-JSON | Mittel (well-understood) |
| **Order-Management-System (OMS)** | **Es gibt keinen Execution-Pfad — nirgends.** `OrderRequest`/`OrderReceipt`-Typen existieren, aber kein Lebenszyklus | **Hoch — sicherheitskritisch, echtes Geld** |
| **Commodity-Domänenschicht** | Kontraktspezifikationen, Roll-Kalender, FND-Logik, Margin-Modell | Mittel (Domänenwissen nötig, s. §6) |

---

## 3. Definition der "stärksten Version"

Die stärkste Version ist **nicht** "exotische Arbitrage über alle Rohstoffe". Für einen From-Scratch-Operator ist die stärkste *realistische* Version:

> Ein deterministisches, auditierbares, risiko-gegateter Ausführungssystem auf IBKR, das ökonomisch fundierte Spread-Strategien handelt (Calendar-Rolls, Crack/Crush-Mean-Reversion), seine Konvictions- und Regime-Filter aus der Resonanzschicht bezieht, und über die Laufzeit eine Korrelations- und Constraint-Karte des Rohstoff-Ökosystems akkumuliert, die die Signalqualität monatlich verbessert.

**Ehrliche Positionierung der Resonanzschicht:** Die ψ/ρ/ω-Metriken finden *kein Geld für sich*. Der ökonomische Edge kommt aus (a) der Spread-Ökonomie selbst, (b) Ausführungsqualität und (c) Risikokontrolle. Die Resonanz-/Gate-Maschinerie ist ein **Qualitäts-, Stabilitäts- und Regime-Filter** *über* fundamental verankerten Signalen — sie entscheidet *wann* und *mit welcher Konviktion* gehandelt wird, nicht *ob ein Edge existiert*. Wer das umdreht, baut ein elegantes Verlustsystem. Diese Ehrlichkeit ist der Kern des Experten-Urteils.

---

## 4. Zielarchitektur

### 4.1 Schichtenmodell

```
┌─────────────────────────────────────────────────────────────┐
│  GENUTZT (geerntet, asset-agnostisch)                         │
│  fsr-fixed · fsr-temporal · fsr-resonance · fsr-gate          │
│  fsr-nullcenter · fsr-csp · fsr-mirror · fsr-hedge            │
│  fsr-chain · fsr-governance · fsr-isls · fsr-mcce · fsr-ecls  │
│  fsr-calibration                                              │
├─────────────────────────────────────────────────────────────┤
│  ADAPTIERT                                                    │
│  fsr-types (+ ContractSpec) · fsr-runtime/engine (+Bridge)    │
│  fsr-candidates (+ Spread-Discovery)                          │
├─────────────────────────────────────────────────────────────┤
│  NEU                                                          │
│  fsr-contract   — Kontrakt-/Roll-/FND-Domäne                  │
│  fsr-spread     — Spread-Signal-Engine (ersetzt DSHAE)        │
│  fsr-ibkr       — TWS-Marktdaten + Order-Adapter              │
│  fsr-oms        — Order-Lebenszyklus, Positions-/PnL-Reconcile│
└─────────────────────────────────────────────────────────────┘
```

### 4.2 Neue Crate: `fsr-contract` (Domänenfundament)

Der heute fehlende Layer. Ersetzt die naive "Preis = i64 basis points"-Annahme durch echte Kontraktsemantik.

```rust
/// Eindeutige Kontrakt-Identität. Kompatibel mit TradingPair(String,String)
/// als Wire-Format: TradingPair(symbol_root, yyyymm).
pub struct ContractId {
    pub root: Symbol,          // "CL", "NG", "ZC", "GC", "ES"
    pub expiry: YearMonth,     // 2026-07
    pub exchange: Exchange,    // NYMEX, CBOT, COMEX, ICE, CME
}

/// Per-Kontrakt-Skalierung. Löst das globale-bp-Problem.
pub struct ContractSpec {
    pub id: ContractId,
    pub tick_size: Q32,        // CL: 0.01 $/bbl · ES: 0.25 idx · ZC: 0.25 ¢/bu
    pub tick_value: Q32,       // $ pro Tick (tick_size × multiplier)
    pub multiplier: Q32,       // CL: 1000 bbl · ES: 50 · ZC: 5000 bu
    pub currency: Ccy,
    pub price_unit: PriceUnit, // UsdPerBbl, UsdCentsPerLb, UsdCentsPerBu, ...
    pub first_notice_day: Date,
    pub last_trading_day: Date,
    pub settlement: Settlement, // Physical | Cash
}

/// Spread-Topologie — ersetzt die hart codierte Krypto-Triangel.
pub enum SpreadKind {
    Calendar  { near: ContractId, far: ContractId },              // 2-Bein
    InterComdty { leg_a: ContractId, leg_b: ContractId, ratio: (i32,i32) },
    Crack     { crude: ContractId, gasoline: ContractId, heat: ContractId, recipe: CrackRecipe }, // 3-2-1 etc.
    Crush     { beans: ContractId, oil: ContractId, meal: ContractId, board: CrushRecipe },
    Butterfly { near: ContractId, mid: ContractId, far: ContractId },
    Outright  { contract: ContractId },
}
```

### 4.3 Neue Crate: `fsr-spread` (Signal-Engine, ersetzt DSHAE)

Übernimmt das *Konzept* von DSHAE (Deviationssignal → Manifold → Konvergenz-Kristall), aber mit korrekter, spread-typ-spezifischer Deviationsformel. Crack/Crush erben die 3-Bein-Tensor-Maschinerie aus DSHAE; Calendar nutzt eine 2-Bein-Mean-Reversion-Variante.

```rust
/// Deviationssignal pro Spread-Typ — das ökonomisch fundierte Analogon zu D_ijk.
pub trait SpreadModel {
    /// Theoretischer fairer Spread-Wert (z.B. Cost-of-Carry, Recipe-Fair-Value).
    fn fair_value(&self, quotes: &QuoteSet) -> Q32;
    /// Beobachteter Spread aus den Legs.
    fn observed(&self, quotes: &QuoteSet) -> Q32;
    /// Deviation = observed − fair (das handelbare Signal).
    fn deviation(&self, quotes: &QuoteSet) -> Q32 {
        self.observed(quotes) - self.fair_value(quotes)
    }
}

// Beispiele (Mechanik in §6 verifiziert):
//   Calendar: fair = F_near · e^{carry·Δt};  deviation → Roll-Yield-Signal
//   Crack 3-2-1: fair = (2·RBOB·42 + 1·HO·42)/3 − Crude;  in $/bbl normalisiert
//   Crush (board): fair = Oil·11·... + Meal·... − Beans;  GPM in $/bu
```

Das Resonanz-Manifold bleibt: Aus dem Strom der Deviationen werden ψ/ρ/ω berechnet (Qualität = Signalstärke, Stabilität = Mean-Reversion-Konsistenz, Effizienz = nach Kosten/Slippage/Carry). Die TTCP-Kristallisation markiert konvergierende Spread-Cluster. Das MCCE-Myzel kartiert, *welche* Spreads über die Zeit fundamental gekoppelt sind.

### 4.4 Neue Crate: `fsr-ibkr` (Marktdaten + Order-Adapter)

```rust
/// Implementiert den existierenden VenueBroker-Trait (binance.rs:16) für Marktdaten.
impl VenueBroker for IbkrMarketData {
    fn advance_all(&mut self);              // TWS-Ticks aus mpsc-Channel drainen
    fn all_books(&mut self) -> Vec<OrderBook>; // normalisiert via ContractSpec
}

/// NEU: Order-Seite (existiert im Repo nirgends).
pub trait ExecutionVenue {
    fn submit(&mut self, order: &OrderRequest) -> Result<BrokerOrderId>;
    fn cancel(&mut self, id: BrokerOrderId) -> Result<()>;
    fn poll_events(&mut self) -> Vec<ExecEvent>; // Ack | PartialFill | Fill | Reject | Cancelled
}
```

IBKR-Spezifika (s. §7): TWS/IB-Gateway-Socket, native **Combo/BAG-Orders** für Spreads (ein Spread = eine atomare Order — eliminiert Leg-Risk!), Pacing-Limits, Market-Data-Subscriptions für Futures.

### 4.5 Neue Crate: `fsr-oms` (der sicherheitskritische Brocken)

Der gesamte Order-Lebenszyklus, den das Repo heute nicht hat. Muss **deterministisch und idempotent** sein, um zur Hash-Chain-Philosophie zu passen.

- Order-State-Machine: `Pending → Acked → (PartialFill)* → Filled | Rejected | Cancelled`
- Positions-Ledger mit Fill-Reconciliation gegen IBKR `execDetails`/`commissionReport`
- Realisiertes/unrealisiertes PnL, Mark-to-Market via ContractSpec
- Reconnect-Recovery: bei Verbindungsverlust offene Orders über `reqOpenOrders`/`reqExecutions` rekonstruieren
- Jeder Order-/Fill-Event wird in die Commitment-Chain geschrieben → vollständiger Audit-Trail

### 4.6 Adaptierter Makrozyklus

Der Krypto-Zyklus bleibt strukturell erhalten; die rohstoff-spezifischen Schritte werden als **neue Bridge-Schritte** eingefügt (exakt wie `DshaeBridge` bei engine.rs:468):

```
OBSERVE        → IBKR-Quotes (L1/L2 Futures)
NORMALIZE      → via ContractSpec (Tick/Multiplier) statt globaler bp
ISLS-PERSIST   → unverändert
MCCE-UPDATE    → Inter-Contract-Korrelationsgraph (WTI↔Brent, Corn↔Ethanol, ...)
EXTRACT        → ψ/ρ/ω aus Spread-Deviationen (fsr-spread)
TEMPORAL       → + Roll-Kalender-Awareness
RESOURCE       → + SPAN/Margin-Budget-Check
REGIME         → Contango/Backwardation als Regime-Input
ECLS-SCAN      → Granger (Feedstock-Ketten) + saisonale Templates
NULLCENTER     → unverändert
CANDIDATES     → Spread-Universe: statisch (Crack/Crush/Calendar) + MCCE-dynamisch
CSP            → + Roll-Date- & FND-Checks vor Commitment
EXECUTE        → fsr-oms → IBKR Combo-Order
SPREAD-TICK    → fsr-spread Manifold/Kristall (NEUER Bridge-Schritt)
HEDGE          → + Margin-aware Sizing
EVIDENCE       → Order/Fill in Commitment-Chain
INVARIANTS     → 15 + INV-16..18 (s. §4.7)
SAFE-HOLD      → friert Execution bei Verletzung
```

### 4.7 Neue Invarianten

| ID | Invariante | Begründung |
|---|---|---|
| INV-16 | Keine offene Position in einem Kontrakt nach dessen First Notice Day | Vermeidet ungewollte physische Lieferung |
| INV-17 | Margin-Auslastung < konfigurierbarer Cap (z.B. 70%) | Schutz vor Margin-Call/Zwangsliquidation |
| INV-18 | Jeder Spread netto-delta-konsistent (Beine im korrekten Ratio) | Verhindert un-gehedgte Leg-Exposure |
| INV-19 | Keine neue Position innerhalb von N Tagen vor LTD ohne Roll-Plan | Liquiditätsschutz am Kontraktende |

---

## 5. Warum Rohstoffe besser passen als Krypto

| Aspekt | Krypto (Original) | Rohstoffe (Ziel) |
|---|---|---|
| Korrelationsstruktur | Labil, regime-shifting | **Fundamental stabil** (Crush, Crack, Substitution) |
| Arbitrage-Typ | Triangular (3 Token) | Calendar + Inter-Commodity + strukturelle Spreads |
| MCCE-Lernen | Token-Cluster | **Ökosystem-Cluster** (Energie/Getreide/Metalle + Cross-Links) |
| ECLS Granger | Schwaches Signal | **Stark**: Crude→Products, Beans→Oil/Meal |
| Regime-Erkennung | Sentiment-getrieben | **Supply/Demand-Fundamentals**, stabiler |
| Determinismus | Nett zu haben | **Compliance-kritisch** (CFTC/NFA, s. §9) |
| Native Spread-Execution | Nein (Leg-Risk) | **Ja** — IBKR Combo-Orders, atomar |

---

## 6. Rohstoff-Domänenmechanik (was From-Scratch-Bauer falsch machen)

Dies ist der Teil, an dem die meisten scheitern. Exakte Faktoren **müssen pro Kontraktspezifikation gesetzt werden** — die folgenden sind die Standard-Board-Konventionen.

### 6.1 Crack-Spread (Raffineriemarge)
- **3-2-1**: 3 Fass Rohöl → 2 Fass Benzin (RBOB) + 1 Fass Heizöl (HO).
- **Einheiten-Falle**: Rohöl/WTI in **$/bbl**, RBOB & HO in **$/gallon**. Umrechnung **× 42 gal/bbl**.
- Crack (in $/bbl) = (2 × RBOB × 42 + 1 × HO × 42) / 3 − WTI.
- Varianten: 1-1, 5-3-2. Saisonalität: Benzin-Crack stärker im Sommer (Fahrsaison), Heizöl im Winter.

### 6.2 Crush-Spread (Sojaverarbeitung)
- 1 Bushel Sojabohnen (60 lb) → ~11 lb Sojaöl + ~44 lb Sojamehl (+ Schalen/Verlust).
- **Einheiten-Falle**: Bohnen (ZS) in **¢/bushel**, Öl (ZL) in **¢/lb**, Mehl (ZM) in **$/short ton**.
- Board-Crush (GPM, $/bu) = Öl-Beitrag + Mehl-Beitrag − Bohnenpreis, mit den lb/ton-Umrechnungsfaktoren.
- Reverse-Crush bei negativen Margen. Saisonalität: US-Ernte (Sep–Nov), südamerikanischer Zyklus.

### 6.3 Calendar-Spread / Roll
- Cost-of-Carry: F(T2) ≈ F(T1) · e^{(r + Lager − Convenience-Yield)·Δt}.
- **Contango** (ferne > nahe): Roll-Verlust beim Halten von Long-Futures. **Backwardation**: Roll-Gewinn.
- Der Roll-Yield ist bei Rohstoffen oft *der* dominante Return-Treiber — nicht die Spot-Bewegung.

### 6.4 First Notice Day / Last Trading Day (kritisch)
- Physisch gelieferte Kontrakte (CL, NG, ZC, ZS, GC): Long-Positionen **vor FND** schließen/rollen, sonst Lieferungs-/Lagerrisiko.
- → INV-16. Der Roll-Kalender ist Pflicht-Infrastruktur, kein Nice-to-have.

### 6.5 Margin (SPAN/SPAN2)
- CME nutzt SPAN-Portfolio-Margin; Spreads haben **stark reduzierte Margin** (Spread-Credits) vs. Outrights — das macht Spread-Strategien kapitaleffizient.
- IBKR rechnet Margin in Echtzeit; `reqAccountSummary`/Margin-Felder abfragen → INV-17.

> Hinweis: Alle numerischen Faktoren (lb/bushel, gal/bbl, Tick-Values) sind aus den offiziellen Kontraktspezifikationen der jeweiligen Börse zu übernehmen und in `ContractSpec` zu hinterlegen — nicht hartzucodieren.

---

## 7. IBKR-Integrationsspezifika

- **Verbindung**: TWS oder IB Gateway als lokaler Socket-Endpunkt. Paper-Trading: TWS Port 7497 / Gateway 4002; Live: 7496 / 4001. Kein direkter Cloud-API-Zugang — eine lokale Bridge-Instanz ist erforderlich.
- **Rust-Client**: Es existiert ein Community-Crate für die TWS-API (`ibapi` / „rust-ibapi"). Reifegrad vor Produktiveinsatz prüfen; alternativ dünner eigener Socket-Client gegen das dokumentierte TWS-Protokoll. (Wissensstand-Vorbehalt — vor Festlegung verifizieren.)
- **Native Spreads (Combo/BAG)**: IBKR unterstützt Spread-Kontrakte als atomare Combo-Order. **Das eliminiert Leg-Risk** und ist der entscheidende Vorteil gegenüber dem Krypto-Modell (das jedes Bein einzeln ausführt). `fsr-spread` sollte primär Combo-Orders erzeugen.
- **Pacing**: Message-Rate-Limits (~50 msg/s) beachten → Rate-Limiter im OMS.
- **Marktdaten**: Futures-Marktdaten erfordern passende IBKR-Daten-Abos pro Börse; Verzögerte vs. Echtzeit-Daten unterscheiden. `reqMktData` (Top) bzw. `reqMktDepth` (L2).
- **Paper-First**: Der gesamte Pfad lässt sich gegen IBKR Paper-Trading deterministisch validieren, bevor ein Cent Realgeld fließt — passt exakt zur 5-Gate-Promotion-Pipeline (`fsr-calibration`).

---

## 8. Risiko, Sicherheit, Determinismus & Compliance

Das ist der unterschätzte strategische Wert von fixpoint für Futures:

- **Tamper-evidenter Audit-Trail**: Jede Order, jeder Fill, jede Entscheidung in einer SHA-256-Kette. Für eine Futures-Operation (CFTC/NFA-Aufzeichnungspflichten) ist ein deterministischer, reproduzierbarer Entscheidungs-Log **Gold wert** — die meisten Eigenbau-Bots haben das nicht.
- **Deterministischer Replay**: Jeder Handelstag exakt reproduzierbar → Post-Mortem, Backtest-Live-Parität, Debugging ohne Heisenbugs.
- **Invarianten-getriebene Sicherheit**: 15+ harte Invarianten, SafeHold friert die Aktuierung bei Verletzung ein. Bei echtem Geld ist das der Unterschied zwischen „Bug" und „Konto leer".
- **Nullcenter-Barriere**: Keine konsequente Aktion umgeht die Admissibility-Prüfung — strukturell erzwungen, nicht per Konvention.

---

## 9. Phasen-Roadmap (nicht-brechend, Paper-First)

| Phase | Inhalt | Aufwand (grob) | Gate |
|---|---|---|---|
| **A — Domäne** | `fsr-contract`: ContractSpec, Roll-Kalender, FND-Logik, SpreadKind | 1–2 Wo | Unit-Tests gegen echte Kontraktspecs |
| **B — IBKR-Daten** | `fsr-ibkr` Marktdaten via `VenueBroker`, Normalisierung via ContractSpec | 2–3 Wo | Live-L2-Quotes fließen deterministisch in Engine |
| **C — OMS** | `fsr-oms`: Order-Lebenszyklus, Reconcile, Reconnect, Chain-Integration | **3–5 Wo (kritisch)** | Paper-Orders end-to-end mit korrektem Fill-/PnL-Abgleich |
| **D — Spread-Engine** | `fsr-spread`: Calendar + Crack/Crush-Modelle, Bridge-Schritt | 3–4 Wo | Signale match'en manuelle Spread-Berechnung |
| **E — Invarianten** | INV-16..19, Margin-Modell, SafeHold-Integration | 1–2 Wo | Invarianten-Suite grün |
| **F — Promotion** | 5-Gate gegen IBKR-Paper, dann Mikro-Live | laufend | Paper→Live nur nach Validierung |
| **G — Lernen** | MCCE akkumuliert Graph, ECLS entdeckt saisonale/Granger-Constraints | laufend | Signalqualität steigt messbar |

Kritischer Pfad: **C (OMS)** dominiert das Risiko und die Zeit. Alles andere ist konzeptionell entschärft.

---

## 10. Explizite Nicht-Ziele & Fallstricke

- **DSHAE nicht zwanghaft generalisieren.** Die Variable-Cycle-Length-Verallgemeinerung (2/N-Bein) ist 6–12 Wochen Aufwand mit Korrektheitsrisiko. Stattdessen `fsr-spread` neu, sauber, spread-typ-spezifisch. DSHAE für Krypto-Triangeln optional belassen.
- **Resonanz nicht als Alpha-Quelle missverstehen** (s. §3). Filter, nicht Edge.
- **Nicht live gehen ohne OMS-Härtung.** Teilausführungen, Rejects, Reconnects sind die häufigsten Real-Geld-Bugs.
- **Tick/Multiplier nicht global annehmen.** Pro Kontrakt aus `ContractSpec`.
- **FND/Roll nicht aufschieben.** Ohne Roll-Kalender liefert man irgendwann physisch Rohöl an — INV-16 von Tag 1.
- **Single-Strategy-Start.** Mit einem gut verstandenen Spread (z.B. WTI-Calendar oder 3-2-1-Crack) live gehen, nicht mit dem ganzen Universum.

---

## 11. Offene Entscheidungen (für den Operator)

1. **Strategie-Fokus zuerst**: Energie-Crack, Soja-Crush, oder Calendar-Rolls als erste Live-Strategie?
2. **Kapital & Margin-Budget**: bestimmt Kontraktauswahl (Full-Size vs. Micro-Futures wie MCL/MES).
3. **Rust-IBKR-Client**: Community-Crate vs. dünner Eigenbau-Socket-Client?
4. **Hosting der TWS/Gateway-Bridge**: lokal vs. VPS nahe der Börse (Latenz vs. Komplexität)?
5. **Daten-Abos**: welche Börsen-Echtzeitdaten werden lizenziert?

---

## Anhang A: Verifizierte Code-Anker

| Behauptung | Datei:Zeile | Befund |
|---|---|---|
| Venue-Trait sauber, 2 Methoden | `fsr-runtime/src/binance.rs:16` | `trait VenueBroker { advance_all; all_books }` |
| Engine voll entkoppelt | `fsr-runtime/src/engine.rs:166,173` | nimmt vorgefertigte `OrderBook`s |
| Bridge-Erweiterungs-Seam | `fsr-runtime/src/engine.rs:468` | `state.dshae.tick(&books, tick)` |
| Kein Execution-Pfad | `fsr-runtime/src/binance.rs:189` | `BinanceExecutor` = Stub, nicht verdrahtet |
| TradingPair generisch | `fsr-types/src/ids.rs:8` | `TradingPair(pub String, pub String)` |
| Preise asset-agnostisch | `fsr-types/src/market.rs:9` | `price_bp: i64`, `quantity_lots: u64` |
| 3-Bein hart codiert | `fsr-dshae/src/him.rs:67`, `basket.rs:133`, `simplex.rs:76` | `C(n,3)`, `D_ijk`, Alpha/Beta |
| Order-Typen existieren | `fsr-types/src/market.rs:46` | `OrderRequest`, `OrderReceipt` |
| 15 Invarianten + SafeHold | `fsr-governance/src/invariants.rs` | erweiterbar |

---

*Dieses Dokument ist das Ergebnis eines Forschungslaufs auf Branch `claude/custom-system-architecture-pqc49t`. Es beschreibt eine Zielarchitektur; es wurde noch kein Produktivcode der neuen Crates erstellt.*
