use std::collections::VecDeque;
use super::{Trade, Side};

/// Adverse Selection Detector
/// Detects when market maker is being adversely selected by informed traders
/// Uses multiple signals including:
/// - Trade direction prediction accuracy
/// - Post-trade price movement
/// - Order fill rate asymmetry
/// - Effective spread realization
pub struct AdverseSelectionDetector {
    /// Recent trades
    trades: VecDeque<TradeRecord>,

    /// Window size for analysis
    window_size: usize,

    /// Fill tracking
    bid_fills: VecDeque<FillRecord>,
    ask_fills: VecDeque<FillRecord>,

    /// Adverse selection score (0-1, higher is worse)
    current_score: f64,
}

#[derive(Debug, Clone)]
struct TradeRecord {
    timestamp: i64,
    price: f64,
    side: Side,
    quantity: f64,
}

#[derive(Debug, Clone)]
struct FillRecord {
    timestamp: i64,
    price: f64,
    quantity: f64,
    post_trade_price: Option<f64>,
}

impl AdverseSelectionDetector {
    pub fn new(window_size: usize) -> Self {
        Self {
            trades: VecDeque::with_capacity(window_size),
            window_size,
            bid_fills: VecDeque::with_capacity(window_size),
            ask_fills: VecDeque::with_capacity(window_size),
            current_score: 0.0,
        }
    }

    /// Record a market trade
    pub fn record_trade(&mut self, trade: Trade) {
        self.trades.push_back(TradeRecord {
            timestamp: trade.timestamp,
            price: trade.price,
            side: trade.side,
            quantity: trade.quantity,
        });

        if self.trades.len() > self.window_size {
            self.trades.pop_front();
        }
    }

    /// Record a fill of our bid order
    pub fn record_bid_fill(&mut self, timestamp: i64, price: f64, quantity: f64) {
        self.bid_fills.push_back(FillRecord {
            timestamp,
            price,
            quantity,
            post_trade_price: None,
        });

        if self.bid_fills.len() > self.window_size {
            self.bid_fills.pop_front();
        }
    }

    /// Record a fill of our ask order
    pub fn record_ask_fill(&mut self, timestamp: i64, price: f64, quantity: f64) {
        self.ask_fills.push_back(FillRecord {
            timestamp,
            price,
            quantity,
            post_trade_price: None,
        });

        if self.ask_fills.len() > self.window_size {
            self.ask_fills.pop_front();
        }
    }

    /// Update post-trade prices for recent fills
    pub fn update_post_trade_prices(&mut self, current_price: f64) {
        // Update bid fills
        for fill in self.bid_fills.iter_mut() {
            if fill.post_trade_price.is_none() {
                fill.post_trade_price = Some(current_price);
            }
        }

        // Update ask fills
        for fill in self.ask_fills.iter_mut() {
            if fill.post_trade_price.is_none() {
                fill.post_trade_price = Some(current_price);
            }
        }
    }

    /// Calculate realized spread for bid fills
    /// Negative values indicate adverse selection
    fn bid_realized_spread(&self) -> f64 {
        if self.bid_fills.is_empty() {
            return 0.0;
        }

        let total_pnl: f64 = self.bid_fills
            .iter()
            .filter_map(|fill| {
                fill.post_trade_price.map(|post_price| {
                    // We bought at fill.price, current value is post_price
                    (post_price - fill.price) * fill.quantity
                })
            })
            .sum();

        let total_quantity: f64 = self.bid_fills.iter().map(|f| f.quantity).sum();

        if total_quantity == 0.0 {
            return 0.0;
        }

        total_pnl / total_quantity
    }

    /// Calculate realized spread for ask fills
    /// Negative values indicate adverse selection
    fn ask_realized_spread(&self) -> f64 {
        if self.ask_fills.is_empty() {
            return 0.0;
        }

        let total_pnl: f64 = self.ask_fills
            .iter()
            .filter_map(|fill| {
                fill.post_trade_price.map(|post_price| {
                    // We sold at fill.price, opportunity cost is post_price
                    (fill.price - post_price) * fill.quantity
                })
            })
            .sum();

        let total_quantity: f64 = self.ask_fills.iter().map(|f| f.quantity).sum();

        if total_quantity == 0.0 {
            return 0.0;
        }

        total_pnl / total_quantity
    }

    /// Calculate fill rate imbalance
    /// High imbalance suggests directional flow
    fn fill_imbalance(&self) -> f64 {
        let bid_count = self.bid_fills.len() as f64;
        let ask_count = self.ask_fills.len() as f64;

        if bid_count + ask_count == 0.0 {
            return 0.0;
        }

        ((bid_count - ask_count) / (bid_count + ask_count)).abs()
    }

    /// Calculate trade flow toxicity
    /// Based on volume-weighted trade direction
    fn flow_toxicity(&self) -> f64 {
        if self.trades.len() < 10 {
            return 0.0;
        }

        let mut buy_volume = 0.0;
        let mut sell_volume = 0.0;

        for trade in &self.trades {
            let volume = trade.price * trade.quantity;
            match trade.side {
                Side::Buy => buy_volume += volume,
                Side::Sell => sell_volume += volume,
            }
        }

        if buy_volume + sell_volume == 0.0 {
            return 0.0;
        }

        ((buy_volume - sell_volume) / (buy_volume + sell_volume)).abs()
    }

    /// Calculate Book Exhaustion Rate (BER)
    /// Measures how quickly liquidity is being consumed
    pub fn book_exhaustion_rate(&self, time_window_seconds: i64) -> f64 {
        if self.trades.len() < 2 {
            return 0.0;
        }

        let current_time = self.trades.back().map(|t| t.timestamp).unwrap_or(0);
        let cutoff_time = current_time - time_window_seconds;

        let recent_trades: Vec<&TradeRecord> = self.trades
            .iter()
            .filter(|t| t.timestamp >= cutoff_time)
            .collect();

        if recent_trades.is_empty() {
            return 0.0;
        }

        let total_volume: f64 = recent_trades
            .iter()
            .map(|t| t.price * t.quantity)
            .sum();

        total_volume / time_window_seconds as f64
    }

    /// Update adverse selection score
    pub fn update_score(&mut self) {
        let bid_spread = self.bid_realized_spread();
        let ask_spread = self.ask_realized_spread();
        let imbalance = self.fill_imbalance();
        let toxicity = self.flow_toxicity();

        // Negative realized spreads indicate adverse selection
        let spread_score = if bid_spread < 0.0 || ask_spread < 0.0 {
            0.5
        } else {
            0.0
        };

        // High imbalance suggests one-sided flow
        let imbalance_score = imbalance;

        // High toxicity suggests informed trading
        let toxicity_score = toxicity;

        // Weighted combination
        self.current_score = 0.4 * spread_score + 0.3 * imbalance_score + 0.3 * toxicity_score;
    }

    /// Get current adverse selection score (0-1)
    pub fn get_score(&self) -> f64 {
        self.current_score
    }

    /// Check if adverse selection is detected
    pub fn is_adverse(&self, threshold: f64) -> bool {
        self.current_score > threshold
    }

    /// Get recommendation for spread adjustment
    /// Returns multiplier for spread (>1.0 means widen spread)
    pub fn spread_adjustment_factor(&self) -> f64 {
        if self.current_score < 0.3 {
            1.0 // Normal conditions
        } else if self.current_score < 0.5 {
            1.2 // Slightly widen
        } else if self.current_score < 0.7 {
            1.5 // Moderately widen
        } else {
            2.0 // Significantly widen or pause
        }
    }

    /// Get recommendation for size adjustment
    /// Returns multiplier for order size (<1.0 means reduce size)
    pub fn size_adjustment_factor(&self) -> f64 {
        if self.current_score < 0.3 {
            1.0 // Normal size
        } else if self.current_score < 0.5 {
            0.8 // Slightly reduce
        } else if self.current_score < 0.7 {
            0.5 // Moderately reduce
        } else {
            0.2 // Significantly reduce or pause
        }
    }

    /// Get statistics
    pub fn statistics(&self) -> AdverseSelectionStats {
        AdverseSelectionStats {
            score: self.current_score,
            bid_realized_spread: self.bid_realized_spread(),
            ask_realized_spread: self.ask_realized_spread(),
            fill_imbalance: self.fill_imbalance(),
            flow_toxicity: self.flow_toxicity(),
            bid_fills: self.bid_fills.len(),
            ask_fills: self.ask_fills.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AdverseSelectionStats {
    pub score: f64,
    pub bid_realized_spread: f64,
    pub ask_realized_spread: f64,
    pub fill_imbalance: f64,
    pub flow_toxicity: f64,
    pub bid_fills: usize,
    pub ask_fills: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adverse_selection_balanced() {
        let mut detector = AdverseSelectionDetector::new(100);

        // Record balanced fills
        for i in 0..10 {
            if i % 2 == 0 {
                detector.record_bid_fill(i, 50000.0, 0.1);
            } else {
                detector.record_ask_fill(i, 50000.0, 0.1);
            }
        }

        detector.update_post_trade_prices(50000.0);
        detector.update_score();

        // Should show low adverse selection for balanced fills
        assert!(detector.get_score() < 0.5);
    }
}
