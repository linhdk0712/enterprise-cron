// Dead Letter Queue handling for failed jobs
// Requirements: 4.8, 4.10
// Property 36: Dead letter queue placement
// Property 38: Dead letter queue isolation

use crate::errors::ExecutionError;
use crate::models::{ExecutionStatus, JobExecution};
use crate::retry::MAX_RETRIES;
use tracing::{info, warn};
use uuid::Uuid;

/// Dead Letter Queue handler for managing failed jobs
#[derive(Debug, Clone)]
pub struct DeadLetterQueue {
    /// Name for logging purposes
    name: String,
}

impl DeadLetterQueue {
    /// Create a new Dead Letter Queue handler
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    /// Create a default Dead Letter Queue handler
    pub fn default() -> Self {
        Self::new("default")
    }

    /// Check if a job execution should be moved to the Dead Letter Queue
    /// Returns true if the execution has exhausted all retry attempts
    pub fn should_move_to_dlq(&self, execution: &JobExecution) -> bool {
        // Move to DLQ if:
        // 1. Status is Failed or Timeout
        // 2. Attempt count has reached or exceeded MAX_RETRIES
        matches!(
            execution.status,
            ExecutionStatus::Failed | ExecutionStatus::Timeout
        ) && execution.attempt >= MAX_RETRIES as i32
    }

    /// Move a job execution to the Dead Letter Queue
    /// This marks the execution with DeadLetter status and logs the event
    pub async fn move_to_dlq(
        &self,
        execution: &mut JobExecution,
        reason: impl Into<String>,
    ) -> Result<(), ExecutionError> {
        let reason = reason.into();

        // Verify the execution should be moved to DLQ
        if !self.should_move_to_dlq(execution) {
            return Err(ExecutionError::InvalidJobDefinition(format!(
                "Execution {} does not meet criteria for DLQ (status: {:?}, attempt: {})",
                execution.id, execution.status, execution.attempt
            )));
        }

        warn!(
            dlq = %self.name,
            execution_id = %execution.id,
            job_id = %execution.job_id,
            attempt = execution.attempt,
            reason = %reason,
            "Moving job execution to Dead Letter Queue"
        );

        // Update execution status to DeadLetter
        execution.status = ExecutionStatus::DeadLetter;

        // Append DLQ information to error message
        let dlq_info = format!(
            " [Moved to DLQ after {} attempts: {}]",
            execution.attempt, reason
        );
        execution.error = Some(match &execution.error {
            Some(existing_error) => format!("{}{}", existing_error, dlq_info),
            None => dlq_info,
        });

        Ok(())
    }

    /// Check if a job execution is in the Dead Letter Queue
    pub fn is_in_dlq(&self, execution: &JobExecution) -> bool {
        execution.status == ExecutionStatus::DeadLetter
    }

    /// Prevent automatic re-execution of DLQ jobs
    /// Returns an error if attempting to re-execute a DLQ job
    pub fn check_dlq_isolation(&self, execution: &JobExecution) -> Result<(), ExecutionError> {
        if self.is_in_dlq(execution) {
            return Err(ExecutionError::CircuitBreakerOpen(format!(
                "Execution {} is in Dead Letter Queue and cannot be automatically re-executed. Manual intervention required.",
                execution.id
            )));
        }
        Ok(())
    }

    /// Manually retry a job from the Dead Letter Queue
    /// This creates a new execution with attempt count reset
    pub async fn manual_retry(
        &self,
        execution: &JobExecution,
    ) -> Result<JobExecution, ExecutionError> {
        if !self.is_in_dlq(execution) {
            return Err(ExecutionError::InvalidJobDefinition(format!(
                "Execution {} is not in Dead Letter Queue",
                execution.id
            )));
        }

        info!(
            dlq = %self.name,
            execution_id = %execution.id,
            job_id = %execution.job_id,
            "Manually retrying job from Dead Letter Queue"
        );

        // Create a new execution with reset attempt count
        let mut new_execution = execution.clone();
        new_execution.id = Uuid::new_v4();
        new_execution.status = ExecutionStatus::Pending;
        new_execution.attempt = 0;
        new_execution.started_at = None;
        new_execution.completed_at = None;
        new_execution.result = None;
        new_execution.error = Some(format!(
            "Manual retry from DLQ (original execution: {})",
            execution.id
        ));

        Ok(new_execution)
    }

    /// Get statistics about the Dead Letter Queue
    pub fn get_stats(&self, executions: &[JobExecution]) -> DlqStats {
        let dlq_count = executions.iter().filter(|e| self.is_in_dlq(e)).count();

        let failed_count = executions
            .iter()
            .filter(|e| matches!(e.status, ExecutionStatus::Failed))
            .count();

        let timeout_count = executions
            .iter()
            .filter(|e| matches!(e.status, ExecutionStatus::Timeout))
            .count();

        DlqStats {
            total_dlq: dlq_count,
            total_failed: failed_count,
            total_timeout: timeout_count,
        }
    }
}

/// Statistics about the Dead Letter Queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlqStats {
    pub total_dlq: usize,
    pub total_failed: usize,
    pub total_timeout: usize,
}

use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn create_test_execution(status: ExecutionStatus, attempt: i32) -> JobExecution {
        JobExecution {
            id: Uuid::new_v4(),
            job_id: Uuid::new_v4(),
            idempotency_key: "test-key".to_string(),
            status,
            attempt,
            trigger_source: crate::models::TriggerSource::Scheduled,
            current_step: None,
            minio_context_path: "test/path".to_string(),
            started_at: Some(Utc::now()),
            completed_at: None,
            result: None,
            error: Some("Test error".to_string()),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn test_should_move_to_dlq_after_max_retries() {
        let dlq = DeadLetterQueue::default();

        // Failed execution with MAX_RETRIES attempts
        let execution = create_test_execution(ExecutionStatus::Failed, MAX_RETRIES as i32);
        assert!(dlq.should_move_to_dlq(&execution));

        // Timeout execution with MAX_RETRIES attempts
        let execution = create_test_execution(ExecutionStatus::Timeout, MAX_RETRIES as i32);
        assert!(dlq.should_move_to_dlq(&execution));
    }

    #[test]
    fn test_should_not_move_to_dlq_before_max_retries() {
        let dlq = DeadLetterQueue::default();

        // Failed execution with fewer than MAX_RETRIES attempts
        let execution = create_test_execution(ExecutionStatus::Failed, MAX_RETRIES as i32 - 1);
        assert!(!dlq.should_move_to_dlq(&execution));
    }

    #[test]
    fn test_should_not_move_successful_to_dlq() {
        let dlq = DeadLetterQueue::default();

        // Successful execution should never go to DLQ
        let execution = create_test_execution(ExecutionStatus::Success, MAX_RETRIES as i32);
        assert!(!dlq.should_move_to_dlq(&execution));

        // Running execution should never go to DLQ
        let execution = create_test_execution(ExecutionStatus::Running, MAX_RETRIES as i32);
        assert!(!dlq.should_move_to_dlq(&execution));

        // Pending execution should never go to DLQ
        let execution = create_test_execution(ExecutionStatus::Pending, MAX_RETRIES as i32);
        assert!(!dlq.should_move_to_dlq(&execution));
    }

    #[tokio::test]
    async fn test_move_to_dlq() {
        let dlq = DeadLetterQueue::default();
        let mut execution = create_test_execution(ExecutionStatus::Failed, MAX_RETRIES as i32);

        let result = dlq
            .move_to_dlq(&mut execution, "Max retries exceeded")
            .await;
        assert!(result.is_ok());
        assert_eq!(execution.status, ExecutionStatus::DeadLetter);
        assert!(execution.error.as_ref().unwrap().contains("Moved to DLQ"));
    }

    #[tokio::test]
    async fn test_move_to_dlq_invalid_execution() {
        let dlq = DeadLetterQueue::default();
        let mut execution = create_test_execution(ExecutionStatus::Failed, 5); // Less than MAX_RETRIES

        let result = dlq.move_to_dlq(&mut execution, "Test reason").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_is_in_dlq() {
        let dlq = DeadLetterQueue::default();

        let execution = create_test_execution(ExecutionStatus::DeadLetter, MAX_RETRIES as i32);
        assert!(dlq.is_in_dlq(&execution));

        let execution = create_test_execution(ExecutionStatus::Failed, MAX_RETRIES as i32);
        assert!(!dlq.is_in_dlq(&execution));
    }

    #[test]
    fn test_check_dlq_isolation() {
        let dlq = DeadLetterQueue::default();

        // DLQ execution should be isolated
        let execution = create_test_execution(ExecutionStatus::DeadLetter, MAX_RETRIES as i32);
        let result = dlq.check_dlq_isolation(&execution);
        assert!(result.is_err());

        // Non-DLQ execution should pass
        let execution = create_test_execution(ExecutionStatus::Failed, 5);
        let result = dlq.check_dlq_isolation(&execution);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_manual_retry() {
        let dlq = DeadLetterQueue::default();
        let execution = create_test_execution(ExecutionStatus::DeadLetter, MAX_RETRIES as i32);

        let result = dlq.manual_retry(&execution).await;
        assert!(result.is_ok());

        let new_execution = result.unwrap();
        assert_eq!(new_execution.status, ExecutionStatus::Pending);
        assert_eq!(new_execution.attempt, 0);
        assert_ne!(new_execution.id, execution.id);
        assert!(new_execution
            .error
            .as_ref()
            .unwrap()
            .contains("Manual retry"));
    }

    #[tokio::test]
    async fn test_manual_retry_non_dlq_execution() {
        let dlq = DeadLetterQueue::default();
        let execution = create_test_execution(ExecutionStatus::Failed, 5);

        let result = dlq.manual_retry(&execution).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_get_stats() {
        let dlq = DeadLetterQueue::default();

        let executions = vec![
            create_test_execution(ExecutionStatus::DeadLetter, MAX_RETRIES as i32),
            create_test_execution(ExecutionStatus::DeadLetter, MAX_RETRIES as i32),
            create_test_execution(ExecutionStatus::Failed, 5),
            create_test_execution(ExecutionStatus::Timeout, 3),
            create_test_execution(ExecutionStatus::Success, 1),
        ];

        let stats = dlq.get_stats(&executions);
        assert_eq!(stats.total_dlq, 2);
        assert_eq!(stats.total_failed, 1);
        assert_eq!(stats.total_timeout, 1);
    }
}
