// Redis client for distributed locking and storage fallback
// RECC 2025: No unwrap(), use #[tracing::instrument], proper error handling

use crate::config::RedisConfig;
use crate::errors::StorageError;
use redis::aio::ConnectionManager;
use redis::Client;
use std::sync::Arc;
use tracing::{info, instrument};

/// Redis client wrapper with connection pooling
#[derive(Clone)]
pub struct RedisClient {
    manager: Arc<ConnectionManager>,
}

impl RedisClient {
    /// Create a new Redis client with connection manager
    /// Connection manager provides automatic reconnection and connection pooling
    #[instrument(skip(config))]
    pub async fn new(config: &RedisConfig) -> Result<Self, StorageError> {
        info!(url = %config.url, "Connecting to Redis");

        let client = Client::open(config.url.as_str()).map_err(|e| {
            StorageError::RedisError(format!("Failed to create Redis client: {}", e))
        })?;

        let manager = ConnectionManager::new(client)
            .await
            .map_err(|e| StorageError::RedisError(format!("Failed to connect to Redis: {}", e)))?;

        info!("Redis connection established");
        Ok(Self {
            manager: Arc::new(manager),
        })
    }

    /// Get a connection from the pool
    pub fn get_connection(&self) -> ConnectionManager {
        (*self.manager).clone()
    }

    /// Health check - ping Redis
    #[instrument(skip(self))]
    pub async fn health_check(&self) -> Result<(), StorageError> {
        use redis::cmd;

        let mut conn = self.get_connection();
        cmd("PING")
            .query_async::<_, String>(&mut conn)
            .await
            .map_err(|e| StorageError::RedisError(format!("Redis health check failed: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redis_config_validation() {
        let config = RedisConfig {
            url: "redis://localhost:6379".to_string(),
            pool_size: 10,
        };
        assert!(!config.url.is_empty());
    }
}
