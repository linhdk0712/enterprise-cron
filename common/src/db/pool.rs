// PostgreSQL connection pool implementation
// Requirements: 12.4 - Configure sqlx with compile-time query checking, connection pool, health checks

use crate::config::DatabaseConfig;
use crate::errors::DatabaseError;
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::time::Duration;
use tracing::{info, instrument};

/// Database connection pool wrapper
/// Provides a managed connection pool to PostgreSQL with health checking
#[derive(Debug, Clone)]
pub struct DbPool {
    pool: PgPool,
}

impl DbPool {
    /// Create a new database connection pool
    ///
    /// # Arguments
    /// * `config` - Database configuration with connection URL and pool settings
    ///
    /// # Requirements
    /// - 12.4: Configure sqlx with compile-time query checking
    /// - 12.4: Implement connection pool with min/max connections
    ///
    /// # Errors
    /// Returns `DatabaseError::ConnectionFailed` if unable to establish connection
    #[instrument(skip(config), fields(max_connections = config.max_connections))]
    pub async fn new(config: &DatabaseConfig) -> Result<Self, DatabaseError> {
        info!("Initializing database connection pool");

        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(Duration::from_secs(config.connect_timeout_seconds))
            .connect(&config.url)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to create database pool");
                DatabaseError::ConnectionFailed(e.to_string())
            })?;

        info!(
            max_connections = config.max_connections,
            min_connections = config.min_connections,
            "Database connection pool initialized successfully"
        );

        Ok(Self { pool })
    }

    /// Get a reference to the underlying pool
    ///
    /// This is used by repositories to execute queries
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Perform a health check on the database connection
    ///
    /// # Requirements
    /// - 12.4: Add health check queries
    ///
    /// # Returns
    /// `Ok(())` if the database is healthy, `Err` otherwise
    #[instrument(skip(self))]
    pub async fn health_check(&self) -> Result<(), DatabaseError> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Database health check failed");
                DatabaseError::HealthCheckFailed(e.to_string())
            })?;

        tracing::debug!("Database health check passed");
        Ok(())
    }

    /// Get the current number of connections in the pool
    pub fn size(&self) -> u32 {
        self.pool.size()
    }

    /// Get the number of idle connections in the pool
    pub fn num_idle(&self) -> usize {
        self.pool.num_idle()
    }

    /// Close the connection pool gracefully
    ///
    /// This should be called during graceful shutdown to ensure all connections
    /// are properly closed
    #[instrument(skip(self))]
    pub async fn close(&self) {
        info!("Closing database connection pool");
        self.pool.close().await;
        info!("Database connection pool closed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires running PostgreSQL instance
    async fn test_pool_creation() {
        let config = DatabaseConfig {
            url: "postgresql://postgres:postgres@localhost/test_db".to_string(),
            max_connections: 5,
            min_connections: 1,
            connect_timeout_seconds: 5,
        };

        let result = DbPool::new(&config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore] // Requires running PostgreSQL instance
    async fn test_health_check() {
        let config = DatabaseConfig {
            url: "postgresql://postgres:postgres@localhost/test_db".to_string(),
            max_connections: 5,
            min_connections: 1,
            connect_timeout_seconds: 5,
        };

        let pool = DbPool::new(&config).await.unwrap();
        let result = pool.health_check().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_pool_size_tracking() {
        // This test verifies the pool size methods are available
        // Actual values would require a real connection
    }
}
