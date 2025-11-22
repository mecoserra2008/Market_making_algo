use serde::{Deserialize, Serialize};
use std::time::Duration;

// =============================================================================
// System Status Models
// =============================================================================

#[derive(Debug, Serialize)]
pub struct SystemStatusResponse {
    pub trading_state: String,
    pub uptime_seconds: u64,
    pub last_restart: String,
    pub version: String,
    pub connections: ConnectionsStatus,
    pub resources: ResourceUsage,
}

#[derive(Debug, Serialize, Clone)]
pub struct ConnectionsStatus {
    pub bybit_websocket: ConnectionInfo,
    pub bybit_rest: ConnectionInfo,
    pub deribit_rest: ConnectionInfo,
}

#[derive(Debug, Serialize, Clone)]
pub struct ConnectionInfo {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_call: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ResourceUsage {
    pub cpu_percent: f64,
    pub memory_mb: u64,
}

// =============================================================================
// Position Models
// =============================================================================

#[derive(Debug, Serialize)]
pub struct PositionsResponse {
    pub bybit: BybitPosition,
    pub deribit: Vec<DeribitPosition>,
    pub net_delta: f64,
    pub realized_pnl_session: f64,
    pub realized_pnl_total: f64,
}

#[derive(Debug, Serialize)]
pub struct BybitPosition {
    pub symbol: String,
    pub position_size: f64,
    pub entry_price: f64,
    pub mark_price: f64,
    pub unrealized_pnl: f64,
    pub unrealized_pnl_percent: f64,
    pub delta: f64,
}

#[derive(Debug, Serialize)]
pub struct DeribitPosition {
    pub instrument: String,
    pub position: i32,
    pub delta: f64,
    pub mark_price: f64,
}

// =============================================================================
// Risk Models
// =============================================================================

#[derive(Debug, Serialize)]
pub struct RiskMetricsResponse {
    pub circuit_breaker: CircuitBreakerInfo,
    pub risk_score: f64,
    pub position_utilization: Utilization,
    pub drawdown: DrawdownInfo,
    pub margin_usage_percent: f64,
    pub alerts: Vec<Alert>,
}

#[derive(Debug, Serialize)]
pub struct CircuitBreakerInfo {
    pub state: String,
    pub failure_rate: f64,
    pub consecutive_failures: usize,
    pub total_calls: u64,
}

#[derive(Debug, Serialize)]
pub struct Utilization {
    pub current: f64,
    pub limit: f64,
    pub percent: f64,
}

#[derive(Debug, Serialize)]
pub struct DrawdownInfo {
    pub current: f64,
    pub maximum: f64,
    pub limit: f64,
    pub percent: f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct Alert {
    pub severity: String,
    pub message: String,
    pub timestamp: String,
}

// =============================================================================
// Performance Metrics Models
// =============================================================================

#[derive(Debug, Serialize)]
pub struct PerformanceMetricsResponse {
    pub latency: LatencyMetrics,
    pub trading: TradingMetrics,
    pub market_quality: MarketQualityMetrics,
    pub error_rate: f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct LatencyMetrics {
    pub orderbook: LatencyStats,
    pub trades: LatencyStats,
    pub order_placement: LatencyStats,
}

#[derive(Debug, Serialize, Clone)]
pub struct LatencyStats {
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub count: usize,
}

#[derive(Debug, Serialize, Clone)]
pub struct TradingMetrics {
    pub volume_traded: f64,
    pub num_trades: u64,
    pub avg_spread_bps: f64,
    pub fill_ratio: f64,
}

#[derive(Debug, Serialize, Clone)]
pub struct MarketQualityMetrics {
    pub vpin_score: f64,
    pub adverse_selection_score: f64,
}

// =============================================================================
// Order Models
// =============================================================================

#[derive(Debug, Serialize)]
pub struct OrdersResponse {
    pub total: usize,
    pub orders: Vec<OrderInfo>,
}

#[derive(Debug, Serialize)]
pub struct OrderInfo {
    pub order_id: String,
    pub timestamp: String,
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub price: f64,
    pub quantity: f64,
    pub filled_quantity: f64,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_fill_price: Option<f64>,
}

// =============================================================================
// Configuration Models
// =============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigResponse {
    pub strategy: StrategyConfig,
    pub risk: RiskConfig,
    pub system: SystemConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub risk_aversion: f64,
    pub volatility: f64,
    pub target_inventory: f64,
    pub num_levels: usize,
    pub level_spacing: f64,
    pub vpin_threshold: f64,
    pub adverse_selection_threshold: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RiskConfig {
    pub max_position: f64,
    pub max_drawdown: f64,
    pub min_spread_bps: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemConfig {
    pub paper_trading: bool,
    pub quote_refresh_ms: u64,
    pub hedge_interval_ms: u64,
}

#[derive(Debug, Deserialize)]
pub struct ConfigUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<StrategyConfigUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk: Option<RiskConfigUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemConfigUpdate>,
}

#[derive(Debug, Deserialize)]
pub struct StrategyConfigUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_aversion: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volatility: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_inventory: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_levels: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level_spacing: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vpin_threshold: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adverse_selection_threshold: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct RiskConfigUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_position: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_drawdown: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_spread_bps: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct SystemConfigUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paper_trading: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote_refresh_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hedge_interval_ms: Option<u64>,
}

// =============================================================================
// Control Operations Models
// =============================================================================

#[derive(Debug, Serialize)]
pub struct ControlResponse {
    pub success: bool,
    pub message: String,
    pub timestamp: String,
}

// =============================================================================
// Log Models
// =============================================================================

#[derive(Debug, Serialize)]
pub struct LogsResponse {
    pub total: usize,
    pub logs: Vec<LogEntry>,
}

#[derive(Debug, Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub message: String,
    pub module: String,
}

// =============================================================================
// Generic Response Models
// =============================================================================

#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub details: Option<String>,
}

// =============================================================================
// WebSocket Models
// =============================================================================

#[derive(Debug, Serialize, Clone)]
#[serde(tag = "type")]
pub enum WsMessage {
    #[serde(rename = "orderbook_update")]
    OrderBookUpdate {
        symbol: String,
        timestamp: i64,
        bids: Vec<(f64, f64)>,
        asks: Vec<(f64, f64)>,
        mid_price: f64,
        spread: f64,
        spread_bps: f64,
        imbalance: f64,
    },
    #[serde(rename = "trade")]
    Trade {
        symbol: String,
        timestamp: i64,
        price: f64,
        quantity: f64,
        side: String,
    },
    #[serde(rename = "position_update")]
    PositionUpdate {
        timestamp: i64,
        position_size: f64,
        entry_price: f64,
        mark_price: f64,
        unrealized_pnl: f64,
        realized_pnl_session: f64,
        delta: f64,
        net_delta: f64,
    },
    #[serde(rename = "metrics_update")]
    MetricsUpdate {
        timestamp: i64,
        latency: LatencyMetrics,
        trading: TradingMetrics,
        market_quality: MarketQualityMetrics,
    },
    #[serde(rename = "risk_update")]
    RiskUpdate {
        timestamp: i64,
        circuit_breaker_state: String,
        risk_score: f64,
        position_utilization_percent: f64,
        drawdown_percent: f64,
        margin_usage_percent: f64,
        new_alerts: Vec<Alert>,
    },
    #[serde(rename = "order_update")]
    OrderUpdate {
        order_id: String,
        timestamp: i64,
        symbol: String,
        side: String,
        price: f64,
        quantity: f64,
        filled_quantity: f64,
        status: String,
        avg_fill_price: Option<f64>,
    },
    #[serde(rename = "log")]
    Log {
        timestamp: i64,
        level: String,
        message: String,
        module: String,
    },
    #[serde(rename = "health_update")]
    HealthUpdate {
        timestamp: i64,
        trading_state: String,
        connections: ConnectionsStatus,
        cpu_percent: f64,
        memory_mb: u64,
    },
}
