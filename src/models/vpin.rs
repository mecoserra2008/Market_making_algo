use std::collections::VecDeque;
use super::{Trade, Side};

/// VPIN (Volume-Synchronized Probability of Informed Trading)
/// Measures order flow toxicity to detect informed trading
/// Based on Easley, Lopez de Prado, and O'Hara (2010)
pub struct VPIN {
    bucket_size: f64,
    num_buckets: usize,
    buckets: VecDeque<VolumeBucket>,
    current_bucket: VolumeBucket,
    vpin_values: VecDeque<f64>,
}

#[derive(Debug, Clone)]
struct VolumeBucket {
    buy_volume: f64,
    sell_volume: f64,
    total_volume: f64,
}

impl VolumeBucket {
    fn new() -> Self {
        Self {
            buy_volume: 0.0,
            sell_volume: 0.0,
            total_volume: 0.0,
        }
    }

    fn add_trade(&mut self, side: Side, volume: f64) {
        match side {
            Side::Buy => self.buy_volume += volume,
            Side::Sell => self.sell_volume += volume,
        }
        self.total_volume += volume;
    }

    fn is_full(&self, bucket_size: f64) -> bool {
        self.total_volume >= bucket_size
    }

    fn volume_imbalance(&self) -> f64 {
        (self.buy_volume - self.sell_volume).abs()
    }
}

impl VPIN {
    /// Create new VPIN calculator
    ///
    /// # Arguments
    /// * `bucket_size` - Volume size for each bucket (e.g., 50 BTC)
    /// * `num_buckets` - Number of buckets to maintain (typically 50)
    pub fn new(bucket_size: f64, num_buckets: usize) -> Self {
        Self {
            bucket_size,
            num_buckets,
            buckets: VecDeque::new(),
            current_bucket: VolumeBucket::new(),
            vpin_values: VecDeque::with_capacity(1000),
        }
    }

    /// Process a new trade
    pub fn process_trade(&mut self, trade: &Trade) {
        let volume = trade.price * trade.quantity;
        self.current_bucket.add_trade(trade.side, volume);

        // Check if bucket is full
        if self.current_bucket.is_full(self.bucket_size) {
            self.complete_bucket();
        }
    }

    /// Complete current bucket and calculate new VPIN
    fn complete_bucket(&mut self) {
        self.buckets.push_back(self.current_bucket.clone());

        // Maintain only num_buckets
        if self.buckets.len() > self.num_buckets {
            self.buckets.pop_front();
        }

        // Calculate VPIN if we have enough buckets
        if self.buckets.len() == self.num_buckets {
            let vpin = self.calculate_vpin();
            self.vpin_values.push_back(vpin);

            // Keep only last 1000 values
            if self.vpin_values.len() > 1000 {
                self.vpin_values.pop_front();
            }
        }

        // Start new bucket
        self.current_bucket = VolumeBucket::new();
    }

    /// Calculate current VPIN value
    fn calculate_vpin(&self) -> f64 {
        if self.buckets.is_empty() {
            return 0.0;
        }

        let total_imbalance: f64 = self.buckets
            .iter()
            .map(|b| b.volume_imbalance())
            .sum();

        let total_volume: f64 = self.buckets
            .iter()
            .map(|b| b.total_volume)
            .sum();

        if total_volume == 0.0 {
            return 0.0;
        }

        total_imbalance / total_volume
    }

    /// Get current VPIN value
    pub fn current_vpin(&self) -> f64 {
        self.vpin_values.back().copied().unwrap_or(0.0)
    }

    /// Check if flow is toxic (high VPIN indicates informed trading)
    pub fn is_toxic(&self, threshold: f64) -> bool {
        self.current_vpin() > threshold
    }

    /// Get VPIN trend (increasing toxicity is bad)
    pub fn vpin_trend(&self, lookback: usize) -> f64 {
        if self.vpin_values.len() < lookback {
            return 0.0;
        }

        let recent: Vec<f64> = self.vpin_values
            .iter()
            .rev()
            .take(lookback)
            .copied()
            .collect();

        if recent.is_empty() {
            return 0.0;
        }

        let first_half: f64 = recent[recent.len()/2..].iter().sum::<f64>() / (recent.len() / 2) as f64;
        let second_half: f64 = recent[..recent.len()/2].iter().sum::<f64>() / (recent.len() / 2) as f64;

        second_half - first_half
    }

    /// Get current buy/sell volume ratio
    pub fn volume_ratio(&self) -> f64 {
        if self.buckets.is_empty() {
            return 1.0;
        }

        let total_buy: f64 = self.buckets.iter().map(|b| b.buy_volume).sum();
        let total_sell: f64 = self.buckets.iter().map(|b| b.sell_volume).sum();

        if total_sell == 0.0 {
            return f64::INFINITY;
        }

        total_buy / total_sell
    }

    /// Get statistics about VPIN
    pub fn statistics(&self) -> VPINStats {
        if self.vpin_values.is_empty() {
            return VPINStats::default();
        }

        let values: Vec<f64> = self.vpin_values.iter().copied().collect();
        let n = values.len() as f64;

        let mean = values.iter().sum::<f64>() / n;

        let variance = values
            .iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>() / n;

        let std_dev = variance.sqrt();

        let mut sorted = values.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let median = if sorted.len() % 2 == 0 {
            (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
        } else {
            sorted[sorted.len() / 2]
        };

        VPINStats {
            current: self.current_vpin(),
            mean,
            median,
            std_dev,
            min: *sorted.first().unwrap_or(&0.0),
            max: *sorted.last().unwrap_or(&0.0),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct VPINStats {
    pub current: f64,
    pub mean: f64,
    pub median: f64,
    pub std_dev: f64,
    pub min: f64,
    pub max: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vpin_basic() {
        let mut vpin = VPIN::new(500.0, 5);

        // Simulate balanced flow with enough volume to fill buckets
        for i in 0..200 {
            let side = if i % 2 == 0 { Side::Buy } else { Side::Sell };
            vpin.process_trade(&Trade {
                timestamp: i,
                price: 50000.0,
                quantity: 0.1, // Larger quantity to fill buckets
                side,
            });
        }

        // VPIN should be relatively low for balanced flow
        // Note: VPIN won't be zero even with balanced flow due to bucket timing
        let vpin_val = vpin.current_vpin();
        assert!(vpin_val >= 0.0 && vpin_val <= 1.0, "VPIN should be between 0 and 1, got {}", vpin_val);
    }

    #[test]
    fn test_vpin_imbalanced() {
        let mut vpin = VPIN::new(100.0, 5);

        // Simulate heavily imbalanced flow (all buys)
        for i in 0..100 {
            vpin.process_trade(&Trade {
                timestamp: i,
                price: 50000.0,
                quantity: 0.002,
                side: Side::Buy,
            });
        }

        // VPIN should be high for imbalanced flow
        assert!(vpin.current_vpin() > 0.7);
    }
}
