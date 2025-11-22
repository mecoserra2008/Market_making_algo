use std::time::{SystemTime, UNIX_EPOCH};

/// Get current timestamp in milliseconds
pub fn current_timestamp_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// Get current timestamp in seconds
pub fn current_timestamp_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// Convert basis points to decimal
pub fn bps_to_decimal(bps: f64) -> f64 {
    bps / 10000.0
}

/// Convert decimal to basis points
pub fn decimal_to_bps(decimal: f64) -> f64 {
    decimal * 10000.0
}

/// Round to tick size
pub fn round_to_tick(price: f64, tick_size: f64) -> f64 {
    (price / tick_size).round() * tick_size
}

/// Round down to step size
pub fn round_to_step_size(quantity: f64, step_size: f64) -> f64 {
    (quantity / step_size).floor() * step_size
}

/// Calculate percentage change
pub fn pct_change(old: f64, new: f64) -> f64 {
    if old == 0.0 {
        return 0.0;
    }
    ((new - old) / old) * 100.0
}

/// Exponential moving average
pub struct EMA {
    alpha: f64,
    value: Option<f64>,
}

impl EMA {
    pub fn new(period: usize) -> Self {
        let alpha = 2.0 / (period as f64 + 1.0);
        Self {
            alpha,
            value: None,
        }
    }

    pub fn update(&mut self, new_value: f64) -> f64 {
        match self.value {
            None => {
                self.value = Some(new_value);
                new_value
            }
            Some(old_value) => {
                let new_ema = self.alpha * new_value + (1.0 - self.alpha) * old_value;
                self.value = Some(new_ema);
                new_ema
            }
        }
    }

    pub fn get(&self) -> Option<f64> {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bps_conversion() {
        assert_eq!(bps_to_decimal(100.0), 0.01);
        assert_eq!(decimal_to_bps(0.01), 100.0);
    }

    #[test]
    fn test_ema() {
        let mut ema = EMA::new(10);
        let v1 = ema.update(100.0);
        assert_eq!(v1, 100.0);

        let v2 = ema.update(110.0);
        assert!(v2 > 100.0 && v2 < 110.0);
    }
}
