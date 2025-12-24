// Step executor - handles individual step execution with retry and circuit breaker
// Requirements: 13.4 - Execute steps sequentially with retry logic

use crate::db::repositories::execution::ExecutionRepository;

use crate::executor::JobExecutor;
use crate::models::{ExecutionStatus, Job, JobContext, JobExecution, JobStep, JobType, StepOutput};
use crate::retry::RetryStrategy;
use crate::storage::StorageService;
use crate::worker::reference::ReferenceResolver;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use tracing::{error, info, instrument, warn};

use super::CircuitBreakerManager;

/// Step executor handles execution of individual job steps
pub struct StepExecutor {
    http_executor: Arc<dyn JobExecutor>,
    database_executor: Arc<dyn JobExecutor>,
    file_executor: Arc<dyn JobExecutor>,
    storage_service: Arc<dyn StorageService>,
    _reference_resolver: Arc<ReferenceResolver>,
    circuit_breaker_manager: Arc<CircuitBreakerManager>,
    retry_strategy: Arc<dyn RetryStrategy>,
    execution_repo: Arc<ExecutionRepository>,
}

impl StepExecutor {
    /// Create a new step executor
    pub fn new(
        http_executor: Arc<dyn JobExecutor>,
        database_executor: Arc<dyn JobExecutor>,
        file_executor: Arc<dyn JobExecutor>,
        storage_service: Arc<dyn StorageService>,
        reference_resolver: Arc<ReferenceResolver>,
        circuit_breaker_manager: Arc<CircuitBreakerManager>,
        retry_strategy: Arc<dyn RetryStrategy>,
        execution_repo: Arc<ExecutionRepository>,
    ) -> Self {
        Self {
            http_executor,
            database_executor,
            file_executor,
            storage_service,
            _reference_resolver: reference_resolver,
            circuit_breaker_manager,
            retry_strategy,
            execution_repo,
        }
    }

    /// Execute all job steps sequentially
    #[instrument(skip(self, job, context, execution), fields(job_id = %job.id, job_name = %job.name))]
    pub async fn execute_all_steps(
        &self,
        job: &Job,
        context: &mut JobContext,
        execution: &mut JobExecution,
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

            // Check for cancellation
            if self.check_cancellation(execution).await? {
                return Err(anyhow::anyhow!("Execution cancelled"));
            }

            // Update current step
            execution.current_step = Some(step.id.clone());
            if let Err(e) = self.execution_repo.update(execution).await {
                warn!(error = %e, "Failed to update current step");
            }

            // Check step condition
            if let Some(condition) = &step.condition {
                info!(condition = %condition, "Step has condition, evaluating");
                // TODO: Implement proper condition evaluation
            }

            // Execute step with timeout
            let timeout_duration = Duration::from_secs(job.timeout_seconds as u64);
            let step_result =
                timeout(timeout_duration, self.execute_single_step(step, context)).await;

            match step_result {
                Ok(Ok(step_output)) => {
                    info!(step_id = %step.id, "Step completed successfully");
                    context.set_step_output(step.id.clone(), step_output);

                    // Persist context after each step
                    if let Err(e) = self.storage_service.store_context(context).await {
                        error!(error = %e, "Failed to save context to storage after step");
                        return Err(anyhow::anyhow!("Failed to save context: {}", e));
                    }

                    info!(
                        step_id = %step.id,
                        completed_steps = context.completed_steps_count(),
                        "Context saved to storage after step completion"
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

    /// Check if execution has been cancelled
    async fn check_cancellation(&self, execution: &JobExecution) -> Result<bool, anyhow::Error> {
        match self.execution_repo.find_by_id(execution.id).await {
            Ok(Some(current_execution)) => match current_execution.status {
                ExecutionStatus::Cancelling => {
                    info!("Graceful cancellation requested, stopping after current step");
                    Ok(true)
                }
                ExecutionStatus::Cancelled => {
                    info!("Force cancellation detected, stopping immediately");
                    Ok(true)
                }
                _ => Ok(false),
            },
            Ok(None) => {
                warn!("Execution not found in database, continuing");
                Ok(false)
            }
            Err(e) => {
                warn!(error = %e, "Failed to check cancellation status, continuing");
                Ok(false)
            }
        }
    }

    /// Execute a single job step with retry logic
    #[instrument(skip(self, step, context), fields(step_id = %step.id, step_name = %step.name))]
    async fn execute_single_step(
        &self,
        step: &JobStep,
        context: &mut JobContext,
    ) -> Result<StepOutput, anyhow::Error> {
        // Route to appropriate executor
        let executor: &Arc<dyn JobExecutor> = match &step.step_type {
            JobType::HttpRequest { .. } => &self.http_executor,
            JobType::DatabaseQuery { .. } => &self.database_executor,
            JobType::FileProcessing { .. } => &self.file_executor,
            JobType::Sftp { .. } => {
                return Err(anyhow::anyhow!("SFTP not yet implemented"));
            }
        };

        // Execute with retry logic
        let mut attempt = 0;
        let mut last_error = None;

        while self.retry_strategy.should_retry(attempt) {
            info!(attempt = attempt + 1, "Executing step attempt");

            // Get circuit breaker
            let circuit_breaker = self
                .circuit_breaker_manager
                .get_or_create(&format!("{}_{}", step.id, step.name))
                .await;

            // Clone context for this attempt
            let mut context_clone = context.clone();

            // Execute with circuit breaker
            match circuit_breaker
                .call(executor.execute(step, &mut context_clone))
                .await
            {
                Ok(step_output) => {
                    info!("Step execution successful");
                    *context = context_clone;
                    return Ok(step_output);
                }
                Err(e) => {
                    warn!(error = %e, attempt = attempt + 1, "Step execution failed");
                    last_error = Some(e);
                    attempt += 1;

                    // Calculate delay before next retry
                    if let Some(delay) = self.retry_strategy.next_delay(attempt) {
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
}
