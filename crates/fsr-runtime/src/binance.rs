//! Binance venue adapter (Phase 2 §2.6) — gated behind feature flags.
//!
//! Features:
//!   live-data  → WebSocket order book subscription (tokio-tungstenite)
//!   live-exec  → REST order submission (reqwest, implies live-data)
//!
//! All Q32 / basis-point conversions happen at this boundary.
//! Default (no feature): only the paper-mode PaperBroker is available.
//!
//! The VenueBroker trait unifies PaperBroker and BinanceAdapter so higher
//! layers can accept either without changing their signatures.

use fsr_types::market::OrderBook;

// ── VenueBroker trait ─────────────────────────────────────────────────────────

/// Uniform interface for paper and live brokers.
pub trait VenueBroker: Send {
    /// Advance the internal clock / receive the next data frame.
    fn advance_all(&mut self);
    /// Return current order books for all tracked symbols.
    /// `&mut self` because synthetic brokers regenerate books on call.
    fn all_books(&mut self) -> Vec<OrderBook>;
}

// ── PaperBroker implements VenueBroker ────────────────────────────────────────

impl VenueBroker for crate::paper::PaperBroker {
    fn advance_all(&mut self) {
        crate::paper::PaperBroker::advance_all(self);
    }
    fn all_books(&mut self) -> Vec<OrderBook> {
        crate::paper::PaperBroker::all_books(self)
    }
}

// ── Binance adapter (live-data feature) ───────────────────────────────────────

#[cfg(feature = "live-data")]
pub mod live {
    //! Live Binance WebSocket partial depth stream (5-level, 100 ms updates).
    //! All price/qty values converted to fsr-types at this boundary.

    use super::VenueBroker;
    use anyhow::{Context, Result};
    use fsr_types::{
        ids::{TradingPair, VenueId},
        market::{OrderBook, PriceLevel},
    };
    use futures_util::StreamExt;
    use std::sync::{Arc, Mutex};
    use tokio::runtime::Runtime;
    use tokio::sync::mpsc;
    use tokio_tungstenite::{connect_async, tungstenite::Message};

    /// Raw Binance depth message (partial depth, sorted).
    #[derive(serde::Deserialize)]
    struct DepthMsg {
        bids: Vec<[String; 2]>,
        asks: Vec<[String; 2]>,
    }

    /// Parse a Binance price string (e.g. "29123.45") to basis points (integer).
    /// 1 USD = 100 basis points.
    fn price_to_bp(s: &str) -> i64 {
        let f: f64 = s.parse().unwrap_or(0.0);
        (f * 100.0) as i64
    }

    /// Parse a Binance qty string (e.g. "0.500") to integer lots (× 1000).
    fn qty_to_lots(s: &str) -> u64 {
        let f: f64 = s.parse().unwrap_or(0.0);
        (f * 1000.0) as u64
    }

    fn parse_levels(raw: &[[String; 2]]) -> Vec<PriceLevel> {
        raw.iter()
            .map(|row| PriceLevel {
                price_bp: price_to_bp(&row[0]),
                quantity_lots: qty_to_lots(&row[1]),
            })
            .collect()
    }

    /// Live Binance order-book adapter.
    pub struct BinanceAdapter {
        venue_id: VenueId,
        pair: TradingPair,
        _rt: Runtime,
        book_rx: mpsc::Receiver<OrderBook>,
        current_book: Arc<Mutex<Option<OrderBook>>>,
    }

    impl BinanceAdapter {
        /// Connect to Binance partial depth stream for `symbol` (e.g. "btcusdt").
        pub fn new(symbol: &str, base: &str, quote: &str) -> Result<Self> {
            let rt = Runtime::new().context("create tokio runtime")?;
            let (tx, rx) = mpsc::channel(64);
            let symbol_lc = symbol.to_lowercase();
            let ws_url = format!(
                "wss://stream.binance.com:9443/ws/{}@depth5@100ms",
                symbol_lc
            );
            let venue_id = VenueId("binance".to_string());
            let pair = TradingPair::new(base, quote);
            let vid2 = venue_id.clone();
            let pair2 = pair.clone();

            rt.spawn(async move {
                loop {
                    match connect_async(&ws_url).await {
                        Ok((mut ws, _)) => {
                            while let Some(Ok(msg)) = ws.next().await {
                                if let Message::Text(text) = msg {
                                    if let Ok(depth) =
                                        serde_json::from_str::<DepthMsg>(&text)
                                    {
                                        let book = OrderBook {
                                            venue: vid2.clone(),
                                            pair: pair2.clone(),
                                            bids: parse_levels(&depth.bids),
                                            asks: parse_levels(&depth.asks),
                                            timestamp_us: 0, // filled by advance_all
                                        };
                                        let _ = tx.send(book).await;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "Binance WS disconnected, retrying in 2s");
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        }
                    }
                }
            });

            Ok(BinanceAdapter {
                venue_id,
                pair,
                _rt: rt,
                book_rx: rx,
                current_book: Arc::new(Mutex::new(None)),
            })
        }
    }

    impl VenueBroker for BinanceAdapter {
        fn advance_all(&mut self) {
            use std::time::{SystemTime, UNIX_EPOCH};
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_micros() as u64)
                .unwrap_or(0);
            // Drain all pending messages; keep the latest.
            while let Ok(mut book) = self.book_rx.try_recv() {
                book.timestamp_us = ts;
                *self.current_book.lock().unwrap() = Some(book);
            }
        }

        fn all_books(&mut self) -> Vec<OrderBook> {
            self.current_book
                .lock()
                .unwrap()
                .clone()
                .map(|b| vec![b])
                .unwrap_or_default()
        }
    }
}

// ── REST execution (live-exec feature) ────────────────────────────────────────

#[cfg(feature = "live-exec")]
pub mod exec {
    //! Binance REST order execution (feature = live-exec).
    //! Intentionally minimal stub — requires HMAC-SHA256 signing for production use.

    use anyhow::Result;
    use fsr_types::{
        ids::{OrderId, VenueId},
        market::OrderReceipt,
        Q32,
    };

    /// Binance REST executor stub.
    pub struct BinanceExecutor {
        pub api_key: String,
        base_url: String,
        #[allow(dead_code)]
        client: reqwest::Client,
    }

    impl BinanceExecutor {
        pub fn new(api_key: String, testnet: bool) -> Self {
            let base_url = if testnet {
                "https://testnet.binance.vision".to_string()
            } else {
                "https://api.binance.com".to_string()
            };
            BinanceExecutor {
                api_key,
                base_url,
                client: reqwest::Client::new(),
            }
        }

        /// Submit a market order. Returns an OrderReceipt.
        /// ⚠ STUB — production use requires HMAC-SHA256 signature + live credentials.
        pub async fn submit_market_order(
            &self,
            symbol: &str,
            side: &str,
            qty_lots: u64,
        ) -> Result<OrderReceipt> {
            use std::time::{SystemTime, UNIX_EPOCH};
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64;

            tracing::warn!(
                symbol,
                side,
                qty_lots,
                "BinanceExecutor: STUB — wire HMAC signing before live use"
            );

            Ok(OrderReceipt {
                order_id: OrderId(ts),
                venue: VenueId("binance".to_string()),
                filled_lots: qty_lots,
                executed_price_bp: 0,
                timestamp_us: ts,
            })
        }
    }
}

// ── Offline tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_venue_broker_trait_paper() {
        let mut broker = crate::paper::default_paper_broker();
        let broker_ref: &mut dyn VenueBroker = &mut broker;
        broker_ref.advance_all();
        let books = broker_ref.all_books();
        assert!(!books.is_empty(), "paper broker should provide books");
    }
}
