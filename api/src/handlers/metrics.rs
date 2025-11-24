use axum::{http::StatusCode, response::IntoResponse};

/// Prometheus metrics endpoint
#[tracing::instrument]
pub async fn metrics_handler() -> impl IntoResponse {
    // Get metrics from the global metrics exporter
    let recorder = metrics_exporter_prometheus::PrometheusBuilder::new().build_recorder();
    let handle = recorder.handle();
    (StatusCode::OK, handle.render())
}
