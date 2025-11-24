use anyhow::Result;
use futures::StreamExt;
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod handlers;
mod middleware;
mod routes;
mod state;

use common::config::Settings;
use common::db::DbPool;
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
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "api=info,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    tracing::info!("Starting API server");

    // Load configuration
    let config = Settings::load()?;
    tracing::info!(
        host = %config.server.host,
        port = %config.server.port,
        "Configuration loaded"
    );

    // Initialize database connection pool
    let pg_pool = PgPoolOptions::new()
        .max_connections(config.database.max_connections as u32)
        .min_connections(config.database.min_connections as u32)
        .connect(&config.database.url)
        .await?;
    let db_pool = DbPool::new(&config.database).await?;
    tracing::info!("Database connection pool established");

    // Run migrations (commented out for local development - migrations already exist)
    // sqlx::migrate!("../migrations").run(db_pool.pool()).await?;
    tracing::info!("Database migrations skipped (already applied)");

    // Initialize Redis client
    let redis_client = redis::Client::open(config.redis.url.clone())?;
    tracing::info!("Redis client initialized");

    // Initialize NATS client
    let nats_client = async_nats::connect(&config.nats.url).await?;
    tracing::info!("NATS client connected");

    // Initialize MinIO client
    let minio_bucket = s3::Bucket::new(
        &config.minio.bucket,
        s3::Region::Custom {
            region: config.minio.region.clone(),
            endpoint: config.minio.endpoint.clone(),
        },
        s3::creds::Credentials::new(
            Some(&config.minio.access_key),
            Some(&config.minio.secret_key),
            None,
            None,
            None,
        )?,
    )?;
    tracing::info!("MinIO client initialized");

    // Initialize Prometheus metrics exporter
    let metrics_handle =
        metrics_exporter_prometheus::PrometheusBuilder::new().install_recorder()?;
    tracing::info!(port = %config.observability.metrics_port, "Metrics exporter initialized");

    // Create application state
    let state = AppState::new(
        db_pool,
        redis_client,
        nats_client.clone(),
        minio_bucket,
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
