// Job consumer implementation for NATS JetStream

use crate::errors::QueueError;
use crate::queue::nats::NatsClient;
use crate::queue::publisher::JobMessage;
use async_nats::jetstream::consumer::PullConsumer;
use async_nats::jetstream::Message;
use futures::StreamExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;
use tracing::{error, info, instrument, warn};

/// Job consumer trait for consuming jobs from the queue
#[async_trait::async_trait]
pub trait JobConsumer: Send + Sync {
    /// Start consuming jobs from the queue
    /// Returns when shutdown is requested
    async fn start(&self) -> Result<(), QueueError>;

    /// Request graceful shutdown
    fn shutdown(&self);
}

/// Callback function type for processing job messages
pub type JobHandler = Arc<
    dyn Fn(JobMessage) -> futures::future::BoxFuture<'static, Result<(), anyhow::Error>>
        + Send
        + Sync,
>;

/// NATS-based job consumer implementation
pub struct NatsJobConsumer {
    client: NatsClient,
    consumer: PullConsumer,
    handler: JobHandler,
    batch_size: usize,
    shutdown_flag: Arc<AtomicBool>,
    shutdown_notify: Arc<Notify>,
}

impl NatsJobConsumer {
    /// Create a new NATS job consumer
    #[instrument(skip(client, handler))]
    pub async fn new(client: NatsClient, handler: JobHandler) -> Result<Self, QueueError> {
        info!("Creating NATS job consumer");

        // Get or create consumer
        let consumer = client.get_or_create_consumer().await?;

        Ok(Self {
            client,
            consumer,
            handler,
            batch_size: 10,
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            shutdown_notify: Arc::new(Notify::new()),
        })
    }

    /// Set the batch size for fetching messages
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size;
        self
    }

    /// Process a single message with exactly-once semantics
    #[instrument(skip(self, message), fields(
        message_id = ?message.info().map(|i| i.stream_sequence),
    ))]
    async fn process_message(&self, message: Message) -> Result<(), QueueError> {
        // Extract message info
        let info = message
            .info()
            .map_err(|e| QueueError::ConsumeFailed(format!("Failed to get message info: {}", e)))?;

        info!(stream_sequence = info.stream_sequence, "Processing message");

        // Deserialize job message
        let job_message: JobMessage = serde_json::from_slice(&message.payload).map_err(|e| {
            QueueError::DeserializationFailed(format!("Failed to deserialize job message: {}", e))
        })?;

        info!(
            execution_id = %job_message.execution_id,
            job_id = %job_message.job_id,
            idempotency_key = %job_message.idempotency_key,
            "Deserialized job message"
        );

        // Call the handler
        match (self.handler)(job_message.clone()).await {
            Ok(()) => {
                info!(
                    execution_id = %job_message.execution_id,
                    "Job processed successfully"
                );

                // Acknowledge the message (exactly-once delivery)
                message.ack().await.map_err(|e| {
                    QueueError::AckFailed(format!("Failed to acknowledge message: {}", e))
                })?;

                info!(
                    stream_sequence = info.stream_sequence,
                    "Message acknowledged"
                );

                Ok(())
            }
            Err(e) => {
                error!(
                    execution_id = %job_message.execution_id,
                    error = %e,
                    "Job processing failed"
                );

                // Negative acknowledge - message will be redelivered
                message
                    .ack_with(async_nats::jetstream::AckKind::Nak(None))
                    .await
                    .map_err(|e| {
                        QueueError::AckFailed(format!(
                            "Failed to negative acknowledge message: {}",
                            e
                        ))
                    })?;

                warn!(
                    stream_sequence = info.stream_sequence,
                    "Message negatively acknowledged for redelivery"
                );

                Err(QueueError::ConsumeFailed(format!(
                    "Job processing failed: {}",
                    e
                )))
            }
        }
    }
}

#[async_trait::async_trait]
impl JobConsumer for NatsJobConsumer {
    #[instrument(skip(self))]
    async fn start(&self) -> Result<(), QueueError> {
        info!("Starting job consumer");

        // Create a stream of messages
        let mut messages = self.consumer.messages().await.map_err(|e| {
            QueueError::ConsumeFailed(format!("Failed to create message stream: {}", e))
        })?;

        info!("Consumer started, waiting for messages");

        // Process messages until shutdown is requested
        loop {
            // Check shutdown flag
            if self.shutdown_flag.load(Ordering::Relaxed) {
                info!("Shutdown requested, stopping consumer");
                break;
            }

            // Fetch next batch of messages with timeout
            let timeout_duration = Duration::from_secs(5);

            tokio::select! {
                // Wait for next message
                message_result = messages.next() => {
                    match message_result {
                        Some(Ok(message)) => {
                            // Process message
                            if let Err(e) = self.process_message(message).await {
                                error!(error = %e, "Failed to process message");
                                // Continue processing other messages
                            }
                        }
                        Some(Err(e)) => {
                            error!(error = %e, "Error receiving message");
                            // Wait a bit before retrying
                            tokio::time::sleep(Duration::from_secs(1)).await;
                        }
                        None => {
                            warn!("Message stream ended unexpectedly");
                            break;
                        }
                    }
                }
                // Wait for shutdown notification
                _ = self.shutdown_notify.notified() => {
                    info!("Shutdown notification received");
                    break;
                }
                // Timeout to check shutdown flag periodically
                _ = tokio::time::sleep(timeout_duration) => {
                    // Just loop back to check shutdown flag
                    continue;
                }
            }
        }

        info!("Consumer stopped gracefully");
        Ok(())
    }

    fn shutdown(&self) {
        info!("Requesting consumer shutdown");
        self.shutdown_flag.store(true, Ordering::Relaxed);
        self.shutdown_notify.notify_waiters();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn test_job_message_deserialization() {
        let message = JobMessage {
            execution_id: Uuid::new_v4(),
            job_id: Uuid::new_v4(),
            idempotency_key: "test-key".to_string(),
            attempt: 1,
            published_at: Utc::now(),
        };

        let json = serde_json::to_vec(&message).unwrap();
        let deserialized: JobMessage = serde_json::from_slice(&json).unwrap();

        assert_eq!(message.execution_id, deserialized.execution_id);
        assert_eq!(message.job_id, deserialized.job_id);
        assert_eq!(message.idempotency_key, deserialized.idempotency_key);
    }

    #[tokio::test]
    async fn test_shutdown_flag() {
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        assert!(!shutdown_flag.load(Ordering::Relaxed));

        shutdown_flag.store(true, Ordering::Relaxed);
        assert!(shutdown_flag.load(Ordering::Relaxed));
    }
}
