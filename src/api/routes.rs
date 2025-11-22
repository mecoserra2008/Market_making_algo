use super::handlers;
use super::state::AppState;
use super::websocket;
use axum::{
    extract::Path,
    routing::{delete, get, post, put},
    Router,
};

/// Create API routes (REST endpoints)
pub fn api_routes() -> Router<AppState> {
    Router::new()
        // System status
        .route("/api/v1/status", get(handlers::get_status))
        .route("/api/v1/health", get(handlers::health_check))
        // Positions
        .route("/api/v1/positions", get(handlers::get_positions))
        // Risk metrics
        .route("/api/v1/risk", get(handlers::get_risk))
        // Performance metrics
        .route("/api/v1/metrics", get(handlers::get_metrics))
        // Orders
        .route("/api/v1/orders", get(handlers::get_orders))
        .route(
            "/api/v1/orders/:order_id",
            delete(|Path(order_id): Path<String>, state| {
                handlers::cancel_order(state, order_id)
            }),
        )
        .route("/api/v1/orders/all", delete(handlers::cancel_all_orders))
        // Configuration
        .route("/api/v1/config", get(handlers::get_config))
        .route("/api/v1/config", put(handlers::update_config))
        // Control operations
        .route("/api/v1/control/start", post(handlers::start_trading))
        .route("/api/v1/control/stop", post(handlers::stop_trading))
        .route("/api/v1/control/pause", post(handlers::pause_trading))
}

/// Create WebSocket routes
pub fn websocket_routes() -> Router<AppState> {
    Router::new()
        .route("/ws/market", get(websocket::ws_market_handler))
        .route("/ws/positions", get(websocket::ws_positions_handler))
        .route("/ws/performance", get(websocket::ws_performance_handler))
        .route("/ws/risk", get(websocket::ws_risk_handler))
        .route("/ws/orders", get(websocket::ws_orders_handler))
        .route("/ws/logs", get(websocket::ws_logs_handler))
        .route("/ws/health", get(websocket::ws_health_handler))
}
