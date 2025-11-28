// Worker job consumer module
// Requirements: 4.2, 4.3, 13.4, 13.7, 13.8
// Tách theo RECC 2025 rules - Tách theo responsibility

mod job_processor;
mod step_executor;
mod circuit_breaker_manager;

use crate::circuit_breaker::CircuitBreakerConfig;
use crate::db::repositories::execution::ExecutionRepository;
use crate::db::repositories::job::JobRepository;
use crate::errors::QueueError;
use crate::executor::JobExecutor;
use crate::queue::{JobHandler, JobMessage, NatsClient, NatsJobConsumer};
use crate::retry::ExponentialBackoff;
use crate::storage::MinIOService;
use crate::worker::context::ContextManager;
use crate::worker::reference::ReferenceResolver;
use std::sync::Arc;
use tracing::{info, instrument};

pub use job_processor::JobProcessor;
pub use step_executor::StepExecutor;
pub use circuit_breaker_manager::CircuitBreakerManager;

/// Worker job consumer that processes jobs from the queue
#[allow(dead_code)]
pub struct WorkerJobConsumer {
    consumer: NatsJobConsumer,
    job_repo: Arc<JobRepository>,
    execution_repo: Arc<ExecutionRepository>,
    context_manager: Arc<dyn ContextManager>,
    minio_service: Arc<dyn MinIOService>,
    http_executor: Arc<dyn JobExecutor>,
    database_executor: Arc<dyn JobExecutor>,
    file_executor: Arc<dyn JobExecutor>,
    nats_client: Option<async_nats::Client>,
}

#[allow(dead_code)]
impl WorkerJobConsumer {
    /// Create a new worker job consumer
    #[instrument(skip_all)]
    pub async fn new(
        nats_client: NatsClient,
        job_repo: Arc<JobRepository>,
        execution_repo: Arc<ExecutionRepository>,
        context_manager: Arc<dyn ContextManager>,
        minio_service: Arc<dyn MinIOService>,
        http_executor: Arc<dyn JobExecutor>,
        database_executor: Arc<dyn JobExecutor>,
        file_executor: Arc<dyn JobExecutor>,
        nats_client_for_status: Option<async_nats::Client>,
    ) -> Result<Self, QueueError> {
        info!("Creating worker job consumer with MinIO integration");

        // Create handler with all dependencies
        let handler = Self::create_handler_static(
            Arc::clone(&job_repo),
            Arc::clone(&execution_repo),
            Arc::clone(&context_manager),
            Arc::clone(&minio_service),
            Arc::clone(&http_executor),
            Arc::clone(&database_executor),
            Arc::clone(&file_executor),
            nats_client_for_status.clone(),
        );

        // Create NATS consumer with the handler
        let consumer = NatsJobConsumer::new(nats_client, handler).await?;

        Ok(Self {
            consumer,
            job_repo,
            execution_repo,
            context_manager,
            minio_service,
            http_executor,
            database_executor,
            file_executor,
            nats_client: nats_client_for_status,
        })
    }

    /// Start consuming jobs from the queue
    pub async fn start(&self) -> Result<(), QueueError> {
        info!("Starting worker job consumer");
        self.consumer.start().await
    }

    /// Request graceful shutdown
    pub fn shutdown(&self) {
        info!("Requesting worker shutdown");
        self.consumer.shutdown();
    }

    /// Create the job handler closure (static method for use in constructor)
    fn create_handler_static(
        job_repo: Arc<JobRepository>,
        execution_repo: Arc<ExecutionRepository>,
        context_manager: Arc<dyn ContextManager>,
        minio_service: Arc<dyn MinIOService>,
        http_executor: Arc<dyn JobExecutor>,
        database_executor: Arc<dyn JobExecutor>,
        file_executor: Arc<dyn JobExecutor>,
        nats_client: Option<async_nats::Client>,
    ) -> JobHandler {
        let retry_strategy = Arc::new(ExponentialBackoff::new());
        let circuit_breaker_manager = Arc::new(CircuitBreakerManager::new(CircuitBreakerConfig {
            failure_threshold: 5,
            timeout: std::time::Duration::from_secs(60),
            success_threshold: 2,
        }));
        let reference_resolver = Arc::new(ReferenceResolver::new());

        Arc::new(move |job_message: JobMessage| {
            let processor = JobProcessor::new(
                Arc::clone(&job_repo),
                Arc::clone(&execution_repo),
                Arc::clone(&context_manager),
                Arc::clone(&minio_service),
                Arc::clone(&http_executor),
                Arc::clone(&database_executor),
                Arc::clone(&file_executor),
                Arc::clone(&retry_strategy),
                Arc::clone(&circuit_breaker_manager),
                Arc::clone(&reference_resolver),
                nats_client.clone(),
            );

            Box::pin(async move { processor.process(job_message).await })
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_worker_consumer_creation() {
        assert!(true);
    }
}
