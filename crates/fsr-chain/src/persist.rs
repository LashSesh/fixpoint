//! Persistence layer for fsr-chain: segment files + snapshots (spec §Phase-2, step 2.1).
//!
//! ChainFileWriter:  rotates binary (bincode) segment files at configurable event count.
//! SystemSnapshot:   serde_json checkpoint of key SystemState metrics at interval ticks.
//! ChainReader:      reads segment files back for replay/resume.
//!
//! Path conventions:
//!   data/chains/{run_id}/shadow_{seq:04}.bin
//!   data/chains/{run_id}/commitment_{seq:04}.bin
//!   data/snapshots/{run_id}/snap_{tick:010}.json

use anyhow::{Context, Result};
use fsr_types::{ChainEvent, Hash256, IntegrityPosture, Q32, RegimeState};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

// ── ChainFileWriter ────────────────────────────────────────────────────────────

/// Writes ChainEvents to rotating binary segment files using bincode encoding.
pub struct ChainFileWriter {
    pub run_id: String,
    dir: PathBuf,
    segment_size: usize,
    current_segment: u32,
    current_count: usize,
    writer: BufWriter<File>,
    prefix: String, // "shadow" or "commitment"
}

impl ChainFileWriter {
    /// Create a new writer. Opens (or creates) the first segment file.
    pub fn new(
        run_id: &str,
        dir: &Path,
        prefix: &str,
        segment_size: usize,
    ) -> Result<Self> {
        fs::create_dir_all(dir)
            .with_context(|| format!("create chain dir {:?}", dir))?;
        let path = segment_path(dir, prefix, 0);
        let file = File::create(&path)
            .with_context(|| format!("create segment {:?}", path))?;
        Ok(ChainFileWriter {
            run_id: run_id.to_string(),
            dir: dir.to_path_buf(),
            segment_size,
            current_segment: 0,
            current_count: 0,
            writer: BufWriter::new(file),
            prefix: prefix.to_string(),
        })
    }

    /// Encode and append one event. Rotates segment file if limit reached.
    pub fn append_event(&mut self, event: &ChainEvent) -> Result<()> {
        let encoded = bincode::serialize(event)
            .context("bincode serialize ChainEvent")?;
        // Write length-prefixed frame: [u32 len][bytes]
        let len = encoded.len() as u32;
        self.writer.write_all(&len.to_le_bytes()).context("write frame len")?;
        self.writer.write_all(&encoded).context("write frame bytes")?;
        self.current_count += 1;
        if self.current_count >= self.segment_size {
            self.flush_and_rotate()?;
        }
        Ok(())
    }

    /// Flush current buffer and open a new segment file.
    pub fn flush_and_rotate(&mut self) -> Result<()> {
        self.writer.flush().context("flush segment")?;
        self.current_segment += 1;
        self.current_count = 0;
        let path = segment_path(&self.dir, &self.prefix, self.current_segment);
        let file = File::create(&path)
            .with_context(|| format!("create rotated segment {:?}", path))?;
        self.writer = BufWriter::new(file);
        Ok(())
    }

    /// Final flush on shutdown.
    pub fn flush(&mut self) -> Result<()> {
        self.writer.flush().context("final flush")
    }
}

fn segment_path(dir: &Path, prefix: &str, seq: u32) -> PathBuf {
    dir.join(format!("{}_{:04}.bin", prefix, seq))
}

// ── ChainReader ────────────────────────────────────────────────────────────────

/// Reads all ChainEvents from a directory of segment files (in sequence order).
pub struct ChainReader;

impl ChainReader {
    /// Scan dir for `{prefix}_NNNN.bin` files (sorted) and decode all events.
    pub fn read_all_events(dir: &Path, prefix: &str) -> Result<Vec<ChainEvent>> {
        let mut entries: Vec<PathBuf> = fs::read_dir(dir)
            .with_context(|| format!("read chain dir {:?}", dir))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension().map_or(false, |ext| ext == "bin")
                    && p.file_stem()
                        .and_then(|s| s.to_str())
                        .map_or(false, |s| s.starts_with(prefix))
            })
            .collect();
        entries.sort();

        let mut events = Vec::new();
        for path in &entries {
            let file = File::open(path)
                .with_context(|| format!("open segment {:?}", path))?;
            let mut reader = BufReader::new(file);
            loop {
                let mut len_buf = [0u8; 4];
                match reader.read_exact(&mut len_buf) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                    Err(e) => return Err(e).context("read frame len"),
                }
                let len = u32::from_le_bytes(len_buf) as usize;
                let mut buf = vec![0u8; len];
                reader.read_exact(&mut buf).context("read frame bytes")?;
                let event: ChainEvent =
                    bincode::deserialize(&buf).context("bincode deserialize ChainEvent")?;
                events.push(event);
            }
        }
        Ok(events)
    }
}

// ── SystemSnapshot ─────────────────────────────────────────────────────────────

/// Key SystemState metrics at a checkpoint tick.
/// Full FSM state is NOT stored here — it is deterministically replayable from the chain.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SystemSnapshot {
    pub run_id: String,
    pub tick: u64,
    pub timestamp_unix_ms: u64,
    pub chain_shadow_head: Hash256,
    pub chain_commitment_head: Hash256,
    pub regime_state: RegimeState,
    pub integrity_state: IntegrityPosture,
    pub settled_cycles: u64,
    pub aborted_cycles: u64,
    pub resonance_psi: Q32,
    pub resonance_rho: Q32,
    pub resonance_omega: Q32,
    pub shadow_event_count: u64,
    pub commitment_event_count: u64,
}

impl SystemSnapshot {
    /// Serialize to JSON and write to `dir/snap_{tick:010}.json`.
    pub fn write(&self, dir: &Path) -> Result<PathBuf> {
        fs::create_dir_all(dir)
            .with_context(|| format!("create snapshot dir {:?}", dir))?;
        let filename = format!("snap_{:010}.json", self.tick);
        let path = dir.join(&filename);
        let json = serde_json::to_string_pretty(self).context("serialize SystemSnapshot")?;
        fs::write(&path, json).with_context(|| format!("write snapshot {:?}", path))?;
        Ok(path)
    }

    /// Read the snapshot with the highest tick number from `dir`.
    pub fn read_latest(dir: &Path) -> Result<Option<Self>> {
        if !dir.exists() {
            return Ok(None);
        }
        let mut entries: Vec<PathBuf> = fs::read_dir(dir)
            .with_context(|| format!("read snapshot dir {:?}", dir))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension().map_or(false, |ext| ext == "json")
                    && p.file_stem()
                        .and_then(|s| s.to_str())
                        .map_or(false, |s| s.starts_with("snap_"))
            })
            .collect();
        if entries.is_empty() {
            return Ok(None);
        }
        entries.sort();
        let latest = entries.last().unwrap();
        let json = fs::read_to_string(latest)
            .with_context(|| format!("read snapshot {:?}", latest))?;
        let snap: SystemSnapshot =
            serde_json::from_str(&json).context("deserialize SystemSnapshot")?;
        Ok(Some(snap))
    }
}

/// Current Unix milliseconds (non-critical: display + sorting only).
pub fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_types::{EventTag, Freshness, TemporalKey};

    fn make_event(tag: EventTag, tick: u64) -> ChainEvent {
        ChainEvent {
            tag,
            payload: vec![tick as u8],
            temporal_key: TemporalKey {
                commit_tick: 0,
                intrinsic_tick: tick,
                phase_bin: 0,
                wind_count: 0,
                freshness: Freshness::Fresh,
            },
            prev_digest: Hash256::ZERO,
            digest: Hash256::ZERO,
        }
    }

    #[test]
    fn test_chain_writer_reader_roundtrip() {
        let dir = std::env::temp_dir().join("fsr_chain_test_rw");
        let _ = fs::remove_dir_all(&dir);

        let mut writer = ChainFileWriter::new("run_test", &dir, "shadow", 5).unwrap();
        for i in 0..12u64 {
            writer.append_event(&make_event(EventTag::MacroCycleEnd, i)).unwrap();
        }
        writer.flush_and_rotate().unwrap();
        writer.flush().unwrap();

        let events = ChainReader::read_all_events(&dir, "shadow").unwrap();
        assert_eq!(events.len(), 12, "all 12 events should round-trip");
        assert_eq!(events[0].temporal_key.intrinsic_tick, 0);
        assert_eq!(events[11].temporal_key.intrinsic_tick, 11);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_chain_writer_rotates_segments() {
        let dir = std::env::temp_dir().join("fsr_chain_test_rotate");
        let _ = fs::remove_dir_all(&dir);

        let mut writer = ChainFileWriter::new("run_rot", &dir, "shadow", 3).unwrap();
        for i in 0..9u64 {
            writer.append_event(&make_event(EventTag::MacroCycleStart, i)).unwrap();
        }
        writer.flush().unwrap();

        // Should have produced at least 3 segment files (0, 1, 2)
        let count = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |x| x == "bin"))
            .count();
        assert!(count >= 3, "expected ≥3 segments, got {}", count);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_snapshot_write_and_read_latest() {
        let dir = std::env::temp_dir().join("fsr_snap_test");
        let _ = fs::remove_dir_all(&dir);

        let snap = SystemSnapshot {
            run_id: "test_run".to_string(),
            tick: 42,
            timestamp_unix_ms: 0,
            chain_shadow_head: Hash256::ZERO,
            chain_commitment_head: Hash256::ZERO,
            regime_state: RegimeState::Alpha,
            integrity_state: IntegrityPosture::Healthy,
            settled_cycles: 5,
            aborted_cycles: 2,
            resonance_psi: 100,
            resonance_rho: 200,
            resonance_omega: 300,
            shadow_event_count: 10,
            commitment_event_count: 1,
        };
        snap.write(&dir).unwrap();

        let loaded = SystemSnapshot::read_latest(&dir).unwrap().unwrap();
        assert_eq!(loaded.tick, 42);
        assert_eq!(loaded.settled_cycles, 5);
        assert_eq!(loaded.run_id, "test_run");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_snapshot_read_latest_none_when_empty() {
        let dir = std::env::temp_dir().join("fsr_snap_empty");
        let _ = fs::remove_dir_all(&dir);
        let result = SystemSnapshot::read_latest(&dir).unwrap();
        assert!(result.is_none());
    }
}
