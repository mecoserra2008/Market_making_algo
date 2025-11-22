use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use chrono::Utc;
use anyhow::{Result, anyhow};
use serde::{Serialize, Deserialize};

use super::{OrderSide, OrderStatus};

/// Proper order state management with fill tracking
/// This fixes the critical issue of position tracking
#[derive(Clone)]
pub struct OrderManager {
    orders: Arc<RwLock<HashMap<String, OrderState>>>,
    fills: Arc<RwLock<HashMap<String, Vec<Fill>>>>,
    client_order_id_map: Arc<RwLock<HashMap<String, String>>>,
    position_cache: Arc<RwLock<HashMap<String, f64>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderState {
    pub order_id: String,
    pub client_order_id: String,
    pub symbol: String,
    pub side: OrderSide,
    pub price: Option<f64>,
    pub quantity: f64,
    pub filled_quantity: f64,
    pub remaining_quantity: f64,
    pub status: OrderStatus,
    pub create_time: i64,
    pub update_time: i64,
    pub avg_fill_price: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fill {
    pub fill_id: String,
    pub order_id: String,
    pub symbol: String,
    pub side: OrderSide,
    pub price: f64,
    pub quantity: f64,
    pub timestamp: i64,
    pub fee: f64,
    pub fee_currency: String,
}

impl OrderManager {
    pub fn new() -> Self {
        Self {
            orders: Arc::new(RwLock::new(HashMap::new())),
            fills: Arc::new(RwLock::new(HashMap::new())),
            client_order_id_map: Arc::new(RwLock::new(HashMap::new())),
            position_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new order
    pub fn add_order(&self, order: OrderState) {
        let order_id = order.order_id.clone();
        let client_id = order.client_order_id.clone();

        self.orders.write().insert(order_id.clone(), order);
        self.client_order_id_map.write().insert(client_id, order_id);
    }

    /// Update order status (from exchange updates)
    pub fn update_order(&self, order_id: &str, status: OrderStatus, filled_qty: f64) -> Result<()> {
        let mut orders = self.orders.write();

        let order = orders.get_mut(order_id)
            .ok_or_else(|| anyhow!("Order {} not found", order_id))?;

        order.status = status;
        order.filled_quantity = filled_qty;
        order.remaining_quantity = order.quantity - filled_qty;
        order.update_time = Utc::now().timestamp_millis();

        // Update position cache
        self.update_position_cache(order);

        Ok(())
    }

    /// Record a fill
    pub fn add_fill(&self, fill: Fill) {
        let order_id = fill.order_id.clone();

        // Add to fills list
        self.fills.write()
            .entry(order_id.clone())
            .or_insert_with(Vec::new)
            .push(fill.clone());

        // Update order state
        if let Some(order) = self.orders.write().get_mut(&order_id) {
            let total_filled = self.get_total_filled(&order_id);
            order.filled_quantity = total_filled;
            order.remaining_quantity = order.quantity - total_filled;
            order.avg_fill_price = Some(self.calculate_avg_fill_price(&order_id));
            order.update_time = fill.timestamp;

            if order.filled_quantity >= order.quantity {
                order.status = OrderStatus::Filled;
            } else if order.filled_quantity > 0.0 {
                order.status = OrderStatus::PartiallyFilled;
            }

            // Update position
            self.update_position_cache(order);
        }
    }

    /// Update position cache - recalculate from all fills
    fn update_position_cache(&self, _order: &OrderState) {
        // Recalculate position from all fills to avoid double-counting
        // This is called after every fill, so we need to recompute
        let fills = self.fills.read();
        let mut positions: HashMap<String, f64> = HashMap::new();

        for fills_vec in fills.values() {
            for fill in fills_vec {
                let pos = positions.entry(fill.symbol.clone()).or_insert(0.0);
                match fill.side {
                    OrderSide::Buy => *pos += fill.quantity,
                    OrderSide::Sell => *pos -= fill.quantity,
                }
            }
        }

        *self.position_cache.write() = positions;
    }

    /// Get total filled quantity for an order
    fn get_total_filled(&self, order_id: &str) -> f64 {
        self.fills.read()
            .get(order_id)
            .map(|fills| fills.iter().map(|f| f.quantity).sum())
            .unwrap_or(0.0)
    }

    /// Calculate average fill price
    fn calculate_avg_fill_price(&self, order_id: &str) -> f64 {
        let fills = self.fills.read();

        if let Some(fills_vec) = fills.get(order_id) {
            if fills_vec.is_empty() {
                return 0.0;
            }

            let total_value: f64 = fills_vec.iter()
                .map(|f| f.price * f.quantity)
                .sum();

            let total_qty: f64 = fills_vec.iter()
                .map(|f| f.quantity)
                .sum();

            if total_qty > 0.0 {
                total_value / total_qty
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    /// Get order by ID
    pub fn get_order(&self, order_id: &str) -> Option<OrderState> {
        self.orders.read().get(order_id).cloned()
    }

    /// Get order by client ID
    pub fn get_order_by_client_id(&self, client_order_id: &str) -> Option<OrderState> {
        let client_map = self.client_order_id_map.read();
        let order_id = client_map.get(client_order_id)?;
        self.orders.read().get(order_id).cloned()
    }

    /// Get all active orders for a symbol
    pub fn get_active_orders(&self, symbol: &str) -> Vec<OrderState> {
        self.orders.read()
            .values()
            .filter(|o| {
                o.symbol == symbol &&
                matches!(o.status, OrderStatus::New | OrderStatus::PartiallyFilled)
            })
            .cloned()
            .collect()
    }

    /// Get all fills for a symbol
    pub fn get_fills(&self, symbol: &str) -> Vec<Fill> {
        self.fills.read()
            .values()
            .flatten()
            .filter(|f| f.symbol == symbol)
            .cloned()
            .collect()
    }

    /// Get current position for symbol
    pub fn get_position(&self, symbol: &str) -> f64 {
        *self.position_cache.read().get(symbol).unwrap_or(&0.0)
    }

    /// Calculate realized PnL from fills
    pub fn calculate_realized_pnl(&self, symbol: &str) -> f64 {
        let fills = self.get_fills(symbol);

        let mut position = 0.0;
        let mut cost_basis = 0.0;
        let mut realized_pnl = 0.0;

        for fill in fills {
            match fill.side {
                OrderSide::Buy => {
                    cost_basis += fill.price * fill.quantity;
                    position += fill.quantity;
                }
                OrderSide::Sell => {
                    // Realize PnL
                    if position > 0.0 {
                        let avg_cost = cost_basis / position;
                        let pnl = (fill.price - avg_cost) * fill.quantity;
                        realized_pnl += pnl;

                        position -= fill.quantity;
                        if position > 0.0 {
                            cost_basis = avg_cost * position;
                        } else {
                            cost_basis = 0.0;
                        }
                    }
                }
            }

            // Subtract fees
            realized_pnl -= fill.fee;
        }

        realized_pnl
    }

    /// Reconcile positions with exchange
    pub async fn reconcile<F, Fut>(&self, symbol: &str, fetch_position: F) -> Result<PositionReconciliation>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<f64>>,
    {
        let internal_position = self.get_position(symbol);
        let exchange_position = fetch_position().await?;

        let discrepancy = (exchange_position - internal_position).abs();

        if discrepancy > 0.001 {
            Ok(PositionReconciliation {
                symbol: symbol.to_string(),
                internal_position,
                exchange_position,
                discrepancy,
                reconciled: false,
            })
        } else {
            Ok(PositionReconciliation {
                symbol: symbol.to_string(),
                internal_position,
                exchange_position,
                discrepancy,
                reconciled: true,
            })
        }
    }

    /// Force update position (after reconciliation)
    pub fn force_update_position(&self, symbol: &str, position: f64) {
        self.position_cache.write().insert(symbol.to_string(), position);
    }

    /// Clean up old completed orders
    pub fn cleanup_old_orders(&self, hours: i64) {
        let cutoff = Utc::now().timestamp_millis() - (hours * 3600 * 1000);

        self.orders.write().retain(|_, order| {
            !matches!(order.status, OrderStatus::Filled | OrderStatus::Cancelled) ||
            order.update_time > cutoff
        });

        // Also clean fills
        self.fills.write().retain(|order_id, _| {
            self.orders.read().contains_key(order_id)
        });
    }

    /// Get statistics
    pub fn get_stats(&self, symbol: &str) -> OrderStats {
        let orders = self.orders.read();
        let symbol_orders: Vec<&OrderState> = orders.values()
            .filter(|o| o.symbol == symbol)
            .collect();

        let total_orders = symbol_orders.len();
        let filled_orders = symbol_orders.iter()
            .filter(|o| o.status == OrderStatus::Filled)
            .count();
        let partial_orders = symbol_orders.iter()
            .filter(|o| o.status == OrderStatus::PartiallyFilled)
            .count();
        let cancelled_orders = symbol_orders.iter()
            .filter(|o| o.status == OrderStatus::Cancelled)
            .count();

        let total_volume: f64 = symbol_orders.iter()
            .map(|o| o.filled_quantity)
            .sum();

        OrderStats {
            total_orders,
            filled_orders,
            partial_orders,
            cancelled_orders,
            total_volume,
            current_position: self.get_position(symbol),
            realized_pnl: self.calculate_realized_pnl(symbol),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PositionReconciliation {
    pub symbol: String,
    pub internal_position: f64,
    pub exchange_position: f64,
    pub discrepancy: f64,
    pub reconciled: bool,
}

#[derive(Debug, Clone)]
pub struct OrderStats {
    pub total_orders: usize,
    pub filled_orders: usize,
    pub partial_orders: usize,
    pub cancelled_orders: usize,
    pub total_volume: f64,
    pub current_position: f64,
    pub realized_pnl: f64,
}

impl Default for OrderManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_lifecycle() {
        let manager = OrderManager::new();

        let order = OrderState {
            order_id: "order1".to_string(),
            client_order_id: "client1".to_string(),
            symbol: "BTCUSDT".to_string(),
            side: OrderSide::Buy,
            price: Some(50000.0),
            quantity: 1.0,
            filled_quantity: 0.0,
            remaining_quantity: 1.0,
            status: OrderStatus::New,
            create_time: Utc::now().timestamp_millis(),
            update_time: Utc::now().timestamp_millis(),
            avg_fill_price: None,
        };

        manager.add_order(order);

        // Add partial fill
        manager.add_fill(Fill {
            fill_id: "fill1".to_string(),
            order_id: "order1".to_string(),
            symbol: "BTCUSDT".to_string(),
            side: OrderSide::Buy,
            price: 50000.0,
            quantity: 0.5,
            timestamp: Utc::now().timestamp_millis(),
            fee: 5.0,
            fee_currency: "USDT".to_string(),
        });

        let order = manager.get_order("order1").unwrap();
        assert_eq!(order.filled_quantity, 0.5);
        assert_eq!(order.status, OrderStatus::PartiallyFilled);
        assert_eq!(manager.get_position("BTCUSDT"), 0.5);

        // Complete fill
        manager.add_fill(Fill {
            fill_id: "fill2".to_string(),
            order_id: "order1".to_string(),
            symbol: "BTCUSDT".to_string(),
            side: OrderSide::Buy,
            price: 50100.0,
            quantity: 0.5,
            timestamp: Utc::now().timestamp_millis(),
            fee: 5.0,
            fee_currency: "USDT".to_string(),
        });

        let order = manager.get_order("order1").unwrap();
        assert_eq!(order.filled_quantity, 1.0);
        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(manager.get_position("BTCUSDT"), 1.0);

        // Check average fill price
        let avg_price = order.avg_fill_price.unwrap();
        assert!((avg_price - 50050.0).abs() < 1.0); // Average of 50000 and 50100
    }

    #[test]
    fn test_realized_pnl() {
        let manager = OrderManager::new();

        // Create orders first
        let order1 = OrderState {
            order_id: "order1".to_string(),
            client_order_id: "client1".to_string(),
            symbol: "BTCUSDT".to_string(),
            side: OrderSide::Buy,
            price: Some(50000.0),
            quantity: 1.0,
            filled_quantity: 0.0,
            remaining_quantity: 1.0,
            status: OrderStatus::New,
            create_time: Utc::now().timestamp_millis(),
            update_time: Utc::now().timestamp_millis(),
            avg_fill_price: None,
        };

        let order2 = OrderState {
            order_id: "order2".to_string(),
            client_order_id: "client2".to_string(),
            symbol: "BTCUSDT".to_string(),
            side: OrderSide::Sell,
            price: Some(51000.0),
            quantity: 1.0,
            filled_quantity: 0.0,
            remaining_quantity: 1.0,
            status: OrderStatus::New,
            create_time: Utc::now().timestamp_millis(),
            update_time: Utc::now().timestamp_millis(),
            avg_fill_price: None,
        };

        manager.add_order(order1);
        manager.add_order(order2);

        // Buy at 50000
        manager.add_fill(Fill {
            fill_id: "fill1".to_string(),
            order_id: "order1".to_string(),
            symbol: "BTCUSDT".to_string(),
            side: OrderSide::Buy,
            price: 50000.0,
            quantity: 1.0,
            timestamp: Utc::now().timestamp_millis(),
            fee: 10.0,
            fee_currency: "USDT".to_string(),
        });

        // Sell at 51000
        manager.add_fill(Fill {
            fill_id: "fill2".to_string(),
            order_id: "order2".to_string(),
            symbol: "BTCUSDT".to_string(),
            side: OrderSide::Sell,
            price: 51000.0,
            quantity: 1.0,
            timestamp: Utc::now().timestamp_millis(),
            fee: 10.0,
            fee_currency: "USDT".to_string(),
        });

        let pnl = manager.calculate_realized_pnl("BTCUSDT");
        // Profit: 51000 - 50000 = 1000
        // Fees: 10 + 10 = 20
        // Net: 1000 - 20 = 980
        assert!((pnl - 980.0).abs() < 0.01);
    }
}
