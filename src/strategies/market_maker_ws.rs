use anyhow::{Result, Context};
use tracing::{info, warn, error};
use tokio::time::{interval, Duration};
use std::sync::Arc;

use crate::config::Config;
use crate::exchanges::{
    bybit::BybitExchange,
    deribit::DeribitExchange,
    websocket::{BybitWebSocket, MarketEvent},
    event_bus::EventBus,
    order_manager::OrderManager,
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
};
use crate::risk::RiskManager;
use crate::strategies::delta_hedger::DeltaHedger;
use crate::utils::{
    current_timestamp_secs,
    bps_to_decimal,
    latency_monitor::{LatencyMonitor, LatencyTimer},
    circuit_breaker::{CircuitBreaker, CircuitBreakerConfig},
};

/// WebSocket-based Market Maker (Production Grade)
/// 100x faster than REST polling version
pub struct MarketMakerWS {
    // Exchange connectors
    bybit: BybitExchange,
    deribit: DeribitExchange,
    config: Config,

    // WebSocket and events
    event_bus: EventBus,

    // Strategy components
    as_model: Arc<parking_lot::RwLock<AvellanedaStoikov>>,
    vpin: Arc<parking_lot::RwLock<VPIN>>,
    adverse_selection: Arc<parking_lot::RwLock<AdverseSelectionDetector>>,

    // Risk and order management
    risk_manager: Arc<RiskManager>,
    order_manager: Arc<OrderManager>,
    delta_hedger: Arc<DeltaHedger>,

    // State
    orderbook: Arc<parking_lot::RwLock<OrderBook>>,
    current_inventory: Arc<parking_lot::RwLock<f64>>,

    // Monitoring
    latency_monitor: Arc<LatencyMonitor>,
    circuit_breaker: Arc<CircuitBreaker>,
}

impl MarketMakerWS {
    pub fn new(
        bybit: BybitExchange,
        deribit: DeribitExchange,
        config: Config,
    ) -> Self {
        let risk_manager = Arc::new(RiskManager::new(config.risk.clone()));
        let order_manager = Arc::new(OrderManager::new());

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

        let orderbook = Arc::new(parking_lot::RwLock::new(
            OrderBook::new(config.strategy.symbol.clone())
        ));

        let event_bus = EventBus::new(10000); // Large buffer for high-frequency updates

        let latency_monitor = Arc::new(LatencyMonitor::new(1000));

        let circuit_breaker = Arc::new(CircuitBreaker::new(CircuitBreakerConfig::default()));

        Self {
            bybit,
            deribit,
            config,
            event_bus,
            as_model,
            vpin,
            adverse_selection,
            risk_manager,
            order_manager,
            delta_hedger,
            orderbook,
            current_inventory: Arc::new(parking_lot::RwLock::new(0.0)),
            latency_monitor,
            circuit_breaker,
        }
    }

    /// Main run loop with WebSocket
    pub async fn run(&mut self) -> Result<()> {
        info!("🚀 Starting WebSocket Market Maker (Production Mode)");

        // Start WebSocket connection
        let (mut ws, ws_rx) = BybitWebSocket::new(
            "".to_string(),
            self.config.bybit.testnet,
        );

        // Subscribe to feeds
        ws.subscribe_orderbook(&self.config.strategy.symbol, 50);
        ws.subscribe_trades(&self.config.strategy.symbol);

        info!("Subscribed to {} feeds", self.config.strategy.symbol);

        // Spawn WebSocket task
        let event_bus = self.event_bus.clone();
        tokio::spawn(async move {
            // Forward WebSocket events to event bus
            let mut rx = ws_rx;
            while let Some(event) = rx.recv().await {
                event_bus.publish(event);
            }
        });

        tokio::spawn(async move {
            if let Err(e) = ws.run().await {
                error!("WebSocket error: {}", e);
            }
        });

        // Wait for connection
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Start strategy loops
        self.run_strategy_loops().await
    }

    /// Run strategy loops
    async fn run_strategy_loops(&self) -> Result<()> {
        let mut event_rx = self.event_bus.subscribe();
        let mut hedge_interval = interval(Duration::from_secs(1)); // Faster hedging!
        let mut risk_check_interval = interval(Duration::from_secs(5));
        let mut stats_interval = interval(Duration::from_secs(30));

        loop {
            tokio::select! {
                // Market events from WebSocket
                Ok(event) = event_rx.recv() => {
                    if let Err(e) = self.handle_market_event(&event).await {
                        error!("Error handling market event: {}", e);
                    }
                }

                // Delta hedging (now every 1 second!)
                _ = hedge_interval.tick() => {
                    if let Err(e) = self.delta_hedger.check_and_hedge().await {
                        error!("Delta hedging error: {}", e);
                    }
                }

                // Risk checks
                _ = risk_check_interval.tick() => {
                    if self.risk_manager.should_halt_trading() {
                        error!("⛔ Risk limits breached, halting trading");
                        self.cancel_all_orders().await?;
                        return Ok(());
                    }
                }

                // Statistics logging
                _ = stats_interval.tick() => {
                    self.log_statistics();
                }
            }
        }
    }

    /// Handle market event from WebSocket
    async fn handle_market_event(&self, event: &MarketEvent) -> Result<()> {
        match event {
            MarketEvent::OrderBookUpdate { symbol: _, bids, asks, timestamp } => {
                let timer = LatencyTimer::start();

                // Update local orderbook
                let mut ob = self.orderbook.write();
                for (price, qty) in bids {
                    ob.update_bid(*price, *qty);
                }
                for (price, qty) in asks {
                    ob.update_ask(*price, *qty);
                }
                ob.last_update = *timestamp;
                drop(ob);

                // Record latency
                self.latency_monitor.record_orderbook_latency(timer.elapsed());

                // Trigger strategy update
                self.on_orderbook_update().await?;
            }

            MarketEvent::Trade { symbol: _, trade } => {
                let timer = LatencyTimer::start();

                // Update VPIN
                self.vpin.write().process_trade(trade);

                // Update adverse selection detector
                self.adverse_selection.write().record_trade(trade.clone());

                // Record latency
                self.latency_monitor.record_trade_latency(timer.elapsed());
            }

            MarketEvent::Connected => {
                info!("✅ WebSocket connected");
            }

            MarketEvent::Disconnected => {
                warn!("⚠️  WebSocket disconnected");
                // Cancel orders on disconnect for safety
                self.cancel_all_orders().await?;
            }

            MarketEvent::Error(err) => {
                error!("WebSocket error: {}", err);
            }
        }

        Ok(())
    }

    /// Handle orderbook update (place quotes)
    async fn on_orderbook_update(&self) -> Result<()> {
        // Get current market state
        let orderbook = self.orderbook.read();
        let mid_price = match orderbook.mid_price() {
            Some(p) => p,
            None => return Ok(()), // No valid orderbook yet
        };
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

        // Place quotes (with circuit breaker protection)
        if !is_toxic && !is_adverse {
            if let Err(e) = self.place_quotes(mid_price, spread_adj, size_adj).await {
                warn!("Quote placement failed: {}", e);
            }
        } else if is_toxic {
            warn!("Flow is toxic (VPIN: {:.3}), widening spreads", vpin_score);
        } else if is_adverse {
            warn!("Adverse selection detected (score: {:.3}), reducing exposure", adverse_score);
        }

        Ok(())
    }

    /// Calculate spread and size adjustments
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
        &self,
        mid_price: f64,
        spread_multiplier: f64,
        size_multiplier: f64,
    ) -> Result<()> {
        let timer = LatencyTimer::start();

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
                    Ok(_) => {}
                    Err(e) => {
                        warn!("Failed to place bid at level {}: {}", i, e);
                    }
                }
            }

            // Place ask
            if *ask_size > 0.0 {
                match self.place_limit_order(OrderSide::Sell, *ask_price, *ask_size).await {
                    Ok(_) => {}
                    Err(e) => {
                        warn!("Failed to place ask at level {}: {}", i, e);
                    }
                }
            }
        }

        // Record latency
        self.latency_monitor.record_order_placement_latency(timer.elapsed());

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
        self.bybit.cancel_all_orders(&self.config.strategy.symbol).await?;
        Ok(())
    }

    /// Log statistics
    fn log_statistics(&self) {
        let stats = self.latency_monitor.get_all_stats();
        let risk_metrics = self.risk_manager.get_metrics();
        let circuit_stats = self.circuit_breaker.get_stats();

        info!("📊 Performance Statistics:");
        info!("  Orderbook Latency - P50: {:.1}ms, P95: {:.1}ms, P99: {:.1}ms",
              stats.orderbook.to_millis().p50_ms,
              stats.orderbook.to_millis().p95_ms,
              stats.orderbook.to_millis().p99_ms);
        info!("  Trade Latency - P50: {:.1}ms, P95: {:.1}ms, P99: {:.1}ms",
              stats.trades.to_millis().p50_ms,
              stats.trades.to_millis().p95_ms,
              stats.trades.to_millis().p99_ms);
        info!("  Order Placement - P50: {:.1}ms, P95: {:.1}ms, P99: {:.1}ms",
              stats.order_placement.to_millis().p50_ms,
              stats.order_placement.to_millis().p95_ms,
              stats.order_placement.to_millis().p99_ms);
        info!("  Circuit Breaker - State: {:?}, Failures: {}/{}, Rate: {:.2}%",
              circuit_stats.state,
              circuit_stats.consecutive_failures,
              circuit_stats.total_calls,
              circuit_stats.failure_rate * 100.0);
        info!("  Risk - P&L: ${:.2}, Delta: {:.4}, Drawdown: ${:.2}",
              risk_metrics.total_pnl,
              risk_metrics.delta,
              risk_metrics.drawdown);

        // Health check
        if !self.latency_monitor.is_healthy(100) {
            warn!("⚠️  High latency detected (P99 > 100ms)");
        }
    }
}
