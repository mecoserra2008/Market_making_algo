use anyhow::{Result, anyhow, Context};
use futures::{StreamExt, SinkExt, stream::{SplitSink, SplitStream}};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream, MaybeTlsStream};
use tracing::{info, warn, error, debug};
use std::sync::Arc;
use parking_lot::RwLock;

use crate::models::orderbook::OrderBook;
use crate::models::{Trade, Side};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsSink = SplitSink<WsStream, Message>;
type WsSource = SplitStream<WsStream>;

/// WebSocket client for Bybit with automatic reconnection
pub struct BybitWebSocket {
    url: String,
    subscriptions: Vec<String>,
    orderbooks: Arc<dashmap::DashMap<String, Arc<RwLock<OrderBook>>>>,
    event_tx: mpsc::UnboundedSender<MarketEvent>,
    connection_state: Arc<RwLock<ConnectionState>>,
}

#[derive(Debug, Clone)]
pub struct ConnectionState {
    pub connected: bool,
    pub last_message_time: Instant,
    pub reconnect_count: u32,
    pub total_messages: u64,
}

#[derive(Debug, Clone)]
pub enum MarketEvent {
    OrderBookUpdate {
        symbol: String,
        bids: Vec<(f64, f64)>,
        asks: Vec<(f64, f64)>,
        timestamp: i64,
    },
    Trade {
        symbol: String,
        trade: Trade,
    },
    Connected,
    Disconnected,
    Error(String),
}

#[derive(Debug, Deserialize)]
struct BybitWsMessage {
    topic: Option<String>,
    #[serde(rename = "type")]
    msg_type: Option<String>,
    data: Option<Value>,
    ts: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct OrderBookData {
    s: String,  // symbol
    b: Vec<(String, String)>,  // bids
    a: Vec<(String, String)>,  // asks
    u: i64,  // update id
    seq: i64,  // sequence
}

#[derive(Debug, Deserialize)]
struct TradeData {
    #[serde(rename = "T")]
    timestamp: i64,
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "S")]
    side: String,
    #[serde(rename = "v")]
    size: String,
    #[serde(rename = "p")]
    price: String,
}

impl BybitWebSocket {
    pub fn new(url: String, testnet: bool) -> (Self, mpsc::UnboundedReceiver<MarketEvent>) {
        let ws_url = if testnet {
            "wss://stream-testnet.bybit.com/v5/public/spot".to_string()
        } else {
            "wss://stream.bybit.com/v5/public/spot".to_string()
        };

        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let client = Self {
            url: ws_url,
            subscriptions: Vec::new(),
            orderbooks: Arc::new(dashmap::DashMap::new()),
            event_tx,
            connection_state: Arc::new(RwLock::new(ConnectionState {
                connected: false,
                last_message_time: Instant::now(),
                reconnect_count: 0,
                total_messages: 0,
            })),
        };

        (client, event_rx)
    }

    /// Subscribe to orderbook for a symbol
    pub fn subscribe_orderbook(&mut self, symbol: &str, depth: u32) {
        let topic = format!("orderbook.{}.{}", depth, symbol);
        self.subscriptions.push(topic);

        // Initialize orderbook
        self.orderbooks.insert(
            symbol.to_string(),
            Arc::new(RwLock::new(OrderBook::new(symbol.to_string()))),
        );
    }

    /// Subscribe to trades for a symbol
    pub fn subscribe_trades(&mut self, symbol: &str) {
        let topic = format!("publicTrade.{}", symbol);
        self.subscriptions.push(topic);
    }

    /// Start WebSocket connection and event loop
    pub async fn run(mut self) -> Result<()> {
        let mut reconnect_delay = Duration::from_secs(1);
        let max_reconnect_delay = Duration::from_secs(60);

        loop {
            info!("Connecting to Bybit WebSocket: {}", self.url);

            match self.connect_and_run().await {
                Ok(_) => {
                    info!("WebSocket connection closed normally");
                    reconnect_delay = Duration::from_secs(1);
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    self.connection_state.write().connected = false;
                    self.connection_state.write().reconnect_count += 1;

                    let _ = self.event_tx.send(MarketEvent::Disconnected);
                    let _ = self.event_tx.send(MarketEvent::Error(e.to_string()));

                    warn!("Reconnecting in {:?}...", reconnect_delay);
                    tokio::time::sleep(reconnect_delay).await;

                    // Exponential backoff
                    reconnect_delay = (reconnect_delay * 2).min(max_reconnect_delay);
                }
            }
        }
    }

    /// Connect and run message loop
    async fn connect_and_run(&mut self) -> Result<()> {
        // Connect to WebSocket
        let (ws_stream, response) = connect_async(&self.url)
            .await
            .context("Failed to connect to WebSocket")?;

        info!("WebSocket connected: {:?}", response.status());

        let (mut write, read) = ws_stream.split();

        // Update connection state
        {
            let mut state = self.connection_state.write();
            state.connected = true;
            state.last_message_time = Instant::now();
        }

        let _ = self.event_tx.send(MarketEvent::Connected);

        // Subscribe to topics
        self.send_subscriptions(&mut write).await?;

        // Run message loop
        self.message_loop(write, read).await
    }

    /// Send subscriptions
    async fn send_subscriptions(&self, write: &mut WsSink) -> Result<()> {
        if !self.subscriptions.is_empty() {
            let subscribe_msg = serde_json::json!({
                "op": "subscribe",
                "args": self.subscriptions,
            });

            let msg = Message::Text(subscribe_msg.to_string());
            write.send(msg).await
                .context("Failed to send subscription")?;

            info!("Subscribed to {} topics", self.subscriptions.len());
        }

        Ok(())
    }

    /// Main message processing loop
    async fn message_loop(&mut self, mut write: WsSink, mut read: WsSource) -> Result<()> {
        let mut ping_interval = interval(Duration::from_secs(20));
        let mut health_check_interval = interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                // Incoming messages
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            self.handle_message(&text).await?;
                        }
                        Some(Ok(Message::Ping(data))) => {
                            debug!("Received ping");
                            write.send(Message::Pong(data)).await?;
                        }
                        Some(Ok(Message::Pong(_))) => {
                            debug!("Received pong");
                        }
                        Some(Ok(Message::Close(frame))) => {
                            warn!("WebSocket closed: {:?}", frame);
                            return Err(anyhow!("WebSocket closed by server"));
                        }
                        Some(Err(e)) => {
                            error!("WebSocket error: {}", e);
                            return Err(e.into());
                        }
                        None => {
                            warn!("WebSocket stream ended");
                            return Err(anyhow!("WebSocket stream ended"));
                        }
                        _ => {}
                    }
                }

                // Send periodic pings
                _ = ping_interval.tick() => {
                    debug!("Sending ping");
                    write.send(Message::Ping(vec![])).await?;
                }

                // Health check
                _ = health_check_interval.tick() => {
                    let state = self.connection_state.read();
                    let elapsed = state.last_message_time.elapsed();

                    if elapsed > Duration::from_secs(60) {
                        error!("No messages received for {:?}, reconnecting", elapsed);
                        return Err(anyhow!("Connection appears dead"));
                    }
                }
            }
        }
    }

    /// Handle incoming message
    async fn handle_message(&mut self, text: &str) -> Result<()> {
        // Update last message time
        self.connection_state.write().last_message_time = Instant::now();
        self.connection_state.write().total_messages += 1;

        // Try to parse as Bybit message
        let msg: BybitWsMessage = match serde_json::from_str(text) {
            Ok(m) => m,
            Err(e) => {
                debug!("Failed to parse message: {} - {}", e, text);
                return Ok(());
            }
        };

        // Handle different message types
        if let Some(ref topic) = msg.topic {
            if topic.starts_with("orderbook") {
                self.handle_orderbook_update(msg)?;
            } else if topic.starts_with("publicTrade") {
                self.handle_trade_update(msg)?;
            }
        }

        Ok(())
    }

    /// Handle orderbook update
    fn handle_orderbook_update(&self, msg: BybitWsMessage) -> Result<()> {
        let timestamp = msg.ts;
        let data = msg.data.ok_or_else(|| anyhow!("No data in orderbook message"))?;

        let ob_data: OrderBookData = serde_json::from_value(data)
            .context("Failed to parse orderbook data")?;

        // Parse bids and asks
        let bids: Vec<(f64, f64)> = ob_data.b.iter()
            .filter_map(|(price, qty)| {
                Some((price.parse().ok()?, qty.parse().ok()?))
            })
            .collect();

        let asks: Vec<(f64, f64)> = ob_data.a.iter()
            .filter_map(|(price, qty)| {
                Some((price.parse().ok()?, qty.parse().ok()?))
            })
            .collect();

        // Update local orderbook
        if let Some(ob_arc) = self.orderbooks.get(&ob_data.s) {
            let mut ob = ob_arc.write();
            ob.sequence = ob_data.seq as u64;
            ob.last_update = timestamp.unwrap_or(0);

            // Update bids
            for (price, qty) in &bids {
                ob.update_bid(*price, *qty);
            }

            // Update asks
            for (price, qty) in &asks {
                ob.update_ask(*price, *qty);
            }
        }

        // Send event
        let _ = self.event_tx.send(MarketEvent::OrderBookUpdate {
            symbol: ob_data.s.clone(),
            bids,
            asks,
            timestamp: timestamp.unwrap_or(0),
        });

        Ok(())
    }

    /// Handle trade update
    fn handle_trade_update(&self, msg: BybitWsMessage) -> Result<()> {
        let data = msg.data.ok_or_else(|| anyhow!("No data in trade message"))?;

        // Bybit sends array of trades
        let trades: Vec<TradeData> = serde_json::from_value(data)
            .context("Failed to parse trade data")?;

        for trade_data in trades {
            let side = if trade_data.side == "Buy" {
                Side::Buy
            } else {
                Side::Sell
            };

            let trade = Trade {
                timestamp: trade_data.timestamp,
                price: trade_data.price.parse().unwrap_or(0.0),
                quantity: trade_data.size.parse().unwrap_or(0.0),
                side,
            };

            let _ = self.event_tx.send(MarketEvent::Trade {
                symbol: trade_data.symbol.clone(),
                trade,
            });
        }

        Ok(())
    }

    /// Get orderbook for symbol
    pub fn get_orderbook(&self, symbol: &str) -> Option<Arc<RwLock<OrderBook>>> {
        self.orderbooks.get(symbol).map(|r| r.clone())
    }

    /// Get connection state
    pub fn get_connection_state(&self) -> ConnectionState {
        self.connection_state.read().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_websocket_creation() {
        let (ws, _rx) = BybitWebSocket::new("test".to_string(), true);
        assert_eq!(ws.subscriptions.len(), 0);
    }

    #[test]
    fn test_subscribe_orderbook() {
        let (mut ws, _rx) = BybitWebSocket::new("test".to_string(), true);
        ws.subscribe_orderbook("BTCUSDT", 50);
        assert_eq!(ws.subscriptions.len(), 1);
        assert!(ws.subscriptions[0].contains("orderbook"));
    }

    #[test]
    fn test_subscribe_trades() {
        let (mut ws, _rx) = BybitWebSocket::new("test".to_string(), true);
        ws.subscribe_trades("BTCUSDT");
        assert_eq!(ws.subscriptions.len(), 1);
        assert!(ws.subscriptions[0].contains("publicTrade"));
    }
}
