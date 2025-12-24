// Job details handlers
// Requirements: 6.3 - Display job details with execution history

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::Html,
};
use common::db::repositories::job::JobStats;
use tera::Context;
use uuid::Uuid;

use super::shared_utils::{get_schedule_type_str, load_job_from_storage};
use crate::handlers::ErrorResponse;
use crate::state::AppState;
use crate::templates::TEMPLATES;

/// Prepare job data for template rendering
fn prepare_job_data(
    job: &common::models::Job,
    full_job: Option<&common::models::Job>,
    stats: Option<&JobStats>,
) -> serde_json::Value {
    let schedule_type = full_job.and_then(|fj| get_schedule_type_str(&fj.schedule));

    let schedule_config = full_job.and_then(|fj| {
        fj.schedule.as_ref().map(|s| match s {
            common::models::Schedule::Cron {
                expression,
                timezone,
                ..
            } => {
                serde_json::json!({
                    "expression": expression,
                    "timezone": timezone.to_string()
                })
            }
            common::models::Schedule::FixedDelay { delay_seconds } => {
                serde_json::json!({
                    "delay_seconds": delay_seconds
                })
            }
            common::models::Schedule::FixedRate { interval_seconds } => {
                serde_json::json!({
                    "interval_seconds": interval_seconds
                })
            }
            common::models::Schedule::OneTime { execute_at } => {
                serde_json::json!({
                    "execute_at": execute_at.to_rfc3339()
                })
            }
        })
    });

    let steps = full_job.map(|fj| {
        fj.steps
            .iter()
            .map(|step| {
                let step_type = match &step.step_type {
                    common::models::JobType::HttpRequest { .. } => "HttpRequest",
                    common::models::JobType::DatabaseQuery { .. } => "DatabaseQuery",
                    common::models::JobType::Sftp { .. } => "SftpOperation",
                    common::models::JobType::FileProcessing { .. } => "FileProcessing",
                };
                serde_json::json!({
                    "name": step.name,
                    "step_type": step_type
                })
            })
            .collect::<Vec<_>>()
    });

    serde_json::json!({
        "id": job.id.to_string(),
        "name": job.name,
        "description": job.description,
        "enabled": job.enabled,
        "schedule_type": schedule_type,
        "schedule_config": schedule_config,
        "timeout_seconds": job.timeout_seconds,
        "max_retries": job.max_retries,
        "allow_concurrent": job.allow_concurrent,
        "created_at": job.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        "updated_at": job.updated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        "total_executions": stats.map(|s| s.total_executions).unwrap_or(0),
        "successful_executions": stats.map(|s| s.successful_executions).unwrap_or(0),
        "failed_executions": stats.map(|s| s.failed_executions).unwrap_or(0),
        "last_execution_at": stats.and_then(|s| s.last_execution_at.map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())),
        "steps": steps,
    })
}

/// Job details modal content (HTMX)
#[tracing::instrument(skip(state))]
pub async fn job_details_modal(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, ErrorResponse> {
    let mut context = Context::new();

    // Fetch job from database
    let job_repo = common::db::repositories::JobRepository::new(state.db_pool.clone());
    let job = job_repo
        .find_by_id(id)
        .await
        .map_err(|e| ErrorResponse::new("database_error", &format!("Database error: {}", e)))?
        .ok_or_else(|| ErrorResponse::new("not_found", &format!("Job not found: {}", id)))?;

    // Fetch job statistics
    let stats = job_repo
        .get_stats(id)
        .await
        .map_err(|e| ErrorResponse::new("database_error", &format!("Database error: {}", e)))?;

    // Load full job definition from storage with Redis cache fallback
    let full_job = load_job_from_storage(state.storage_service.as_ref(), id).await;

    // Prepare job data for template
    let job_data = prepare_job_data(&job, full_job.as_ref(), stats.as_ref());
    context.insert("job", &job_data);

    let html = TEMPLATES
        .render("_job_details_modal_content.html", &context)
        .map_err(|e| {
            tracing::error!(error = %e, job_id = %id, "Template rendering failed");
            ErrorResponse::new(
                "template_error",
                &format!("Failed to render job details modal: {}", e),
            )
        })?;

    Ok(Html(html))
}

/// Job details partial (HTMX)
#[tracing::instrument(skip(state, headers))]
pub async fn job_details_partial(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, ErrorResponse> {
    let mut context = Context::new();
    context.insert("active_page", "jobs");

    let is_htmx = headers.get("HX-Request").is_some();
    context.insert("is_htmx", &is_htmx);

    // Fetch job from database
    let job_repo = common::db::repositories::JobRepository::new(state.db_pool.clone());
    let job = job_repo
        .find_by_id(id)
        .await
        .map_err(|e| ErrorResponse::new("database_error", &format!("Database error: {}", e)))?
        .ok_or_else(|| ErrorResponse::new("not_found", &format!("Job not found: {}", id)))?;

    // Fetch job statistics
    let stats = job_repo
        .get_stats(id)
        .await
        .map_err(|e| ErrorResponse::new("database_error", &format!("Database error: {}", e)))?;

    // Load full job definition from storage with Redis cache fallback
    let full_job = load_job_from_storage(state.storage_service.as_ref(), id).await;

    // Prepare job data for template
    let job_data = prepare_job_data(&job, full_job.as_ref(), stats.as_ref());
    context.insert("job", &job_data);

    // If HTMX request, return only the content partial
    // Otherwise, return the full page with layout
    let template = if is_htmx {
        "_job_details_content.html"
    } else {
        "job_details.html"
    };

    let html = TEMPLATES.render(template, &context).map_err(|e| {
        tracing::error!(error = %e, job_id = %id, template = template, "Template rendering failed");
        ErrorResponse::new(
            "template_error",
            &format!("Failed to render '{}': {}", template, e),
        )
    })?;

    Ok(Html(html))
}
