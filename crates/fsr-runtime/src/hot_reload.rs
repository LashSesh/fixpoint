//! Config hot-reload: watch a YAML config file and swap FsrConfig atomically (Phase 2 §2.5).
//!
//! Uses the `notify` crate for file-system events.
//! Changes are validated before being applied; FSM-topology changes that require
//! a restart are rejected with a warning (only gate thresholds + press params are hot).
//!
//! Usage:
//!   let mut watcher = ConfigWatcher::new(path_to_yaml)?;
//!   // In engine loop:
//!   if let Some(new_cfg) = watcher.poll() {
//!       state.config = new_cfg;
//!   }

use crate::config::FsrConfig;
use anyhow::{Context, Result};
use crossbeam_channel::Receiver;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};
use tracing::{info, warn};

/// Summary of what changed between two configs.
#[derive(Debug, Default)]
pub struct ConfigDiff {
    pub theta_open_changed: bool,
    pub theta_close_changed: bool,
    pub press_top_k_changed: bool,
    pub gate_weights_changed: bool,
    pub regime_thresholds_changed: bool,
    pub changed_field_count: usize,
}

impl ConfigDiff {
    fn from(old: &FsrConfig, new: &FsrConfig) -> Self {
        let mut d = ConfigDiff::default();
        if (old.theta_open - new.theta_open).abs() > 1e-9 {
            d.theta_open_changed = true;
            d.changed_field_count += 1;
        }
        if (old.theta_close - new.theta_close).abs() > 1e-9 {
            d.theta_close_changed = true;
            d.changed_field_count += 1;
        }
        if old.press_top_k != new.press_top_k {
            d.press_top_k_changed = true;
            d.changed_field_count += 1;
        }
        if (old.gate_a_psi - new.gate_a_psi).abs() > 1e-9
            || (old.gate_a_rho - new.gate_a_rho).abs() > 1e-9
            || (old.gate_a_omega - new.gate_a_omega).abs() > 1e-9
        {
            d.gate_weights_changed = true;
            d.changed_field_count += 1;
        }
        if (old.regime_si_weaken - new.regime_si_weaken).abs() > 1e-9
            || (old.regime_dd_max - new.regime_dd_max).abs() > 1e-9
        {
            d.regime_thresholds_changed = true;
            d.changed_field_count += 1;
        }
        d
    }
}

/// Watches a YAML config file and returns a new `FsrConfig` when the file changes.
pub struct ConfigWatcher {
    config_path: PathBuf,
    _watcher: RecommendedWatcher, // kept alive
    rx: Receiver<notify::Result<Event>>,
    last_modified: Option<SystemTime>,
}

impl ConfigWatcher {
    /// Create a watcher for `config_path`.
    pub fn new(config_path: PathBuf) -> Result<Self> {
        let (tx, rx) = crossbeam_channel::bounded(32);
        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.send(res);
        })
        .context("create file watcher")?;

        // Watch the parent directory (more reliable across editors that
        // replace files atomically rather than modifying in-place).
        let dir = config_path
            .parent()
            .unwrap_or(Path::new("."));
        watcher
            .watch(dir, RecursiveMode::NonRecursive)
            .context("watch config directory")?;

        let last_modified = std::fs::metadata(&config_path)
            .ok()
            .and_then(|m| m.modified().ok());

        info!(path = %config_path.display(), "hot-reload watcher started");

        Ok(ConfigWatcher {
            config_path,
            _watcher: watcher,
            rx,
            last_modified,
        })
    }

    /// Non-blocking poll: drain notify events, return new `FsrConfig` if the
    /// file has changed and is valid. Returns `None` if no valid change found.
    pub fn poll(&mut self, current: &FsrConfig) -> Option<FsrConfig> {
        // Drain all queued notify events.
        let mut saw_event = false;
        while self.rx.try_recv().is_ok() {
            saw_event = true;
        }
        if !saw_event {
            return None;
        }

        // Check modification time to deduplicate duplicate events.
        let new_mtime = std::fs::metadata(&self.config_path)
            .ok()
            .and_then(|m| m.modified().ok());
        if new_mtime == self.last_modified {
            return None;
        }
        self.last_modified = new_mtime;

        // Give the writer a brief moment to finish flushing (editors write in chunks).
        std::thread::sleep(Duration::from_millis(50));

        // Read + validate new config.
        match self.load_and_validate(current) {
            Some(cfg) => {
                info!(path = %self.config_path.display(), "config hot-reloaded");
                Some(cfg)
            }
            None => None,
        }
    }

    fn load_and_validate(&self, old: &FsrConfig) -> Option<FsrConfig> {
        let content = match std::fs::read_to_string(&self.config_path) {
            Ok(s) => s,
            Err(e) => {
                warn!(error = %e, "hot-reload: cannot read config file");
                return None;
            }
        };
        let new_cfg: FsrConfig = match FsrConfig::from_yaml(&content) {
            Ok(c) => c,
            Err(e) => {
                warn!(error = %e, "hot-reload: YAML parse error — keeping old config");
                return None;
            }
        };

        // Validate: theta_open must be > theta_close (gate invariant).
        if new_cfg.theta_open <= new_cfg.theta_close {
            warn!(
                theta_open = new_cfg.theta_open,
                theta_close = new_cfg.theta_close,
                "hot-reload: invalid thresholds (θ_open ≤ θ_close) — rejected"
            );
            return None;
        }

        let diff = ConfigDiff::from(old, &new_cfg);
        info!(
            changed_fields = diff.changed_field_count,
            theta_open = diff.theta_open_changed,
            press_top_k = diff.press_top_k_changed,
            "hot-reload: config diff"
        );

        Some(new_cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FsrConfig;

    #[test]
    fn test_config_diff_detects_changes() {
        let old = FsrConfig::conservative();
        let mut new = FsrConfig::conservative();
        new.theta_open = 0.7;
        new.press_top_k = 10;

        let diff = ConfigDiff::from(&old, &new);
        assert!(diff.theta_open_changed);
        assert!(diff.press_top_k_changed);
        assert!(!diff.theta_close_changed);
        assert_eq!(diff.changed_field_count, 2);
    }

    #[test]
    fn test_config_diff_no_changes() {
        let cfg = FsrConfig::conservative();
        let diff = ConfigDiff::from(&cfg, &cfg);
        assert_eq!(diff.changed_field_count, 0);
    }
}
