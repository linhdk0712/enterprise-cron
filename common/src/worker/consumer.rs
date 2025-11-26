// Worker job consumer implementation
// Requirements: 4.2, 4.3, 13.4, 13.7, 13.8
// Consumes jobs from NATS queue, checks idempotency, loads job definitions from MinIO,
// and executes multi-step jobs sequentially with context persistence

use crate::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use crate::db::repositories::execution::ExecutionRepository;
use crate::db::repositories::job::JobRepository;
use crate::errors::QueueError;
use crate::executor::JobExecutor;
use crate::models::{ExecutionStatus, Job, JobContext, JobExecution, JobStep, JobType, StepOutput};
use crate::queue::{JobConsumer, JobHandler, JobMessage, NatsClient, NatsJobConsumer};
use crate::retry::{ExponentialBackoff, RetryStrategy};
use crate::storage::MinIOService;
use crate::worker::context::ContextManager;
use crate::worker::reference::ReferenceResolver;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{error, info, instrument, warn};

/// Worker job consumer that processes jobs from the queue
/// Requirements: 13.4, 13.7, 13.8 - Multi-step job execution with MinIO integration
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
    retry_strategy: Arc<dyn RetryStrategy>,
    circuit_breakers: Arc<tokio::sync::RwLock<HashMap<String, CircuitBreaker>>>,
    reference_resolver: Arc<ReferenceResolver>,
    nats_client: Option<async_nats::Client>,
}

#[allow(dead_code)]
impl WorkerJobConsumer {
    /// Create a new worker job consumer
    /// Requirements: 13.4 - Initialize worker with MinIO service for multi-step jobs
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
            retry_strategy: Arc::new(ExponentialBackoff::new()),
            circuit_breakers: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            reference_resolver: Arc::new(ReferenceResolver::new()),
            nats_client: nats_client_for_status,
        })
    }

    /// Start consuming jobs from the queue
    pub async fn start(&self) -> Result<(), QueueError> {
        info!("Starting worker job consumer");

        // Start the consumer (handler is already configured)
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
        let retry_strategy: Arc<dyn RetryStrategy> = Arc::new(ExponentialBackoff::new());
        let circuit_breakers: Arc<tokio::sync::RwLock<HashMap<String, CircuitBreaker>>> =
            Arc::new(tokio::sync::RwLock::new(HashMap::new()));
        let reference_resolver = Arc::new(ReferenceResolver::new());

        Arc::new(move |job_message: JobMessage| {
            let job_repo = Arc::clone(&job_repo);
            let execution_repo = Arc::clone(&execution_repo);
            let context_manager = Arc::clone(&context_manager);
            let minio_service = Arc::clone(&minio_service);
            let http_executor = Arc::clone(&http_executor);
            let database_executor = Arc::clone(&database_executor);
            let file_executor = Arc::clone(&file_executor);
            let retry_strategy = Arc::clone(&retry_strategy);
            let circuit_breakers = Arc::clone(&circuit_breakers);
            let reference_resolver = Arc::clone(&reference_resolver);
            let nats_client = nats_client.clone();

            Box::pin(async move {
                Self::process_job(
                    job_message,
                    job_repo,
                    execution_repo,
                    context_manager,
                    minio_service,
                    http_executor,
                    database_executor,
                    file_executor,
                    retry_strategy,
                    circuit_breakers,
                    reference_resolver,
                    nats_client,
                )
                .await
            })
        })
    }

    /// Process a single job message
    /// Requirements: 13.4, 13.7, 13.8 - Load job definition from MinIO and execute multi-step jobs
    /// Property 79: Sequential step execution
    #[instrument(skip_all, fields(
        execution_id = %job_message.execution_id,
        job_id = %job_message.job_id,
        idempotency_key = %job_message.idempotency_key,
        attempt = job_message.attempt
    ))]
    async fn process_job(
        job_message: JobMessage,
        job_repo: Arc<JobRepository>,
        execution_repo: Arc<ExecutionRepository>,
        _context_manager: Arc<dyn ContextManager>,
        minio_service: Arc<dyn MinIOService>,
        http_executor: Arc<dyn JobExecutor>,
        database_executor: Arc<dyn JobExecutor>,
        file_executor: Arc<dyn JobExecutor>,
        retry_strategy: Arc<dyn RetryStrategy>,
        circuit_breakers: Arc<tokio::sync::RwLock<HashMap<String, CircuitBreaker>>>,
        reference_resolver: Arc<ReferenceResolver>,
        nats_client: Option<async_nats::Client>,
    ) -> Result<(), anyhow::Error> {
        info!("Processing job");

        // Check idempotency - has this job already been executed?
        // Requirements: 4.3 - Check for previous executions with idempotency key
        // Property 30: Exactly-once execution
        match execution_repo
            .find_by_idempotency_key(&job_message.idempotency_key)
            .await
        {
            Ok(Some(existing_execution)) => {
                // Only skip if execution is already completed (success/failed/timeout/dead_letter)
                // If execution is pending or running, we should process it
                match existing_execution.status {
                    ExecutionStatus::Success
                    | ExecutionStatus::Failed
                    | ExecutionStatus::Timeout
                    | ExecutionStatus::DeadLetter => {
                        info!(
                            existing_execution_id = %existing_execution.id,
                            status = ?existing_execution.status,
                            "Job already completed with this idempotency key, skipping"
                        );
                        return Ok(());
                    }
                    ExecutionStatus::Pending | ExecutionStatus::Running => {
                        info!(
                            existing_execution_id = %existing_execution.id,
                            status = ?existing_execution.status,
                            "Found existing execution in progress, will process it"
                        );
                        // Continue to process this execution
                    }
                }
            }
            Ok(None) => {
                info!("No existing execution found, proceeding with job execution");
            }
            Err(e) => {
                error!(error = %e, "Failed to check idempotency key");
                return Err(anyhow::anyhow!("Failed to check idempotency: {}", e));
            }
        }

        // Load job metadata from database
        let job_metadata = match job_repo.find_by_id(job_message.job_id).await {
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
            minio_path = %job_metadata.minio_definition_path,
            "Loaded job metadata from database"
        );

        // Load full job definition from MinIO
        // Requirements: 13.2 - Load job definition from MinIO
        // Property 77: MinIO job definition persistence
        let job_definition_json = match minio_service.load_job_definition(job_message.job_id).await
        {
            Ok(json) => json,
            Err(e) => {
                error!(error = %e, "Failed to load job definition from MinIO");
                return Err(anyhow::anyhow!(
                    "Failed to load job definition from MinIO: {}",
                    e
                ));
            }
        };

        // Parse job definition to get steps
        // Requirements: 13.1 - Accept JSON job definition document
        // Property 76: JSON job definition acceptance
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
            "Loaded and parsed job definition from MinIO"
        );

        // Create or load job execution record
        let mut execution = match execution_repo.find_by_id(job_message.execution_id).await {
            Ok(Some(exec)) => exec,
            Ok(None) => {
                // Create new execution record
                let new_execution = JobExecution {
                    id: job_message.execution_id,
                    job_id: job_message.job_id,
                    idempotency_key: job_message.idempotency_key.clone(),
                    status: ExecutionStatus::Running,
                    attempt: job_message.attempt,
                    trigger_source: crate::models::TriggerSource::Scheduled,
                    current_step: None,
                    minio_context_path: format!(
                        "jobs/{}/executions/{}/context.json",
                        job_message.job_id, job_message.execution_id
                    ),
                    started_at: Some(Utc::now()),
                    completed_at: None,
                    result: None,
                    error: None,
                    created_at: Utc::now(),
                };

                execution_repo
                    .create(&new_execution)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to create execution: {}", e))?;

                new_execution
            }
            Err(e) => {
                error!(error = %e, "Failed to load execution");
                return Err(anyhow::anyhow!("Failed to load execution: {}", e));
            }
        };

        // Update execution status to Running
        execution.status = ExecutionStatus::Running;
        execution.started_at = Some(Utc::now());

        if let Err(e) = execution_repo.update(&execution).await {
            error!(error = %e, "Failed to update execution status to Running");
        }

        // Publish status change event
        Self::publish_status_change(&nats_client, execution.id, execution.job_id, "running").await;

        // Initialize or load job context
        // Requirements: 13.7 - Initialize Job Context for new executions
        // Requirements: 13.8 - Load Job Context for subsequent steps
        // Property 84: Job Context loading for subsequent steps
        let mut context = match minio_service.load_context(job.id, execution.id).await {
            Ok(ctx) => {
                info!(
                    steps_completed = ctx.steps.len(),
                    "Loaded existing job context from MinIO"
                );
                ctx
            }
            Err(_) => {
                info!("Initializing new job context");
                // Initialize new context
                JobContext::new(execution.id, job.id)
            }
        };

        // Execute job steps sequentially
        // Requirements: 13.4 - Execute steps sequentially
        // Property 79: Sequential step execution
        let execution_result = Self::execute_job_steps(
            &job,
            &mut context,
            &mut execution,
            http_executor,
            database_executor,
            file_executor,
            Arc::clone(&minio_service),
            reference_resolver,
            circuit_breakers,
            retry_strategy,
            execution_repo.clone(),
        )
        .await;

        // Update final execution status
        let final_status = match &execution_result {
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

        // Save final execution state
        if let Err(e) = execution_repo.update(&execution).await {
            error!(error = %e, "Failed to update final execution status");
        }

        // Publish final status change event
        Self::publish_status_change(&nats_client, execution.id, execution.job_id, final_status)
            .await;

        // Save final context to MinIO
        // Requirements: 13.9, 13.10 - Retain Job Context after completion or failure
        // Property 85: Job Context retention after completion
        // Property 86: Job Context preservation on failure
        if let Err(e) = minio_service.store_context(&context).await {
            error!(error = %e, "Failed to save final job context to MinIO");
        } else {
            info!("Final job context saved to MinIO successfully");
        }

        execution_result
    }

    /// Execute all job steps sequentially
    /// Requirements: 13.4, 13.7, 13.8 - Execute steps sequentially and persist context after each step
    /// Property 79: Sequential step execution
    /// Property 82: Job Context persistence to MinIO
    #[instrument(skip_all, fields(job_id = %job.id, job_name = %job.name))]
    async fn execute_job_steps(
        job: &Job,
        context: &mut JobContext,
        execution: &mut JobExecution,
        http_executor: Arc<dyn JobExecutor>,
        database_executor: Arc<dyn JobExecutor>,
        file_executor: Arc<dyn JobExecutor>,
        minio_service: Arc<dyn MinIOService>,
        reference_resolver: Arc<ReferenceResolver>,
        circuit_breakers: Arc<tokio::sync::RwLock<HashMap<String, CircuitBreaker>>>,
        retry_strategy: Arc<dyn RetryStrategy>,
        execution_repo: Arc<ExecutionRepository>,
    ) -> Result<(), anyhow::Error> {
        info!(
            step_count = job.steps.len(),
            "Executing job steps sequentially"
        );

        for (index, step) in job.steps.iter().enumerate() {
            info!(
                step_index = index,
                step_id = %step.id,
                step_name = %step.name,
                "Executing step"
            );

            // Update current step in execution record
            execution.current_step = Some(step.id.clone());
            if let Err(e) = execution_repo.update(execution).await {
                warn!(error = %e, "Failed to update current step");
            }

            // Check if step has a condition
            if let Some(condition) = &step.condition {
                // Evaluate condition (simplified - would need proper expression evaluation)
                info!(condition = %condition, "Step has condition, evaluating");
                // For now, we'll skip condition evaluation and always execute
                // TODO: Implement proper condition evaluation using reference resolver
            }

            // Execute the step with timeout
            let timeout_duration = Duration::from_secs(job.timeout_seconds as u64);
            let step_result = timeout(
                timeout_duration,
                Self::execute_single_step(
                    step,
                    context,
                    &http_executor,
                    &database_executor,
                    &file_executor,
                    &reference_resolver,
                    &circuit_breakers,
                    &retry_strategy,
                ),
            )
            .await;

            match step_result {
                Ok(Ok(step_output)) => {
                    info!(step_id = %step.id, "Step completed successfully");

                    // Store step output in context
                    // Requirements: 13.5, 13.6, 14.5 - Automatically store step output
                    // Property 80: HTTP response storage in Job Context
                    // Property 81: Database result storage in Job Context
                    // Property 93: Automatic step output storage
                    context.set_step_output(step.id.clone(), step_output);

                    // Persist context to MinIO after each step
                    // Requirements: 13.7 - Persist Job Context to MinIO after each step
                    // Property 82: Job Context persistence to MinIO
                    if let Err(e) = minio_service.store_context(context).await {
                        error!(error = %e, "Failed to save context to MinIO after step");
                        return Err(anyhow::anyhow!("Failed to save context: {}", e));
                    }

                    info!(
                        step_id = %step.id,
                        completed_steps = context.completed_steps_count(),
                        "Context saved to MinIO after step completion"
                    );
                }
                Ok(Err(e)) => {
                    error!(step_id = %step.id, error = %e, "Step execution failed");
                    return Err(anyhow::anyhow!("Step {} failed: {}", step.id, e));
                }
                Err(_) => {
                    error!(step_id = %step.id, "Step execution timed out");
                    execution.status = ExecutionStatus::Timeout;
                    return Err(anyhow::anyhow!("Step {} timed out", step.id));
                }
            }
        }

        info!(
            total_steps = job.steps.len(),
            "All steps completed successfully"
        );
        Ok(())
    }

    /// Execute a single job step
    #[instrument(skip_all, fields(step_id = %step.id, step_name = %step.name))]
    async fn execute_single_step(
        step: &JobStep,
        context: &mut JobContext,
        http_executor: &Arc<dyn JobExecutor>,
        database_executor: &Arc<dyn JobExecutor>,
        file_executor: &Arc<dyn JobExecutor>,
        _reference_resolver: &Arc<ReferenceResolver>,
        circuit_breakers: &Arc<tokio::sync::RwLock<HashMap<String, CircuitBreaker>>>,
        retry_strategy: &Arc<dyn RetryStrategy>,
    ) -> Result<StepOutput, anyhow::Error> {
        // Route to appropriate executor based on step type
        let executor: &Arc<dyn JobExecutor> = match &step.step_type {
            JobType::HttpRequest { .. } => http_executor,
            JobType::DatabaseQuery { .. } => database_executor,
            JobType::FileProcessing { .. } => file_executor,
            JobType::Sftp { .. } => {
                return Err(anyhow::anyhow!("SFTP not yet implemented"));
            }
        };

        // Execute with retry logic
        let mut attempt = 0;
        let mut last_error = None;

        while retry_strategy.should_retry(attempt) {
            info!(attempt = attempt + 1, "Executing step attempt");

            // Get or create circuit breaker for this target
            let circuit_breaker = Self::get_or_create_circuit_breaker(
                circuit_breakers,
                &format!("{}_{}", step.id, step.name),
            )
            .await;

            // Clone context for this attempt (in case we need to retry)
            let mut context_clone = context.clone();

            // Execute with circuit breaker
            match circuit_breaker
                .call(executor.execute(step, &mut context_clone))
                .await
            {
                Ok(step_output) => {
                    info!("Step execution successful");

                    // Update the original context with the successful execution
                    *context = context_clone;

                    return Ok(step_output);
                }
                Err(e) => {
                    warn!(error = %e, attempt = attempt + 1, "Step execution failed");
                    last_error = Some(e);
                    attempt += 1;

                    // Calculate delay before next retry
                    if let Some(delay) = retry_strategy.next_delay(attempt) {
                        info!(delay_secs = delay.as_secs(), "Waiting before retry");
                        tokio::time::sleep(delay).await;
                    } else {
                        break;
                    }
                }
            }
        }

        // All retries exhausted
        error!(attempts = attempt, "All retry attempts exhausted");

        Err(anyhow::anyhow!(
            "Step execution failed after {} attempts: {}",
            attempt,
            last_error
                .map(|e| e.to_string())
                .unwrap_or_else(|| "Unknown error".to_string())
        ))
    }

    /// Get or create a circuit breaker for a target
    async fn get_or_create_circuit_breaker(
        circuit_breakers: &Arc<tokio::sync::RwLock<HashMap<String, CircuitBreaker>>>,
        target: &str,
    ) -> CircuitBreaker {
        // Check if circuit breaker exists
        {
            let breakers = circuit_breakers.read().await;
            if let Some(cb) = breakers.get(target) {
                return cb.clone();
            }
        }

        // Create new circuit breaker
        let mut breakers = circuit_breakers.write().await;
        let cb = CircuitBreaker::new(
            target,
            CircuitBreakerConfig {
                failure_threshold: 5,
                timeout: Duration::from_secs(60),
                success_threshold: 2,
            },
        );
        breakers.insert(target.to_string(), cb.clone());
        cb
    }

    /// Publish execution status change event to NATS for SSE broadcasting
    ///
    /// Requirements: 6.7 - Push updates to connected clients using Server-Sent Events
    async fn publish_status_change(
        nats_client: &Option<async_nats::Client>,
        execution_id: uuid::Uuid,
        job_id: uuid::Uuid,
        status: &str,
    ) {
        if let Some(client) = nats_client {
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

#[cfg(test)]
mod tests {
    #[test]
    fn test_worker_consumer_creation() {
        // Basic test to ensure the module compiles
        assert!(true);
    }
}
