// Circuit breaker implementation for external system failures
// Requirements: 4.7
// Property 35: Circuit breaker activation

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Circuit is closed, requests are allowed
    Closed,
    /// Circuit is open, requests are rejected
    Open,
    /// Circuit is half-open, testing if service recovered
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening the circuit
    pub failure_threshold: u32,
    /// Duration to wait before transitioning from Open to HalfOpen
    pub timeout: Duration,
    /// Number of successful requests in HalfOpen state before closing
    pub success_threshold: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            timeout: Duration::from_secs(60),
            success_threshold: 2,
        }
    }
}

/// Internal state of the circuit breaker
#[derive(Debug)]
struct CircuitBreakerState {
    state: CircuitState,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
}

impl CircuitBreakerState {
    fn new() -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            success_count: 0,
            last_failure_time: None,
        }
    }
}

/// Circuit breaker for protecting against cascading failures
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    name: String,
    config: CircuitBreakerConfig,
    state: Arc<RwLock<CircuitBreakerState>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with the given name and configuration
    pub fn new(name: impl Into<String>, config: CircuitBreakerConfig) -> Self {
        Self {
            name: name.into(),
            config,
            state: Arc::new(RwLock::new(CircuitBreakerState::new())),
        }
    }

    /// Create a new circuit breaker with default configuration
    pub fn with_defaults(name: impl Into<String>) -> Self {
        Self::new(name, CircuitBreakerConfig::default())
    }

    /// Get the current state of the circuit breaker
    pub async fn get_state(&self) -> CircuitState {
        self.state.read().await.state
    }

    /// Get the current failure count
    pub async fn get_failure_count(&self) -> u32 {
        self.state.read().await.failure_count
    }

    /// Get the current success count (in HalfOpen state)
    pub async fn get_success_count(&self) -> u32 {
        self.state.read().await.success_count
    }

    /// Execute a function with circuit breaker protection
    pub async fn call<F, T, E>(&self, f: F) -> Result<T, CircuitBreakerError<E>>
    where
        F: std::future::Future<Output = Result<T, E>>,
    {
        // Check if we should allow the request
        self.check_and_update_state().await.map_err(|e| match e {
            CircuitBreakerError::CircuitOpen { name } => CircuitBreakerError::CircuitOpen { name },
            CircuitBreakerError::RequestFailed(_) => unreachable!(),
        })?;

        // Execute the function
        match f.await {
            Ok(result) => {
                self.on_success().await;
                Ok(result)
            }
            Err(err) => {
                self.on_failure().await;
                Err(CircuitBreakerError::RequestFailed(err))
            }
        }
    }

    /// Check the current state and update if necessary
    async fn check_and_update_state(&self) -> Result<(), CircuitBreakerError<()>> {
        let mut state = self.state.write().await;

        match state.state {
            CircuitState::Closed => {
                // Allow request
                Ok(())
            }
            CircuitState::Open => {
                // Check if timeout has elapsed
                if let Some(last_failure) = state.last_failure_time {
                    if last_failure.elapsed() >= self.config.timeout {
                        // Transition to HalfOpen
                        info!(
                            circuit_breaker = %self.name,
                            "Circuit breaker transitioning from Open to HalfOpen"
                        );
                        state.state = CircuitState::HalfOpen;
                        state.success_count = 0;
                        Ok(())
                    } else {
                        // Circuit is still open
                        Err(CircuitBreakerError::CircuitOpen {
                            name: self.name.clone(),
                        })
                    }
                } else {
                    // Should not happen, but handle gracefully
                    Err(CircuitBreakerError::CircuitOpen {
                        name: self.name.clone(),
                    })
                }
            }
            CircuitState::HalfOpen => {
                // Allow request to test if service recovered
                Ok(())
            }
        }
    }

    /// Handle successful request
    async fn on_success(&self) {
        let mut state = self.state.write().await;

        match state.state {
            CircuitState::Closed => {
                // Reset failure count on success
                state.failure_count = 0;
            }
            CircuitState::HalfOpen => {
                // Increment success count
                state.success_count += 1;

                // Check if we should close the circuit
                if state.success_count >= self.config.success_threshold {
                    info!(
                        circuit_breaker = %self.name,
                        "Circuit breaker transitioning from HalfOpen to Closed"
                    );
                    state.state = CircuitState::Closed;
                    state.failure_count = 0;
                    state.success_count = 0;
                    state.last_failure_time = None;
                }
            }
            CircuitState::Open => {
                // Should not happen, but handle gracefully
            }
        }
    }

    /// Handle failed request
    async fn on_failure(&self) {
        let mut state = self.state.write().await;

        match state.state {
            CircuitState::Closed => {
                // Increment failure count
                state.failure_count += 1;
                state.last_failure_time = Some(Instant::now());

                // Check if we should open the circuit
                if state.failure_count >= self.config.failure_threshold {
                    warn!(
                        circuit_breaker = %self.name,
                        failure_count = state.failure_count,
                        threshold = self.config.failure_threshold,
                        "Circuit breaker transitioning from Closed to Open"
                    );
                    state.state = CircuitState::Open;
                }
            }
            CircuitState::HalfOpen => {
                // Failure in HalfOpen state, go back to Open
                warn!(
                    circuit_breaker = %self.name,
                    "Circuit breaker transitioning from HalfOpen to Open due to failure"
                );
                state.state = CircuitState::Open;
                state.failure_count = self.config.failure_threshold; // Keep it at threshold
                state.success_count = 0;
                state.last_failure_time = Some(Instant::now());
            }
            CircuitState::Open => {
                // Already open, update last failure time
                state.last_failure_time = Some(Instant::now());
            }
        }
    }

    /// Manually reset the circuit breaker to Closed state
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        info!(
            circuit_breaker = %self.name,
            "Circuit breaker manually reset to Closed"
        );
        state.state = CircuitState::Closed;
        state.failure_count = 0;
        state.success_count = 0;
        state.last_failure_time = None;
    }
}

/// Circuit breaker errors
#[derive(Debug, thiserror::Error)]
pub enum CircuitBreakerError<E> {
    #[error("Circuit breaker '{name}' is open")]
    CircuitOpen { name: String },

    #[error("Request failed: {0}")]
    RequestFailed(E),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_circuit_breaker_closed_state() {
        let cb = CircuitBreaker::with_defaults("test");
        assert_eq!(cb.get_state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_after_threshold() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            timeout: Duration::from_secs(60),
            success_threshold: 2,
        };
        let cb = CircuitBreaker::new("test", config);

        // Simulate failures
        for i in 0..3 {
            let result: Result<(), CircuitBreakerError<String>> = cb
                .call(async { Err::<(), String>("error".to_string()) })
                .await;
            assert!(result.is_err());

            if i < 2 {
                assert_eq!(cb.get_state().await, CircuitState::Closed);
            } else {
                assert_eq!(cb.get_state().await, CircuitState::Open);
            }
        }

        // Verify circuit is open
        assert_eq!(cb.get_state().await, CircuitState::Open);
        assert_eq!(cb.get_failure_count().await, 3);
    }

    #[tokio::test]
    async fn test_circuit_breaker_rejects_when_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_secs(60),
            success_threshold: 2,
        };
        let cb = CircuitBreaker::new("test", config);

        // Open the circuit
        for _ in 0..2 {
            let _: Result<(), CircuitBreakerError<String>> = cb
                .call(async { Err::<(), String>("error".to_string()) })
                .await;
        }

        assert_eq!(cb.get_state().await, CircuitState::Open);

        // Try to make a request - should be rejected
        let result: Result<(), CircuitBreakerError<String>> =
            cb.call(async { Ok::<(), String>(()) }).await;

        assert!(matches!(
            result,
            Err(CircuitBreakerError::CircuitOpen { .. })
        ));
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_transition() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_millis(100),
            success_threshold: 2,
        };
        let cb = CircuitBreaker::new("test", config);

        // Open the circuit
        for _ in 0..2 {
            let _: Result<(), CircuitBreakerError<String>> = cb
                .call(async { Err::<(), String>("error".to_string()) })
                .await;
        }

        assert_eq!(cb.get_state().await, CircuitState::Open);

        // Wait for timeout
        sleep(Duration::from_millis(150)).await;

        // Next request should transition to HalfOpen
        let result: Result<(), CircuitBreakerError<String>> =
            cb.call(async { Ok::<(), String>(()) }).await;

        assert!(result.is_ok());
        assert_eq!(cb.get_state().await, CircuitState::HalfOpen);
    }

    #[tokio::test]
    async fn test_circuit_breaker_closes_after_success_threshold() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_millis(100),
            success_threshold: 2,
        };
        let cb = CircuitBreaker::new("test", config);

        // Open the circuit
        for _ in 0..2 {
            let _: Result<(), CircuitBreakerError<String>> = cb
                .call(async { Err::<(), String>("error".to_string()) })
                .await;
        }

        // Wait for timeout
        sleep(Duration::from_millis(150)).await;

        // Make successful requests to close the circuit
        for i in 0..2 {
            let result: Result<(), CircuitBreakerError<String>> =
                cb.call(async { Ok::<(), String>(()) }).await;
            assert!(result.is_ok());

            if i < 1 {
                assert_eq!(cb.get_state().await, CircuitState::HalfOpen);
            } else {
                assert_eq!(cb.get_state().await, CircuitState::Closed);
            }
        }

        assert_eq!(cb.get_state().await, CircuitState::Closed);
        assert_eq!(cb.get_failure_count().await, 0);
    }

    #[tokio::test]
    async fn test_circuit_breaker_reopens_on_half_open_failure() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_millis(100),
            success_threshold: 2,
        };
        let cb = CircuitBreaker::new("test", config);

        // Open the circuit
        for _ in 0..2 {
            let _: Result<(), CircuitBreakerError<String>> = cb
                .call(async { Err::<(), String>("error".to_string()) })
                .await;
        }

        // Wait for timeout
        sleep(Duration::from_millis(150)).await;

        // Make a successful request (transition to HalfOpen)
        let _: Result<(), CircuitBreakerError<String>> =
            cb.call(async { Ok::<(), String>(()) }).await;
        assert_eq!(cb.get_state().await, CircuitState::HalfOpen);

        // Make a failed request (should go back to Open)
        let _: Result<(), CircuitBreakerError<String>> = cb
            .call(async { Err::<(), String>("error".to_string()) })
            .await;

        assert_eq!(cb.get_state().await, CircuitState::Open);
    }

    #[tokio::test]
    async fn test_circuit_breaker_reset() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            timeout: Duration::from_secs(60),
            success_threshold: 2,
        };
        let cb = CircuitBreaker::new("test", config);

        // Open the circuit
        for _ in 0..2 {
            let _: Result<(), CircuitBreakerError<String>> = cb
                .call(async { Err::<(), String>("error".to_string()) })
                .await;
        }

        assert_eq!(cb.get_state().await, CircuitState::Open);

        // Reset the circuit
        cb.reset().await;

        assert_eq!(cb.get_state().await, CircuitState::Closed);
        assert_eq!(cb.get_failure_count().await, 0);
    }

    #[tokio::test]
    async fn test_circuit_breaker_success_resets_failure_count() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            timeout: Duration::from_secs(60),
            success_threshold: 2,
        };
        let cb = CircuitBreaker::new("test", config);

        // Make some failures
        for _ in 0..2 {
            let _: Result<(), CircuitBreakerError<String>> = cb
                .call(async { Err::<(), String>("error".to_string()) })
                .await;
        }

        assert_eq!(cb.get_failure_count().await, 2);
        assert_eq!(cb.get_state().await, CircuitState::Closed);

        // Make a successful request
        let result: Result<(), CircuitBreakerError<String>> =
            cb.call(async { Ok::<(), String>(()) }).await;
        assert!(result.is_ok());

        // Failure count should be reset
        assert_eq!(cb.get_failure_count().await, 0);
        assert_eq!(cb.get_state().await, CircuitState::Closed);
    }
}
