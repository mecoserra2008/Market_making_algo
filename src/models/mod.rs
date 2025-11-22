pub mod orderbook;
pub mod vpin;
pub mod avellaneda_stoikov;
pub mod adverse_selection;
pub mod options;

use serde::{Deserialize, Serialize};
use ordered_float::OrderedFloat;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub timestamp: i64,
    pub price: f64,
    pub quantity: f64,
    pub side: Side,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: OrderedFloat<f64>,
    pub quantity: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketData {
    pub timestamp: i64,
    pub bid: f64,
    pub ask: f64,
    pub mid: f64,
    pub bid_size: f64,
    pub ask_size: f64,
    pub last_price: f64,
}

#[derive(Debug, Clone)]
pub struct Position {
    pub symbol: String,
    pub quantity: f64,
    pub avg_price: f64,
    pub unrealized_pnl: f64,
    pub realized_pnl: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub bid_price: f64,
    pub bid_size: f64,
    pub ask_price: f64,
    pub ask_size: f64,
    pub timestamp: i64,
}
