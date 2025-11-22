use std::time::{Duration, Instant};
use std::sync::Arc;
use parking_lot::RwLock;
use anyhow::{Result, anyhow};
use tracing::{warn, error, info};

/// Circuit Breaker Pattern for fault tolerance
/// Prevents cascading failures during exchange outages
#[derive(Clone)]
pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitBreakerState>>,
    config: CircuitBreakerConfig,
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening
    pub failure_threshold: usize,

    /// Time to wait before attempting to close circuit
    pub timeout: Duration,

    /// Number of successes needed to close circuit from half-open
    pub success_threshold: usize,

    /// Max time circuit can stay open (forces half-open attempt)
    pub max_open_duration: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            timeout: Duration::from_secs(60),
            success_threshold: 3,
            max_open_duration: Duration::from_secs(300),
        }
    }
}

#[derive(Debug)]
struct CircuitBreakerState {
    state: State,
    consecutive_failures: usize,
    consecutive_successes: usize,
    last_failure_time: Option<Instant>,
    last_success_time: Option<Instant>,
    state_change_time: Instant,
    total_calls: u64,
    total_failures: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Normal operation - all requests pass through
    Closed,

    /// Circuit opened - all requests fail fast
    Open,

    /// Testing if system recovered - limited requests pass through
    HalfOpen,
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitBreakerState {
                state: State::Closed,
                consecutive_failures: 0,
                consecutive_successes: 0,
                last_failure_time: None,
                last_success_time: None,
                state_change_time: Instant::now(),
                total_calls: 0,
                total_failures: 0,
            })),
            config,
        }
    }

    /// Execute a function with circuit breaker protection
    pub async fn call<F, T, E>(&self, operation_name: &str, f: F) -> Result<T>
    where
        F: std::future::Future<Output = std::result::Result<T, E>>,
        E: std::fmt::Display,
    {
        // Check if we should allow the call
        self.before_call(operation_name)?;

        // Execute the operation
        let start = Instant::now();
        match f.await {
            Ok(result) => {
                let latency = start.elapsed();
                self.on_success(operation_name, latency);
                Ok(result)
            }
            Err(e) => {
                let latency = start.elapsed();
                self.on_failure(operation_name, latency, &e.to_string());
                Err(anyhow!("Circuit breaker: operation failed: {}", e))
            }
        }
    }

    /// Check if call should be allowed
    fn before_call(&self, operation_name: &str) -> Result<()> {
        let mut state = self.state.write();
        state.total_calls += 1;

        match state.state {
            State::Closed => Ok(()),

            State::Open => {
                // Check if we should attempt recovery
                if self.should_attempt_reset(&state) {
                    info!(
                        "Circuit breaker [{}]: Attempting recovery (Half-Open)",
                        operation_name
                    );
                    state.state = State::HalfOpen;
                    state.consecutive_successes = 0;
                    state.state_change_time = Instant::now();
                    Ok(())
                } else {
                    Err(anyhow!(
                        "Circuit breaker OPEN for [{}] - failing fast",
                        operation_name
                    ))
                }
            }

            State::HalfOpen => {
                // Allow limited requests through
                Ok(())
            }
        }
    }

    /// Called on successful operation
    fn on_success(&self, operation_name: &str, latency: Duration) {
        let mut state = self.state.write();
        state.consecutive_failures = 0;
        state.consecutive_successes += 1;
        state.last_success_time = Some(Instant::now());

        match state.state {
            State::HalfOpen => {
                if state.consecutive_successes >= self.config.success_threshold {
                    info!(
                        "Circuit breaker [{}]: Recovered - Closing circuit (success_count={})",
                        operation_name,
                        state.consecutive_successes
                    );
                    state.state = State::Closed;
                    state.consecutive_successes = 0;
                    state.state_change_time = Instant::now();
                }
            }
            State::Closed => {
                // Normal operation
            }
            State::Open => {
                // Shouldn't happen, but recover anyway
                warn!("Circuit breaker [{}]: Success while Open - recovering", operation_name);
                state.state = State::Closed;
                state.state_change_time = Instant::now();
            }
        }
    }

    /// Called on failed operation
    fn on_failure(&self, operation_name: &str, latency: Duration, error: &str) {
        let mut state = self.state.write();
        state.consecutive_failures += 1;
        state.consecutive_successes = 0;
        state.last_failure_time = Some(Instant::now());
        state.total_failures += 1;

        let failure_rate = state.total_failures as f64 / state.total_calls as f64;

        match state.state {
            State::Closed => {
                if state.consecutive_failures >= self.config.failure_threshold {
                    error!(
                        "Circuit breaker [{}]: OPENING circuit (failures={}, rate={:.2}%, error={})",
                        operation_name,
                        state.consecutive_failures,
                        failure_rate * 100.0,
                        error
                    );
                    state.state = State::Open;
                    state.state_change_time = Instant::now();
                }
            }

            State::HalfOpen => {
                warn!(
                    "Circuit breaker [{}]: Recovery failed - reopening circuit",
                    operation_name
                );
                state.state = State::Open;
                state.consecutive_failures = 1;
                state.state_change_time = Instant::now();
            }

            State::Open => {
                // Already open, just count failures
            }
        }
    }

    /// Check if we should attempt to reset the circuit
    fn should_attempt_reset(&self, state: &CircuitBreakerState) -> bool {
        let time_open = state.state_change_time.elapsed();

        // Force reset attempt after max duration
        if time_open >= self.config.max_open_duration {
            return true;
        }

        // Normal timeout
        if time_open >= self.config.timeout {
            return true;
        }

        false
    }

    /// Get current state
    pub fn get_state(&self) -> State {
        self.state.read().state
    }

    /// Force close circuit (emergency recovery)
    pub fn force_close(&self, operation_name: &str) {
        let mut state = self.state.write();
        warn!("Circuit breaker [{}]: FORCE CLOSE", operation_name);
        state.state = State::Closed;
        state.consecutive_failures = 0;
        state.consecutive_successes = 0;
        state.state_change_time = Instant::now();
    }

    /// Force open circuit (emergency halt)
    pub fn force_open(&self, operation_name: &str) {
        let mut state = self.state.write();
        error!("Circuit breaker [{}]: FORCE OPEN", operation_name);
        state.state = State::Open;
        state.state_change_time = Instant::now();
    }

    /// Get statistics
    pub fn get_stats(&self) -> CircuitBreakerStats {
        let state = self.state.read();

        CircuitBreakerStats {
            state: state.state,
            consecutive_failures: state.consecutive_failures,
            consecutive_successes: state.consecutive_successes,
            total_calls: state.total_calls,
            total_failures: state.total_failures,
            failure_rate: if state.total_calls > 0 {
                state.total_failures as f64 / state.total_calls as f64
            } else {
                0.0
            },
            time_in_current_state: state.state_change_time.elapsed(),
        }
    }

    /// Reset statistics
    pub fn reset_stats(&self) {
        let mut state = self.state.write();
        state.total_calls = 0;
        state.total_failures = 0;
    }
}

#[derive(Debug, Clone)]
pub struct CircuitBreakerStats {
    pub state: State,
    pub consecutive_failures: usize,
    pub consecutive_successes: usize,
    pub total_calls: u64,
    pub total_failures: u64,
    pub failure_rate: f64,
    pub time_in_current_state: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_circuit_opens_on_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            timeout: Duration::from_millis(100),
            success_threshold: 2,
            max_open_duration: Duration::from_secs(10),
        };

        let breaker = CircuitBreaker::new(config);

        // Simulate 3 failures
        for _ in 0..3 {
            let result = breaker.call("test", async {
                Err::<(), &str>("simulated error")
            }).await;
            assert!(result.is_err());
        }

        // Circuit should be open
        assert_eq!(breaker.get_state(), State::Open);

        // Next call should fail fast
        let result = breaker.call("test", async {
            Ok::<(), &str>(())
        }).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("OPEN"));
    }

    #[tokio::test]
    async fn test_circuit_recovers() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_millis(50),
            success_threshold: 2,
            max_open_duration: Duration::from_secs(10),
        };

        let breaker = CircuitBreaker::new(config);

        // Open circuit
        for _ in 0..2 {
            let _ = breaker.call("test", async {
                Err::<(), &str>("error")
            }).await;
        }

        assert_eq!(breaker.get_state(), State::Open);

        // Wait for timeout
        sleep(Duration::from_millis(100)).await;

        // Should enter half-open
        let result = breaker.call("test", async {
            Ok::<(), &str>(())
        }).await;
        assert!(result.is_ok());

        // One more success should close it
        let result = breaker.call("test", async {
            Ok::<(), &str>(())
        }).await;
        assert!(result.is_ok());

        assert_eq!(breaker.get_state(), State::Closed);
    }
}
