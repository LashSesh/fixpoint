#![allow(dead_code)]
//! Kraken Venue Adapter (Phase 3 §7).
//!
//! Same VenueBroker trait as Binance. Feature-gated behind `live-data`.
//! Implements Kraken WebSocket v2 order book subscription.
//!
//! Cross-venue candidate discovery (spec §7.2):
//!   With two venues, candidates can span Binance + Kraken legs.
//!   Pair name normalization is handled via PAIR_MAPPING.
//!
//! Pair normalization:
//!   BTCUSDT ↔ XBTUSDT, ETHUSDT ↔ ETHUSDT, ETHBTC ↔ ETHXBT

use fsr_types::{
    ids::TradingPair,
    market::{CandidateLeg, OrderBook, Side},
};
#[cfg(feature = "live-data")]
use fsr_types::ids::VenueId;

// ── Pair name normalization ────────────────────────────────────────────────────

/// Normalize a Kraken pair name to a canonical pair used across venues.
/// e.g. "XBT/USDT" or "XBTUSDT" → TradingPair("BTC", "USDT")
pub fn normalize_kraken_pair(kraken_pair: &str) -> Option<TradingPair> {
    let normalized = kraken_pair
        .replace("/", "")
        .replace("XBT", "BTC");
    match normalized.as_str() {
        "BTCUSDT" => Some(TradingPair::new("BTC", "USDT")),
        "ETHUSDT" => Some(TradingPair::new("ETH", "USDT")),
        "ETHBTC" | "ETHXBT" => Some(TradingPair::new("ETH", "BTC")),
        "BTCEUR" | "XBTEUR" => Some(TradingPair::new("BTC", "EUR")),
        "ETHEUR" => Some(TradingPair::new("ETH", "EUR")),
        other => {
            // Try generic split: first 3 chars = base, rest = quote
            if other.len() >= 6 {
                Some(TradingPair::new(&other[..3], &other[3..]))
            } else {
                None
            }
        }
    }
}

/// Convert a canonical TradingPair to the Kraken pair name for WebSocket subscription.
pub fn canonical_to_kraken(pair: &TradingPair) -> String {
    let base = if pair.0 == "BTC" { "XBT" } else { &pair.0 };
    format!("{}/{}", base, pair.1)
}

// ── Cross-venue candidate discovery ───────────────────────────────────────────

/// Discover cross-venue candidate legs from two sets of order books.
/// Returns Vec<Vec<CandidateLeg>> where each inner Vec is a potential cycle.
/// Currently implements triangular detection: buy A on venue 1, sell A on venue 2.
pub fn find_cross_venue_candidates(
    binance_books: &[OrderBook],
    kraken_books: &[OrderBook],
) -> Vec<Vec<CandidateLeg>> {
    let mut candidates = Vec::new();

    for bin_book in binance_books {
        for krk_book in kraken_books {
            // Same canonical pair?
            if bin_book.pair != krk_book.pair {
                continue;
            }
            let bin_ask = match bin_book.best_ask_bp() {
                Some(p) => p,
                None => continue,
            };
            let krk_bid = match krk_book.best_bid_bp() {
                Some(p) => p,
                None => continue,
            };

            // Arbitrage: buy on Binance (lower ask), sell on Kraken (higher bid)
            if bin_ask < krk_bid {
                let spread = krk_bid - bin_ask;
                let bin_qty = bin_book.asks.first().map(|l| l.quantity_lots as i64).unwrap_or(0);
                let krk_qty = krk_book.bids.first().map(|l| l.quantity_lots as i64).unwrap_or(0);
                let qty = bin_qty.min(krk_qty);
                if qty > 0 && spread > 0 {
                    candidates.push(vec![
                        CandidateLeg {
                            venue: bin_book.venue.clone(),
                            pair: bin_book.pair.clone(),
                            side: Side::Buy,
                            price: bin_ask,
                            available_qty: qty,
                        },
                        CandidateLeg {
                            venue: krk_book.venue.clone(),
                            pair: krk_book.pair.clone(),
                            side: Side::Sell,
                            price: krk_bid,
                            available_qty: qty,
                        },
                    ]);
                }
            }

            // Reverse: buy on Kraken, sell on Binance
            let krk_ask = match krk_book.best_ask_bp() {
                Some(p) => p,
                None => continue,
            };
            let bin_bid = match bin_book.best_bid_bp() {
                Some(p) => p,
                None => continue,
            };
            if krk_ask < bin_bid {
                let spread = bin_bid - krk_ask;
                let krk_qty = krk_book.asks.first().map(|l| l.quantity_lots as i64).unwrap_or(0);
                let bin_qty = bin_book.bids.first().map(|l| l.quantity_lots as i64).unwrap_or(0);
                let qty = bin_qty.min(krk_qty);
                if qty > 0 && spread > 0 {
                    candidates.push(vec![
                        CandidateLeg {
                            venue: krk_book.venue.clone(),
                            pair: krk_book.pair.clone(),
                            side: Side::Buy,
                            price: krk_ask,
                            available_qty: qty,
                        },
                        CandidateLeg {
                            venue: bin_book.venue.clone(),
                            pair: bin_book.pair.clone(),
                            side: Side::Sell,
                            price: bin_bid,
                            available_qty: qty,
                        },
                    ]);
                }
            }
        }
    }
    candidates
}

// ── Live Kraken adapter (feature = live-data) ─────────────────────────────────

#[cfg(feature = "live-data")]
pub mod live {
    //! Live Kraken WebSocket v2 order book adapter.
    //! Subscribes to book channels for configured pairs.

    use super::*;
    use crate::binance::VenueBroker;
    use anyhow::{Context, Result};
    use fsr_types::market::PriceLevel;
    use futures_util::StreamExt;
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };
    use tokio::runtime::Runtime;
    use tokio::sync::mpsc;
    use tokio_tungstenite::{connect_async, tungstenite::Message};

    /// Raw Kraken book snapshot message (simplified).
    #[derive(serde::Deserialize)]
    struct KrakenBookMsg {
        #[serde(rename = "type")]
        msg_type: String,
        #[serde(rename = "symbol")]
        symbol: Option<String>,
        bids: Option<Vec<[serde_json::Value; 2]>>,
        asks: Option<Vec<[serde_json::Value; 2]>>,
    }

    fn parse_kraken_price(v: &serde_json::Value) -> i64 {
        match v {
            serde_json::Value::String(s) => {
                let f: f64 = s.parse().unwrap_or(0.0);
                (f * 100.0) as i64
            }
            serde_json::Value::Number(n) => {
                let f = n.as_f64().unwrap_or(0.0);
                (f * 100.0) as i64
            }
            _ => 0,
        }
    }

    fn parse_kraken_qty(v: &serde_json::Value) -> u64 {
        match v {
            serde_json::Value::String(s) => {
                let f: f64 = s.parse().unwrap_or(0.0);
                (f * 1000.0) as u64
            }
            serde_json::Value::Number(n) => {
                let f = n.as_f64().unwrap_or(0.0);
                (f * 1000.0) as u64
            }
            _ => 0,
        }
    }

    /// Live Kraken WebSocket adapter implementing VenueBroker.
    pub struct KrakenAdapter {
        venue_id: VenueId,
        _rt: Runtime,
        book_rx: mpsc::Receiver<OrderBook>,
        current_books: Arc<Mutex<HashMap<String, OrderBook>>>,
    }

    impl KrakenAdapter {
        /// Connect to Kraken WebSocket v2 for the given pairs (e.g. ["BTC/USDT", "ETH/USDT"]).
        pub fn new(pairs: &[&str]) -> Result<Self> {
            let rt = Runtime::new().context("create tokio runtime")?;
            let (tx, rx) = mpsc::channel::<OrderBook>(128);
            let pairs_owned: Vec<String> = pairs.iter().map(|s| s.to_string()).collect();

            let ws_url = "wss://ws.kraken.com/v2";
            let venue_id = VenueId("kraken".to_string());
            let vid2 = venue_id.clone();

            rt.spawn(async move {
                loop {
                    match connect_async(ws_url).await {
                        Ok((mut ws, _)) => {
                            // Subscribe to book channel
                            let sub = serde_json::json!({
                                "method": "subscribe",
                                "params": {
                                    "channel": "book",
                                    "symbol": pairs_owned,
                                    "depth": 5
                                }
                            });
                            if let Ok(msg) = serde_json::to_string(&sub) {
                                let _ = ws.send(Message::Text(msg)).await;
                            }
                            while let Some(Ok(Message::Text(text))) = ws.next().await {
                                if let Ok(book_msg) =
                                    serde_json::from_str::<KrakenBookMsg>(&text)
                                {
                                    if book_msg.msg_type == "snapshot"
                                        || book_msg.msg_type == "update"
                                    {
                                        if let (Some(sym), Some(bids_raw), Some(asks_raw)) = (
                                            book_msg.symbol.as_ref(),
                                            book_msg.bids.as_ref(),
                                            book_msg.asks.as_ref(),
                                        ) {
                                            if let Some(pair) =
                                                super::normalize_kraken_pair(sym)
                                            {
                                                use std::time::{SystemTime, UNIX_EPOCH};
                                                let ts = SystemTime::now()
                                                    .duration_since(UNIX_EPOCH)
                                                    .map(|d| d.as_micros() as u64)
                                                    .unwrap_or(0);

                                                let bids: Vec<PriceLevel> = bids_raw
                                                    .iter()
                                                    .map(|row| PriceLevel {
                                                        price_bp: parse_kraken_price(&row[0]),
                                                        quantity_lots: parse_kraken_qty(&row[1]),
                                                    })
                                                    .collect();
                                                let asks: Vec<PriceLevel> = asks_raw
                                                    .iter()
                                                    .map(|row| PriceLevel {
                                                        price_bp: parse_kraken_price(&row[0]),
                                                        quantity_lots: parse_kraken_qty(&row[1]),
                                                    })
                                                    .collect();

                                                let book = OrderBook {
                                                    venue: vid2.clone(),
                                                    pair,
                                                    bids,
                                                    asks,
                                                    timestamp_us: ts,
                                                };
                                                let _ = tx.send(book).await;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "Kraken WS disconnected, retrying in 2s");
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        }
                    }
                }
            });

            Ok(KrakenAdapter {
                venue_id,
                _rt: rt,
                book_rx: rx,
                current_books: Arc::new(Mutex::new(HashMap::new())),
            })
        }
    }

    impl VenueBroker for KrakenAdapter {
        fn advance_all(&mut self) {
            while let Ok(book) = self.book_rx.try_recv() {
                let key = format!("{}/{}", book.pair.0, book.pair.1);
                self.current_books.lock().unwrap().insert(key, book);
            }
        }

        fn all_books(&mut self) -> Vec<OrderBook> {
            self.current_books
                .lock()
                .unwrap()
                .values()
                .cloned()
                .collect()
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use fsr_types::{ids::VenueId, market::PriceLevel};

    fn make_book(venue: &str, pair: TradingPair, bid: i64, ask: i64) -> OrderBook {
        OrderBook {
            venue: VenueId(venue.to_string()),
            pair,
            bids: vec![PriceLevel { price_bp: bid, quantity_lots: 100 }],
            asks: vec![PriceLevel { price_bp: ask, quantity_lots: 100 }],
            timestamp_us: 0,
        }
    }

    #[test]
    fn test_normalize_kraken_pair() {
        assert_eq!(
            normalize_kraken_pair("XBT/USDT"),
            Some(TradingPair::new("BTC", "USDT"))
        );
        assert_eq!(
            normalize_kraken_pair("XBTUSDT"),
            Some(TradingPair::new("BTC", "USDT"))
        );
        assert_eq!(
            normalize_kraken_pair("ETH/USDT"),
            Some(TradingPair::new("ETH", "USDT"))
        );
        assert_eq!(
            normalize_kraken_pair("ETH/XBT"),
            Some(TradingPair::new("ETH", "BTC"))
        );
    }

    #[test]
    fn test_canonical_to_kraken() {
        let pair = TradingPair::new("BTC", "USDT");
        assert_eq!(canonical_to_kraken(&pair), "XBT/USDT");
        let pair2 = TradingPair::new("ETH", "USDT");
        assert_eq!(canonical_to_kraken(&pair2), "ETH/USDT");
    }

    #[test]
    fn test_cross_venue_arb_detected() {
        let pair = TradingPair::new("BTC", "USDT");
        // Binance ask = 100_000, Kraken bid = 100_200 → arb exists
        let bin_books = vec![make_book("binance", pair.clone(), 99_900, 100_000)];
        let krk_books = vec![make_book("kraken", pair.clone(), 100_200, 100_400)];

        let candidates = find_cross_venue_candidates(&bin_books, &krk_books);
        assert!(!candidates.is_empty(), "should find cross-venue arb");
        let legs = &candidates[0];
        assert_eq!(legs.len(), 2);
        assert_eq!(legs[0].side, Side::Buy);
        assert_eq!(legs[1].side, Side::Sell);
    }

    #[test]
    fn test_cross_venue_no_arb_when_equal() {
        let pair = TradingPair::new("BTC", "USDT");
        // Same prices → no arb
        let bin_books = vec![make_book("binance", pair.clone(), 100_000, 100_100)];
        let krk_books = vec![make_book("kraken", pair.clone(), 100_000, 100_100)];
        let candidates = find_cross_venue_candidates(&bin_books, &krk_books);
        assert!(candidates.is_empty(), "no arb when prices equal");
    }

    #[test]
    fn test_cross_venue_no_arb_different_pairs() {
        let bin_books = vec![make_book("binance", TradingPair::new("BTC", "USDT"), 100_000, 100_100)];
        let krk_books = vec![make_book("kraken", TradingPair::new("ETH", "USDT"), 300_000, 300_100)];
        let candidates = find_cross_venue_candidates(&bin_books, &krk_books);
        assert!(candidates.is_empty(), "no arb across different pairs");
    }
}
