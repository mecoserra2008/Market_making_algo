use std::collections::BTreeMap;
use ordered_float::OrderedFloat;
use super::{PriceLevel, Side, Trade};

/// High-performance Limit Order Book implementation
/// Optimized for market making with fast updates and queries
#[derive(Debug, Clone)]
pub struct OrderBook {
    pub symbol: String,
    pub bids: BTreeMap<OrderedFloat<f64>, f64>, // Price -> Quantity (sorted descending)
    pub asks: BTreeMap<OrderedFloat<f64>, f64>, // Price -> Quantity (sorted ascending)
    pub last_update: i64,
    pub sequence: u64,
}

impl OrderBook {
    pub fn new(symbol: String) -> Self {
        Self {
            symbol,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            last_update: 0,
            sequence: 0,
        }
    }

    /// Update bid side of the order book
    pub fn update_bid(&mut self, price: f64, quantity: f64) {
        let price_key = OrderedFloat(price);
        if quantity > 0.0 {
            self.bids.insert(price_key, quantity);
        } else {
            self.bids.remove(&price_key);
        }
    }

    /// Update ask side of the order book
    pub fn update_ask(&mut self, price: f64, quantity: f64) {
        let price_key = OrderedFloat(price);
        if quantity > 0.0 {
            self.asks.insert(price_key, quantity);
        } else {
            self.asks.remove(&price_key);
        }
    }

    /// Get best bid price
    pub fn best_bid(&self) -> Option<f64> {
        self.bids.keys().next_back().map(|p| p.0)
    }

    /// Get best ask price
    pub fn best_ask(&self) -> Option<f64> {
        self.asks.keys().next().map(|p| p.0)
    }

    /// Get mid price
    pub fn mid_price(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => Some((bid + ask) / 2.0),
            _ => None,
        }
    }

    /// Get current spread in basis points
    pub fn spread_bps(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some(bid), Some(ask)) => {
                Some(((ask - bid) / bid * 10000.0))
            }
            _ => None,
        }
    }

    /// Get bid quantity at best price
    pub fn best_bid_size(&self) -> f64 {
        self.bids.values().next_back().copied().unwrap_or(0.0)
    }

    /// Get ask quantity at best price
    pub fn best_ask_size(&self) -> f64 {
        self.asks.values().next().copied().unwrap_or(0.0)
    }

    /// Get total bid liquidity up to N levels
    pub fn bid_liquidity(&self, levels: usize) -> f64 {
        self.bids.values().rev().take(levels).sum()
    }

    /// Get total ask liquidity up to N levels
    pub fn ask_liquidity(&self, levels: usize) -> f64 {
        self.asks.values().take(levels).sum()
    }

    /// Calculate order book imbalance
    /// Returns value between -1 (all asks) and 1 (all bids)
    pub fn imbalance(&self, levels: usize) -> f64 {
        let bid_liq = self.bid_liquidity(levels);
        let ask_liq = self.ask_liquidity(levels);

        if bid_liq + ask_liq == 0.0 {
            return 0.0;
        }

        (bid_liq - ask_liq) / (bid_liq + ask_liq)
    }

    /// Get price levels for bids (up to depth)
    pub fn get_bid_levels(&self, depth: usize) -> Vec<PriceLevel> {
        self.bids
            .iter()
            .rev()
            .take(depth)
            .map(|(price, qty)| PriceLevel {
                price: *price,
                quantity: *qty,
            })
            .collect()
    }

    /// Get price levels for asks (up to depth)
    pub fn get_ask_levels(&self, depth: usize) -> Vec<PriceLevel> {
        self.asks
            .iter()
            .take(depth)
            .map(|(price, qty)| PriceLevel {
                price: *price,
                quantity: *qty,
            })
            .collect()
    }

    /// Calculate volume-weighted average price (VWAP) for buying quantity
    pub fn vwap_buy(&self, quantity: f64) -> Option<f64> {
        let mut remaining = quantity;
        let mut total_cost = 0.0;

        for (price, &qty) in self.asks.iter() {
            if remaining <= 0.0 {
                break;
            }

            let fill_qty = remaining.min(qty);
            total_cost += price.0 * fill_qty;
            remaining -= fill_qty;
        }

        if remaining > 0.0 {
            None // Not enough liquidity
        } else {
            Some(total_cost / quantity)
        }
    }

    /// Calculate volume-weighted average price (VWAP) for selling quantity
    pub fn vwap_sell(&self, quantity: f64) -> Option<f64> {
        let mut remaining = quantity;
        let mut total_value = 0.0;

        for (price, &qty) in self.bids.iter().rev() {
            if remaining <= 0.0 {
                break;
            }

            let fill_qty = remaining.min(qty);
            total_value += price.0 * fill_qty;
            remaining -= fill_qty;
        }

        if remaining > 0.0 {
            None // Not enough liquidity
        } else {
            Some(total_value / quantity)
        }
    }

    /// Estimate market impact for a trade
    /// Returns the price impact in basis points
    pub fn estimate_impact(&self, side: Side, quantity: f64) -> Option<f64> {
        let reference_price = self.mid_price()?;

        let execution_price = match side {
            Side::Buy => self.vwap_buy(quantity)?,
            Side::Sell => self.vwap_sell(quantity)?,
        };

        let impact = ((execution_price - reference_price) / reference_price).abs() * 10000.0;
        Some(impact)
    }

    /// Clear the order book
    pub fn clear(&mut self) {
        self.bids.clear();
        self.asks.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orderbook_basic() {
        let mut ob = OrderBook::new("BTCUSDT".to_string());

        ob.update_bid(50000.0, 1.0);
        ob.update_bid(49999.0, 2.0);
        ob.update_ask(50001.0, 1.5);
        ob.update_ask(50002.0, 2.5);

        assert_eq!(ob.best_bid(), Some(50000.0));
        assert_eq!(ob.best_ask(), Some(50001.0));
        assert_eq!(ob.mid_price(), Some(50000.5));
    }

    #[test]
    fn test_imbalance() {
        let mut ob = OrderBook::new("BTCUSDT".to_string());

        ob.update_bid(100.0, 10.0);
        ob.update_ask(101.0, 5.0);

        let imbalance = ob.imbalance(1);
        assert!(imbalance > 0.0); // More bids than asks
    }
}
