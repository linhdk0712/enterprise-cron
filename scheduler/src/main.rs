// Scheduler binary entry point
// Requirements: 9.4, 12.3, 7.6

use common::config::Settings;
use common::db::{DbPool, RedisPool};
use common::lock::RedLock;
use common::queue::{NatsClient, NatsJobPublisher};
use common::scheduler::{Scheduler, SchedulerConfig, SchedulerEngine};
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize tracing/logging
    // Requirements: 5.1, 5.2, 5.9 - Structured logging with JSON format
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "scheduler=info,common=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().json())
        .init();

    info!("Starting Vietnam Enterprise Cron Scheduler");

    // Load configuration
    // Requirements: 7.5 - Configuration management
    let settings = Settings::load().map_err(|e| {
        error!(error = %e, "Failed to load configuration");
        e
    })?;

    info!(
        database_url = %settings.database.url,
        redis_url = %settings.redis.url,
        nats_url = %settings.nats.url,
        "Configuration loaded"
    );

    // Initialize database connection pool
    // Requirements: 12.4 - PostgreSQL connection pool
    info!("Initializing database connection pool");
    let db_pool = DbPool::new(&settings.database).await.map_err(|e| {
        error!(error = %e, "Failed to initialize database pool");
        e
    })?;
    info!("Database connection pool initialized");

    // Initialize Redis connection pool
    // Requirements: 4.1 - Redis for distributed locking
    info!("Initializing Redis connection pool");
    let redis_pool = RedisPool::new(&settings.redis).await.map_err(|e| {
        error!(error = %e, "Failed to initialize Redis pool");
        e
    })?;
    info!("Redis connection pool initialized");

    // Initialize NATS client
    // Requirements: 4.2 - NATS JetStream for job queue
    info!("Initializing NATS client");
    let nats_config = common::queue::NatsConfig {
        url: settings.nats.url.clone(),
        stream_name: settings.nats.stream_name.clone(),
        subject: "jobs.>".to_string(),
        max_age_seconds: 86400, // 24 hours
        max_messages: 1_000_000,
        consumer_name: settings.nats.consumer_name.clone(),
        max_deliver: 10,
    };
    let nats_client = NatsClient::new(nats_config).await.map_err(|e| {
        error!(error = %e, "Failed to initialize NATS client");
        e
    })?;
    info!("NATS client initialized");

    // Initialize NATS stream
    info!("Initializing NATS stream");
    nats_client.initialize_stream().await.map_err(|e| {
        error!(error = %e, "Failed to initialize NATS stream");
        e
    })?;
    info!("NATS stream initialized");

    // Create distributed lock
    // Requirements: 4.1, 7.1 - Distributed locking for scheduler coordination
    let lock = Arc::new(RedLock::new(redis_pool)) as Arc<dyn common::lock::DistributedLock>;
    info!("Distributed lock initialized");

    // Create job publisher
    // Requirements: 4.2 - Job publisher for NATS queue
    let publisher =
        Arc::new(NatsJobPublisher::new(nats_client)) as Arc<dyn common::queue::JobPublisher>;
    info!("Job publisher initialized");

    // Create scheduler configuration
    let scheduler_config = SchedulerConfig {
        poll_interval_seconds: settings.scheduler.poll_interval_seconds,
        lock_ttl_seconds: settings.scheduler.lock_ttl_seconds,
        max_jobs_per_poll: 100,
    };

    // Create scheduler engine
    // Requirements: 9.4 - Initialize only scheduler-specific components
    let scheduler = SchedulerEngine::new(scheduler_config, db_pool, lock, publisher);
    info!("Scheduler engine created");

    // Set up graceful shutdown
    // Requirements: 7.6 - Handle SIGTERM/SIGINT signals
    let scheduler_clone = Arc::new(scheduler);
    let scheduler_for_shutdown = scheduler_clone.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to listen for Ctrl+C");
        info!("Received Ctrl+C signal, initiating graceful shutdown");
        if let Err(e) = scheduler_for_shutdown.stop().await {
            error!(error = %e, "Error during scheduler shutdown");
        }
    });

    // Start the scheduler
    // Requirements: 7.1 - Start scheduler polling loop
    info!("Starting scheduler polling loop");
    if let Err(e) = scheduler_clone.start().await {
        error!(error = %e, "Scheduler error");
        return Err(e);
    }

    info!("Scheduler stopped");
    Ok(())
}
