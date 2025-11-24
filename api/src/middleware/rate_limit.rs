use redis::AsyncCommands;
use std::time::Duration;
use uuid::Uuid;

/// Rate limiter for webhook endpoints
pub struct RateLimiter {
    redis_client: redis::Client,
}

impl RateLimiter {
    pub fn new(redis_client: redis::Client) -> Self {
        Self { redis_client }
    }

    /// Check if a request should be rate limited
    /// Returns Ok(()) if allowed, Err if rate limit exceeded
    #[tracing::instrument(skip(self))]
    pub async fn check_rate_limit(
        &self,
        job_id: Uuid,
        max_requests: u32,
        window_seconds: u32,
    ) -> Result<(), anyhow::Error> {
        let mut conn = self.redis_client.get_multiplexed_async_connection().await?;

        let key = format!("rate_limit:webhook:{}", job_id);
        let window = Duration::from_secs(window_seconds as u64);

        // Increment counter
        let count: u32 = conn.incr(&key, 1).await?;

        // Set expiry on first request
        if count == 1 {
            conn.expire::<_, ()>(&key, window.as_secs() as i64).await?;
        }

        // Check if rate limit exceeded
        if count > max_requests {
            tracing::warn!(
                job_id = %job_id,
                count = count,
                max_requests = max_requests,
                "Rate limit exceeded"
            );
            return Err(anyhow::anyhow!("Rate limit exceeded"));
        }

        Ok(())
    }
}
