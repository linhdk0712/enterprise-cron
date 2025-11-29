// Job publisher implementation for NATS JetStream

use crate::errors::QueueError;
use crate::models::JobExecution;
use crate::queue::nats::NatsClient;
use async_nats::jetstream::context::PublishAckFuture;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, instrument, warn};
use uuid::Uuid;

/// Message format for job execution in the queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobMessage {
    /// Unique execution ID
    pub execution_id: Uuid,
    /// Job ID
    pub job_id: Uuid,
    /// Idempotency key for exactly-once processing
    pub idempotency_key: String,
    /// Current attempt number
    pub attempt: i32,
    /// Timestamp when message was published
    pub published_at: chrono::DateTime<chrono::Utc>,
}

impl From<&JobExecution> for JobMessage {
    fn from(execution: &JobExecution) -> Self {
        Self {
            execution_id: execution.id,
            job_id: execution.job_id,
            idempotency_key: execution.idempotency_key.clone(),
            attempt: execution.attempt,
            published_at: chrono::Utc::now(),
        }
    }
}

/// JobPublisher trait for publishing jobs to the queue
#[async_trait::async_trait]
pub trait JobPublisher: Send + Sync {
    /// Publish a job execution to the queue
    async fn publish(&self, execution: &JobExecution) -> Result<(), QueueError>;

    /// Publish a job execution with retry logic
    async fn publish_with_retry(
        &self,
        execution: &JobExecution,
        max_retries: u32,
    ) -> Result<(), QueueError>;
}

/// NATS-based job publisher implementation
pub struct NatsJobPublisher {
    client: NatsClient,
    subject_prefix: String,
    publish_timeout: Duration,
}

impl NatsJobPublisher {
    /// Create a new NATS job publisher
    pub fn new(client: NatsClient) -> Self {
        let subject_prefix = format!("jobs.{}", client.config().stream_name.to_lowercase());
        Self {
            client,
            subject_prefix,
            publish_timeout: Duration::from_secs(5),
        }
    }

    /// Create a new NATS job publisher with custom timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.publish_timeout = timeout;
        self
    }

    /// Get the subject for a job
    fn get_subject(&self, job_id: &Uuid) -> String {
        format!("{}.{}", self.subject_prefix, job_id)
    }
}

#[async_trait::async_trait]
impl JobPublisher for NatsJobPublisher {
    #[instrument(skip(self), fields(
        execution_id = %execution.id,
        job_id = %execution.job_id,
        idempotency_key = %execution.idempotency_key,
        attempt = execution.attempt
    ))]
    async fn publish(&self, execution: &JobExecution) -> Result<(), QueueError> {
        info!("Publishing job execution to queue");

        // Create message from execution
        let message = JobMessage::from(execution);

        // Serialize message
        let payload = serde_json::to_vec(&message).map_err(|e| {
            QueueError::SerializationFailed(format!("Failed to serialize job message: {}", e))
        })?;

        // Get subject for this job
        let subject = self.get_subject(&execution.job_id);

        // Publish to JetStream with deduplication headers
        let jetstream = self.client.jetstream();

        // Create headers for deduplication
        let mut headers = async_nats::HeaderMap::new();
        headers.insert("Nats-Msg-Id", execution.idempotency_key.as_str());
        headers.insert("Job-Id", execution.job_id.to_string().as_str());
        headers.insert("Execution-Id", execution.id.to_string().as_str());

        let publish_future: PublishAckFuture = jetstream
            .publish_with_headers(subject.clone(), headers, payload.into())
            .await
            .map_err(|e| QueueError::PublishFailed(format!("Failed to publish message: {}", e)))?;

        // Wait for acknowledgment with timeout
        let ack_result = tokio::time::timeout(self.publish_timeout, publish_future).await;

        match ack_result {
            Ok(Ok(_ack)) => {
                info!(
                    subject = %subject,
                    "Job execution published successfully"
                );
                Ok(())
            }
            Ok(Err(e)) => Err(QueueError::PublishFailed(format!(
                "Failed to get publish acknowledgment: {}",
                e
            ))),
            Err(_) => Err(QueueError::Timeout(format!(
                "Publish acknowledgment timeout after {:?}",
                self.publish_timeout
            ))),
        }
    }

    #[instrument(skip(self), fields(
        execution_id = %execution.id,
        job_id = %execution.job_id,
        max_retries = max_retries
    ))]
    async fn publish_with_retry(
        &self,
        execution: &JobExecution,
        max_retries: u32,
    ) -> Result<(), QueueError> {
        let mut attempt = 0;
        let mut last_error = None;

        while attempt <= max_retries {
            match self.publish(execution).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    attempt += 1;
                    last_error = Some(e);

                    if attempt <= max_retries {
                        let delay = Duration::from_millis(100 * 2_u64.pow(attempt - 1));
                        warn!(
                            attempt = attempt,
                            delay_ms = delay.as_millis(),
                            "Publish failed, retrying"
                        );
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            QueueError::PublishFailed("Unknown error during publish with retry".to_string())
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_job_message_from_execution() {
        let execution = JobExecution {
            id: Uuid::new_v4(),
            job_id: Uuid::new_v4(),
            idempotency_key: "test-key".to_string(),
            status: crate::models::ExecutionStatus::Pending,
            attempt: 1,
            trigger_source: crate::models::TriggerSource::Scheduled,
            trigger_metadata: None,
            current_step: None,
            context: serde_json::json!({}),
            started_at: None,
            completed_at: None,
            result: None,
            error: None,
            created_at: Utc::now(),
        };

        let message = JobMessage::from(&execution);
        assert_eq!(message.execution_id, execution.id);
        assert_eq!(message.job_id, execution.job_id);
        assert_eq!(message.idempotency_key, execution.idempotency_key);
        assert_eq!(message.attempt, execution.attempt);
    }

    #[test]
    fn test_job_message_serialization() {
        let message = JobMessage {
            execution_id: Uuid::new_v4(),
            job_id: Uuid::new_v4(),
            idempotency_key: "test-key".to_string(),
            attempt: 1,
            published_at: Utc::now(),
        };

        let json = serde_json::to_string(&message).unwrap();
        let deserialized: JobMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(message.execution_id, deserialized.execution_id);
        assert_eq!(message.job_id, deserialized.job_id);
        assert_eq!(message.idempotency_key, deserialized.idempotency_key);
    }
}
