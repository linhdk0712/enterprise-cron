use crate::errors::StorageError;
use redis::AsyncCommands;
use uuid::Uuid;

/// RateLimiter provides rate limiting functionality using Redis
/// Requirements: 16.11 - Webhook rate limiting with Redis
pub struct RateLimiter {
    redis_client: redis::Client,
}

impl RateLimiter {
    pub fn new(redis_client: redis::Client) -> Self {
        Self { redis_client }
    }

    /// Check if a webhook request should be rate limited
    /// Requirements: 16.11 - Enforce per-job rate limits, return 429 for violations
    ///
    /// # Arguments
    /// * `webhook_id` - The webhook ID
    /// * `max_requests` - Maximum requests allowed in the window
    /// * `window_seconds` - Time window in seconds
    ///
    /// # Returns
    /// * `Ok(true)` if request is allowed
    /// * `Ok(false)` if request should be rate limited
    #[tracing::instrument(skip(self))]
    pub async fn check_rate_limit(
        &self,
        webhook_id: Uuid,
        max_requests: u32,
        window_seconds: u32,
    ) -> Result<bool, StorageError> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        let key = format!("rate_limit:webhook:{}", webhook_id);
        let now = chrono::Utc::now().timestamp();
        let window_start = now - window_seconds as i64;

        // Use Redis sorted set with timestamps as scores
        // Remove old entries outside the window
        let _: () = conn.zrembyscore(&key, 0, window_start).await?;

        // Count current requests in the window
        let count: u32 = conn.zcard(&key).await?;

        if count >= max_requests {
            tracing::warn!(
                webhook_id = %webhook_id,
                count = count,
                max_requests = max_requests,
                "Rate limit exceeded"
            );
            return Ok(false);
        }

        // Add current request to the sorted set
        let request_id = Uuid::new_v4().to_string();
        let _: () = conn.zadd(&key, request_id, now).await?;

        // Set expiration on the key (window + buffer)
        let _: () = conn.expire(&key, (window_seconds + 60) as i64).await?;

        tracing::debug!(
            webhook_id = %webhook_id,
            count = count + 1,
            max_requests = max_requests,
            "Rate limit check passed"
        );

        Ok(true)
    }

    /// Get current request count for a webhook
    /// Requirements: 16.11 - Track rate limit usage
    #[tracing::instrument(skip(self))]
    pub async fn get_current_count(
        &self,
        webhook_id: Uuid,
        window_seconds: u32,
    ) -> Result<u32, StorageError> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        let key = format!("rate_limit:webhook:{}", webhook_id);
        let now = chrono::Utc::now().timestamp();
        let window_start = now - window_seconds as i64;

        // Remove old entries
        let _: () = conn.zrembyscore(&key, 0, window_start).await?;

        // Count current requests
        let count: u32 = conn.zcard(&key).await?;

        Ok(count)
    }

    /// Reset rate limit for a webhook (for testing or manual intervention)
    /// Requirements: 16.11 - Rate limit management
    #[tracing::instrument(skip(self))]
    pub async fn reset_rate_limit(&self, webhook_id: Uuid) -> Result<(), StorageError> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        let key = format!("rate_limit:webhook:{}", webhook_id);
        let _: () = conn.del(&key).await?;

        tracing::info!(webhook_id = %webhook_id, "Reset rate limit");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a running Redis instance
    // They are integration tests and should be run with --ignored flag

    #[tokio::test]
    #[ignore]
    async fn test_rate_limit_allows_requests_within_limit() {
        let redis_client = redis::Client::open("redis://127.0.0.1:6379").unwrap();
        let rate_limiter = RateLimiter::new(redis_client);
        let webhook_id = Uuid::new_v4();

        // Reset before test
        rate_limiter.reset_rate_limit(webhook_id).await.unwrap();

        // Should allow first 3 requests
        for i in 1..=3 {
            let allowed = rate_limiter
                .check_rate_limit(webhook_id, 5, 60)
                .await
                .unwrap();
            assert!(allowed, "Request {} should be allowed", i);
        }

        // Check count
        let count = rate_limiter
            .get_current_count(webhook_id, 60)
            .await
            .unwrap();
        assert_eq!(count, 3);
    }

    #[tokio::test]
    #[ignore]
    async fn test_rate_limit_blocks_requests_over_limit() {
        let redis_client = redis::Client::open("redis://127.0.0.1:6379").unwrap();
        let rate_limiter = RateLimiter::new(redis_client);
        let webhook_id = Uuid::new_v4();

        // Reset before test
        rate_limiter.reset_rate_limit(webhook_id).await.unwrap();

        // Fill up the limit
        for _ in 1..=5 {
            let allowed = rate_limiter
                .check_rate_limit(webhook_id, 5, 60)
                .await
                .unwrap();
            assert!(allowed);
        }

        // Next request should be blocked
        let allowed = rate_limiter
            .check_rate_limit(webhook_id, 5, 60)
            .await
            .unwrap();
        assert!(!allowed, "Request over limit should be blocked");
    }

    #[tokio::test]
    #[ignore]
    async fn test_rate_limit_resets_after_window() {
        let redis_client = redis::Client::open("redis://127.0.0.1:6379").unwrap();
        let rate_limiter = RateLimiter::new(redis_client);
        let webhook_id = Uuid::new_v4();

        // Reset before test
        rate_limiter.reset_rate_limit(webhook_id).await.unwrap();

        // Use a short window for testing (2 seconds)
        for _ in 1..=3 {
            let allowed = rate_limiter
                .check_rate_limit(webhook_id, 3, 2)
                .await
                .unwrap();
            assert!(allowed);
        }

        // Should be at limit
        let allowed = rate_limiter
            .check_rate_limit(webhook_id, 3, 2)
            .await
            .unwrap();
        assert!(!allowed);

        // Wait for window to expire
        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

        // Should allow requests again
        let allowed = rate_limiter
            .check_rate_limit(webhook_id, 3, 2)
            .await
            .unwrap();
        assert!(allowed, "Request should be allowed after window expires");
    }
}
