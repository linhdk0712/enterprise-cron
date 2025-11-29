// Job processor - handles job message processing and execution orchestration
// Requirements: 13.4, 13.7, 13.8 - Load job definition from storage and execute multi-step jobs

use crate::db::repositories::execution::ExecutionRepository;
use crate::db::repositories::job::JobRepository;
use crate::executor::JobExecutor;
use crate::models::{ExecutionStatus, Job, JobContext, JobExecution, TriggerSource};
use crate::queue::JobMessage;
use crate::retry::RetryStrategy;
use crate::storage::StorageService;
use crate::worker::context::ContextManager;
use crate::worker::reference::ReferenceResolver;
use chrono::Utc;
use std::sync::Arc;
use tracing::{error, info, instrument, warn};

use super::{CircuitBreakerManager, StepExecutor};

/// Job processor handles the complete job execution lifecycle
pub struct JobProcessor {
    job_repo: Arc<JobRepository>,
    execution_repo: Arc<ExecutionRepository>,
    _context_manager: Arc<dyn ContextManager>,
    storage_service: Arc<dyn StorageService>,
    http_executor: Arc<dyn JobExecutor>,
    database_executor: Arc<dyn JobExecutor>,
    file_executor: Arc<dyn JobExecutor>,
    retry_strategy: Arc<dyn RetryStrategy>,
    circuit_breaker_manager: Arc<CircuitBreakerManager>,
    reference_resolver: Arc<ReferenceResolver>,
    nats_client: Option<async_nats::Client>,
}

impl JobProcessor {
    /// Create a new job processor
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        job_repo: Arc<JobRepository>,
        execution_repo: Arc<ExecutionRepository>,
        context_manager: Arc<dyn ContextManager>,
        storage_service: Arc<dyn StorageService>,
        http_executor: Arc<dyn JobExecutor>,
        database_executor: Arc<dyn JobExecutor>,
        file_executor: Arc<dyn JobExecutor>,
        retry_strategy: Arc<dyn RetryStrategy>,
        circuit_breaker_manager: Arc<CircuitBreakerManager>,
        reference_resolver: Arc<ReferenceResolver>,
        nats_client: Option<async_nats::Client>,
    ) -> Self {
        Self {
            job_repo,
            execution_repo,
            _context_manager: context_manager,
            storage_service,
            http_executor,
            database_executor,
            file_executor,
            retry_strategy,
            circuit_breaker_manager,
            reference_resolver,
            nats_client,
        }
    }

    /// Process a single job message
    #[instrument(skip(self), fields(
        execution_id = %job_message.execution_id,
        job_id = %job_message.job_id,
        idempotency_key = %job_message.idempotency_key,
        attempt = job_message.attempt
    ))]
    pub async fn process(&self, job_message: JobMessage) -> Result<(), anyhow::Error> {
        info!("Processing job");

        // Check idempotency
        if self.check_idempotency(&job_message).await? {
            return Ok(());
        }

        // Load job metadata and definition
        let (_job_metadata, job_definition) = self.load_job(&job_message).await?;

        // Create or load execution record
        let mut execution = self.create_or_load_execution(&job_message).await?;

        // Update status to Running
        execution.status = ExecutionStatus::Running;
        execution.started_at = Some(Utc::now());
        if let Err(e) = self.execution_repo.update(&execution).await {
            error!(error = %e, "Failed to update execution status to Running");
        }

        self.publish_status_change(execution.id, execution.job_id, "running").await;

        // Initialize or load job context
        let mut context = self.load_or_initialize_context(&job_definition, &execution).await?;

        // Execute job steps
        let step_executor = StepExecutor::new(
            Arc::clone(&self.http_executor),
            Arc::clone(&self.database_executor),
            Arc::clone(&self.file_executor),
            Arc::clone(&self.storage_service),
            Arc::clone(&self.reference_resolver),
            Arc::clone(&self.circuit_breaker_manager),
            Arc::clone(&self.retry_strategy),
            Arc::clone(&self.execution_repo),
        );

        let execution_result = step_executor
            .execute_all_steps(&job_definition, &mut context, &mut execution)
            .await;

        // Update final execution status
        let final_status = self.finalize_execution(&mut execution, execution_result).await;

        // Save final context to storage
        if let Err(e) = self.storage_service.store_context(&context).await {
            error!(error = %e, "Failed to save final job context to storage");
        } else {
            info!("Final job context saved to storage successfully");
        }

        self.publish_status_change(execution.id, execution.job_id, final_status).await;

        Ok(())
    }

    /// Check if job has already been executed (idempotency check)
    async fn check_idempotency(&self, job_message: &JobMessage) -> Result<bool, anyhow::Error> {
        match self.execution_repo.find_by_idempotency_key(&job_message.idempotency_key).await {
            Ok(Some(existing_execution)) => {
                match existing_execution.status {
                    ExecutionStatus::Success
                    | ExecutionStatus::Failed
                    | ExecutionStatus::Timeout
                    | ExecutionStatus::DeadLetter
                    | ExecutionStatus::Cancelled => {
                        info!(
                            existing_execution_id = %existing_execution.id,
                            status = ?existing_execution.status,
                            "Job already completed with this idempotency key, skipping"
                        );
                        Ok(true)
                    }
                    _ => Ok(false),
                }
            }
            Ok(None) => {
                info!("No existing execution found, proceeding with job execution");
                Ok(false)
            }
            Err(e) => {
                error!(error = %e, "Failed to check idempotency key");
                Err(anyhow::anyhow!("Failed to check idempotency: {}", e))
            }
        }
    }

    /// Load job metadata and definition from database and MinIO
    async fn load_job(&self, job_message: &JobMessage) -> Result<(Job, Job), anyhow::Error> {
        // Load job metadata from database
        let job_metadata = match self.job_repo.find_by_id(job_message.job_id).await {
            Ok(Some(job)) => job,
            Ok(None) => {
                error!("Job not found");
                return Err(anyhow::anyhow!("Job not found: {}", job_message.job_id));
            }
            Err(e) => {
                error!(error = %e, "Failed to load job metadata");
                return Err(anyhow::anyhow!("Failed to load job metadata: {}", e));
            }
        };

        info!(
            job_name = %job_metadata.name,
            job_id = %job_metadata.id,
            "Loaded job metadata from database"
        );

        // Load full job definition from storage
        let job_definition_json = match self.storage_service.load_job_definition(job_message.job_id).await {
            Ok(json) => json,
            Err(e) => {
                error!(error = %e, "Failed to load job definition from storage");
                return Err(anyhow::anyhow!("Failed to load job definition from storage: {}", e));
            }
        };

        // Parse job definition
        let job: Job = match serde_json::from_str(&job_definition_json) {
            Ok(job) => job,
            Err(e) => {
                error!(error = %e, "Failed to parse job definition JSON");
                return Err(anyhow::anyhow!("Failed to parse job definition: {}", e));
            }
        };

        info!(
            job_name = %job.name,
            step_count = job.steps.len(),
            "Loaded and parsed job definition from storage"
        );

        Ok((job_metadata, job))
    }

    /// Create new or load existing execution record
    async fn create_or_load_execution(&self, job_message: &JobMessage) -> Result<JobExecution, anyhow::Error> {
        match self.execution_repo.find_by_id(job_message.execution_id).await {
            Ok(Some(exec)) => Ok(exec),
            Ok(None) => {
                // Create new execution using factory method
                let mut new_execution = JobExecution::new_with_params(
                    job_message.job_id,
                    job_message.idempotency_key.clone(),
                    TriggerSource::Scheduled,
                    job_message.attempt,
                );
                
                // Override ID to match the message (important for consistency)
                new_execution.id = job_message.execution_id;
                new_execution.status = ExecutionStatus::Running;
                new_execution.started_at = Some(Utc::now());

                self.execution_repo.create(&new_execution).await
                    .map_err(|e| anyhow::anyhow!("Failed to create execution: {}", e))?;

                Ok(new_execution)
            }
            Err(e) => {
                error!(error = %e, "Failed to load execution");
                Err(anyhow::anyhow!("Failed to load execution: {}", e))
            }
        }
    }

    /// Load existing context or initialize new one
    async fn load_or_initialize_context(&self, job: &Job, execution: &JobExecution) -> Result<JobContext, anyhow::Error> {
        match self.storage_service.load_context(job.id, execution.id).await {
            Ok(ctx) => {
                info!(steps_completed = ctx.steps.len(), "Loaded existing job context from storage");
                Ok(ctx)
            }
            Err(_) => {
                info!("Initializing new job context");
                Ok(JobContext::new(execution.id, job.id))
            }
        }
    }

    /// Finalize execution with result
    async fn finalize_execution(&self, execution: &mut JobExecution, result: Result<(), anyhow::Error>) -> &'static str {
        let final_status = match &result {
            Ok(()) => {
                info!("Job execution completed successfully");
                execution.status = ExecutionStatus::Success;
                execution.completed_at = Some(Utc::now());
                execution.result = Some("Job completed successfully".to_string());
                "success"
            }
            Err(e) => {
                error!(error = %e, "Job execution failed");
                execution.status = ExecutionStatus::Failed;
                execution.completed_at = Some(Utc::now());
                execution.error = Some(e.to_string());
                "failed"
            }
        };

        if let Err(e) = self.execution_repo.update(execution).await {
            error!(error = %e, "Failed to update final execution status");
        }

        final_status
    }

    /// Publish execution status change event to NATS for SSE broadcasting
    async fn publish_status_change(&self, execution_id: uuid::Uuid, job_id: uuid::Uuid, status: &str) {
        if let Some(client) = &self.nats_client {
            let event = serde_json::json!({
                "type": "execution_status_changed",
                "execution_id": execution_id,
                "job_id": job_id,
                "status": status,
            });

            if let Ok(payload) = serde_json::to_vec(&event) {
                let subject = format!("status.execution.{}", execution_id);
                if let Err(e) = client.publish(subject, payload.into()).await {
                    warn!(error = %e, "Failed to publish status change event");
                }
            }
        }
    }
}
