use std::sync::Arc;
use tokio::sync::broadcast;

use common::config::Settings;
use common::db::DbPool;
use common::storage::StorageService;

/// Application state shared across all handlers
#[derive(Clone)]
pub struct AppState {
    pub db_pool: DbPool,
    pub redis_client: redis::Client,
    pub nats_client: async_nats::Client,
    pub storage_service: Arc<dyn StorageService>,
    pub config: Arc<Settings>,
    pub sse_tx: broadcast::Sender<SseEvent>,
}

// Manual Debug implementation for cleaner output
impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("db_pool", &self.db_pool)
            .field("redis_client", &"<redis::Client>")
            .field("nats_client", &"<async_nats::Client>")
            .field("storage_service", &"<Arc<dyn StorageService>>")
            .field("config", &self.config)
            .field("sse_tx", &self.sse_tx)
            .finish()
    }
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
    /// Create a new AppState instance with Storage service (PostgreSQL + Redis + Filesystem)
    pub fn new(
        db_pool: DbPool,
        redis_client: redis::Client,
        nats_client: async_nats::Client,
        storage_service: Arc<dyn StorageService>,
        config: Settings,
    ) -> Self {
        let (sse_tx, _) = broadcast::channel(100);

        Self {
            db_pool,
            redis_client,
            nats_client,
            storage_service,
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
