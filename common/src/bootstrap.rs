// Bootstrap utilities for binary initialization
// Purpose: Eliminate code duplication across main.rs files (api, worker, scheduler)
// RECC 2025: Descriptive file name, single responsibility, DRY principle

use crate::config::Settings;
use crate::db::{DbPool, RedisPool};
use crate::queue::{NatsClient, NatsConfig};
use crate::storage::{StorageService, StorageServiceImpl};
use anyhow::{Context, Result};
use redis::aio::ConnectionManager as RedisConnectionManager;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

/// Initialize Redis connection manager for storage cache
/// Used by: API server, Worker
///
/// # Errors
/// Returns error if Redis client creation or connection manager initialization fails
#[tracing::instrument(skip(settings))]
pub async fn init_redis_connection_manager(settings: &Settings) -> Result<RedisConnectionManager> {
    info!("Initializing Redis connection manager");

    let redis_client = redis::Client::open(settings.redis.url.as_str())
        .context("Failed to create Redis client")?;

    let redis_conn_manager = RedisConnectionManager::new(redis_client)
        .await
        .context("Failed to initialize Redis connection manager")?;

    info!("Redis connection manager initialized");
    Ok(redis_conn_manager)
}

/// Initialize Storage service (PostgreSQL + Redis + Filesystem)
/// Used by: API server, Worker
///
/// # Errors
/// Returns error if storage service initialization fails
#[tracing::instrument(skip(settings, db_pool, redis_conn_manager))]
pub async fn init_storage_service(
    settings: &Settings,
    db_pool: DbPool,
    redis_conn_manager: Arc<RedisConnectionManager>,
) -> Result<Arc<dyn StorageService>> {
    info!("Initializing Storage service (PostgreSQL + Redis + Filesystem)");

    let file_base_path = PathBuf::from(&settings.storage.file_base_path);

    // Create file storage directory if it doesn't exist
    if !file_base_path.exists() {
        tokio::fs::create_dir_all(&file_base_path)
            .await
            .context("Failed to create file storage directory")?;
        info!(path = %file_base_path.display(), "Created file storage directory");
    }

    let storage_service = Arc::new(StorageServiceImpl::new(
        db_pool.pool().clone(),
        redis_conn_manager,
        Some(file_base_path.clone()),
    )) as Arc<dyn StorageService>;

    info!(
        file_base_path = %file_base_path.display(),
        "Storage service initialized"
    );
    Ok(storage_service)
}

/// Initialize NATS client with standard configuration
/// Used by: Worker, Scheduler
///
/// # Arguments
/// * `settings` - Application settings
/// * `consumer_name` - Name for the NATS consumer (e.g., "worker-consumer", "scheduler-consumer")
///
/// # Errors
/// Returns error if NATS client initialization fails
#[tracing::instrument(skip(settings))]
pub async fn init_nats_client(settings: &Settings, consumer_name: &str) -> Result<NatsClient> {
    info!(consumer_name = %consumer_name, "Initializing NATS client");

    let nats_config = NatsConfig {
        url: settings.nats.url.clone(),
        stream_name: settings.nats.stream_name.clone(),
        subject: "jobs.>".to_string(),
        max_age_seconds: 86400, // 24 hours
        max_messages: 1_000_000,
        consumer_name: consumer_name.to_string(),
        max_deliver: 10,
    };

    let nats_client = NatsClient::new(nats_config)
        .await
        .context("Failed to initialize NATS client")?;

    info!("NATS client initialized");
    Ok(nats_client)
}

/// Initialize database pool
/// Used by: API server, Worker, Scheduler
///
/// # Errors
/// Returns error if database pool initialization fails
#[tracing::instrument(skip(settings))]
pub async fn init_database_pool(settings: &Settings) -> Result<DbPool> {
    info!("Initializing database pool");

    let db_pool = DbPool::new(&settings.database)
        .await
        .context("Failed to initialize database pool")?;

    info!("Database pool initialized");
    Ok(db_pool)
}

/// Initialize Redis pool for distributed locking
/// Used by: Scheduler
///
/// # Errors
/// Returns error if Redis pool initialization fails
#[tracing::instrument(skip(settings))]
pub async fn init_redis_pool(settings: &Settings) -> Result<RedisPool> {
    info!("Initializing Redis pool");

    let redis_pool = RedisPool::new(&settings.redis)
        .await
        .context("Failed to initialize Redis pool")?;

    info!("Redis pool initialized");
    Ok(redis_pool)
}

/// Initialize tracing for JSON logging
/// Used by: Worker, Scheduler
///
/// This sets up structured JSON logging with thread IDs and log levels
pub fn init_json_tracing() {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_level(true)
        .json()
        .init();
}

/// Initialize tracing for human-readable logging
/// Used by: API server (development)
///
/// This sets up human-readable logging with environment filter
pub fn init_human_tracing() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "api=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exists() {
        // Smoke test to ensure module compiles
        assert!(true);
    }
}
