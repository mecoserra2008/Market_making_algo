use std::collections::VecDeque;
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use std::sync::Arc;

/// Latency monitor for tracking system performance
pub struct LatencyMonitor {
    orderbook_latencies: Arc<RwLock<VecDeque<Duration>>>,
    trade_latencies: Arc<RwLock<VecDeque<Duration>>>,
    order_placement_latencies: Arc<RwLock<VecDeque<Duration>>>,
    max_samples: usize,
}

impl LatencyMonitor {
    pub fn new(max_samples: usize) -> Self {
        Self {
            orderbook_latencies: Arc::new(RwLock::new(VecDeque::with_capacity(max_samples))),
            trade_latencies: Arc::new(RwLock::new(VecDeque::with_capacity(max_samples))),
            order_placement_latencies: Arc::new(RwLock::new(VecDeque::with_capacity(max_samples))),
            max_samples,
        }
    }

    /// Record orderbook update latency
    pub fn record_orderbook_latency(&self, latency: Duration) {
        let mut latencies = self.orderbook_latencies.write();
        latencies.push_back(latency);
        while latencies.len() > self.max_samples {
            latencies.pop_front();
        }
    }

    /// Record trade latency
    pub fn record_trade_latency(&self, latency: Duration) {
        let mut latencies = self.trade_latencies.write();
        latencies.push_back(latency);
        while latencies.len() > self.max_samples {
            latencies.pop_front();
        }
    }

    /// Record order placement latency
    pub fn record_order_placement_latency(&self, latency: Duration) {
        let mut latencies = self.order_placement_latencies.write();
        latencies.push_back(latency);
        while latencies.len() > self.max_samples {
            latencies.pop_front();
        }
    }

    /// Get orderbook latency statistics
    pub fn get_orderbook_stats(&self) -> LatencyStats {
        self.calculate_stats(&self.orderbook_latencies.read())
    }

    /// Get trade latency statistics
    pub fn get_trade_stats(&self) -> LatencyStats {
        self.calculate_stats(&self.trade_latencies.read())
    }

    /// Get order placement latency statistics
    pub fn get_order_placement_stats(&self) -> LatencyStats {
        self.calculate_stats(&self.order_placement_latencies.read())
    }

    /// Calculate statistics from latency samples
    fn calculate_stats(&self, samples: &VecDeque<Duration>) -> LatencyStats {
        if samples.is_empty() {
            return LatencyStats::default();
        }

        let mut sorted: Vec<Duration> = samples.iter().copied().collect();
        sorted.sort();

        let sum: Duration = sorted.iter().sum();
        let mean = sum / sorted.len() as u32;

        let p50 = sorted[sorted.len() / 2];
        let p95 = sorted[(sorted.len() * 95) / 100];
        let p99 = sorted[(sorted.len() * 99) / 100];

        LatencyStats {
            count: sorted.len(),
            min: *sorted.first().unwrap(),
            max: *sorted.last().unwrap(),
            mean,
            p50,
            p95,
            p99,
        }
    }

    /// Get all statistics
    pub fn get_all_stats(&self) -> AllLatencyStats {
        AllLatencyStats {
            orderbook: self.get_orderbook_stats(),
            trades: self.get_trade_stats(),
            order_placement: self.get_order_placement_stats(),
        }
    }

    /// Check if latencies are healthy
    pub fn is_healthy(&self, max_p99_ms: u64) -> bool {
        let max_duration = Duration::from_millis(max_p99_ms);

        let ob_stats = self.get_orderbook_stats();
        let trade_stats = self.get_trade_stats();
        let order_stats = self.get_order_placement_stats();

        ob_stats.p99 < max_duration &&
        trade_stats.p99 < max_duration &&
        order_stats.p99 < max_duration
    }
}

#[derive(Debug, Clone, Default)]
pub struct LatencyStats {
    pub count: usize,
    pub min: Duration,
    pub max: Duration,
    pub mean: Duration,
    pub p50: Duration,
    pub p95: Duration,
    pub p99: Duration,
}

impl LatencyStats {
    pub fn to_millis(&self) -> LatencyStatsMillis {
        LatencyStatsMillis {
            count: self.count,
            min_ms: self.min.as_millis() as f64,
            max_ms: self.max.as_millis() as f64,
            mean_ms: self.mean.as_millis() as f64,
            p50_ms: self.p50.as_millis() as f64,
            p95_ms: self.p95.as_millis() as f64,
            p99_ms: self.p99.as_millis() as f64,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LatencyStatsMillis {
    pub count: usize,
    pub min_ms: f64,
    pub max_ms: f64,
    pub mean_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
}

#[derive(Debug, Clone)]
pub struct AllLatencyStats {
    pub orderbook: LatencyStats,
    pub trades: LatencyStats,
    pub order_placement: LatencyStats,
}

/// Timer for measuring latency
pub struct LatencyTimer {
    start: Instant,
}

impl LatencyTimer {
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_monitor() {
        let monitor = LatencyMonitor::new(100);

        // Record some latencies
        for i in 1..=10 {
            monitor.record_orderbook_latency(Duration::from_millis(i));
        }

        let stats = monitor.get_orderbook_stats();
        assert_eq!(stats.count, 10);
        assert_eq!(stats.min, Duration::from_millis(1));
        assert_eq!(stats.max, Duration::from_millis(10));
    }

    #[test]
    fn test_max_samples() {
        let monitor = LatencyMonitor::new(5);

        for i in 1..=10 {
            monitor.record_orderbook_latency(Duration::from_millis(i));
        }

        let stats = monitor.get_orderbook_stats();
        assert_eq!(stats.count, 5); // Only keeps last 5
        assert_eq!(stats.min, Duration::from_millis(6));
        assert_eq!(stats.max, Duration::from_millis(10));
    }

    #[test]
    fn test_health_check() {
        let monitor = LatencyMonitor::new(100);

        for _ in 0..10 {
            monitor.record_orderbook_latency(Duration::from_millis(5));
        }

        assert!(monitor.is_healthy(100)); // 5ms < 100ms threshold
        assert!(!monitor.is_healthy(1)); // 5ms > 1ms threshold
    }
}
