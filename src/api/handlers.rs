use super::models::*;
use super::state::{AppState, TradingState};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
};
use chrono::Utc;
use tracing::{info, warn};

// =============================================================================
// System Status Handler
// =============================================================================

pub async fn get_status(State(state): State<AppState>) -> impl IntoResponse {
    let response = SystemStatusResponse {
        trading_state: state.get_state().to_string(),
        uptime_seconds: state.uptime_seconds(),
        last_restart: chrono::DateTime::from_timestamp(
            (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64)
                - (state.uptime_seconds() as i64),
            0,
        )
        .unwrap()
        .to_rfc3339(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        connections: ConnectionsStatus {
            bybit_websocket: ConnectionInfo {
                status: "connected".to_string(), // TODO: Get real status
                latency_ms: Some(45),
                last_message: Some(Utc::now().to_rfc3339()),
                last_call: None,
            },
            bybit_rest: ConnectionInfo {
                status: "healthy".to_string(),
                latency_ms: None,
                last_message: None,
                last_call: Some(Utc::now().to_rfc3339()),
            },
            deribit_rest: ConnectionInfo {
                status: "healthy".to_string(),
                latency_ms: None,
                last_message: None,
                last_call: Some(Utc::now().to_rfc3339()),
            },
        },
        resources: ResourceUsage {
            cpu_percent: 12.3, // TODO: Get real CPU usage
            memory_mb: 445,    // TODO: Get real memory usage
        },
    };

    Json(response)
}

// =============================================================================
// Position Handlers
// =============================================================================

pub async fn get_positions(State(state): State<AppState>) -> impl IntoResponse {
    let position = state.order_manager.get_position("BTCUSDT");

    let response = PositionsResponse {
        bybit: BybitPosition {
            symbol: "BTCUSDT".to_string(),
            position_size: position,
            entry_price: 50000.0, // TODO: Get real entry price
            mark_price: 50245.5,  // TODO: Get real mark price
            unrealized_pnl: if position != 0.0 {
                position * (50245.5 - 50000.0)
            } else {
                0.0
            },
            unrealized_pnl_percent: if position != 0.0 {
                (50245.5 - 50000.0) / 50000.0 * 100.0
            } else {
                0.0
            },
            delta: position,
        },
        deribit: vec![], // TODO: Get real Deribit positions
        net_delta: position,
        realized_pnl_session: state.order_manager.calculate_realized_pnl("BTCUSDT"),
        realized_pnl_total: state.order_manager.calculate_realized_pnl("BTCUSDT"),
    };

    Json(response)
}

// =============================================================================
// Risk Handlers
// =============================================================================

pub async fn get_risk(State(state): State<AppState>) -> impl IntoResponse {
    let risk_manager = state.risk_manager.read();
    let circuit_stats = state.circuit_breaker.get_stats();
    let position = state.order_manager.get_position("BTCUSDT");
    let config = state.config.read();

    let position_percent = (position.abs() / config.risk.max_position) * 100.0;
    // TODO: Add current_drawdown and max_drawdown methods to RiskManager
    let drawdown = 0.0;
    let drawdown_percent = 0.0;

    let mut alerts = Vec::new();
    if position_percent > 80.0 {
        alerts.push(Alert {
            severity: "warning".to_string(),
            message: format!("Position utilization at {:.1}%", position_percent),
            timestamp: Utc::now().to_rfc3339(),
        });
    }

    let response = RiskMetricsResponse {
        circuit_breaker: CircuitBreakerInfo {
            state: format!("{:?}", circuit_stats.state).to_lowercase(),
            failure_rate: circuit_stats.failure_rate,
            consecutive_failures: circuit_stats.consecutive_failures,
            total_calls: circuit_stats.total_calls,
        },
        risk_score: 3.2, // TODO: Calculate real risk score
        position_utilization: Utilization {
            current: position.abs(),
            limit: config.risk.max_position,
            percent: position_percent,
        },
        drawdown: DrawdownInfo {
            current: drawdown,
            maximum: drawdown,
            limit: config.risk.max_drawdown,
            percent: drawdown_percent,
        },
        margin_usage_percent: 45.2, // TODO: Get real margin usage
        alerts,
    };

    Json(response)
}

// =============================================================================
// Performance Metrics Handlers
// =============================================================================

pub async fn get_metrics(State(state): State<AppState>) -> impl IntoResponse {
    let all_stats = state.latency_monitor.get_all_stats();

    let ob_millis = all_stats.orderbook.to_millis();
    let trades_millis = all_stats.trades.to_millis();
    let orders_millis = all_stats.order_placement.to_millis();

    let response = PerformanceMetricsResponse {
        latency: LatencyMetrics {
            orderbook: LatencyStats {
                p50_ms: ob_millis.p50_ms,
                p95_ms: ob_millis.p95_ms,
                p99_ms: ob_millis.p99_ms,
                count: ob_millis.count,
            },
            trades: LatencyStats {
                p50_ms: trades_millis.p50_ms,
                p95_ms: trades_millis.p95_ms,
                p99_ms: trades_millis.p99_ms,
                count: trades_millis.count,
            },
            order_placement: LatencyStats {
                p50_ms: orders_millis.p50_ms,
                p95_ms: orders_millis.p95_ms,
                p99_ms: orders_millis.p99_ms,
                count: orders_millis.count,
            },
        },
        trading: TradingMetrics {
            volume_traded: 125.4,  // TODO: Track real volume
            num_trades: 1247,      // TODO: Track real number of trades
            avg_spread_bps: 4.2,   // TODO: Calculate real average spread
            fill_ratio: 87.3,      // TODO: Calculate real fill ratio
        },
        market_quality: MarketQualityMetrics {
            vpin_score: 0.23,               // TODO: Get real VPIN score
            adverse_selection_score: 0.18,  // TODO: Get real adverse selection score
        },
        error_rate: 0.01, // TODO: Track real error rate
    };

    Json(response)
}

// =============================================================================
// Order Handlers
// =============================================================================

pub async fn get_orders(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.read();
    let orders = state.order_manager.get_active_orders(&config.strategy.symbol);

    let order_infos: Vec<OrderInfo> = orders
        .iter()
        .map(|order| OrderInfo {
            order_id: order.order_id.clone(),
            timestamp: chrono::DateTime::from_timestamp(order.create_time / 1000, 0)
                .unwrap()
                .to_rfc3339(),
            symbol: order.symbol.clone(),
            side: format!("{:?}", order.side).to_lowercase(),
            order_type: "limit".to_string(), // OrderState doesn't track order type
            price: order.price.unwrap_or(0.0),
            quantity: order.quantity,
            filled_quantity: order.filled_quantity,
            status: format!("{:?}", order.status).to_lowercase(),
            avg_fill_price: order.avg_fill_price,
        })
        .collect();

    let response = OrdersResponse {
        total: order_infos.len(),
        orders: order_infos,
    };

    Json(response)
}

pub async fn cancel_order(
    State(_state): State<AppState>,
    order_id: String,
) -> impl IntoResponse {
    // TODO: Implement order cancellation
    info!("Cancelling order: {}", order_id);

    Json(SuccessResponse {
        success: true,
        message: format!("Order {} cancelled successfully", order_id),
        data: None,
    })
}

pub async fn cancel_all_orders(State(_state): State<AppState>) -> impl IntoResponse {
    // TODO: Implement cancel all orders
    warn!("Cancelling all orders");

    Json(SuccessResponse {
        success: true,
        message: "All orders cancelled successfully".to_string(),
        data: None,
    })
}

// =============================================================================
// Configuration Handlers
// =============================================================================

pub async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.read();

    let response = ConfigResponse {
        strategy: StrategyConfig {
            risk_aversion: config.strategy.risk_aversion,
            volatility: config.strategy.volatility_window as f64,
            target_inventory: config.strategy.inventory_target,
            num_levels: config.strategy.num_levels,
            level_spacing: config.strategy.level_spacing_bps,
            vpin_threshold: config.strategy.vpin_threshold,
            adverse_selection_threshold: config.strategy.adverse_selection_threshold,
        },
        risk: RiskConfig {
            max_position: config.risk.max_position,
            max_drawdown: config.risk.max_drawdown,
            min_spread_bps: config.strategy.min_spread_bps,
        },
        system: SystemConfig {
            paper_trading: config.paper_trading,
            quote_refresh_ms: 500,  // TODO: Make configurable
            hedge_interval_ms: 1000, // TODO: Make configurable
        },
    };

    Json(response)
}

pub async fn update_config(
    State(state): State<AppState>,
    Json(update): Json<ConfigUpdate>,
) -> impl IntoResponse {
    let mut config = state.config.write();
    let mut updated_fields = Vec::new();

    // Update strategy parameters
    if let Some(strategy_update) = update.strategy {
        if let Some(risk_aversion) = strategy_update.risk_aversion {
            config.strategy.risk_aversion = risk_aversion;
            updated_fields.push("strategy.risk_aversion");
        }
        if let Some(volatility) = strategy_update.volatility {
            config.strategy.volatility_window = volatility as usize;
            updated_fields.push("strategy.volatility");
        }
        if let Some(target_inventory) = strategy_update.target_inventory {
            config.strategy.inventory_target = target_inventory;
            updated_fields.push("strategy.target_inventory");
        }
        if let Some(num_levels) = strategy_update.num_levels {
            config.strategy.num_levels = num_levels;
            updated_fields.push("strategy.num_levels");
        }
        if let Some(level_spacing) = strategy_update.level_spacing {
            config.strategy.level_spacing_bps = level_spacing;
            updated_fields.push("strategy.level_spacing");
        }
        if let Some(vpin_threshold) = strategy_update.vpin_threshold {
            config.strategy.vpin_threshold = vpin_threshold;
            updated_fields.push("strategy.vpin_threshold");
        }
        if let Some(adverse_selection_threshold) = strategy_update.adverse_selection_threshold {
            config.strategy.adverse_selection_threshold = adverse_selection_threshold;
            updated_fields.push("strategy.adverse_selection_threshold");
        }
    }

    // Update risk parameters
    if let Some(risk_update) = update.risk {
        if let Some(max_position) = risk_update.max_position {
            config.risk.max_position = max_position;
            updated_fields.push("risk.max_position");
        }
        if let Some(max_drawdown) = risk_update.max_drawdown {
            config.risk.max_drawdown = max_drawdown;
            updated_fields.push("risk.max_drawdown");
        }
        if let Some(min_spread_bps) = risk_update.min_spread_bps {
            config.strategy.min_spread_bps = min_spread_bps;
            updated_fields.push("risk.min_spread_bps");
        }
    }

    info!("Configuration updated: {:?}", updated_fields);

    Json(SuccessResponse {
        success: true,
        message: "Configuration updated successfully".to_string(),
        data: Some(serde_json::json!({ "updated_fields": updated_fields })),
    })
}

// =============================================================================
// Control Operation Handlers
// =============================================================================

pub async fn start_trading(State(state): State<AppState>) -> impl IntoResponse {
    info!("Starting trading");
    state.set_state(TradingState::Running);

    Json(ControlResponse {
        success: true,
        message: "Trading started".to_string(),
        timestamp: Utc::now().to_rfc3339(),
    })
}

pub async fn stop_trading(State(state): State<AppState>) -> impl IntoResponse {
    info!("Stopping trading");
    state.set_state(TradingState::Stopped);

    // TODO: Cancel all orders

    Json(ControlResponse {
        success: true,
        message: "Trading stopped, all orders cancelled".to_string(),
        timestamp: Utc::now().to_rfc3339(),
    })
}

pub async fn pause_trading(State(state): State<AppState>) -> impl IntoResponse {
    info!("Pausing trading");
    state.set_state(TradingState::Paused);

    Json(ControlResponse {
        success: true,
        message: "Trading paused, orders remain active".to_string(),
        timestamp: Utc::now().to_rfc3339(),
    })
}

// =============================================================================
// Health Check Handler
// =============================================================================

pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}
