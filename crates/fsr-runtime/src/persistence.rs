//! PersistenceManager: coordinates ChainFileWriter + SystemSnapshot (Phase 2 §2.1).
//!
//! When enabled (--persist flag), writes:
//!   data/chains/{run_id}/shadow_NNNN.bin      — shadow-chain events (bincode)
//!   data/chains/{run_id}/commitment_NNNN.bin  — commitment-chain events (bincode)
//!   data/snapshots/{run_id}/snap_{tick}.json  — periodic SystemSnapshot (JSON)
//!
//! All I/O errors are non-fatal: logged and ignored so the engine keeps running.

use anyhow::Result;
use fsr_chain::persist::{ChainFileWriter, SystemSnapshot, unix_ms};
use fsr_types::{ChainEvent, IntegrityPosture, RegimeState, Q32};
use std::path::{Path, PathBuf};
use tracing::{error, info};

/// Configuration for persistence behaviour.
pub struct PersistenceConfig {
    /// Where to root all data files (e.g. "data" or "/tmp/fsr").
    pub data_dir: PathBuf,
    /// How many events per binary segment file before rotation.
    pub segment_size: usize,
    /// Take a snapshot every N ticks.
    pub snapshot_interval: u64,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        PersistenceConfig {
            data_dir: PathBuf::from("data"),
            segment_size: 1000,
            snapshot_interval: 100,
        }
    }
}

/// Coordinates persistence of chain events and periodic state snapshots.
pub struct PersistenceManager {
    pub run_id: String,
    pub enabled: bool,
    pub snapshot_interval: u64,
    pub data_dir: PathBuf,
    shadow_writer: Option<ChainFileWriter>,
    commitment_writer: Option<ChainFileWriter>,
}

impl PersistenceManager {
    /// Disabled (no-op) instance — used when --persist flag is absent.
    pub fn disabled() -> Self {
        PersistenceManager {
            run_id: String::new(),
            enabled: false,
            snapshot_interval: u64::MAX,
            data_dir: PathBuf::new(),
            shadow_writer: None,
            commitment_writer: None,
        }
    }

    /// Enabled instance: creates directory structure and opens first segment files.
    pub fn new(run_id: String, cfg: PersistenceConfig) -> Result<Self> {
        let chain_dir = cfg.data_dir.join("chains").join(&run_id);
        let shadow_writer =
            ChainFileWriter::new(&run_id, &chain_dir, "shadow", cfg.segment_size)?;
        let commitment_writer =
            ChainFileWriter::new(&run_id, &chain_dir, "commitment", cfg.segment_size)?;

        info!(run_id = %run_id, data_dir = %cfg.data_dir.display(),
              snapshot_interval = cfg.snapshot_interval, "Persistence enabled");

        Ok(PersistenceManager {
            run_id,
            enabled: true,
            snapshot_interval: cfg.snapshot_interval,
            data_dir: cfg.data_dir,
            shadow_writer: Some(shadow_writer),
            commitment_writer: Some(commitment_writer),
        })
    }

    /// Append a shadow-chain event to disk.
    /// Errors are non-fatal (logged).
    pub fn flush_shadow_event(&mut self, event: &ChainEvent) {
        if !self.enabled {
            return;
        }
        if let Some(w) = self.shadow_writer.as_mut() {
            if let Err(e) = w.append_event(event) {
                error!(error = %e, "persistence: failed to write shadow event");
            }
        }
    }

    /// Append a commitment-chain event to disk.
    pub fn flush_commitment_event(&mut self, event: &ChainEvent) {
        if !self.enabled {
            return;
        }
        if let Some(w) = self.commitment_writer.as_mut() {
            if let Err(e) = w.append_event(event) {
                error!(error = %e, "persistence: failed to write commitment event");
            }
        }
    }

    /// Write a SystemSnapshot if `tick` is a multiple of snapshot_interval.
    /// Inputs are individual fields extracted from SystemState to avoid circular deps.
    #[allow(clippy::too_many_arguments)]
    pub fn maybe_snapshot(
        &mut self,
        tick: u64,
        shadow_head: fsr_types::Hash256,
        commitment_head: fsr_types::Hash256,
        regime_state: RegimeState,
        integrity_state: IntegrityPosture,
        settled_cycles: u64,
        aborted_cycles: u64,
        psi: Q32,
        rho: Q32,
        omega: Q32,
        shadow_event_count: u64,
        commitment_event_count: u64,
    ) {
        if !self.enabled {
            return;
        }
        if tick == 0 || tick % self.snapshot_interval != 0 {
            return;
        }

        let snap = SystemSnapshot {
            run_id: self.run_id.clone(),
            tick,
            timestamp_unix_ms: unix_ms(),
            chain_shadow_head: shadow_head,
            chain_commitment_head: commitment_head,
            regime_state,
            integrity_state,
            settled_cycles,
            aborted_cycles,
            resonance_psi: psi,
            resonance_rho: rho,
            resonance_omega: omega,
            shadow_event_count,
            commitment_event_count,
        };

        let snap_dir = self.data_dir.join("snapshots").join(&self.run_id);
        match snap.write(&snap_dir) {
            Ok(path) => info!(tick, path = %path.display(), "snapshot written"),
            Err(e) => error!(tick, error = %e, "persistence: snapshot write failed"),
        }
    }

    /// Find the most recent run_id under `data_dir/snapshots/` for resume.
    pub fn latest_run_id(data_dir: &Path) -> Option<String> {
        let snap_root = data_dir.join("snapshots");
        if !snap_root.exists() {
            return None;
        }
        let mut entries: Vec<_> = std::fs::read_dir(&snap_root)
            .ok()?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();
        if entries.is_empty() {
            return None;
        }
        // Sort by modification time (newest first) to find most recent run.
        entries.sort_by_key(|e| {
            e.metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH)
        });
        entries
            .last()
            .and_then(|e| e.file_name().into_string().ok())
    }

    /// Flush and close all writers. Call on shutdown.
    pub fn shutdown(&mut self) {
        if let Some(w) = self.shadow_writer.as_mut() {
            let _ = w.flush();
        }
        if let Some(w) = self.commitment_writer.as_mut() {
            let _ = w.flush();
        }
    }
}
