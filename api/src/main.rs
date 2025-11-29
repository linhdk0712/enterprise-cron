use anyhow::Result;
use common::bootstrap;
use common::config::Settings;
use futures::StreamExt;
use std::net::SocketAddr;

mod handlers;
mod middleware;
mod routes;
mod state;
mod templates;

use state::{AppState, SseEvent};

/// Subscribe to status change events from worker via NATS
///
/// Requirements: 6.7 - Push updates to connected clients using Server-Sent Events
#[tracing::instrument(skip(state))]
async fn subscribe_to_status_changes(state: AppState) -> Result<()> {
    let mut subscriber = state
        .nats_client
        .subscribe("status.>")
        .await
        .map_err(|e| anyhow::anyhow!("Failed to subscribe to status changes: {}", e))?;

    tracing::info!("Subscribed to status change events");

    while let Some(msg) = subscriber.next().await {
        // Parse the status change message
        if let Ok(event) = serde_json::from_slice::<StatusChangeEvent>(&msg.payload) {
            tracing::debug!(event = ?event, "Received status change event");

            // Convert to SSE event and broadcast
            match event {
                StatusChangeEvent::ExecutionStatusChanged {
                    execution_id,
                    job_id,
                    status,
                } => {
                    state.broadcast_event(SseEvent::ExecutionStatusChanged {
                        execution_id,
                        job_id,
                        status,
                    });
                }
                StatusChangeEvent::JobStatusChanged { job_id, status } => {
                    state.broadcast_event(SseEvent::JobStatusChanged { job_id, status });
                }
            }
        } else {
            tracing::warn!(
                payload = ?String::from_utf8_lossy(&msg.payload),
                "Failed to parse status change event"
            );
        }
    }

    Ok(())
}

/// Status change events published by worker
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StatusChangeEvent {
    ExecutionStatusChanged {
        execution_id: uuid::Uuid,
        job_id: uuid::Uuid,
        status: String,
    },
    JobStatusChanged {
        job_id: uuid::Uuid,
        status: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    bootstrap::init_human_tracing();

    tracing::info!("Starting API server");

    // Load configuration
    let config = Settings::load()?;
    tracing::info!(
        host = %config.server.host,
        port = %config.server.port,
        "Configuration loaded"
    );

    // Initialize database connection pool
    let db_pool = bootstrap::init_database_pool(&config).await?;

    // Run migrations (commented out for local development - migrations already exist)
    // sqlx::migrate!("../migrations").run(db_pool.pool()).await?;
    tracing::info!("Database migrations skipped (already applied)");

    // Initialize Redis client (for rate limiting and other features)
    let redis_client = redis::Client::open(config.redis.url.clone())?;
    tracing::info!("Redis client initialized");

    // Initialize Redis connection manager for storage cache
    let redis_conn_manager = bootstrap::init_redis_connection_manager(&config).await?;

    // Initialize NATS client
    let nats_client = async_nats::connect(&config.nats.url).await?;
    tracing::info!("NATS client connected");

    // Initialize Storage service (PostgreSQL + Redis + Filesystem)
    let storage_service = bootstrap::init_storage_service(
        &config,
        db_pool.clone(),
        std::sync::Arc::new(redis_conn_manager)
    ).await?;

    // Initialize Prometheus metrics exporter
    let _metrics_handle =
        metrics_exporter_prometheus::PrometheusBuilder::new().install_recorder()?;
    tracing::info!(port = %config.observability.metrics_port, "Metrics exporter initialized");

    // Create application state
    let state = AppState::new(
        db_pool,
        redis_client,
        nats_client.clone(),
        storage_service,
        config.clone(),
    );

    // Start background task to listen for status changes from worker
    let state_clone = state.clone();
    tokio::spawn(async move {
        if let Err(e) = subscribe_to_status_changes(state_clone).await {
            tracing::error!(error = %e, "Status change subscription error");
        }
    });

    // Create router
    let app = routes::create_router(state);

    // Start server
    let addr = SocketAddr::from((
        config.server.host.parse::<std::net::IpAddr>()?,
        config.server.port,
    ));
    tracing::info!(addr = %addr, "Starting HTTP server");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("API server stopped");
    Ok(())
}

/// Graceful shutdown signal handler
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C signal");
        },
        _ = terminate => {
            tracing::info!("Received SIGTERM signal");
        },
    }

    tracing::info!("Initiating graceful shutdown");
}
