use super::models::WsMessage;
use super::state::AppState;
use axum::{
    extract::{
        ws::{Message, WebSocket},
        State, WebSocketUpgrade,
    },
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error, info};

// =============================================================================
// WebSocket Handler Functions
// =============================================================================

pub async fn ws_market_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| ws_market_connection(socket, state))
}

pub async fn ws_positions_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| ws_positions_connection(socket, state))
}

pub async fn ws_performance_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| ws_performance_connection(socket, state))
}

pub async fn ws_risk_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| ws_risk_connection(socket, state))
}

pub async fn ws_orders_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| ws_orders_connection(socket, state))
}

pub async fn ws_logs_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| ws_logs_connection(socket, state))
}

pub async fn ws_health_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| ws_health_connection(socket, state))
}

// =============================================================================
// WebSocket Connection Handlers
// =============================================================================

async fn ws_market_connection(socket: WebSocket, state: AppState) {
    info!("New WebSocket connection: /ws/market");

    let (mut sender, mut receiver) = socket.split();

    // Subscribe to market events from EventBus
    let mut event_rx = state.event_bus.subscribe();

    // Spawn task to forward market events to WebSocket
    let mut send_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            // Convert MarketEvent to WsMessage
            let ws_msg = match event.as_ref() {
                crate::exchanges::websocket::MarketEvent::OrderBookUpdate {
                    symbol,
                    bids,
                    asks,
                    timestamp,
                } => {
                    let mid_price = if !bids.is_empty() && !asks.is_empty() {
                        (bids[0].0 + asks[0].0) / 2.0
                    } else {
                        0.0
                    };

                    let spread = if !bids.is_empty() && !asks.is_empty() {
                        asks[0].0 - bids[0].0
                    } else {
                        0.0
                    };

                    let spread_bps = if mid_price > 0.0 {
                        (spread / mid_price) * 10000.0
                    } else {
                        0.0
                    };

                    let total_bid_vol: f64 = bids.iter().map(|(_, v)| v).sum();
                    let total_ask_vol: f64 = asks.iter().map(|(_, v)| v).sum();
                    let imbalance = if total_bid_vol + total_ask_vol > 0.0 {
                        total_bid_vol / (total_bid_vol + total_ask_vol)
                    } else {
                        0.5
                    };

                    WsMessage::OrderBookUpdate {
                        symbol: symbol.clone(),
                        timestamp: *timestamp,
                        bids: bids.clone(),
                        asks: asks.clone(),
                        mid_price,
                        spread,
                        spread_bps,
                        imbalance,
                    }
                }
                crate::exchanges::websocket::MarketEvent::Trade { symbol, trade } => {
                    WsMessage::Trade {
                        symbol: symbol.clone(),
                        timestamp: trade.timestamp,
                        price: trade.price,
                        quantity: trade.quantity,
                        side: format!("{:?}", trade.side).to_lowercase(),
                    }
                }
                _ => continue, // Ignore other event types
            };

            let json = serde_json::to_string(&ws_msg).unwrap();
            if sender.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    });

    // Receive messages from client (for heartbeat/close)
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    info!("WebSocket connection closed: /ws/market");
}

async fn ws_positions_connection(socket: WebSocket, _state: AppState) {
    info!("New WebSocket connection: /ws/positions");

    let (mut sender, mut receiver) = socket.split();

    // Send position updates every 1 second
    let mut interval = interval(Duration::from_secs(1));

    let mut send_task = tokio::spawn(async move {
        loop {
            interval.tick().await;

            // TODO: Get real position data
            let msg = WsMessage::PositionUpdate {
                timestamp: chrono::Utc::now().timestamp(),
                position_size: 2.5,
                entry_price: 50000.0,
                mark_price: 50245.5,
                unrealized_pnl: 613.75,
                realized_pnl_session: 1247.50,
                delta: 2.5,
                net_delta: 2.1,
            };

            let json = serde_json::to_string(&msg).unwrap();
            if sender.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    info!("WebSocket connection closed: /ws/positions");
}

async fn ws_performance_connection(socket: WebSocket, state: AppState) {
    info!("New WebSocket connection: /ws/performance");

    let (mut sender, mut receiver) = socket.split();

    // Send performance metrics every 30 seconds
    let mut interval = interval(Duration::from_secs(30));

    let mut send_task = tokio::spawn(async move {
        loop {
            interval.tick().await;

            let all_stats = state.latency_monitor.get_all_stats();
            let ob_millis = all_stats.orderbook.to_millis();
            let trades_millis = all_stats.trades.to_millis();
            let orders_millis = all_stats.order_placement.to_millis();

            let msg = WsMessage::MetricsUpdate {
                timestamp: chrono::Utc::now().timestamp(),
                latency: super::models::LatencyMetrics {
                    orderbook: super::models::LatencyStats {
                        p50_ms: ob_millis.p50_ms,
                        p95_ms: ob_millis.p95_ms,
                        p99_ms: ob_millis.p99_ms,
                        count: ob_millis.count,
                    },
                    trades: super::models::LatencyStats {
                        p50_ms: trades_millis.p50_ms,
                        p95_ms: trades_millis.p95_ms,
                        p99_ms: trades_millis.p99_ms,
                        count: trades_millis.count,
                    },
                    order_placement: super::models::LatencyStats {
                        p50_ms: orders_millis.p50_ms,
                        p95_ms: orders_millis.p95_ms,
                        p99_ms: orders_millis.p99_ms,
                        count: orders_millis.count,
                    },
                },
                trading: super::models::TradingMetrics {
                    volume_traded: 125.4,
                    num_trades: 1247,
                    avg_spread_bps: 4.2,
                    fill_ratio: 87.3,
                },
                market_quality: super::models::MarketQualityMetrics {
                    vpin_score: 0.23,
                    adverse_selection_score: 0.18,
                },
            };

            let json = serde_json::to_string(&msg).unwrap();
            if sender.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    info!("WebSocket connection closed: /ws/performance");
}

async fn ws_risk_connection(socket: WebSocket, state: AppState) {
    info!("New WebSocket connection: /ws/risk");

    let (mut sender, mut receiver) = socket.split();

    // Send risk updates every 5 seconds
    let mut interval = interval(Duration::from_secs(5));

    let mut send_task = tokio::spawn(async move {
        loop {
            interval.tick().await;

            let circuit_stats = state.circuit_breaker.get_stats();

            let msg = WsMessage::RiskUpdate {
                timestamp: chrono::Utc::now().timestamp(),
                circuit_breaker_state: format!("{:?}", circuit_stats.state).to_lowercase(),
                risk_score: 3.2,
                position_utilization_percent: 50.0,
                drawdown_percent: 24.5,
                margin_usage_percent: 45.2,
                new_alerts: vec![],
            };

            let json = serde_json::to_string(&msg).unwrap();
            if sender.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    info!("WebSocket connection closed: /ws/risk");
}

async fn ws_orders_connection(socket: WebSocket, _state: AppState) {
    info!("New WebSocket connection: /ws/orders");

    let (mut sender, mut receiver) = socket.split();

    // TODO: Subscribe to order events and forward to WebSocket

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    // Keep connection open
    let mut send_task = tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            if sender.send(Message::Ping(vec![])).await.is_err() {
                break;
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    info!("WebSocket connection closed: /ws/orders");
}

async fn ws_logs_connection(socket: WebSocket, _state: AppState) {
    info!("New WebSocket connection: /ws/logs");

    let (mut sender, mut receiver) = socket.split();

    // TODO: Subscribe to log events and forward to WebSocket

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    // Keep connection open
    let mut send_task = tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            if sender.send(Message::Ping(vec![])).await.is_err() {
                break;
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    info!("WebSocket connection closed: /ws/logs");
}

async fn ws_health_connection(socket: WebSocket, state: AppState) {
    info!("New WebSocket connection: /ws/health");

    let (mut sender, mut receiver) = socket.split();

    // Send health updates every 5 seconds
    let mut interval = interval(Duration::from_secs(5));

    let mut send_task = tokio::spawn(async move {
        loop {
            interval.tick().await;

            let msg = WsMessage::HealthUpdate {
                timestamp: chrono::Utc::now().timestamp(),
                trading_state: state.get_state().to_string(),
                connections: super::models::ConnectionsStatus {
                    bybit_websocket: super::models::ConnectionInfo {
                        status: "connected".to_string(),
                        latency_ms: Some(45),
                        last_message: Some(chrono::Utc::now().to_rfc3339()),
                        last_call: None,
                    },
                    bybit_rest: super::models::ConnectionInfo {
                        status: "healthy".to_string(),
                        latency_ms: None,
                        last_message: None,
                        last_call: Some(chrono::Utc::now().to_rfc3339()),
                    },
                    deribit_rest: super::models::ConnectionInfo {
                        status: "healthy".to_string(),
                        latency_ms: None,
                        last_message: None,
                        last_call: Some(chrono::Utc::now().to_rfc3339()),
                    },
                },
                cpu_percent: 12.3,
                memory_mb: 445,
            };

            let json = serde_json::to_string(&msg).unwrap();
            if sender.send(Message::Text(json)).await.is_err() {
                break;
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    }

    info!("WebSocket connection closed: /ws/health");
}
