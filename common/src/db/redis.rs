// Redis connection pool and health check
// Requirements: 4.1

use crate::config::RedisConfig;
use crate::errors::StorageError;
use redis::aio::ConnectionManager;
use redis::Client;
use tracing::{info, instrument};

/// Redis connection pool wrapper
#[derive(Clone)]
pub struct RedisPool {
    manager: ConnectionManager,
}

impl RedisPool {
    /// Create a new Redis connection pool
    /// Requirements: 4.1 - Redis connection for distributed locking
    #[instrument(skip(config), fields(redis_url = %config.url))]
    pub async fn new(config: &RedisConfig) -> Result<Self, StorageError> {
        info!("Initializing Redis connection pool");

        let client = Client::open(config.url.as_str()).map_err(|e| {
            StorageError::ConnectionFailed(format!("Failed to create Redis client: {}", e))
        })?;

        let manager = ConnectionManager::new(client).await.map_err(|e| {
            StorageError::ConnectionFailed(format!("Failed to create connection manager: {}", e))
        })?;

        info!("Redis connection pool initialized successfully");

        Ok(Self { manager })
    }

    /// Get a connection from the pool
    pub fn get_connection(&self) -> ConnectionManager {
        self.manager.clone()
    }

    /// Health check - verify Redis connection is working
    /// Requirements: 4.1 - Health check for Redis
    #[instrument(skip(self))]
    pub async fn health_check(&self) -> Result<(), StorageError> {
        let mut conn = self.get_connection();

        // Try a simple PING command using redis::cmd
        let response: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| StorageError::RedisError(format!("Health check failed: {}", e)))?;

        if response != "PONG" {
            return Err(StorageError::RedisError(format!(
                "Unexpected PING response: {}",
                response
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires Redis to be running
    async fn test_redis_pool_creation() {
        let config = RedisConfig {
            url: "redis://localhost:6379".to_string(),
            pool_size: 10,
        };

        let pool = RedisPool::new(&config).await;
        assert!(pool.is_ok());
    }

    #[tokio::test]
    #[ignore] // Requires Redis to be running
    async fn test_redis_health_check() {
        let config = RedisConfig {
            url: "redis://localhost:6379".to_string(),
            pool_size: 10,
        };

        let pool = RedisPool::new(&config).await.unwrap();
        let result = pool.health_check().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_redis_pool_invalid_url() {
        let config = RedisConfig {
            url: "redis://invalid-host:9999".to_string(),
            pool_size: 10,
        };

        let result = RedisPool::new(&config).await;
        assert!(result.is_err());
    }
}
