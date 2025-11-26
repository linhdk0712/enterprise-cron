// Worker binary entry point
// Requirements: 9.5, 12.3 - Worker component isolation and binary separation
// Property 74: Worker component isolation

use anyhow::Result;
use common::config::Settings;
use common::db::pool::DbPool;
use common::db::repositories::execution::ExecutionRepository;
use common::db::repositories::job::JobRepository;
use common::executor::database::DatabaseExecutor;
use common::executor::file::FileProcessingExecutor;
use common::executor::http::HttpExecutor;
use common::executor::JobExecutor;
use common::queue::nats::{NatsClient, NatsConfig};
use common::storage::{MinIOService, MinIOServiceImpl, MinioClient};
use common::worker::context::JobContextManager;
use common::worker::WorkerJobConsumer;
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_level(true)
        .json()
        .init();

    info!("Starting Vietnam Enterprise Cron Worker");

    // Load configuration
    let settings = Settings::load().map_err(|e| {
        error!(error = %e, "Failed to load configuration");
        anyhow::anyhow!("Configuration error: {}", e)
    })?;

    info!("Configuration loaded successfully");

    // Initialize database pool
    let db_pool = DbPool::new(&settings.database).await.map_err(|e| {
        error!(error = %e, "Failed to initialize database pool");
        anyhow::anyhow!("Database initialization error: {}", e)
    })?;

    info!("Database pool initialized");

    // Note: Migrations should be run separately before starting the worker
    info!("Skipping migrations (should be run separately)");

    // Initialize repositories
    let job_repo = Arc::new(JobRepository::new(db_pool.clone()));
    let execution_repo = Arc::new(ExecutionRepository::new(db_pool.clone()));

    info!("Repositories initialized");

    // Initialize MinIO client
    // Requirements: 13.2 - MinIO for job definitions and context storage
    let minio_client = MinioClient::new(&settings.minio).await.map_err(|e| {
        error!(error = %e, "Failed to initialize MinIO client");
        anyhow::anyhow!("MinIO initialization error: {}", e)
    })?;

    info!("MinIO client initialized");

    // Initialize MinIO service
    let minio_service: Arc<dyn MinIOService> = Arc::new(MinIOServiceImpl::new(minio_client));

    // Initialize context manager
    let context_manager = Arc::new(JobContextManager::new(minio_service.clone()));

    // Initialize executors
    let http_executor: Arc<dyn JobExecutor> = Arc::new(HttpExecutor::new(30).map_err(|e| {
        error!(error = %e, "Failed to initialize HTTP executor");
        anyhow::anyhow!("HTTP executor initialization error: {}", e)
    })?); // 30 second timeout
    let database_executor: Arc<dyn JobExecutor> = Arc::new(DatabaseExecutor::new(300)); // 5 minute timeout
    let file_executor: Arc<dyn JobExecutor> =
        Arc::new(FileProcessingExecutor::new(minio_service.clone()));

    info!("Executors initialized");

    // Initialize NATS client
    let nats_config = NatsConfig {
        url: settings.nats.url.clone(),
        stream_name: settings.nats.stream_name.clone(),
        subject: "jobs.>".to_string(),
        max_age_seconds: 86400, // 24 hours
        max_messages: 1_000_000,
        consumer_name: "worker-consumer".to_string(),
        max_deliver: 10,
    };

    let nats_client = NatsClient::new(nats_config).await.map_err(|e| {
        error!(error = %e, "Failed to initialize NATS client");
        anyhow::anyhow!("NATS initialization error: {}", e)
    })?;

    info!("NATS client initialized");

    // Clone NATS client for status publishing before moving it
    let nats_client_for_status = nats_client.client().clone();

    // Create worker job consumer with MinIO service
    // Requirements: 13.4 - Worker supports multi-step jobs with MinIO integration
    // WorkerJobConsumer will create NatsJobConsumer internally with proper handler
    let worker_consumer = WorkerJobConsumer::new(
        nats_client,
        job_repo,
        execution_repo,
        context_manager,
        minio_service,
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
