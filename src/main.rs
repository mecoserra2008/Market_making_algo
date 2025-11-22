use anyhow::Result;
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod config;
mod exchanges;
mod models;
mod strategies;
mod risk;
mod utils;

use config::Config;
use exchanges::{bybit::BybitExchange, deribit::DeribitExchange};
use strategies::market_maker_ws::MarketMakerWS;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Bybit Market Maker with Deribit Options Hedging");

    // Load configuration
    let config = Config::load()?;
    info!("Configuration loaded successfully");

    // Initialize exchanges
    let bybit = BybitExchange::new(
        config.bybit.api_key.clone(),
        config.bybit.api_secret.clone(),
        config.bybit.testnet,
    );

    let deribit = DeribitExchange::new(
        config.deribit.api_key.clone(),
        config.deribit.api_secret.clone(),
        config.deribit.testnet,
    );

    info!("Exchange connectors initialized");

    // Initialize WebSocket-based market maker (Production Grade - 100x faster)
    let mut market_maker = MarketMakerWS::new(
        bybit,
        deribit,
        config.clone(),
    );

    info!("WebSocket market maker initialized, starting operations");

    // Run market maker
    match market_maker.run().await {
        Ok(_) => info!("Market maker stopped gracefully"),
        Err(e) => error!("Market maker error: {}", e),
    }

    Ok(())
}
