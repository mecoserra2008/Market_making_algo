use std::collections::VecDeque;

/// Avellaneda-Stoikov Market Making Model
/// Optimal market making strategy balancing inventory risk and profit
/// Based on Avellaneda & Stoikov (2008) "High-frequency trading in a limit order book"
pub struct AvellanedaStoikov {
    /// Risk aversion parameter (gamma)
    risk_aversion: f64,

    /// Target inventory (usually 0 for market neutral)
    inventory_target: f64,

    /// Time horizon (T)
    time_horizon: f64,

    /// Recent volatility estimates
    volatility_window: VecDeque<f64>,
    window_size: usize,

    /// Recent mid prices for volatility calculation
    mid_prices: VecDeque<f64>,
    price_window_size: usize,

    /// Current volatility estimate (sigma)
    current_volatility: f64,
}

impl AvellanedaStoikov {
    pub fn new(
        risk_aversion: f64,
        inventory_target: f64,
        time_horizon: f64,
        window_size: usize,
    ) -> Self {
        Self {
            risk_aversion,
            inventory_target,
            time_horizon,
            volatility_window: VecDeque::with_capacity(window_size),
            window_size,
            mid_prices: VecDeque::with_capacity(window_size),
            price_window_size: window_size,
            current_volatility: 0.01, // Default 1% volatility
        }
    }

    /// Update mid price and recalculate volatility
    pub fn update_mid_price(&mut self, mid_price: f64) {
        self.mid_prices.push_back(mid_price);

        if self.mid_prices.len() > self.price_window_size {
            self.mid_prices.pop_front();
        }

        self.recalculate_volatility();
    }

    /// Calculate volatility from recent mid prices
    fn recalculate_volatility(&mut self) {
        if self.mid_prices.len() < 2 {
            return;
        }

        let returns: Vec<f64> = self.mid_prices
            .iter()
            .zip(self.mid_prices.iter().skip(1))
            .map(|(p1, p2)| (p2 / p1).ln())
            .collect();

        if returns.is_empty() {
            return;
        }

        let mean = returns.iter().sum::<f64>() / returns.len() as f64;

        let variance = returns
            .iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>() / returns.len() as f64;

        let volatility = variance.sqrt();

        // Annualize volatility (assuming 1-second intervals)
        // sqrt(seconds_per_year) = sqrt(365.25 * 24 * 3600) ≈ 5615.55
        self.current_volatility = volatility * (365.25 * 24.0 * 3600.0_f64).sqrt();

        self.volatility_window.push_back(self.current_volatility);

        if self.volatility_window.len() > self.window_size {
            self.volatility_window.pop_front();
        }
    }

    /// Calculate reservation price (indifference price)
    /// r = s - q * gamma * sigma^2 * (T - t)
    /// where:
    /// - s: mid price
    /// - q: current inventory
    /// - gamma: risk aversion
    /// - sigma: volatility
    /// - T - t: time to horizon
    pub fn reservation_price(
        &self,
        mid_price: f64,
        inventory: f64,
        time_remaining: f64,
    ) -> f64 {
        let inventory_deviation = inventory - self.inventory_target;
        mid_price - inventory_deviation * self.risk_aversion * self.current_volatility.powi(2) * time_remaining
    }

    /// Calculate optimal spread (delta)
    /// delta = gamma * sigma^2 * (T - t) + (2/gamma) * ln(1 + gamma/k)
    /// Simplified version using approximation:
    /// delta ≈ gamma * sigma^2 * (T - t)
    pub fn optimal_spread(&self, time_remaining: f64) -> f64 {
        self.risk_aversion * self.current_volatility.powi(2) * time_remaining
    }

    /// Calculate bid and ask quotes
    pub fn calculate_quotes(
        &self,
        mid_price: f64,
        inventory: f64,
        time_remaining: f64,
        min_spread: f64,
        max_spread: f64,
    ) -> (f64, f64) {
        let reservation = self.reservation_price(mid_price, inventory, time_remaining);
        let spread = self.optimal_spread(time_remaining).max(min_spread).min(max_spread);

        let half_spread = spread / 2.0;

        let bid = reservation - half_spread;
        let ask = reservation + half_spread;

        (bid, ask)
    }

    /// Calculate multi-level quotes with stacking
    /// Returns Vec<(bid_price, bid_size, ask_price, ask_size)>
    pub fn calculate_multi_level_quotes(
        &self,
        mid_price: f64,
        inventory: f64,
        time_remaining: f64,
        min_spread: f64,
        max_spread: f64,
        num_levels: usize,
        level_spacing: f64, // in basis points
        base_size: f64,
    ) -> Vec<(f64, f64, f64, f64)> {
        let (base_bid, base_ask) = self.calculate_quotes(
            mid_price,
            inventory,
            time_remaining,
            min_spread,
            max_spread,
        );

        let mut quotes = Vec::new();

        for i in 0..num_levels {
            let level_offset = i as f64 * level_spacing * mid_price / 10000.0;

            // Size decreases with each level (liquidity taper)
            let size_multiplier = 1.0 / (1.0 + i as f64 * 0.5);
            let level_size = base_size * size_multiplier;

            // Adjust for inventory skew
            let inventory_deviation = inventory - self.inventory_target;
            let bid_skew = if inventory_deviation > 0.0 {
                1.0 - (inventory_deviation.abs() * 0.1).min(0.5)
            } else {
                1.0 + (inventory_deviation.abs() * 0.1).min(0.5)
            };

            let ask_skew = if inventory_deviation < 0.0 {
                1.0 - (inventory_deviation.abs() * 0.1).min(0.5)
            } else {
                1.0 + (inventory_deviation.abs() * 0.1).min(0.5)
            };

            let bid_price = base_bid - level_offset;
            let ask_price = base_ask + level_offset;
            let bid_size = level_size * bid_skew;
            let ask_size = level_size * ask_skew;

            quotes.push((bid_price, bid_size, ask_price, ask_size));
        }

        quotes
    }

    /// Get current volatility estimate
    pub fn get_volatility(&self) -> f64 {
        self.current_volatility
    }

    /// Get average volatility over window
    pub fn get_average_volatility(&self) -> f64 {
        if self.volatility_window.is_empty() {
            return self.current_volatility;
        }

        self.volatility_window.iter().sum::<f64>() / self.volatility_window.len() as f64
    }

    /// Update risk aversion dynamically based on market conditions
    pub fn adjust_risk_aversion(&mut self, market_stress: f64) {
        // Increase risk aversion during stressed markets
        // market_stress should be between 0 and 1
        let base_gamma = self.risk_aversion;
        self.risk_aversion = base_gamma * (1.0 + market_stress);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reservation_price() {
        let model = AvellanedaStoikov::new(0.1, 0.0, 1.0, 100);

        let mid = 50000.0;
        let inventory = 0.0;
        let time_remaining = 1.0;

        let reservation = model.reservation_price(mid, inventory, time_remaining);

        // With zero inventory, reservation should equal mid price
        assert!((reservation - mid).abs() < 1.0);
    }

    #[test]
    fn test_quotes_inventory_skew() {
        let model = AvellanedaStoikov::new(0.1, 0.0, 1.0, 100);

        let mid = 50000.0;
        let time_remaining = 1.0;

        // Positive inventory should push reservation price down (want to sell)
        let (bid1, ask1) = model.calculate_quotes(mid, 1.0, time_remaining, 5.0, 50.0);

        // Zero inventory
        let (bid2, ask2) = model.calculate_quotes(mid, 0.0, time_remaining, 5.0, 50.0);

        assert!(bid1 < bid2);
        assert!(ask1 < ask2);
    }

    #[test]
    fn test_multi_level_quotes() {
        let model = AvellanedaStoikov::new(0.1, 0.0, 1.0, 100);

        let quotes = model.calculate_multi_level_quotes(
            50000.0,
            0.0,
            1.0,
            5.0,
            50.0,
            5,
            2.0,
            0.1,
        );

        assert_eq!(quotes.len(), 5);

        // Each level should have wider spread
        for i in 1..quotes.len() {
            assert!(quotes[i].0 <= quotes[i-1].0); // Bids decreasing
            assert!(quotes[i].2 >= quotes[i-1].2); // Asks increasing
        }
    }
}
