#![allow(dead_code)]
//! Historical Data Recorder (Phase 3 §2.1, §2.2).
//!
//! Records live macro-cycle order books to disk as length-prefixed bincode frames.
//! Files rotate every MAX_FILE_BYTES (100 MB default).
//! Format: `data/recordings/{run_id}_{segment:03}.rec`
//!
//! Each `.rec` file is a contiguous seekable stream of length-prefixed MarketFrame records:
//!   [ u32 LE frame_len | bincode(MarketFrame) ] ...

use fsr_types::{
    ids::{TradingPair, VenueId},
    market::OrderBook,
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufWriter, Read, Write},
    path::{Path, PathBuf},
};

/// Maximum file size before rotation (100 MB).
const MAX_FILE_BYTES: u64 = 100 * 1024 * 1024;

/// Snapshot of a single trading pair's order book within one macro-cycle tick.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairSnapshot {
    pub pair: TradingPair,
    /// Top-N bid levels: (price_bp, quantity_lots)
    pub bids: Vec<(i64, u64)>,
    /// Top-N ask levels: (price_bp, quantity_lots)
    pub asks: Vec<(i64, u64)>,
    /// Mid price in basis points
    pub mid_price: i64,
    /// Spread in basis points
    pub spread_bps: i64,
}

/// One tick's worth of recorded market data (spec §2.1).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketFrame {
    /// Wall-clock microseconds since epoch
    pub timestamp_us: u64,
    /// Macro-cycle tick number
    pub tick: u64,
    pub venue: VenueId,
    pub pairs: Vec<PairSnapshot>,
}

impl MarketFrame {
    /// Build a MarketFrame from a set of OrderBooks at the given tick.
    pub fn from_books(tick: u64, books: &[OrderBook]) -> Vec<MarketFrame> {
        use std::collections::HashMap;
        use std::time::{SystemTime, UNIX_EPOCH};
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(tick * 1_000_000);

        // Group by venue
        let mut by_venue: HashMap<String, Vec<&OrderBook>> = HashMap::new();
        for book in books {
            by_venue.entry(book.venue.0.clone()).or_default().push(book);
        }

        by_venue
            .into_iter()
            .map(|(venue_str, venue_books)| {
                let pairs = venue_books
                    .iter()
                    .map(|b| {
                        let mid = b.mid_bp().unwrap_or(0);
                        let spread = b
                            .best_ask_bp()
                            .and_then(|a| b.best_bid_bp().map(|bid| a - bid))
                            .unwrap_or(0);
                        PairSnapshot {
                            pair: b.pair.clone(),
                            bids: b
                                .bids
                                .iter()
                                .take(5)
                                .map(|l| (l.price_bp, l.quantity_lots))
                                .collect(),
                            asks: b
                                .asks
                                .iter()
                                .take(5)
                                .map(|l| (l.price_bp, l.quantity_lots))
                                .collect(),
                            mid_price: mid,
                            spread_bps: spread,
                        }
                    })
                    .collect();
                MarketFrame {
                    timestamp_us: ts,
                    tick,
                    venue: VenueId(venue_str),
                    pairs,
                }
            })
            .collect()
    }

    /// Convert a MarketFrame back to OrderBooks for replay.
    pub fn to_order_books(&self) -> Vec<OrderBook> {
        use fsr_types::market::PriceLevel;
        self.pairs
            .iter()
            .map(|ps| OrderBook {
                venue: self.venue.clone(),
                pair: ps.pair.clone(),
                bids: ps
                    .bids
                    .iter()
                    .map(|&(p, q)| PriceLevel { price_bp: p, quantity_lots: q })
                    .collect(),
                asks: ps
                    .asks
                    .iter()
                    .map(|&(p, q)| PriceLevel { price_bp: p, quantity_lots: q })
                    .collect(),
                timestamp_us: self.timestamp_us,
            })
            .collect()
    }
}

// ── RecordingWriter ────────────────────────────────────────────────────────────

/// Writes MarketFrames to segmented .rec files (length-prefixed bincode).
pub struct RecordingWriter {
    dir: PathBuf,
    run_id: String,
    segment: u32,
    writer: BufWriter<File>,
    bytes_written: u64,
}

impl RecordingWriter {
    /// Open (or create) the first segment file.
    pub fn new(run_id: &str, dir: &Path) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let segment = 0u32;
        let path = segment_path(dir, run_id, segment);
        let file = OpenOptions::new().create(true).write(true).truncate(true).open(&path)?;
        Ok(RecordingWriter {
            dir: dir.to_path_buf(),
            run_id: run_id.to_string(),
            segment,
            writer: BufWriter::new(file),
            bytes_written: 0,
        })
    }

    /// Append a MarketFrame. Rotates file if > MAX_FILE_BYTES.
    pub fn append(&mut self, frame: &MarketFrame) -> io::Result<()> {
        // Rotate if needed
        if self.bytes_written >= MAX_FILE_BYTES {
            self.writer.flush()?;
            self.segment += 1;
            let path = segment_path(&self.dir, &self.run_id, self.segment);
            let file =
                OpenOptions::new().create(true).write(true).truncate(true).open(&path)?;
            self.writer = BufWriter::new(file);
            self.bytes_written = 0;
        }

        let encoded = bincode::serialize(frame)
            .map_err(|e| io::Error::other(e.to_string()))?;
        let len = encoded.len() as u32;
        self.writer.write_all(&len.to_le_bytes())?;
        self.writer.write_all(&encoded)?;
        self.bytes_written += 4 + encoded.len() as u64;
        Ok(())
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

impl Drop for RecordingWriter {
    fn drop(&mut self) {
        let _ = self.writer.flush();
    }
}

// ── RecordingReader ────────────────────────────────────────────────────────────

/// Reads all MarketFrames from one or more .rec files.
pub fn read_recording(paths: &[PathBuf]) -> io::Result<Vec<MarketFrame>> {
    let mut frames = Vec::new();
    for path in paths {
        let mut file = File::open(path)?;
        loop {
            // Read 4-byte length prefix
            let mut len_buf = [0u8; 4];
            match file.read_exact(&mut len_buf) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(e) => return Err(e),
            }
            let len = u32::from_le_bytes(len_buf) as usize;
            let mut data = vec![0u8; len];
            file.read_exact(&mut data)?;
            let frame: MarketFrame = bincode::deserialize(&data)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
            frames.push(frame);
        }
    }
    Ok(frames)
}

/// Discover all segment files for a given run_id in a directory.
pub fn discover_segments(dir: &Path, run_id: &str) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = (0..=999u32)
        .map(|seg| segment_path(dir, run_id, seg))
        .take_while(|p| p.exists())
        .collect();
    paths.sort();
    paths
}

fn segment_path(dir: &Path, run_id: &str, segment: u32) -> PathBuf {
    dir.join(format!("{}_{:03}.rec", run_id, segment))
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_types::ids::VenueId;
    use std::fs;

    fn make_frame(tick: u64) -> MarketFrame {
        MarketFrame {
            timestamp_us: tick * 1_000_000,
            tick,
            venue: VenueId("test".to_string()),
            pairs: vec![PairSnapshot {
                pair: TradingPair::new("BTC", "USDT"),
                bids: vec![(100_000, 10), (99_900, 20)],
                asks: vec![(100_100, 10), (100_200, 20)],
                mid_price: 100_050,
                spread_bps: 100,
            }],
        }
    }

    #[test]
    fn test_recording_roundtrip() {
        let dir = std::env::temp_dir().join("fsr_recorder_test");
        let _ = fs::remove_dir_all(&dir);

        let mut writer = RecordingWriter::new("test_run", &dir).unwrap();
        for i in 0..10u64 {
            writer.append(&make_frame(i)).unwrap();
        }
        writer.flush().unwrap();

        let paths = discover_segments(&dir, "test_run");
        assert!(!paths.is_empty(), "at least one segment file");
        let frames = read_recording(&paths).unwrap();
        assert_eq!(frames.len(), 10, "all 10 frames must round-trip");
        assert_eq!(frames[0].tick, 0);
        assert_eq!(frames[9].tick, 9);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_frame_to_order_books() {
        let frame = make_frame(1);
        let books = frame.to_order_books();
        assert_eq!(books.len(), 1);
        let book = &books[0];
        assert_eq!(book.bids[0].price_bp, 100_000);
        assert_eq!(book.asks[0].price_bp, 100_100);
    }

    #[test]
    fn test_frame_from_books_empty() {
        let frames = MarketFrame::from_books(0, &[]);
        assert!(frames.is_empty());
    }
}
