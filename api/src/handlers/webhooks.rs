use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use common::db::repositories::{ExecutionRepository, JobRepository, WebhookRepository};
use common::models::{
    ExecutionStatus, JobContext, JobExecution, TriggerSource, WebhookData, WebhookResponse,
};
use common::queue::publisher::JobPublisher;
use common::storage::service::MinIOService;
use common::webhook::validate_webhook_signature;
use serde::Deserialize;
use std::collections::HashMap;
use uuid::Uuid;

use crate::handlers::{ErrorResponse, SuccessResponse};
use crate::state::AppState;

/// Handle webhook POST requests
/// Requirements: 16.2, 16.3, 16.4, 16.5, 16.7, 16.8, 16.9, 16.10
#[tracing::instrument(skip(state, headers, body))]
pub async fn handle_webhook(
    State(state): State<AppState>,
    Path(url_path): Path<String>,
    Query(query_params): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<WebhookResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Construct full URL path
    let full_path = format!("/webhooks/{}", url_path);

    tracing::info!(url_path = %full_path, "Received webhook request");

    // 1. Find webhook by URL path
    // Requirements: 16.2 - Lookup webhook configuration
    let webhook_repo = WebhookRepository::new(state.db_pool.pool().clone());
    let webhook = webhook_repo
        .find_by_url_path(&full_path)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to lookup webhook");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "internal_error",
                    "Failed to lookup webhook",
                )),
            )
        })?
        .ok_or_else(|| {
            tracing::warn!(url_path = %full_path, "Webhook not found");
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new("not_found", "Webhook not found")),
            )
        })?;

    // 2. Check if webhook is enabled
    // Requirements: 16.10 - Reject disabled job webhooks with 403
    if !webhook.enabled {
        tracing::warn!(
            webhook_id = %webhook.id,
            job_id = %webhook.job_id,
            "Webhook is disabled"
        );
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse::new(
                "webhook_disabled",
                "This webhook is disabled",
            )),
        ));
    }

    // 3. Check if job is enabled
    // Requirements: 16.10 - Reject webhooks for disabled jobs
    let job_repo = JobRepository::new(state.db_pool.clone());
    let job = job_repo
        .find_by_id(webhook.job_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to lookup job");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("internal_error", "Failed to lookup job")),
            )
        })?
        .ok_or_else(|| {
            tracing::warn!(job_id = %webhook.job_id, "Job not found");
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new("not_found", "Job not found")),
            )
        })?;

    if !job.enabled {
        tracing::warn!(
            job_id = %job.id,
            job_name = %job.name,
            "Job is disabled"
        );
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse::new("job_disabled", "This job is disabled")),
        ));
    }

    // 4. Check rate limit
    // Requirements: 16.11 - Enforce per-job rate limits, return 429 for violations
    if let (Some(max_requests), Some(window_seconds)) = (
        webhook.rate_limit_max_requests,
        webhook.rate_limit_window_seconds,
    ) {
        use common::rate_limit::RateLimiter;

        let rate_limiter = RateLimiter::new(state.redis_client.clone());
        let allowed = rate_limiter
            .check_rate_limit(webhook.id, max_requests as u32, window_seconds as u32)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to check rate limit");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse::new(
                        "internal_error",
                        "Failed to check rate limit",
                    )),
                )
            })?;

        if !allowed {
            tracing::warn!(
                webhook_id = %webhook.id,
                max_requests = max_requests,
                window_seconds = window_seconds,
                "Rate limit exceeded"
            );
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                Json(ErrorResponse::new(
                    "rate_limit_exceeded",
                    "Too many requests. Please try again later.",
                )),
            ));
        }
    }

    // 5. Validate HMAC signature
    // Requirements: 16.7, 16.8 - Validate HMAC-SHA256 signatures, reject invalid with 401
    let signature = headers
        .get("X-Webhook-Signature")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            tracing::warn!("Missing X-Webhook-Signature header");
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse::new(
                    "missing_signature",
                    "X-Webhook-Signature header is required",
                )),
            )
        })?;

    let is_valid =
        validate_webhook_signature(&body, signature, &webhook.secret_key).map_err(|e| {
            tracing::error!(error = %e, "Failed to validate signature");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "internal_error",
                    "Failed to validate signature",
                )),
            )
        })?;

    if !is_valid {
        tracing::warn!(
            webhook_id = %webhook.id,
            "Invalid webhook signature"
        );
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse::new(
                "invalid_signature",
                "Webhook signature is invalid",
            )),
        ));
    }

    // 5. Parse JSON payload
    // Requirements: 16.3 - Store webhook payload in Job Context
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap_or(serde_json::json!({}));

    // 6. Extract custom headers (filter out standard headers)
    // Requirements: 16.5 - Store custom headers in Job Context
    let mut custom_headers = HashMap::new();
    for (key, value) in headers.iter() {
        let key_str = key.as_str();
        // Only include custom headers (X- prefix or application-specific)
        if key_str.starts_with("X-") && key_str != "X-Webhook-Signature" {
            if let Ok(value_str) = value.to_str() {
                custom_headers.insert(key_str.to_string(), value_str.to_string());
            }
        }
    }

    // 7. Create webhook data
    // Requirements: 16.3, 16.4, 16.5 - Store payload, query params, headers
    let webhook_data = WebhookData {
        payload,
        query_params,
        headers: custom_headers,
    };

    // 8. Create job execution
    let execution_id = Uuid::new_v4();
    let idempotency_key = format!("webhook-{}-{}", webhook.id, execution_id);
    let minio_context_path = format!("jobs/{}/executions/{}/context.json", job.id, execution_id);

    let execution = JobExecution {
        id: execution_id,
        job_id: job.id,
        idempotency_key: idempotency_key.clone(),
        status: ExecutionStatus::Pending,
        attempt: 1,
        trigger_source: TriggerSource::Webhook {
            webhook_url: full_path.clone(),
        },
        current_step: None,
        minio_context_path: minio_context_path.clone(),
        started_at: None,
        completed_at: None,
        result: None,
        error: None,
        created_at: chrono::Utc::now(),
    };

    // 9. Initialize Job Context with webhook data
    // Requirements: 16.3, 16.4, 16.5 - Store webhook data in Job Context
    let mut context = JobContext::new(execution_id, job.id);
    context.set_webhook_data(webhook_data);

    // 10. Store Job Context to MinIO
    // Requirements: 13.7 - Persist Job Context to MinIO
    let storage_service =
        common::storage::service::MinIOServiceImpl::new(state.minio_client.clone());
    storage_service.store_context(&context).await.map_err(|e| {
        tracing::error!(error = %e, "Failed to store Job Context to MinIO");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(
                "internal_error",
                "Failed to store job context",
            )),
        )
    })?;

    // 11. Save execution to database
    let execution_repo = ExecutionRepository::new(state.db_pool.clone());
    execution_repo.create(&execution).await.map_err(|e| {
        tracing::error!(error = %e, "Failed to create execution");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(
                "internal_error",
                "Failed to create execution",
            )),
        )
    })?;

    // 12. Publish job to queue
    // Requirements: 16.2 - Queue job execution immediately
    let nats_config = common::queue::nats::NatsConfig {
        url: state.config.nats.url.clone(),
        stream_name: state.config.nats.stream_name.clone(),
        subject: "jobs.>".to_string(),
        max_age_seconds: 86400,
        max_messages: 1_000_000,
        consumer_name: state.config.nats.consumer_name.clone(),
        max_deliver: 10,
    };
    let nats_client =
        common::queue::nats::NatsClient::from_client(state.nats_client.clone(), nats_config);
    let publisher = common::queue::publisher::NatsJobPublisher::new(nats_client);
    publisher.publish(&execution).await.map_err(|e| {
        tracing::error!(error = %e, "Failed to publish job to queue");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(
                "internal_error",
                "Failed to queue job execution",
            )),
        )
    })?;

    tracing::info!(
        execution_id = %execution_id,
        job_id = %job.id,
        webhook_id = %webhook.id,
        "Webhook triggered job execution"
    );

    // 13. Return 202 Accepted with execution_id
    // Requirements: 16.9 - Return 202 Accepted with execution_id
    Ok(Json(WebhookResponse {
        execution_id,
        message: "Job execution queued successfully".to_string(),
    }))
}

/// Create a webhook for a job
/// Requirements: 16.1 - Generate unique webhook URL for job
#[tracing::instrument(skip(state))]
#[allow(dead_code)]
pub async fn create_webhook(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
    Json(req): Json<CreateWebhookRequest>,
) -> Result<Json<SuccessResponse<common::models::Webhook>>, (StatusCode, Json<ErrorResponse>)> {
    use common::webhook::{generate_webhook_secret, generate_webhook_url_path};

    // Check if job exists
    let job_repo = JobRepository::new(state.db_pool.clone());
    let _job = job_repo
        .find_by_id(job_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to lookup job");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("internal_error", "Failed to lookup job")),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new("not_found", "Job not found")),
            )
        })?;

    // Check if webhook already exists for this job
    let webhook_repo = WebhookRepository::new(state.db_pool.pool().clone());
    if let Some(_existing) = webhook_repo.find_by_job_id(job_id).await.map_err(|e| {
        tracing::error!(error = %e, "Failed to check existing webhook");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(
                "internal_error",
                "Failed to check existing webhook",
            )),
        )
    })? {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse::new(
                "webhook_exists",
                "Webhook already exists for this job",
            )),
        ));
    }

    // Generate webhook URL and secret
    let url_path = generate_webhook_url_path(job_id);
    let secret_key = generate_webhook_secret();

    // Create webhook
    let webhook = webhook_repo
        .create(
            job_id,
            url_path,
            secret_key,
            req.rate_limit_max_requests,
            req.rate_limit_window_seconds,
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to create webhook");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "internal_error",
                    "Failed to create webhook",
                )),
            )
        })?;

    tracing::info!(
        webhook_id = %webhook.id,
        job_id = %job_id,
        url_path = %webhook.url_path,
        "Created webhook"
    );

    Ok(Json(SuccessResponse::new(webhook)))
}

/// Regenerate webhook URL and secret
/// Requirements: 16.12 - Webhook URL regeneration invalidates previous URL
#[tracing::instrument(skip(state))]
#[allow(dead_code)]
pub async fn regenerate_webhook(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<SuccessResponse<common::models::Webhook>>, (StatusCode, Json<ErrorResponse>)> {
    use common::webhook::{generate_webhook_secret, generate_webhook_url_path};

    let webhook_repo = WebhookRepository::new(state.db_pool.pool().clone());

    // Find existing webhook
    let existing = webhook_repo
        .find_by_job_id(job_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to lookup webhook");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "internal_error",
                    "Failed to lookup webhook",
                )),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new("not_found", "Webhook not found")),
            )
        })?;

    // Generate new URL and secret
    let new_url_path = generate_webhook_url_path(job_id);
    let new_secret_key = generate_webhook_secret();

    // Update webhook
    let webhook = webhook_repo
        .regenerate_secret(existing.id, new_url_path.clone(), new_secret_key)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to regenerate webhook");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "internal_error",
                    "Failed to regenerate webhook",
                )),
            )
        })?;

    tracing::info!(
        webhook_id = %webhook.id,
        job_id = %job_id,
        new_url_path = %new_url_path,
        "Regenerated webhook"
    );

    Ok(Json(SuccessResponse::new(webhook)))
}

/// Get webhook for a job
#[tracing::instrument(skip(state))]
#[allow(dead_code)]
pub async fn get_webhook(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<SuccessResponse<common::models::Webhook>>, (StatusCode, Json<ErrorResponse>)> {
    let webhook_repo = WebhookRepository::new(state.db_pool.pool().clone());

    let webhook = webhook_repo
        .find_by_job_id(job_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to lookup webhook");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "internal_error",
                    "Failed to lookup webhook",
                )),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new("not_found", "Webhook not found")),
            )
        })?;

    Ok(Json(SuccessResponse::new(webhook)))
}

/// Delete webhook for a job
#[tracing::instrument(skip(state))]
#[allow(dead_code)]
pub async fn delete_webhook(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> Result<Json<SuccessResponse<()>>, (StatusCode, Json<ErrorResponse>)> {
    let webhook_repo = WebhookRepository::new(state.db_pool.pool().clone());

    // Find webhook
    let webhook = webhook_repo
        .find_by_job_id(job_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to lookup webhook");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    "internal_error",
                    "Failed to lookup webhook",
                )),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::new("not_found", "Webhook not found")),
            )
        })?;

    // Delete webhook
    webhook_repo.delete(webhook.id).await.map_err(|e| {
        tracing::error!(error = %e, "Failed to delete webhook");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse::new(
                "internal_error",
                "Failed to delete webhook",
            )),
        )
    })?;

    tracing::info!(
        webhook_id = %webhook.id,
        job_id = %job_id,
        "Deleted webhook"
    );

    Ok(Json(SuccessResponse::new(())))
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct CreateWebhookRequest {
    pub rate_limit_max_requests: Option<i32>,
    pub rate_limit_window_seconds: Option<i32>,
}
