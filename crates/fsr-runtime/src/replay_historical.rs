#![allow(dead_code)]
//! Historical Replay Engine (Phase 3 §2.3).
//!
//! HistoricalFeed wraps recorded MarketFrames and feeds them through the macro-cycle
//! as if they were live, enabling deterministic backtesting on real market data.
//!
//! Replay Determinism: same recording + same config = same chain digests (spec §2.3).

use crate::backtest_report::{BacktestAccumulator, BacktestReport};
use crate::config::FsrConfig;
use crate::recorder::MarketFrame;
use fsr_types::{market::OrderBook, Hash256};
use std::path::{Path, PathBuf};
use tracing::info;

/// HistoricalFeed: cursor-driven VenueBroker over recorded MarketFrames.
pub struct HistoricalFeed {
    frames: Vec<MarketFrame>,
    cursor: usize,
}

impl HistoricalFeed {
    /// Load frames from a list of .rec file paths.
    pub fn from_recording_files(paths: &[PathBuf]) -> std::io::Result<Self> {
        let frames = crate::recorder::read_recording(paths)?;
        Ok(HistoricalFeed { frames, cursor: 0 })
    }

    /// Total number of frames available.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Reset cursor to beginning (for determinism testing).
    pub fn reset(&mut self) {
        self.cursor = 0;
    }

    /// Return the current frame without advancing the cursor.
    pub fn current_frame(&self) -> Option<&MarketFrame> {
        self.frames.get(self.cursor)
    }

    /// Return the current frame's books and advance.
    pub fn next_books(&mut self) -> Option<(Vec<OrderBook>, u64)> {
        let frame = self.frames.get(self.cursor)?;
        let books = frame.to_order_books();
        let timestamp = frame.timestamp_us;
        self.cursor += 1;
        Some((books, timestamp))
    }
}

// ── Run replay ────────────────────────────────────────────────────────────────

/// Run a complete historical replay.
/// Returns a BacktestReport and writes chain/snapshot data to output_dir.
pub fn run_replay(
    recording_paths: &[PathBuf],
    config: FsrConfig,
    output_dir: &Path,
    run_id: &str,
) -> std::io::Result<BacktestReport> {
    use crate::engine::run_macro_cycle_with_books;
    use crate::engine::SystemState;
    use crate::persistence::{PersistenceConfig, PersistenceManager};

    info!(run_id, recording_count = recording_paths.len(), "Replay started");

    // Create output directory
    std::fs::create_dir_all(output_dir)?;

    let mut feed = HistoricalFeed::from_recording_files(recording_paths)?;
    let total_frames = feed.len();

    if total_frames == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "no frames in recording",
        ));
    }

    // Compute config hash
    let config_hash = compute_config_hash(&config);

    // Persistence to output_dir
    let pm_result = PersistenceManager::new(
        run_id.to_string(),
        PersistenceConfig {
            data_dir: output_dir.to_path_buf(),
            segment_size: 10000,
            snapshot_interval: 1000,
        },
    );
    let mut pm = pm_result.unwrap_or_else(|_| PersistenceManager::disabled());

    let mut state = SystemState::new(config.clone());
    let mut acc = BacktestAccumulator::new();

    let mut prev_settled = 0u64;
    let mut prev_aborted = 0u64;
    let mut prev_crystals = 0u64;
    let mut last_crystal_psi = 0i64;
    let mut last_crystal_rho = 0i64;
    let mut last_crystal_score = 0i64;

    // Record first timestamp
    let _first_ts = feed.current_frame().map(|f| f.timestamp_us).unwrap_or(0);

    while let Some((books, timestamp_us)) = feed.next_books() {
        let status = run_macro_cycle_with_books(&mut state, books);

        // Persistence
        if pm.enabled {
            if let Some(ev) = state.chain.shadow.events.last() {
                pm.flush_shadow_event(ev);
            }
            pm.maybe_snapshot(
                state.tick,
                state.chain.shadow.head_digest(),
                state.chain.commitment.head_digest(),
                state.regime.state,
                state.integrity.state,
                state.settled_cycles,
                state.aborted_cycles,
                status.psi,
                status.rho,
                status.omega,
                state.chain.shadow.event_count,
                state.chain.commitment.event_count,
            );
        }

        // Detect TTCP crystal
        let crystals_delta = state.ttcp.crystals_found - prev_crystals;
        if crystals_delta > 0 {
            if let Some(_last_crystal) = state.ttcp.last_crystal_tick {
                last_crystal_psi = status.psi;
                last_crystal_rho = status.rho;
                last_crystal_score = status.si;
            }
            prev_crystals = state.ttcp.crystals_found;
        }

        acc.record_tick(
            state.tick,
            &status.regime,
            &status.integrity,
            state.settled_cycles - prev_settled,
            state.aborted_cycles - prev_aborted,
            crystals_delta,
            state.current_drawdown,
            last_crystal_psi,
            last_crystal_rho,
            last_crystal_score,
            timestamp_us,
        );

        prev_settled = state.settled_cycles;
        prev_aborted = state.aborted_cycles;
    }

    pm.shutdown();

    let _last_ts = acc.last_timestamp_us;
    let chain_digest = state.chain.shadow.head_digest();

    let recording_file_strs: Vec<String> = recording_paths
        .iter()
        .filter_map(|p| p.to_str().map(|s| s.to_string()))
        .collect();

    let report = acc.into_report(run_id, config_hash, recording_file_strs, chain_digest);

    // Write output
    report.write_to_dir(output_dir)?;

    // Print summary to stdout
    println!("{}", report.summary_text());

    info!(
        run_id,
        total_ticks = report.total_ticks,
        settled = report.settled_trades,
        "Replay completed"
    );

    Ok(report)
}

/// Compute a deterministic hash of a config (for BacktestReport.config_hash).
fn compute_config_hash(config: &FsrConfig) -> Hash256 {
    use sha2::{Digest, Sha256};
    let yaml = serde_yaml::to_string(config).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(yaml.as_bytes());
    let result = hasher.finalize();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&result);
    Hash256(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::{MarketFrame, PairSnapshot, RecordingWriter};
    use fsr_types::ids::{TradingPair, VenueId};
    use std::fs;

    fn write_test_recording(dir: &Path, n_frames: u64) -> Vec<PathBuf> {
        let mut writer = RecordingWriter::new("replay_test", dir).unwrap();
        for i in 0..n_frames {
            let frame = MarketFrame {
                timestamp_us: i * 1_000_000,
                tick: i,
                venue: VenueId("paper".to_string()),
                pairs: vec![
                    PairSnapshot {
                        pair: TradingPair::new("BTC", "USDT"),
                        bids: vec![(10_000_000 + i as i64 * 10, 100)],
                        asks: vec![(10_000_100 + i as i64 * 10, 100)],
                        mid_price: 10_000_050 + i as i64 * 10,
                        spread_bps: 100,
                    },
                    PairSnapshot {
                        pair: TradingPair::new("ETH", "USDT"),
                        bids: vec![(300_000 + i as i64 * 5, 200)],
                        asks: vec![(300_100 + i as i64 * 5, 200)],
                        mid_price: 300_050 + i as i64 * 5,
                        spread_bps: 100,
                    },
                ],
            };
            writer.append(&frame).unwrap();
        }
        writer.flush().unwrap();
        crate::recorder::discover_segments(dir, "replay_test")
    }

    #[test]
    fn test_historical_feed_cursor() {
        let dir = std::env::temp_dir().join("fsr_replay_feed_test");
        let _ = fs::remove_dir_all(&dir);
        let paths = write_test_recording(&dir, 5);

        let mut feed = HistoricalFeed::from_recording_files(&paths).unwrap();
        assert_eq!(feed.len(), 5);

        let (books, _) = feed.next_books().unwrap();
        assert!(!books.is_empty());
        assert_eq!(feed.cursor, 1);

        feed.reset();
        assert_eq!(feed.cursor, 0);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_replay_deterministic() {
        let dir = std::env::temp_dir().join("fsr_replay_det_test");
        let _ = fs::remove_dir_all(&dir);
        let paths = write_test_recording(&dir, 20);

        let config = FsrConfig::conservative();
        let out1 = dir.join("out1");
        let out2 = dir.join("out2");

        let r1 = run_replay(&paths, config.clone(), &out1, "run_det_1").unwrap();
        let r2 = run_replay(&paths, config.clone(), &out2, "run_det_2").unwrap();

        // Chain digests must be identical for same recording + config
        assert_eq!(
            r1.chain_digest_final, r2.chain_digest_final,
            "replay must be deterministic"
        );
        assert_eq!(r1.total_ticks, r2.total_ticks);
        assert_eq!(r1.settled_trades, r2.settled_trades);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_replay_produces_report_files() {
        let dir = std::env::temp_dir().join("fsr_replay_files_test");
        let _ = fs::remove_dir_all(&dir);
        let paths = write_test_recording(&dir, 10);
        let out = dir.join("report_out");
        let config = FsrConfig::conservative();

        let report = run_replay(&paths, config, &out, "files_test").unwrap();
        assert!(out.join("report.json").exists(), "report.json must be created");
        assert!(out.join("summary.txt").exists(), "summary.txt must be created");
        assert_eq!(report.total_ticks, 10);

        let _ = fs::remove_dir_all(&dir);
    }
}
