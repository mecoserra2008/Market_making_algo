pub mod bybit;
pub mod deribit;
pub mod order_manager;
pub mod websocket;
pub mod event_bus;

use async_trait::async_trait;
use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub order_id: String,
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub price: Option<f64>,
    pub quantity: f64,
    pub filled_quantity: f64,
    pub status: OrderStatus,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Limit,
    Market,
    PostOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    New,
    PartiallyFilled,
    Filled,
    Cancelled,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Balance {
    pub asset: String,
    pub free: f64,
    pub locked: f64,
}

/// Common exchange interface
#[async_trait]
pub trait Exchange: Send + Sync {
    async fn place_order(&self, order: NewOrder) -> Result<Order>;
    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<()>;
    async fn cancel_all_orders(&self, symbol: &str) -> Result<()>;
    async fn get_order(&self, symbol: &str, order_id: &str) -> Result<Order>;
    async fn get_open_orders(&self, symbol: &str) -> Result<Vec<Order>>;
    async fn get_balance(&self) -> Result<Vec<Balance>>;
}

#[derive(Debug, Clone)]
pub struct NewOrder {
    pub symbol: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub price: Option<f64>,
    pub quantity: f64,
    pub time_in_force: TimeInForce,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeInForce {
    GTC,  // Good Till Cancel
    IOC,  // Immediate Or Cancel
    FOK,  // Fill Or Kill
    PostOnly,  // Post-only (maker-only)
}
