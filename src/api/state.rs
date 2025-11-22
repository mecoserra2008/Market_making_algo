use crate::exchanges::event_bus::EventBus;
use crate::exchanges::order_manager::OrderManager;
use crate::models::orderbook::OrderBook;
use crate::risk::RiskManager;
use crate::utils::circuit_breaker::CircuitBreaker;
use crate::utils::latency_monitor::LatencyMonitor;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::broadcast;

/// Shared application state accessible to all API handlers
#[derive(Clone)]
pub struct AppState {
    /// Trading state (running, stopped, paused)
    pub trading_state: Arc<RwLock<TradingState>>,

    /// Event bus for real-time market data
    pub event_bus: EventBus,

    /// Order manager for tracking orders and positions
    pub order_manager: Arc<OrderManager>,

    /// Risk manager for position limits and drawdowns
    pub risk_manager: Arc<RwLock<RiskManager>>,

    /// Circuit breaker for fault tolerance
    pub circuit_breaker: Arc<CircuitBreaker>,

    /// Latency monitor for performance tracking
    pub latency_monitor: Arc<LatencyMonitor>,

    /// Current orderbook (for display purposes)
    pub orderbook: Arc<RwLock<OrderBook>>,

    /// Configuration (for display and updates)
    pub config: Arc<RwLock<crate::config::Config>>,

    /// Shutdown signal sender
    pub shutdown_tx: Arc<RwLock<Option<broadcast::Sender<()>>>>,

    /// System start time
    pub start_time: std::time::Instant,
}

impl AppState {
    /// Create new application state
    pub fn new(
        event_bus: EventBus,
        order_manager: Arc<OrderManager>,
        risk_manager: Arc<RwLock<RiskManager>>,
        circuit_breaker: Arc<CircuitBreaker>,
        latency_monitor: Arc<LatencyMonitor>,
        orderbook: Arc<RwLock<OrderBook>>,
        config: Arc<RwLock<crate::config::Config>>,
    ) -> Self {
        Self {
            trading_state: Arc::new(RwLock::new(TradingState::Stopped)),
            event_bus,
            order_manager,
            risk_manager,
            circuit_breaker,
            latency_monitor,
            orderbook,
            config,
            shutdown_tx: Arc::new(RwLock::new(None)),
            start_time: std::time::Instant::now(),
        }
    }

    /// Get uptime in seconds
    pub fn uptime_seconds(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Check if trading is running
    pub fn is_running(&self) -> bool {
        matches!(*self.trading_state.read(), TradingState::Running)
    }

    /// Get trading state
    pub fn get_state(&self) -> TradingState {
        *self.trading_state.read()
    }

    /// Set trading state
    pub fn set_state(&self, state: TradingState) {
        *self.trading_state.write() = state;
    }
}

/// Trading state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradingState {
    /// Trading is running
    Running,
    /// Trading is stopped
    Stopped,
    /// Trading is paused (orders remain active)
    Paused,
}

impl std::fmt::Display for TradingState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TradingState::Running => write!(f, "running"),
            TradingState::Stopped => write!(f, "stopped"),
            TradingState::Paused => write!(f, "paused"),
        }
    }
}
