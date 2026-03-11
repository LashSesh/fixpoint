//! Structured observability: tracing subscriber init + (optional) metrics.
//!
//! Phase 2 §2.2: Replace ad-hoc println!/eprintln! with tracing spans.
//! Default mode: human-readable ANSI output on stderr.
//! JSON mode (--log-json): JSONL output to data/logs/{run_id}.jsonl.

use anyhow::Result;
use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;

/// Initialize the global tracing subscriber.
///
/// Returns a `WorkerGuard` that **must** be kept alive for the duration of the
/// program — dropping it flushes and closes the log file.
///
/// `json_log = false` → pretty, ANSI-coloured stderr (development).
/// `json_log = true`  → JSONL file at `data_dir/logs/{run_id}.jsonl`.
pub fn init_tracing(run_id: &str, json_log: bool, data_dir: &Path) -> Result<WorkerGuard> {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    if json_log {
        let logs_dir = data_dir.join("logs");
        std::fs::create_dir_all(&logs_dir)?;
        let log_file = format!("{}.jsonl", run_id);
        let file_appender = tracing_appender::rolling::never(&logs_dir, &log_file);
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        tracing_subscriber::registry()
            .with(filter)
            .with(
                fmt::layer()
                    .json()
                    .with_writer(non_blocking)
                    .with_target(true)
                    .with_current_span(false),
            )
            .init();

        Ok(guard)
    } else {
        // stderr pretty output — use a no-op non-blocking sink so we always
        // return a WorkerGuard (the guard wraps stderr, which never blocks).
        let (non_blocking, guard) = tracing_appender::non_blocking(std::io::stderr());

        tracing_subscriber::registry()
            .with(filter)
            .with(
                fmt::layer()
                    .with_writer(non_blocking)
                    .with_target(false)
                    .with_ansi(true),
            )
            .init();

        Ok(guard)
    }
}
