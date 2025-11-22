use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub bybit: ExchangeConfig,
    pub deribit: ExchangeConfig,
    pub strategy: StrategyConfig,
    pub risk: RiskConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeConfig {
    pub api_key: String,
    pub api_secret: String,
    pub testnet: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyConfig {
    pub symbol: String,
    pub inventory_target: f64,
    pub risk_aversion: f64,
    pub volatility_window: usize,
    pub min_spread_bps: f64,
    pub max_spread_bps: f64,
    pub order_quantity: f64,
    pub num_levels: usize,
    pub level_spacing_bps: f64,
    pub vpin_bucket_size: usize,
    pub vpin_threshold: f64,
    pub adverse_selection_threshold: f64,
    pub rebalance_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    pub max_position: f64,
    pub max_drawdown: f64,
    pub max_order_value: f64,
    pub position_limit_pct: f64,
    pub delta_hedge_threshold: f64,
    pub vega_limit: f64,
    pub gamma_limit: f64,
}

impl Config {
    pub fn load() -> Result<Self> {
        // Try to load from config file
        if let Ok(config_str) = fs::read_to_string("config/config.toml") {
            return toml::from_str(&config_str)
                .context("Failed to parse config.toml");
        }

        // Fall back to default configuration
        Ok(Self::default())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bybit: ExchangeConfig {
                api_key: std::env::var("BYBIT_API_KEY").unwrap_or_default(),
                api_secret: std::env::var("BYBIT_API_SECRET").unwrap_or_default(),
                testnet: true,
            },
            deribit: ExchangeConfig {
                api_key: std::env::var("DERIBIT_API_KEY").unwrap_or_default(),
                api_secret: std::env::var("DERIBIT_API_SECRET").unwrap_or_default(),
                testnet: true,
            },
            strategy: StrategyConfig {
                symbol: "BTCUSDT".to_string(),
                inventory_target: 0.0,
                risk_aversion: 0.5,
                volatility_window: 100,
                min_spread_bps: 5.0,
                max_spread_bps: 50.0,
                order_quantity: 0.01,
                num_levels: 5,
                level_spacing_bps: 2.0,
                vpin_bucket_size: 50,
                vpin_threshold: 0.7,
                adverse_selection_threshold: 0.6,
                rebalance_threshold: 0.1,
            },
            risk: RiskConfig {
                max_position: 1.0,
                max_drawdown: 0.05,
                max_order_value: 10000.0,
                position_limit_pct: 0.1,
                delta_hedge_threshold: 0.2,
                vega_limit: 1000.0,
                gamma_limit: 100.0,
            },
        }
    }
}
