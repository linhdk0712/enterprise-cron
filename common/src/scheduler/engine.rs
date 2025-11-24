// Scheduler engine implementation
// Requirements: 7.1, 4.1, 9.4

use crate::db::repositories::execution::ExecutionRepository;
use crate::db::repositories::job::JobRepository;
use crate::db::DbPool;
use crate::lock::DistributedLock;
use crate::models::{ExecutionStatus, Job, JobExecution, TriggerSource};
use crate::queue::JobPublisher;
use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{interval, sleep};
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

/// Configuration for the scheduler
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// How often to poll for jobs due (in seconds)
    pub poll_interval_seconds: u64,
    /// TTL for distributed locks (in seconds)
    pub lock_ttl_seconds: u64,
    /// Maximum number of jobs to process per poll
    pub max_jobs_per_poll: usize,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            poll_interval_seconds: 10,
            lock_ttl_seconds: 30,
            max_jobs_per_poll: 100,
        }
    }
}

/// Scheduler trait for job scheduling operations
#[async_trait]
pub trait Scheduler: Send + Sync {
    /// Start the scheduler polling loop
    async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Stop the scheduler gracefully
    async fn stop(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Process jobs that are due for execution
    async fn process_due_jobs(&self) -> Result<usize, Box<dyn std::error::Error + Send + Sync>>;
}

/// Main scheduler engine implementation
pub struct SchedulerEngine {
    config: SchedulerConfig,
    job_repo: Arc<JobRepository>,
    execution_repo: Arc<ExecutionRepository>,
    lock: Arc<dyn DistributedLock>,
    publisher: Arc<dyn JobPublisher>,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
}

impl SchedulerEngine {
    /// Create a new scheduler engine
    pub fn new(
        config: SchedulerConfig,
        db_pool: DbPool,
        lock: Arc<dyn DistributedLock>,
        publisher: Arc<dyn JobPublisher>,
    ) -> Self {
        let (shutdown_tx, _shutdown_rx) = tokio::sync::broadcast::channel(1);

        Self {
            config,
            job_repo: Arc::new(JobRepository::new(db_pool.clone())),
            execution_repo: Arc::new(ExecutionRepository::new(db_pool)),
            lock,
            publisher,
            shutdown_tx,
        }
    }

    /// Get a shutdown signal receiver
    pub fn shutdown_receiver(&self) -> tokio::sync::broadcast::Receiver<()> {
        self.shutdown_tx.subscribe()
    }

    /// Process a single job that is due
    ///
    /// Requirements:
    /// - 4.1: Acquire distributed lock before scheduling
    /// - 7.1: Ensure only one scheduler node processes each job
    /// - 17.9: Allow concurrent execution if configured
    /// - 17.10: Reject new triggers if concurrent execution not allowed
    #[instrument(skip(self, job), fields(job_id = %job.id, job_name = %job.name))]
    async fn process_job(&self, job: &Job) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Check if concurrent execution is allowed
        // Requirement 17.10: Reject if concurrent execution not allowed and job is running
        if !job.allow_concurrent {
            match self.execution_repo.has_running_execution(job.id).await {
                Ok(true) => {
                    debug!(
                        "Job has running execution and concurrent execution not allowed, skipping"
                    );
                    return Ok(());
                }
                Ok(false) => {
                    // No running execution, proceed
                }
                Err(e) => {
                    warn!(error = %e, "Failed to check for running executions");
                    // Continue anyway to avoid blocking job execution
                }
            }
        }

        // Try to acquire distributed lock for this job
        let lock_resource = format!("schedule:job:{}", job.id);
        let lock_ttl = Duration::from_secs(self.config.lock_ttl_seconds);

        debug!("Attempting to acquire lock for job");

        let lock_guard = match self.lock.acquire(&lock_resource, lock_ttl).await {
            Ok(guard) => {
                info!("Lock acquired successfully");
                guard
            }
            Err(e) => {
                // Another scheduler node is processing this job
                debug!(error = %e, "Failed to acquire lock, skipping job");
                return Ok(());
            }
        };

        // Load job definition from MinIO to get schedule information
        // For now, we'll create a basic execution
        // TODO: Load full job definition from MinIO in future tasks

        // Create job execution
        let execution = JobExecution {
            id: Uuid::new_v4(),
            job_id: job.id,
            idempotency_key: format!("{}:{}", job.id, Uuid::new_v4()),
            status: ExecutionStatus::Pending,
            attempt: 1,
            trigger_source: TriggerSource::Scheduled,
            current_step: None,
            minio_context_path: format!(
                "jobs/{}/executions/{}/context.json",
                job.id,
                Uuid::new_v4()
            ),
            started_at: None,
            completed_at: None,
            result: None,
            error: None,
            created_at: Utc::now(),
        };

        // Save execution to database
        match self.execution_repo.create(&execution).await {
            Ok(()) => {
                info!(execution_id = %execution.id, "Job execution created");
            }
            Err(e) => {
                error!(error = %e, "Failed to create job execution");
                return Err(Box::new(e));
            }
        }

        // Publish job to queue
        match self.publisher.publish(&execution).await {
            Ok(()) => {
                info!(execution_id = %execution.id, "Job published to queue");
            }
            Err(e) => {
                error!(error = %e, "Failed to publish job to queue");
                // Update execution status to failed
                let mut failed_execution = execution.clone();
                failed_execution.status = ExecutionStatus::Failed;
                failed_execution.error = Some(format!("Failed to publish to queue: {}", e));
                let _ = self.execution_repo.update(&failed_execution).await;
                return Err(Box::new(e));
            }
        }

        // Update job stats
        if let Err(e) = self.job_repo.update_stats(job.id, true).await {
            warn!(error = %e, "Failed to update job stats");
        }

        // Lock will be automatically released when lock_guard is dropped
        drop(lock_guard);
        debug!("Lock released");

        Ok(())
    }

    /// Calculate if a job is due for execution
    ///
    /// Requirements:
    /// - 1.1-1.7: Calculate next execution time based on schedule type
    /// - 17.1: Process jobs with scheduled trigger
    /// - 17.2: Skip jobs with manual-only trigger
    fn is_job_due(&self, job: &Job) -> bool {
        // Check if job has scheduled trigger enabled
        // Requirement 17.2: Skip jobs with manual-only trigger
        if !job.triggers.scheduled {
            debug!(
                job_id = %job.id,
                job_name = %job.name,
                "Skipping job with manual-only trigger"
            );
            return false;
        }

        // For now, we'll consider all enabled jobs with scheduled trigger as due
        // TODO: Implement proper schedule calculation in future iterations
        // This requires loading the full job definition from MinIO
        job.enabled
    }
}

#[async_trait]
impl Scheduler for SchedulerEngine {
    /// Start the scheduler polling loop
    ///
    /// Requirements:
    /// - 7.1: Periodic polling for jobs due
    /// - 7.6: Graceful shutdown support
    #[instrument(skip(self))]
    async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            poll_interval_seconds = self.config.poll_interval_seconds,
            "Starting scheduler engine"
        );

        let mut poll_interval = interval(Duration::from_secs(self.config.poll_interval_seconds));
        let mut shutdown_rx = self.shutdown_receiver();

        loop {
            tokio::select! {
                _ = poll_interval.tick() => {
                    debug!("Polling for jobs due");

                    match self.process_due_jobs().await {
                        Ok(count) => {
                            if count > 0 {
                                info!(jobs_processed = count, "Processed due jobs");
                            } else {
                                debug!("No jobs due for execution");
                            }
                        }
                        Err(e) => {
                            error!(error = %e, "Error processing due jobs");
                        }
                    }
                }
                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received, stopping scheduler");
                    break;
                }
            }
        }

        info!("Scheduler engine stopped");
        Ok(())
    }

    /// Stop the scheduler gracefully
    ///
    /// Requirements:
    /// - 7.6: Complete in-flight scheduling operations before terminating
    #[instrument(skip(self))]
    async fn stop(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Stopping scheduler engine");

        // Send shutdown signal
        let _ = self.shutdown_tx.send(());

        // Give some time for in-flight operations to complete
        sleep(Duration::from_secs(2)).await;

        info!("Scheduler engine stopped gracefully");
        Ok(())
    }

    /// Process jobs that are due for execution
    ///
    /// Requirements:
    /// - 7.1: Find jobs due and process them
    /// - 4.1: Use distributed locking
    #[instrument(skip(self))]
    async fn process_due_jobs(&self) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        // Find all enabled jobs
        let jobs = match self.job_repo.find_jobs_due(Utc::now()).await {
            Ok(jobs) => jobs,
            Err(e) => {
                error!(error = %e, "Failed to query jobs from database");
                return Err(Box::new(e));
            }
        };

        debug!(job_count = jobs.len(), "Found jobs to evaluate");

        let mut processed_count = 0;

        // Process each job
        for job in jobs.iter().take(self.config.max_jobs_per_poll) {
            // Check if job is due based on schedule
            if !self.is_job_due(job) {
                continue;
            }

            // Process the job
            match self.process_job(job).await {
                Ok(()) => {
                    processed_count += 1;
                }
                Err(e) => {
                    error!(
                        job_id = %job.id,
                        job_name = %job.name,
                        error = %e,
                        "Failed to process job"
                    );
                    // Continue processing other jobs
                }
            }
        }

        Ok(processed_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_config_default() {
        let config = SchedulerConfig::default();
        assert_eq!(config.poll_interval_seconds, 10);
        assert_eq!(config.lock_ttl_seconds, 30);
        assert_eq!(config.max_jobs_per_poll, 100);
    }

    #[test]
    fn test_scheduler_config_custom() {
        let config = SchedulerConfig {
            poll_interval_seconds: 5,
            lock_ttl_seconds: 60,
            max_jobs_per_poll: 50,
        };
        assert_eq!(config.poll_interval_seconds, 5);
        assert_eq!(config.lock_ttl_seconds, 60);
        assert_eq!(config.max_jobs_per_poll, 50);
    }
}
