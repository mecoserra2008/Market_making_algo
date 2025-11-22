use anyhow::{Result, Context};
use tracing::{info, warn, error};
use tokio::time::{interval, Duration};
use std::sync::Arc;

use crate::config::Config;
use crate::exchanges::{
    bybit::BybitExchange,
    deribit::DeribitExchange,
    Exchange,
    NewOrder,
    OrderSide,
    OrderType,
    TimeInForce,
};
use crate::models::{
    orderbook::OrderBook,
    vpin::VPIN,
    avellaneda_stoikov::AvellanedaStoikov,
    adverse_selection::AdverseSelectionDetector,
    Trade,
};
use crate::risk::RiskManager;
use crate::strategies::delta_hedger::DeltaHedger;
use crate::utils::{current_timestamp_secs, bps_to_decimal};

/// Main Market Making Strategy
/// Integrates all components for optimal market making with hedging
pub struct MarketMaker {
    bybit: BybitExchange,
    deribit: DeribitExchange,
    config: Config,

    // Strategy components
    as_model: Arc<parking_lot::RwLock<AvellanedaStoikov>>,
    vpin: Arc<parking_lot::RwLock<VPIN>>,
    adverse_selection: Arc<parking_lot::RwLock<AdverseSelectionDetector>>,

    // Risk and hedging
    risk_manager: Arc<RiskManager>,
    delta_hedger: Arc<DeltaHedger>,

    // State
    orderbook: Arc<tokio::sync::RwLock<OrderBook>>,
    current_inventory: Arc<parking_lot::RwLock<f64>>,
    active_orders: Arc<parking_lot::RwLock<Vec<String>>>,
}

impl MarketMaker {
    pub fn new(
        bybit: BybitExchange,
        deribit: DeribitExchange,
        config: Config,
    ) -> Self {
        let risk_manager = Arc::new(RiskManager::new(config.risk.clone()));

        let as_model = Arc::new(parking_lot::RwLock::new(
            AvellanedaStoikov::new(
                config.strategy.risk_aversion,
                config.strategy.inventory_target,
                1.0, // 1 hour time horizon
                config.strategy.volatility_window,
            )
        ));

        let vpin = Arc::new(parking_lot::RwLock::new(
            VPIN::new(
                config.strategy.vpin_bucket_size as f64,
                50, // 50 buckets
            )
        ));

        let adverse_selection = Arc::new(parking_lot::RwLock::new(
            AdverseSelectionDetector::new(100)
        ));

        let delta_hedger = Arc::new(DeltaHedger::new(
            deribit.clone(),
            risk_manager.clone(),
            config.risk.clone(),
            "BTC".to_string(),
        ));

        let orderbook = Arc::new(tokio::sync::RwLock::new(
            OrderBook::new(config.strategy.symbol.clone())
        ));

        Self {
            bybit,
            deribit,
            config,
            as_model,
            vpin,
            adverse_selection,
            risk_manager,
            delta_hedger,
            orderbook,
            current_inventory: Arc::new(parking_lot::RwLock::new(0.0)),
            active_orders: Arc::new(parking_lot::RwLock::new(Vec::new())),
        }
    }

    /// Main run loop
    pub async fn run(&mut self) -> Result<()> {
        info!("Market maker starting...");

        // Initialize orderbook
        self.update_orderbook().await?;

        // Start market making loop
        let mut tick_interval = interval(Duration::from_secs(1));
        let mut hedge_interval = interval(Duration::from_secs(10));
        let mut risk_check_interval = interval(Duration::from_secs(5));

        loop {
            tokio::select! {
                _ = tick_interval.tick() => {
                    if let Err(e) = self.market_making_cycle().await {
                        error!("Market making cycle error: {}", e);
                    }
                }
                _ = hedge_interval.tick() => {
                    if let Err(e) = self.delta_hedger.check_and_hedge().await {
                        error!("Delta hedging error: {}", e);
                    }
                }
                _ = risk_check_interval.tick() => {
                    if self.risk_manager.should_halt_trading() {
                        error!("Risk limits breached, halting trading");
                        self.cancel_all_orders().await?;
                        return Ok(());
                    }
                }
            }
        }
    }

    /// Single market making cycle
    async fn market_making_cycle(&mut self) -> Result<()> {
        // Update market data
        self.update_orderbook().await?;
        self.update_trades().await?;

        // Get current market state
        let orderbook = self.orderbook.read().await;
        let mid_price = orderbook.mid_price()
            .context("No mid price available")?;

        drop(orderbook);

        // Update models
        self.as_model.write().update_mid_price(mid_price);

        // Check flow toxicity
        let vpin_score = self.vpin.read().current_vpin();
        let is_toxic = vpin_score > self.config.strategy.vpin_threshold;

        // Check adverse selection
        self.adverse_selection.write().update_post_trade_prices(mid_price);
        self.adverse_selection.write().update_score();
        let adverse_score = self.adverse_selection.read().get_score();
        let is_adverse = adverse_score > self.config.strategy.adverse_selection_threshold;

        // Adjust strategy based on market conditions
        let (spread_adj, size_adj) = self.calculate_adjustments(
            vpin_score,
            adverse_score,
            is_toxic,
            is_adverse,
        );

        info!(
            "Market: ${:.2} | VPIN: {:.3} | Adverse: {:.3} | Spread Adj: {:.2}x | Size Adj: {:.2}x",
            mid_price, vpin_score, adverse_score, spread_adj, size_adj
        );

        // Cancel existing orders if conditions changed significantly
        if is_toxic || is_adverse {
            self.cancel_all_orders().await?;
        }

        // Place new quotes
        if !is_toxic && !is_adverse {
            self.place_quotes(mid_price, spread_adj, size_adj).await?;
        } else if is_toxic {
            warn!("Flow is toxic (VPIN: {:.3}), widening spreads", vpin_score);
            self.place_quotes(mid_price, spread_adj * 2.0, size_adj * 0.5).await?;
        } else if is_adverse {
            warn!("Adverse selection detected (score: {:.3}), reducing exposure", adverse_score);
            self.place_quotes(mid_price, spread_adj * 1.5, size_adj * 0.5).await?;
        }

        // Update position and risk
        self.update_position().await?;

        Ok(())
    }

    /// Calculate spread and size adjustments based on market conditions
    fn calculate_adjustments(
        &self,
        vpin_score: f64,
        _adverse_score: f64,
        _is_toxic: bool,
        _is_adverse: bool,
    ) -> (f64, f64) {
        let mut spread_multiplier = 1.0;
        let mut size_multiplier = 1.0;

        // VPIN adjustments
        if vpin_score > 0.5 {
            spread_multiplier *= 1.0 + (vpin_score - 0.5) * 2.0;
            size_multiplier *= 1.0 - (vpin_score - 0.5) * 0.8;
        }

        // Adverse selection adjustments
        let adverse_adj = self.adverse_selection.read();
        spread_multiplier *= adverse_adj.spread_adjustment_factor();
        size_multiplier *= adverse_adj.size_adjustment_factor();

        // Inventory adjustments
        let inventory = *self.current_inventory.read();
        let inventory_deviation = (inventory - self.config.strategy.inventory_target).abs();
        if inventory_deviation > 0.5 {
            spread_multiplier *= 1.0 + inventory_deviation * 0.5;
        }

        (spread_multiplier, size_multiplier)
    }

    /// Place market making quotes
    async fn place_quotes(
        &mut self,
        mid_price: f64,
        spread_multiplier: f64,
        size_multiplier: f64,
    ) -> Result<()> {
        let inventory = *self.current_inventory.read();

        let time_remaining = 0.1; // 10% of time horizon

        // Calculate base spread
        let min_spread = bps_to_decimal(self.config.strategy.min_spread_bps) * mid_price;
        let max_spread = bps_to_decimal(self.config.strategy.max_spread_bps) * mid_price;

        // Get quotes from Avellaneda-Stoikov model
        let quotes = self.as_model.read().calculate_multi_level_quotes(
            mid_price,
            inventory,
            time_remaining,
            min_spread * spread_multiplier,
            max_spread * spread_multiplier,
            self.config.strategy.num_levels,
            self.config.strategy.level_spacing_bps,
            self.config.strategy.order_quantity * size_multiplier,
        );

        // Place orders for each level
        for (i, (bid_price, bid_size, ask_price, ask_size)) in quotes.iter().enumerate() {
            // Place bid
            if *bid_size > 0.0 {
                match self.place_limit_order(OrderSide::Buy, *bid_price, *bid_size).await {
                    Ok(order_id) => {
                        self.active_orders.write().push(order_id);
                    }
                    Err(e) => {
                        warn!("Failed to place bid at level {}: {}", i, e);
                    }
                }
            }

            // Place ask
            if *ask_size > 0.0 {
                match self.place_limit_order(OrderSide::Sell, *ask_price, *ask_size).await {
                    Ok(order_id) => {
                        self.active_orders.write().push(order_id);
                    }
                    Err(e) => {
                        warn!("Failed to place ask at level {}: {}", i, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Place a limit order
    async fn place_limit_order(
        &self,
        side: OrderSide,
        price: f64,
        quantity: f64,
    ) -> Result<String> {
        // Check risk limits
        self.risk_manager.is_order_allowed(
            &self.config.strategy.symbol,
            side,
            quantity,
            price,
        ).map_err(|e| anyhow::anyhow!(e))?;

        let order = NewOrder {
            symbol: self.config.strategy.symbol.clone(),
            side,
            order_type: OrderType::PostOnly,
            price: Some(price),
            quantity,
            time_in_force: TimeInForce::PostOnly,
        };

        let result = self.bybit.place_order(order).await?;

        info!(
            "Placed {} order: {:.4} @ ${:.2}",
            if side == OrderSide::Buy { "BID" } else { "ASK" },
            quantity,
            price
        );

        Ok(result.order_id)
    }

    /// Cancel all active orders
    async fn cancel_all_orders(&self) -> Result<()> {
        let orders = self.active_orders.read().clone();

        for order_id in &orders {
            if let Err(e) = self.bybit.cancel_order(&self.config.strategy.symbol, order_id).await {
                warn!("Failed to cancel order {}: {}", order_id, e);
            }
        }

        self.active_orders.write().clear();

        Ok(())
    }

    /// Update orderbook from exchange
    async fn update_orderbook(&self) -> Result<()> {
        // In production, this would subscribe to WebSocket orderbook updates
        // For now, we'll fetch via REST API
        let ob = self.bybit.get_orderbook(&self.config.strategy.symbol).await?;
        let mut current_ob = self.orderbook.write().await;
        *current_ob = (*ob.read().await).clone();

        Ok(())
    }

    /// Update trade data and models
    async fn update_trades(&self) -> Result<()> {
        let trades = self.bybit.get_recent_trades(&self.config.strategy.symbol, 100).await?;

        for trade in trades {
            self.vpin.write().process_trade(&trade);
            self.adverse_selection.write().record_trade(trade);
        }

        Ok(())
    }

    /// Update position tracking
    async fn update_position(&self) -> Result<()> {
        let position = self.risk_manager.get_position(&self.config.strategy.symbol);

        if let Some(pos) = position {
            *self.current_inventory.write() = pos.quantity;

            let current_price = self.bybit.get_market_price(&self.config.strategy.symbol).await?;

            self.risk_manager.update_position(
                self.config.strategy.symbol.clone(),
                pos.quantity,
                pos.avg_price,
                current_price,
            );

            self.risk_manager.record_pnl_snapshot(current_timestamp_secs());
        }

        Ok(())
    }
}
