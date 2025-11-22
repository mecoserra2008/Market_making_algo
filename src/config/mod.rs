use serde::{Deserialize, Serialize};
use anyhow::{Result, Context, anyhow};
use std::fs;
use tracing::{warn, error};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub bybit: ExchangeConfig,
    pub deribit: ExchangeConfig,
    pub strategy: StrategyConfig,
    pub risk: RiskConfig,
    #[serde(default)]
    pub paper_trading: bool,
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
        let config = if let Ok(config_str) = fs::read_to_string("config/config.toml") {
            toml::from_str(&config_str)
                .context("Failed to parse config.toml")?
        } else {
            Self::default()
        };

        // Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Comprehensive validation with security checks
    pub fn validate(&self) -> Result<()> {
        // === SECURITY CHECKS ===

        // API keys should come from environment
        if !self.bybit.api_key.is_empty() || !self.deribit.api_key.is_empty() {
            error!("🚨 SECURITY WARNING: API keys found in config file!");
            error!("API keys should ONLY be in environment variables (.env file)");
            return Err(anyhow!("API keys must not be in config file - use environment variables"));
        }

        // Warn if not testnet in config
        if !self.bybit.testnet && !self.paper_trading {
            warn!("⚠️  WARNING: Production mode enabled (testnet=false)");
            warn!("Make sure you intend to trade with REAL money!");
        }

        // === STRATEGY VALIDATION ===

        // Risk aversion
        if self.strategy.risk_aversion < 0.01 || self.strategy.risk_aversion > 10.0 {
            return Err(anyhow!(
                "risk_aversion must be in [0.01, 10.0], got {}",
                self.strategy.risk_aversion
            ));
        }

        // Spread bounds
        if self.strategy.min_spread_bps <= 0.0 {
            return Err(anyhow!("min_spread_bps must be positive"));
        }

        if self.strategy.max_spread_bps <= self.strategy.min_spread_bps {
            return Err(anyhow!(
                "max_spread_bps ({}) must be > min_spread_bps ({})",
                self.strategy.max_spread_bps,
                self.strategy.min_spread_bps
            ));
        }

        // Realistic spread bounds
        if self.strategy.min_spread_bps < 1.0 {
            warn!("⚠️  min_spread_bps < 1.0 is very tight - may not be profitable after fees");
        }

        if self.strategy.max_spread_bps > 1000.0 {
            warn!("⚠️  max_spread_bps > 1000 bps (10%) is very wide");
        }

        // Order quantity
        if self.strategy.order_quantity <= 0.0 {
            return Err(anyhow!("order_quantity must be positive"));
        }

        if self.strategy.order_quantity > 100.0 {
            return Err(anyhow!(
                "order_quantity ({}) seems dangerously high - max 100",
                self.strategy.order_quantity
            ));
        }

        // Number of levels
        if self.strategy.num_levels == 0 || self.strategy.num_levels > 20 {
            return Err(anyhow!(
                "num_levels must be in [1, 20], got {}",
                self.strategy.num_levels
            ));
        }

        // VPIN threshold
        if self.strategy.vpin_threshold < 0.0 || self.strategy.vpin_threshold > 1.0 {
            return Err(anyhow!(
                "vpin_threshold must be in [0, 1], got {}",
                self.strategy.vpin_threshold
            ));
        }

        // Adverse selection threshold
        if self.strategy.adverse_selection_threshold < 0.0
            || self.strategy.adverse_selection_threshold > 1.0
        {
            return Err(anyhow!(
                "adverse_selection_threshold must be in [0, 1], got {}",
                self.strategy.adverse_selection_threshold
            ));
        }

        // === RISK VALIDATION ===

        // Position limits
        if self.risk.max_position <= 0.0 {
            return Err(anyhow!("max_position must be positive"));
        }

        if self.risk.max_position > 1000.0 {
            return Err(anyhow!(
                "max_position ({}) seems dangerously high - max 1000",
                self.risk.max_position
            ));
        }

        // Max order value
        if self.risk.max_order_value <= 0.0 {
            return Err(anyhow!("max_order_value must be positive"));
        }

        if self.risk.max_order_value > 10_000_000.0 {
            return Err(anyhow!(
                "max_order_value (${}) exceeds safety limit of $10M",
                self.risk.max_order_value
            ));
        }

        // Drawdown
        if self.risk.max_drawdown <= 0.0 {
            return Err(anyhow!("max_drawdown must be positive"));
        }

        // Delta hedge threshold
        if self.risk.delta_hedge_threshold <= 0.0 {
            return Err(anyhow!("delta_hedge_threshold must be positive"));
        }

        if self.risk.delta_hedge_threshold > self.risk.max_position {
            warn!(
                "⚠️  delta_hedge_threshold ({}) > max_position ({}) - will never hedge",
                self.risk.delta_hedge_threshold,
                self.risk.max_position
            );
        }

        // === SANITY CHECKS ===

        // Total order value shouldn't exceed max order value
        let total_order_value = self.strategy.order_quantity
            * self.strategy.num_levels as f64
            * 2.0 // bid and ask
            * 100000.0; // assume $100k BTC price

        if total_order_value > self.risk.max_order_value {
            warn!(
                "⚠️  Total order value (${:.0}) may exceed max_order_value (${:.0})",
                total_order_value,
                self.risk.max_order_value
            );
        }

        Ok(())
    }

    /// Load with explicit validation mode
    pub fn load_for_production() -> Result<Self> {
        let config = Self::load()?;

        if config.bybit.testnet || config.deribit.testnet {
            return Err(anyhow!(
                "Cannot use production mode with testnet=true in config"
            ));
        }

        if config.paper_trading {
            return Err(anyhow!(
                "Cannot use production mode with paper_trading=true"
            ));
        }

        println!("⚠️  ========================================");
        println!("⚠️  PRODUCTION MODE - TRADING REAL MONEY");
        println!("⚠️  ========================================");

        Ok(config)
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bybit: ExchangeConfig {
                api_key: String::new(), // Must come from environment
                api_secret: String::new(),
                testnet: true,
            },
            deribit: ExchangeConfig {
                api_key: String::new(), // Must come from environment
                api_secret: String::new(),
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
                max_drawdown: 1000.0, // FIXED: Was 0.05, should be in dollars
                max_order_value: 10000.0,
                position_limit_pct: 0.1,
                delta_hedge_threshold: 0.2,
                vega_limit: 1000.0,
                gamma_limit: 100.0,
            },
            paper_trading: true, // Safe default
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_validates() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_invalid_risk_aversion() {
        let mut config = Config::default();
        config.strategy.risk_aversion = -1.0;
        assert!(config.validate().is_err());

        config.strategy.risk_aversion = 100.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_invalid_spread() {
        let mut config = Config::default();
        config.strategy.min_spread_bps = 100.0;
        config.strategy.max_spread_bps = 50.0; // max < min
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_api_keys_in_config_rejected() {
        let mut config = Config::default();
        config.bybit.api_key = "test_key".to_string();
        assert!(config.validate().is_err());
    }
}
