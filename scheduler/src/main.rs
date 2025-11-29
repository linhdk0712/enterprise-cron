// Scheduler binary entry point
// Requirements: 9.4, 12.3, 7.6

use common::bootstrap;
use common::config::Settings;
use common::lock::RedLock;
use common::queue::NatsJobPublisher;
use common::scheduler::{Scheduler, SchedulerConfig, SchedulerEngine};
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize tracing/logging
    // Requirements: 5.1, 5.2, 5.9 - Structured logging with JSON format
    bootstrap::init_json_tracing();

    info!("Starting Vietnam Enterprise Cron Scheduler");

    // Load configuration
    // Requirements: 7.5 - Configuration management
    let settings = Settings::load()?;

    info!(
        database_url = %settings.database.url,
        redis_url = %settings.redis.url,
        nats_url = %settings.nats.url,
        "Configuration loaded"
    );

    // Initialize database connection pool
    // Requirements: 12.4 - PostgreSQL connection pool
    let db_pool = bootstrap::init_database_pool(&settings).await?;

    // Initialize Redis connection pool
    // Requirements: 4.1 - Redis for distributed locking
    let redis_pool = bootstrap::init_redis_pool(&settings).await?;

    // Initialize NATS client
    // Requirements: 4.2 - NATS JetStream for job queue
    let nats_client = bootstrap::init_nats_client(&settings, &settings.nats.consumer_name).await?;

    // Initialize NATS stream
    info!("Initializing NATS stream");
    nats_client.initialize_stream().await?;
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
        if let Ok(()) = tokio::signal::ctrl_c().await {
            info!("Received Ctrl+C signal, initiating graceful shutdown");
            if let Err(e) = scheduler_for_shutdown.stop().await {
                tracing::error!(error = %e, "Error during scheduler shutdown");
            }
        }
    });

    // Start the scheduler
    // Requirements: 7.1 - Start scheduler polling loop
    info!("Starting scheduler polling loop");
    scheduler_clone.start().await?;

    info!("Scheduler stopped");
    Ok(())
}
