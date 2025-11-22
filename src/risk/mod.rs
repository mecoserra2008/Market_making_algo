use crate::config::RiskConfig;
use crate::models::Position;
use std::collections::HashMap;
use parking_lot::RwLock;
use std::sync::Arc;

/// Risk Manager
/// Monitors and controls trading risk across all positions
pub struct RiskManager {
    config: RiskConfig,
    positions: Arc<RwLock<HashMap<String, Position>>>,
    pnl_history: Arc<RwLock<Vec<PnLRecord>>>,
    total_pnl: Arc<RwLock<f64>>,
    max_drawdown: Arc<RwLock<f64>>,
    peak_pnl: Arc<RwLock<f64>>,
}

#[derive(Debug, Clone)]
pub struct PnLRecord {
    pub timestamp: i64,
    pub realized: f64,
    pub unrealized: f64,
    pub total: f64,
}

#[derive(Debug, Clone)]
pub struct RiskMetrics {
    pub total_pnl: f64,
    pub drawdown: f64,
    pub max_drawdown: f64,
    pub total_position_value: f64,
    pub delta: f64,
    pub gamma: f64,
    pub vega: f64,
    pub position_limit_used_pct: f64,
}

impl RiskManager {
    pub fn new(config: RiskConfig) -> Self {
        Self {
            config,
            positions: Arc::new(RwLock::new(HashMap::new())),
            pnl_history: Arc::new(RwLock::new(Vec::new())),
            total_pnl: Arc::new(RwLock::new(0.0)),
            max_drawdown: Arc::new(RwLock::new(0.0)),
            peak_pnl: Arc::new(RwLock::new(0.0)),
        }
    }

    /// Update position for a symbol
    pub fn update_position(&self, symbol: String, quantity: f64, avg_price: f64, current_price: f64) {
        let unrealized_pnl = if quantity != 0.0 {
            quantity * (current_price - avg_price)
        } else {
            0.0
        };

        let position = Position {
            symbol: symbol.clone(),
            quantity,
            avg_price,
            unrealized_pnl,
            realized_pnl: 0.0,
        };

        self.positions.write().insert(symbol, position);
    }

    /// Update realized PnL from a closed trade
    pub fn record_realized_pnl(&self, pnl: f64) {
        *self.total_pnl.write() += pnl;

        let current_pnl = *self.total_pnl.read();
        let mut peak = self.peak_pnl.write();

        if current_pnl > *peak {
            *peak = current_pnl;
        }

        let drawdown = *peak - current_pnl;
        let mut max_dd = self.max_drawdown.write();

        if drawdown > *max_dd {
            *max_dd = drawdown;
        }
    }

    /// Get current position for a symbol
    pub fn get_position(&self, symbol: &str) -> Option<Position> {
        self.positions.read().get(symbol).cloned()
    }

    /// Get total position value
    pub fn total_position_value(&self) -> f64 {
        self.positions
            .read()
            .values()
            .map(|p| (p.quantity * p.avg_price).abs())
            .sum()
    }

    /// Calculate total delta across all positions
    pub fn total_delta(&self) -> f64 {
        self.positions
            .read()
            .values()
            .map(|p| p.quantity)
            .sum()
    }

    /// Check if order is allowed by risk limits
    pub fn is_order_allowed(
        &self,
        symbol: &str,
        side: crate::exchanges::OrderSide,
        quantity: f64,
        price: f64,
    ) -> Result<(), String> {
        // Check max order value
        let order_value = quantity * price;
        if order_value > self.config.max_order_value {
            return Err(format!(
                "Order value ${:.2} exceeds max ${:.2}",
                order_value, self.config.max_order_value
            ));
        }

        // Check position limits
        let current_position = self.get_position(symbol)
            .map(|p| p.quantity)
            .unwrap_or(0.0);

        let new_position = match side {
            crate::exchanges::OrderSide::Buy => current_position + quantity,
            crate::exchanges::OrderSide::Sell => current_position - quantity,
        };

        if new_position.abs() > self.config.max_position {
            return Err(format!(
                "Position {:.4} would exceed max {:.4}",
                new_position, self.config.max_position
            ));
        }

        // Check drawdown
        let current_dd = *self.peak_pnl.read() - *self.total_pnl.read();
        if current_dd > self.config.max_drawdown {
            return Err(format!(
                "Max drawdown reached: ${:.2}",
                current_dd
            ));
        }

        Ok(())
    }

    /// Check if delta hedge is needed
    pub fn needs_delta_hedge(&self) -> bool {
        let delta = self.total_delta();
        delta.abs() > self.config.delta_hedge_threshold
    }

    /// Calculate required hedge size
    pub fn required_hedge_size(&self) -> f64 {
        -self.total_delta()
    }

    /// Get current risk metrics
    pub fn get_metrics(&self) -> RiskMetrics {
        let total_pnl = *self.total_pnl.read();
        let peak = *self.peak_pnl.read();
        let max_dd = *self.max_drawdown.read();
        let current_dd = peak - total_pnl;

        let position_value = self.total_position_value();
        let position_limit_pct = if self.config.max_position > 0.0 {
            (position_value / self.config.max_position) * 100.0
        } else {
            0.0
        };

        RiskMetrics {
            total_pnl,
            drawdown: current_dd,
            max_drawdown: max_dd,
            total_position_value: position_value,
            delta: self.total_delta(),
            gamma: 0.0, // To be calculated from options positions
            vega: 0.0,  // To be calculated from options positions
            position_limit_used_pct: position_limit_pct,
        }
    }

    /// Record PnL snapshot
    pub fn record_pnl_snapshot(&self, timestamp: i64) {
        let positions = self.positions.read();
        let unrealized: f64 = positions.values().map(|p| p.unrealized_pnl).sum();
        let realized: f64 = positions.values().map(|p| p.realized_pnl).sum();
        let total = unrealized + realized;

        let record = PnLRecord {
            timestamp,
            realized,
            unrealized,
            total,
        };

        self.pnl_history.write().push(record);

        // Keep only last 10000 records
        let mut history = self.pnl_history.write();
        if history.len() > 10000 {
            history.drain(0..1000);
        }
    }

    /// Get PnL history
    pub fn get_pnl_history(&self, limit: usize) -> Vec<PnLRecord> {
        let history = self.pnl_history.read();
        history
            .iter()
            .rev()
            .take(limit)
            .rev()
            .cloned()
            .collect()
    }

    /// Calculate Sharpe ratio (simplified, assumes daily returns)
    pub fn sharpe_ratio(&self, lookback_days: usize) -> Option<f64> {
        let history = self.pnl_history.read();
        if history.len() < lookback_days {
            return None;
        }

        let recent: Vec<f64> = history
            .iter()
            .rev()
            .take(lookback_days)
            .map(|r| r.total)
            .collect();

        if recent.len() < 2 {
            return None;
        }

        let returns: Vec<f64> = recent
            .windows(2)
            .map(|w| w[1] - w[0])
            .collect();

        let mean_return = returns.iter().sum::<f64>() / returns.len() as f64;

        let variance = returns
            .iter()
            .map(|r| (r - mean_return).powi(2))
            .sum::<f64>() / returns.len() as f64;

        let std_dev = variance.sqrt();

        if std_dev == 0.0 {
            return None;
        }

        // Annualize (assuming daily data)
        let sharpe = (mean_return / std_dev) * (365.0_f64).sqrt();

        Some(sharpe)
    }

    /// Emergency stop - cancel all trading
    pub fn should_halt_trading(&self) -> bool {
        let metrics = self.get_metrics();

        // Halt if max drawdown exceeded
        if metrics.drawdown > self.config.max_drawdown {
            return true;
        }

        // Halt if position limits exceeded
        if metrics.position_limit_used_pct > 100.0 {
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position_update() {
        let config = RiskConfig {
            max_position: 10.0,
            max_drawdown: 1000.0,
            max_order_value: 100000.0,
            position_limit_pct: 0.8,
            delta_hedge_threshold: 0.5,
            vega_limit: 1000.0,
            gamma_limit: 100.0,
        };

        let rm = RiskManager::new(config);

        rm.update_position("BTCUSDT".to_string(), 1.0, 50000.0, 51000.0);

        let pos = rm.get_position("BTCUSDT").unwrap();
        assert_eq!(pos.quantity, 1.0);
        assert_eq!(pos.unrealized_pnl, 1000.0);
    }

    #[test]
    fn test_order_validation() {
        let config = RiskConfig {
            max_position: 1.0,
            max_drawdown: 1000.0,
            max_order_value: 10000.0,
            position_limit_pct: 0.8,
            delta_hedge_threshold: 0.5,
            vega_limit: 1000.0,
            gamma_limit: 100.0,
        };

        let rm = RiskManager::new(config);

        // Should allow normal order
        assert!(rm.is_order_allowed(
            "BTCUSDT",
            crate::exchanges::OrderSide::Buy,
            0.1,
            50000.0
        ).is_ok());

        // Should reject order exceeding max value
        assert!(rm.is_order_allowed(
            "BTCUSDT",
            crate::exchanges::OrderSide::Buy,
            1.0,
            50000.0
        ).is_err());
    }
}
