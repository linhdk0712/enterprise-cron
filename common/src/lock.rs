// Distributed locking with Redis RedLock algorithm
// Requirements: 4.1, 7.1

use crate::db::RedisPool;
use crate::errors::StorageError;
use async_trait::async_trait;
use redis::AsyncCommands;
use std::time::Duration;
use tokio::time::{sleep, Instant};
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

/// Distributed lock trait for ensuring exclusive access to resources
#[async_trait]
pub trait DistributedLock: Send + Sync {
    /// Acquire a lock on the specified resource with a TTL
    async fn acquire(&self, resource: &str, ttl: Duration) -> Result<LockGuard, StorageError>;
}

/// Lock guard that automatically releases the lock when dropped
pub struct LockGuard {
    resource: String,
    lock_value: String,
    pool: RedisPool,
    acquired_at: Instant,
    ttl: Duration,
}

impl LockGuard {
    /// Extend the lock TTL for long-running operations
    /// Requirements: 4.1 - Lock extension for long operations
    #[instrument(skip(self), fields(resource = %self.resource, ttl_seconds = ?self.ttl.as_secs()))]
    pub async fn extend(&mut self, additional_ttl: Duration) -> Result<(), StorageError> {
        let mut conn = self.pool.get_connection();
        let key = format!("lock:{}", self.resource);

        // Check if we still own the lock
        let current_value: Option<String> = conn
            .get(&key)
            .await
            .map_err(|e| StorageError::RedisError(format!("Failed to check lock: {}", e)))?;

        if current_value.as_ref() != Some(&self.lock_value) {
            return Err(StorageError::RedisError(
                "Lock no longer owned by this guard".to_string(),
            ));
        }

        // Extend the TTL
        let new_ttl = self.ttl + additional_ttl;
        let _: () = conn
            .expire(&key, new_ttl.as_secs() as i64)
            .await
            .map_err(|e| StorageError::RedisError(format!("Failed to extend lock: {}", e)))?;

        self.ttl = new_ttl;
        info!(
            resource = %self.resource,
            new_ttl_seconds = new_ttl.as_secs(),
            "Lock TTL extended"
        );

        Ok(())
    }

    /// Get the resource name this lock guards
    pub fn resource(&self) -> &str {
        &self.resource
    }

    /// Get the time elapsed since lock acquisition
    pub fn elapsed(&self) -> Duration {
        self.acquired_at.elapsed()
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        // Release the lock asynchronously
        let resource = self.resource.clone();
        let lock_value = self.lock_value.clone();
        let pool = self.pool.clone();

        tokio::spawn(async move {
            if let Err(e) = release_lock(&pool, &resource, &lock_value).await {
                warn!(
                    resource = %resource,
                    error = %e,
                    "Failed to release lock on drop"
                );
            }
        });
    }
}

/// RedLock implementation for distributed locking
/// Requirements: 4.1 - Redis RedLock algorithm for distributed locking
pub struct RedLock {
    pool: RedisPool,
    retry_count: u32,
    retry_delay: Duration,
}

impl RedLock {
    /// Create a new RedLock instance
    pub fn new(pool: RedisPool) -> Self {
        Self {
            pool,
            retry_count: 3,
            retry_delay: Duration::from_millis(200),
        }
    }

    /// Create a RedLock with custom retry configuration
    pub fn with_retry(pool: RedisPool, retry_count: u32, retry_delay: Duration) -> Self {
        Self {
            pool,
            retry_count,
            retry_delay,
        }
    }

    /// Try to acquire the lock once
    async fn try_acquire_once(
        &self,
        resource: &str,
        ttl: Duration,
    ) -> Result<LockGuard, StorageError> {
        let mut conn = self.pool.get_connection();
        let key = format!("lock:{}", resource);
        let lock_value = Uuid::new_v4().to_string();

        // Use SET NX EX to atomically set the key if it doesn't exist with expiration
        let result: Option<String> = redis::cmd("SET")
            .arg(&key)
            .arg(&lock_value)
            .arg("NX") // Only set if not exists
            .arg("EX") // Set expiration in seconds
            .arg(ttl.as_secs())
            .query_async(&mut conn)
            .await
            .map_err(|e| StorageError::RedisError(format!("Failed to acquire lock: {}", e)))?;

        if result.is_some() {
            debug!(
                resource = %resource,
                lock_value = %lock_value,
                ttl_seconds = ttl.as_secs(),
                "Lock acquired"
            );

            Ok(LockGuard {
                resource: resource.to_string(),
                lock_value,
                pool: self.pool.clone(),
                acquired_at: Instant::now(),
                ttl,
            })
        } else {
            Err(StorageError::RedisError(format!(
                "Lock already held for resource: {}",
                resource
            )))
        }
    }
}

#[async_trait]
impl DistributedLock for RedLock {
    /// Acquire a distributed lock with retry logic
    /// Requirements: 4.1 - Lock acquisition with TTL
    #[instrument(skip(self), fields(resource = %resource, ttl_seconds = ?ttl.as_secs()))]
    async fn acquire(&self, resource: &str, ttl: Duration) -> Result<LockGuard, StorageError> {
        let mut attempts = 0;

        loop {
            match self.try_acquire_once(resource, ttl).await {
                Ok(guard) => {
                    info!(
                        resource = %resource,
                        attempts = attempts + 1,
                        "Lock acquired successfully"
                    );
                    return Ok(guard);
                }
                Err(e) => {
                    attempts += 1;
                    if attempts >= self.retry_count {
                        warn!(
                            resource = %resource,
                            attempts,
                            "Failed to acquire lock after all retries"
                        );
                        return Err(e);
                    }

                    debug!(
                        resource = %resource,
                        attempt = attempts,
                        retry_delay_ms = self.retry_delay.as_millis(),
                        "Lock acquisition failed, retrying"
                    );

                    sleep(self.retry_delay).await;
                }
            }
        }
    }
}

/// Release a lock by deleting the key if it matches the lock value
/// Requirements: 4.1 - Lock release
async fn release_lock(
    pool: &RedisPool,
    resource: &str,
    lock_value: &str,
) -> Result<(), StorageError> {
    let mut conn = pool.get_connection();
    let key = format!("lock:{}", resource);

    // Use Lua script to atomically check and delete
    // This ensures we only delete the lock if we own it
    let script = r#"
        if redis.call("get", KEYS[1]) == ARGV[1] then
            return redis.call("del", KEYS[1])
        else
            return 0
        end
    "#;

    let result: i32 = redis::Script::new(script)
        .key(&key)
        .arg(lock_value)
        .invoke_async(&mut conn)
        .await
        .map_err(|e| StorageError::RedisError(format!("Failed to release lock: {}", e)))?;

    if result == 1 {
        debug!(
            resource = %resource,
            lock_value = %lock_value,
            "Lock released successfully"
        );
    } else {
        warn!(
            resource = %resource,
            lock_value = %lock_value,
            "Lock was not owned or already expired"
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RedisConfig;

    #[tokio::test]
    #[ignore] // Requires Redis to be running
    async fn test_lock_acquire_and_release() {
        let config = RedisConfig {
            url: "redis://localhost:6379".to_string(),
            pool_size: 10,
        };
        let pool = RedisPool::new(&config).await.unwrap();
        let lock = RedLock::new(pool);

        let guard = lock
            .acquire("test_resource", Duration::from_secs(10))
            .await
            .unwrap();

        assert_eq!(guard.resource(), "test_resource");
        drop(guard);

        // Should be able to acquire again after release
        let _guard2 = lock
            .acquire("test_resource", Duration::from_secs(10))
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore] // Requires Redis to be running
    async fn test_lock_exclusivity() {
        let config = RedisConfig {
            url: "redis://localhost:6379".to_string(),
            pool_size: 10,
        };
        let pool = RedisPool::new(&config).await.unwrap();
        let lock1 = RedLock::new(pool.clone());
        let lock2 = RedLock::with_retry(pool, 1, Duration::from_millis(10));

        let _guard1 = lock1
            .acquire("exclusive_resource", Duration::from_secs(10))
            .await
            .unwrap();

        // Second acquisition should fail
        let result = lock2
            .acquire("exclusive_resource", Duration::from_secs(10))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    #[ignore] // Requires Redis to be running
    async fn test_lock_extension() {
        let config = RedisConfig {
            url: "redis://localhost:6379".to_string(),
            pool_size: 10,
        };
        let pool = RedisPool::new(&config).await.unwrap();
        let lock = RedLock::new(pool);

        let mut guard = lock
            .acquire("extendable_resource", Duration::from_secs(5))
            .await
            .unwrap();

        // Extend the lock
        let result = guard.extend(Duration::from_secs(5)).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore] // Requires Redis to be running
    async fn test_lock_auto_release_on_drop() {
        let config = RedisConfig {
            url: "redis://localhost:6379".to_string(),
            pool_size: 10,
        };
        let pool = RedisPool::new(&config).await.unwrap();
        let lock = RedLock::new(pool.clone());

        {
            let _guard = lock
                .acquire("auto_release_resource", Duration::from_secs(10))
                .await
                .unwrap();
            // Guard dropped here
        }

        // Give some time for async drop to complete
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Should be able to acquire again
        let lock2 = RedLock::new(pool);
        let _guard2 = lock2
            .acquire("auto_release_resource", Duration::from_secs(10))
            .await
            .unwrap();
    }
}
