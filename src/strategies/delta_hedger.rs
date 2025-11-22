use anyhow::Result;
use tracing::{info, warn};
use crate::exchanges::{deribit::DeribitExchange, Exchange, NewOrder, OrderSide, OrderType, TimeInForce};
use crate::config::RiskConfig;
use crate::risk::RiskManager;
use std::sync::Arc;

/// Delta Hedger
/// Manages delta-neutral positions using Deribit options
pub struct DeltaHedger {
    deribit: DeribitExchange,
    risk_manager: Arc<RiskManager>,
    config: RiskConfig,
    underlying: String,
    current_hedge_position: parking_lot::RwLock<f64>,
}

impl DeltaHedger {
    pub fn new(
        deribit: DeribitExchange,
        risk_manager: Arc<RiskManager>,
        config: RiskConfig,
        underlying: String,
    ) -> Self {
        Self {
            deribit,
            risk_manager,
            config,
            underlying,
            current_hedge_position: parking_lot::RwLock::new(0.0),
        }
    }

    /// Check if delta hedge is needed and execute if necessary
    pub async fn check_and_hedge(&self) -> Result<()> {
        if !self.risk_manager.needs_delta_hedge() {
            return Ok(());
        }

        let required_hedge = self.risk_manager.required_hedge_size();

        info!(
            "Delta hedge required: {:.4} {} (threshold: {:.4})",
            required_hedge,
            self.underlying,
            self.config.delta_hedge_threshold
        );

        // Determine hedge strategy
        if required_hedge.abs() < 0.01 {
            // Too small to hedge
            return Ok(());
        }

        // Option 1: Use perpetual futures for simple delta hedge
        self.hedge_with_perpetual(required_hedge).await?;

        // Option 2: Use options for more sophisticated hedging
        // self.hedge_with_options(required_hedge).await?;

        Ok(())
    }

    /// Hedge using perpetual futures (simple and efficient)
    async fn hedge_with_perpetual(&self, delta: f64) -> Result<()> {
        let symbol = format!("{}-PERPETUAL", self.underlying);

        let side = if delta > 0.0 {
            OrderSide::Buy
        } else {
            OrderSide::Sell
        };

        let quantity = delta.abs();

        info!(
            "Placing hedge order: {} {:.4} {}",
            if delta > 0.0 { "BUY" } else { "SELL" },
            quantity,
            symbol
        );

        let order = NewOrder {
            symbol: symbol.clone(),
            side,
            order_type: OrderType::Market,
            price: None,
            quantity,
            time_in_force: TimeInForce::IOC,
        };

        match self.deribit.place_order(order).await {
            Ok(order) => {
                info!("Hedge order placed: {}", order.order_id);
                *self.current_hedge_position.write() += if side == OrderSide::Buy {
                    quantity
                } else {
                    -quantity
                };
                Ok(())
            }
            Err(e) => {
                warn!("Failed to place hedge order: {}", e);
                Err(e)
            }
        }
    }

    /// Hedge using options (more sophisticated, controls vega and gamma)
    async fn hedge_with_options(&self, target_delta: f64) -> Result<()> {
        // Find suitable option for hedging
        let option = self.deribit
            .find_hedge_option(&self.underlying, target_delta, 30)
            .await?;

        info!(
            "Found hedge option: {} (delta: {:.4}, vega: {:.4})",
            option.instrument_name,
            option.delta,
            option.vega
        );

        // Calculate required quantity
        let quantity = DeribitExchange::calculate_hedge_quantity(
            target_delta,
            option.delta,
        );

        // Check vega limits
        let total_vega = quantity * option.vega;
        if total_vega.abs() > self.config.vega_limit {
            warn!(
                "Vega limit exceeded: {:.2} > {:.2}",
                total_vega,
                self.config.vega_limit
            );
            // Fall back to perpetual hedge
            return self.hedge_with_perpetual(target_delta).await;
        }

        let side = if quantity > 0.0 {
            OrderSide::Buy
        } else {
            OrderSide::Sell
        };

        let order = NewOrder {
            symbol: option.instrument_name.clone(),
            side,
            order_type: OrderType::Limit,
            price: Some(option.mark_price * 1.01), // Slightly above mark for quick fill
            quantity: quantity.abs(),
            time_in_force: TimeInForce::GTC,
        };

        match self.deribit.place_order(order).await {
            Ok(order) => {
                info!(
                    "Options hedge placed: {} x {:.4}",
                    option.instrument_name,
                    quantity
                );
                Ok(())
            }
            Err(e) => {
                warn!("Failed to place options hedge: {}", e);
                // Fall back to perpetual
                self.hedge_with_perpetual(target_delta).await
            }
        }
    }

    /// Get current hedge position
    pub fn get_hedge_position(&self) -> f64 {
        *self.current_hedge_position.read()
    }

    /// Calculate optimal hedge ratio considering options Greeks
    pub async fn calculate_optimal_hedge(&self, spot_delta: f64) -> Result<HedgeRecommendation> {
        // Get available options
        let options = self.deribit.get_options(&self.underlying).await?;

        if options.is_empty() {
            return Ok(HedgeRecommendation {
                instrument: format!("{}-PERPETUAL", self.underlying),
                quantity: -spot_delta,
                hedge_type: HedgeType::Perpetual,
                expected_delta: 0.0,
                expected_vega: 0.0,
                expected_gamma: 0.0,
            });
        }

        // Find option with best delta/cost ratio
        let best_option = options
            .iter()
            .filter(|opt| opt.delta.abs() > 0.1) // Filter out very low delta options
            .min_by(|a, b| {
                let cost_a = a.mark_price / a.delta.abs();
                let cost_b = b.mark_price / b.delta.abs();
                cost_a.partial_cmp(&cost_b).unwrap()
            });

        match best_option {
            Some(opt) => {
                let quantity = -spot_delta / opt.delta;

                Ok(HedgeRecommendation {
                    instrument: opt.instrument_name.clone(),
                    quantity,
                    hedge_type: HedgeType::Option,
                    expected_delta: quantity * opt.delta,
                    expected_vega: quantity * opt.vega,
                    expected_gamma: quantity * opt.gamma,
                })
            }
            None => {
                Ok(HedgeRecommendation {
                    instrument: format!("{}-PERPETUAL", self.underlying),
                    quantity: -spot_delta,
                    hedge_type: HedgeType::Perpetual,
                    expected_delta: 0.0,
                    expected_vega: 0.0,
                    expected_gamma: 0.0,
                })
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum HedgeType {
    Perpetual,
    Option,
}

#[derive(Debug, Clone)]
pub struct HedgeRecommendation {
    pub instrument: String,
    pub quantity: f64,
    pub hedge_type: HedgeType,
    pub expected_delta: f64,
    pub expected_vega: f64,
    pub expected_gamma: f64,
}
