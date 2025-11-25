use std::sync::Arc;
use tokio::sync::broadcast;

use common::config::Settings;
use common::db::DbPool;
use common::storage::minio::MinioClient;

/// Application state shared across all handlers
#[derive(Clone, Debug)]
pub struct AppState {
    pub db_pool: DbPool,
    pub redis_client: redis::Client,
    pub nats_client: async_nats::Client,
    pub minio_client: MinioClient,
    pub config: Arc<Settings>,
    pub sse_tx: broadcast::Sender<SseEvent>,
}

/// Server-Sent Events message types
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SseEvent {
    JobStatusChanged {
        job_id: uuid::Uuid,
        status: String,
    },
    ExecutionStatusChanged {
        execution_id: uuid::Uuid,
        job_id: uuid::Uuid,
        status: String,
    },
    JobCreated {
        job_id: uuid::Uuid,
        name: String,
    },
    JobDeleted {
        job_id: uuid::Uuid,
    },
}

impl AppState {
    /// Create a new AppState instance
    pub fn new(
        db_pool: DbPool,
        redis_client: redis::Client,
        nats_client: async_nats::Client,
        minio_client: MinioClient,
        config: Settings,
    ) -> Self {
        let (sse_tx, _) = broadcast::channel(100);

        Self {
            db_pool,
            redis_client,
            nats_client,
            minio_client,
            config: Arc::new(config),
            sse_tx,
        }
    }

    /// Broadcast an SSE event to all connected clients
    pub fn broadcast_event(&self, event: SseEvent) {
        // Ignore send errors (no receivers is fine)
        let _ = self.sse_tx.send(event);
    }
}
