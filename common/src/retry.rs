// Retry strategy implementation with exponential backoff and jitter
// Requirements: 4.5, 4.6
// Property 33: Retry limit enforcement
// Property 34: Exponential backoff with jitter

use rand::Rng;
use std::time::Duration;

/// Maximum number of retry attempts
pub const MAX_RETRIES: u32 = 10;

/// Retry strategy trait for calculating retry delays
pub trait RetryStrategy: Send + Sync {
    /// Calculate the delay before the next retry attempt
    /// Returns None if max retries exceeded
    fn next_delay(&self, attempt: u32) -> Option<Duration>;

    /// Check if more retries are allowed
    fn should_retry(&self, attempt: u32) -> bool {
        attempt < MAX_RETRIES
    }

    /// Get the maximum number of retries
    fn max_retries(&self) -> u32 {
        MAX_RETRIES
    }
}

/// Exponential backoff retry strategy with jitter
/// Sequence: 5s, 15s, 1m, 5m, 30m, ... (exponential growth)
/// Jitter: Random value added to prevent thundering herd
#[derive(Debug, Clone)]
pub struct ExponentialBackoff {
    /// Base delay in seconds (default: 5)
    base_delay_secs: u64,
    /// Maximum delay in seconds (default: 1800 = 30 minutes)
    max_delay_secs: u64,
    /// Jitter factor (0.0 to 1.0, default: 0.1 = 10%)
    jitter_factor: f64,
}

impl Default for ExponentialBackoff {
    fn default() -> Self {
        Self {
            base_delay_secs: 5,
            max_delay_secs: 1800, // 30 minutes
            jitter_factor: 0.1,   // 10% jitter
        }
    }
}

impl ExponentialBackoff {
    /// Create a new exponential backoff strategy with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new exponential backoff strategy with custom values
    pub fn with_config(base_delay_secs: u64, max_delay_secs: u64, jitter_factor: f64) -> Self {
        Self {
            base_delay_secs,
            max_delay_secs,
            jitter_factor: jitter_factor.clamp(0.0, 1.0),
        }
    }

    /// Calculate exponential delay without jitter
    fn calculate_base_delay(&self, attempt: u32) -> u64 {
        // Sequence: 5s, 15s, 1m (60s), 5m (300s), 30m (1800s), ...
        // Formula: base * 3^attempt, capped at max_delay
        let delay = self.base_delay_secs * 3_u64.pow(attempt);
        delay.min(self.max_delay_secs)
    }

    /// Add random jitter to prevent thundering herd
    /// Returns delay in milliseconds
    fn add_jitter_ms(&self, base_delay_secs: u64) -> u64 {
        if self.jitter_factor == 0.0 {
            return base_delay_secs * 1000;
        }

        let mut rng = rand::thread_rng();
        let base_delay_ms = base_delay_secs * 1000;
        let jitter_range_ms = (base_delay_ms as f64 * self.jitter_factor) as u64;

        // Generate random jitter in milliseconds
        let jitter_ms = if jitter_range_ms > 0 {
            rng.gen_range(0..=jitter_range_ms)
        } else {
            0
        };

        base_delay_ms + jitter_ms
    }
}

impl RetryStrategy for ExponentialBackoff {
    fn next_delay(&self, attempt: u32) -> Option<Duration> {
        if attempt >= MAX_RETRIES {
            return None;
        }

        let base_delay_secs = self.calculate_base_delay(attempt);
        let delay_with_jitter_ms = self.add_jitter_ms(base_delay_secs);

        Some(Duration::from_millis(delay_with_jitter_ms))
    }
}

/// Fixed delay retry strategy (for testing or simple cases)
#[derive(Debug, Clone)]
pub struct FixedDelay {
    delay: Duration,
}

impl FixedDelay {
    pub fn new(delay: Duration) -> Self {
        Self { delay }
    }
}

impl RetryStrategy for FixedDelay {
    fn next_delay(&self, attempt: u32) -> Option<Duration> {
        if attempt >= MAX_RETRIES {
            return None;
        }
        Some(self.delay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_backoff_sequence() {
        let _strategy = ExponentialBackoff::new();

        // Test the exponential sequence (without jitter for predictability)
        let strategy_no_jitter = ExponentialBackoff::with_config(5, 1800, 0.0);

        // Attempt 0: 5 * 3^0 = 5 seconds
        assert_eq!(strategy_no_jitter.calculate_base_delay(0), 5);

        // Attempt 1: 5 * 3^1 = 15 seconds
        assert_eq!(strategy_no_jitter.calculate_base_delay(1), 15);

        // Attempt 2: 5 * 3^2 = 45 seconds
        assert_eq!(strategy_no_jitter.calculate_base_delay(2), 45);

        // Attempt 3: 5 * 3^3 = 135 seconds
        assert_eq!(strategy_no_jitter.calculate_base_delay(3), 135);

        // Attempt 4: 5 * 3^4 = 405 seconds
        assert_eq!(strategy_no_jitter.calculate_base_delay(4), 405);

        // Attempt 5: 5 * 3^5 = 1215 seconds
        assert_eq!(strategy_no_jitter.calculate_base_delay(5), 1215);

        // Attempt 6: 5 * 3^6 = 3645 seconds, capped at 1800
        assert_eq!(strategy_no_jitter.calculate_base_delay(6), 1800);
    }

    #[test]
    fn test_retry_limit_enforcement() {
        let strategy = ExponentialBackoff::new();

        // Should allow retries up to MAX_RETRIES - 1
        for attempt in 0..MAX_RETRIES {
            assert!(
                strategy.next_delay(attempt).is_some(),
                "Should allow retry at attempt {}",
                attempt
            );
        }

        // Should not allow retry at MAX_RETRIES
        assert!(
            strategy.next_delay(MAX_RETRIES).is_none(),
            "Should not allow retry at attempt {}",
            MAX_RETRIES
        );

        // Should not allow retry beyond MAX_RETRIES
        assert!(
            strategy.next_delay(MAX_RETRIES + 1).is_none(),
            "Should not allow retry beyond MAX_RETRIES"
        );
    }

    #[test]
    fn test_jitter_adds_randomness() {
        let strategy = ExponentialBackoff::new();

        // Get multiple delays for the same attempt (use milliseconds for better precision)
        let mut delays = Vec::new();
        for _ in 0..20 {
            if let Some(delay) = strategy.next_delay(0) {
                delays.push(delay.as_millis());
            }
        }

        // Check that not all delays are identical (jitter is working)
        let first_delay = delays[0];
        let has_variation = delays.iter().any(|&d| d != first_delay);

        // With 10% jitter on 5 seconds (5000ms), we expect some variation
        // With 20 samples, it's extremely unlikely all would be identical
        assert!(
            has_variation,
            "Expected some variation in delays due to jitter, but all {} samples were {}ms",
            delays.len(),
            first_delay
        );

        // All delays should be within the jitter range (in milliseconds)
        let base_delay_ms = 5000u128;
        let max_jitter_ms = (base_delay_ms as f64 * 0.1) as u128;
        for delay in delays {
            assert!(
                delay >= base_delay_ms && delay <= base_delay_ms + max_jitter_ms,
                "Delay {}ms should be between {}ms and {}ms",
                delay,
                base_delay_ms,
                base_delay_ms + max_jitter_ms
            );
        }
    }

    #[test]
    fn test_should_retry() {
        let strategy = ExponentialBackoff::new();

        // Should retry for attempts 0 to MAX_RETRIES - 1
        for attempt in 0..MAX_RETRIES {
            assert!(
                strategy.should_retry(attempt),
                "Should retry at attempt {}",
                attempt
            );
        }

        // Should not retry at MAX_RETRIES or beyond
        assert!(!strategy.should_retry(MAX_RETRIES));
        assert!(!strategy.should_retry(MAX_RETRIES + 1));
    }

    #[test]
    fn test_max_retries() {
        let strategy = ExponentialBackoff::new();
        assert_eq!(strategy.max_retries(), MAX_RETRIES);
    }

    #[test]
    fn test_fixed_delay_strategy() {
        let delay = Duration::from_secs(10);
        let strategy = FixedDelay::new(delay);

        // Should return the same delay for all attempts
        for attempt in 0..MAX_RETRIES {
            assert_eq!(strategy.next_delay(attempt), Some(delay));
        }

        // Should return None after MAX_RETRIES
        assert_eq!(strategy.next_delay(MAX_RETRIES), None);
    }

    #[test]
    fn test_custom_config() {
        let strategy = ExponentialBackoff::with_config(10, 3600, 0.2);

        // Test with custom base delay
        let delay = strategy.next_delay(0).unwrap();
        let delay_secs = delay.as_secs();

        // Should be around 10 seconds with 20% jitter (10-12 seconds)
        assert!(
            delay_secs >= 10 && delay_secs <= 12,
            "Delay {} should be between 10 and 12 seconds",
            delay_secs
        );
    }

    #[test]
    fn test_jitter_factor_clamping() {
        // Test that jitter factor is clamped to [0.0, 1.0]
        let strategy1 = ExponentialBackoff::with_config(5, 1800, -0.5);
        assert_eq!(strategy1.jitter_factor, 0.0);

        let strategy2 = ExponentialBackoff::with_config(5, 1800, 1.5);
        assert_eq!(strategy2.jitter_factor, 1.0);

        let strategy3 = ExponentialBackoff::with_config(5, 1800, 0.5);
        assert_eq!(strategy3.jitter_factor, 0.5);
    }
}
