use axum::{http::StatusCode, response::IntoResponse};

/// Health check endpoint
#[tracing::instrument]
pub async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK")
}
