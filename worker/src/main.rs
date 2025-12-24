// Worker binary entry point
// Requirements: 9.5, 12.3 - Worker component isolation and binary separation
// Property 74: Worker component isolation

use anyhow::Result;
use common::bootstrap;
use common::config::Settings;
use common::db::repositories::execution::ExecutionRepository;
use common::db::repositories::job::JobRepository;
use common::executor::database::DatabaseExecutor;
use common::executor::file::FileProcessingExecutor;
use common::executor::http::HttpExecutor;
use common::executor::JobExecutor;
use common::worker::context::JobContextManager;
use common::worker::WorkerJobConsumer;
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    bootstrap::init_json_tracing();

    info!("Starting Vietnam Enterprise Cron Worker");

    // Load configuration
    let settings = Settings::load()?;
    info!("Configuration loaded successfully");

    // Initialize database pool
    let db_pool = bootstrap::init_database_pool(&settings).await?;

    // Note: Migrations should be run separately before starting the worker
    info!("Skipping migrations (should be run separately)");

    // Initialize repositories
    let job_repo = Arc::new(JobRepository::new(db_pool.clone()));
    let execution_repo = Arc::new(ExecutionRepository::new(db_pool.clone()));
    info!("Repositories initialized");

    // Initialize Redis connection manager for storage cache
    // Requirements: 13.2 - Storage for job definitions and context with Redis cache
    let redis_conn_manager = bootstrap::init_redis_connection_manager(&settings).await?;

    // Initialize Storage service (PostgreSQL + Redis + Filesystem)
    let storage_service =
        bootstrap::init_storage_service(&settings, db_pool.clone(), Arc::new(redis_conn_manager))
            .await?;

    // Initialize context manager
    let context_manager = Arc::new(JobContextManager::new(storage_service.clone()));

    // Initialize executors
    let http_executor: Arc<dyn JobExecutor> = Arc::new(HttpExecutor::new(30)?); // 30 second timeout
    let database_executor: Arc<dyn JobExecutor> = Arc::new(DatabaseExecutor::new(300)); // 5 minute timeout
    let file_executor: Arc<dyn JobExecutor> =
        Arc::new(FileProcessingExecutor::new(storage_service.clone()));
    info!("Executors initialized");

    // Initialize NATS client
    let nats_client = bootstrap::init_nats_client(&settings, "worker-consumer").await?;

    // Clone NATS client for status publishing before moving it
    let nats_client_for_status = nats_client.client().clone();

    // Create worker job consumer with Storage service
    // Requirements: 13.4 - Worker supports multi-step jobs with storage integration
    // WorkerJobConsumer will create NatsJobConsumer internally with proper handler
    let worker_consumer = WorkerJobConsumer::new(
        nats_client,
        job_repo,
        execution_repo,
        context_manager,
        storage_service,
        http_executor,
        database_executor,
        file_executor,
        Some(nats_client_for_status),
    )
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to create worker consumer");
        anyhow::anyhow!("Worker consumer creation error: {}", e)
    })?;

    info!("Worker consumer created, starting job processing");

    // Start the worker in a separate task
    let worker_handle = tokio::spawn(async move {
        if let Err(e) = worker_consumer.start().await {
            error!(error = %e, "Worker consumer error");
        }
    });

    // Wait for shutdown signal
    info!("Worker is running. Press Ctrl+C to shutdown gracefully");

    match signal::ctrl_c().await {
        Ok(()) => {
            info!("Shutdown signal received, initiating graceful shutdown");
        }
        Err(e) => {
            error!(error = %e, "Failed to listen for shutdown signal");
        }
    }

    // Wait for worker to complete
    info!("Waiting for worker to complete in-flight executions");
    let _ = worker_handle.await;

    info!("Worker shutdown complete");
    Ok(())
}
