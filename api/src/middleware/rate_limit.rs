use redis::AsyncCommands;
use std::time::Duration;
use uuid::Uuid;

/// Rate limiter for webhook and login endpoints
pub struct RateLimiter {
    redis_client: redis::Client,
}

impl RateLimiter {
    pub fn new(redis_client: redis::Client) -> Self {
        Self { redis_client }
    }

    /// Check if a webhook request should be rate limited
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

    /// Check if a login attempt should be rate limited
    /// Requirements: 19.14 - Rate limit after multiple failed login attempts (5+ in 15 min)
    #[tracing::instrument(skip(self))]
    pub async fn check_login_rate_limit(&self, ip: &str) -> Result<(), String> {
        let mut conn = self
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| format!("Redis connection error: {}", e))?;

        let key = format!("rate_limit:login:{}", ip);
        let window_seconds = 15 * 60; // 15 minutes
        let max_attempts = 5;

        // Check if currently blocked
        let blocked_key = format!("rate_limit:login:blocked:{}", ip);
        let is_blocked: Option<String> = conn
            .get(&blocked_key)
            .await
            .map_err(|e| format!("Redis error: {}", e))?;

        if is_blocked.is_some() {
            let ttl: i64 = conn
                .ttl(&blocked_key)
                .await
                .map_err(|e| format!("Redis error: {}", e))?;
            return Err(format!(
                "Too many failed login attempts. Please try again in {} seconds.",
                ttl.max(0)
            ));
        }

        // Get current count
        let count: u32 = conn
            .get(&key)
            .await
            .unwrap_or(0);

        if count >= max_attempts {
            // Block this IP for 15 minutes
            let _: () = conn
                .set_ex(&blocked_key, "blocked", window_seconds as u64)
                .await
                .map_err(|e| format!("Redis error: {}", e))?;

            return Err(format!(
                "Too many failed login attempts. Blocked for {} seconds.",
                window_seconds
            ));
        }

        Ok(())
    }

    /// Record a failed login attempt
    /// Requirements: 19.14 - Track failed login attempts
    #[tracing::instrument(skip(self))]
    pub async fn record_failed_login(&self, ip: &str) -> Result<(), String> {
        let mut conn = self
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| format!("Redis connection error: {}", e))?;

        let key = format!("rate_limit:login:{}", ip);
        let window_seconds = 15 * 60; // 15 minutes

        // Increment counter
        let count: u32 = conn
            .incr(&key, 1)
            .await
            .map_err(|e| format!("Redis error: {}", e))?;

        // Set expiry on first request
        if count == 1 {
            let _: () = conn
                .expire(&key, window_seconds)
                .await
                .map_err(|e| format!("Redis error: {}", e))?;
        }

        tracing::warn!(
            ip = %ip,
            attempt_count = count,
            "Failed login attempt recorded"
        );

        Ok(())
    }

    /// Reset rate limit for successful login
    /// Requirements: 19.14 - Reset counter on successful login
    #[tracing::instrument(skip(self))]
    pub async fn reset_login_rate_limit(&self, ip: &str) -> Result<(), String> {
        let mut conn = self
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| format!("Redis connection error: {}", e))?;

        let key = format!("rate_limit:login:{}", ip);
        let _: () = conn
            .del(&key)
            .await
            .map_err(|e| format!("Redis error: {}", e))?;

        tracing::debug!(ip = %ip, "Login rate limit reset");

        Ok(())
    }
}
