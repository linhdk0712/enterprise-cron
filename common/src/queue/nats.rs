// NATS JetStream client implementation for job queue

use crate::errors::QueueError;
use async_nats::jetstream::{
    consumer::PullConsumer,
    stream::{Config as StreamConfig, RetentionPolicy, Stream},
    Context as JetStreamContext,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, instrument};

/// NATS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NatsConfig {
    /// NATS server URL (e.g., "nats://localhost:4222")
    pub url: String,
    /// Stream name for job queue
    pub stream_name: String,
    /// Subject prefix for job messages
    pub subject: String,
    /// Maximum age for messages in the stream (in seconds)
    pub max_age_seconds: u64,
    /// Maximum number of messages to retain
    pub max_messages: i64,
    /// Consumer name for workers
    pub consumer_name: String,
    /// Maximum number of delivery attempts
    pub max_deliver: i64,
}

impl Default for NatsConfig {
    fn default() -> Self {
        Self {
            url: "nats://localhost:4222".to_string(),
            stream_name: "JOBS".to_string(),
            subject: "jobs.>".to_string(),
            max_age_seconds: 86400, // 24 hours
            max_messages: 1_000_000,
            consumer_name: "job-workers".to_string(),
            max_deliver: 10,
        }
    }
}

/// NATS JetStream client
pub struct NatsClient {
    client: async_nats::Client,
    jetstream: JetStreamContext,
    config: NatsConfig,
}

impl NatsClient {
    /// Create a NatsClient from an existing async_nats::Client
    /// Used when the client is already initialized in the application state
    pub fn from_client(client: async_nats::Client, config: NatsConfig) -> Self {
        let jetstream = async_nats::jetstream::new(client.clone());
        Self {
            client,
            jetstream,
            config,
        }
    }

    /// Create a new NATS client and connect to the server
    #[instrument(skip(config), fields(url = %config.url))]
    pub async fn new(config: NatsConfig) -> Result<Self, QueueError> {
        info!("Connecting to NATS server");

        // Connect to NATS server
        let client = async_nats::connect(&config.url)
            .await
            .map_err(|e| QueueError::Connection(format!("Failed to connect to NATS: {}", e)))?;

        info!("Connected to NATS server successfully");

        // Get JetStream context
        let jetstream = async_nats::jetstream::new(client.clone());

        Ok(Self {
            client,
            jetstream,
            config,
        })
    }

    /// Initialize the job stream with retention policy
    #[instrument(skip(self))]
    pub async fn initialize_stream(&self) -> Result<Stream, QueueError> {
        info!(
            stream_name = %self.config.stream_name,
            "Initializing JetStream stream"
        );

        // Create or get existing stream
        let stream_config = StreamConfig {
            name: self.config.stream_name.clone(),
            subjects: vec![self.config.subject.clone()],
            retention: RetentionPolicy::WorkQueue, // Messages deleted after acknowledgment
            max_age: Duration::from_secs(self.config.max_age_seconds),
            max_messages: self.config.max_messages,
            ..Default::default()
        };

        let stream = self
            .jetstream
            .get_or_create_stream(stream_config)
            .await
            .map_err(|e| QueueError::StreamCreation(format!("Failed to create stream: {}", e)))?;

        info!(
            stream_name = %self.config.stream_name,
            "Stream initialized successfully"
        );

        Ok(stream)
    }

    /// Create or get consumer for job processing
    #[instrument(skip(self))]
    pub async fn get_or_create_consumer(&self) -> Result<PullConsumer, QueueError> {
        info!(
            consumer_name = %self.config.consumer_name,
            "Creating consumer"
        );

        // Get the stream first
        let stream = self
            .jetstream
            .get_stream(&self.config.stream_name)
            .await
            .map_err(|e| QueueError::StreamNotFound(format!("Stream not found: {}", e)))?;

        // Create consumer configuration
        let consumer_config = async_nats::jetstream::consumer::pull::Config {
            durable_name: Some(self.config.consumer_name.clone()),
            ack_policy: async_nats::jetstream::consumer::AckPolicy::Explicit,
            max_deliver: self.config.max_deliver,
            ack_wait: Duration::from_secs(300), // 5 minutes timeout
            ..Default::default()
        };

        // Create or get existing consumer
        let consumer = stream
            .get_or_create_consumer(&self.config.consumer_name, consumer_config)
            .await
            .map_err(|e| {
                QueueError::ConsumerCreation(format!("Failed to create consumer: {}", e))
            })?;

        info!(
            consumer_name = %self.config.consumer_name,
            "Consumer created successfully"
        );

        Ok(consumer)
    }

    /// Get the JetStream context for publishing/consuming
    pub fn jetstream(&self) -> &JetStreamContext {
        &self.jetstream
    }

    /// Get the NATS client
    pub fn client(&self) -> &async_nats::Client {
        &self.client
    }

    /// Get the configuration
    pub fn config(&self) -> &NatsConfig {
        &self.config
    }

    /// Health check - verify connection is alive
    #[instrument(skip(self))]
    pub async fn health_check(&self) -> Result<(), QueueError> {
        // Try to get stream info as a health check
        self.jetstream
            .get_stream(&self.config.stream_name)
            .await
            .map_err(|e| QueueError::HealthCheck(format!("Health check failed: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nats_config_default() {
        let config = NatsConfig::default();
        assert_eq!(config.url, "nats://localhost:4222");
        assert_eq!(config.stream_name, "JOBS");
        assert_eq!(config.subject, "jobs.>");
        assert_eq!(config.max_age_seconds, 86400);
        assert_eq!(config.max_messages, 1_000_000);
        assert_eq!(config.consumer_name, "job-workers");
        assert_eq!(config.max_deliver, 10);
    }
}
