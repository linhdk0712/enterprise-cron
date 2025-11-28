// Circuit breaker manager - manages circuit breakers for different targets
// Requirements: 4.5 - Circuit breaker pattern for external system failures

use crate::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Circuit breaker manager handles creation and retrieval of circuit breakers
pub struct CircuitBreakerManager {
    breakers: RwLock<HashMap<String, CircuitBreaker>>,
    config: CircuitBreakerConfig,
}

impl CircuitBreakerManager {
    /// Create a new circuit breaker manager
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            breakers: RwLock::new(HashMap::new()),
            config,
        }
    }

    /// Get or create a circuit breaker for a target
    pub async fn get_or_create(&self, target: &str) -> CircuitBreaker {
        // Check if circuit breaker exists
        {
            let breakers = self.breakers.read().await;
            if let Some(cb) = breakers.get(target) {
                return cb.clone();
            }
        }

        // Create new circuit breaker
        let mut breakers = self.breakers.write().await;
        
        // Double-check after acquiring write lock
        if let Some(cb) = breakers.get(target) {
            return cb.clone();
        }

        let cb = CircuitBreaker::new(target, self.config.clone());
        breakers.insert(target.to_string(), cb.clone());
        cb
    }
}
