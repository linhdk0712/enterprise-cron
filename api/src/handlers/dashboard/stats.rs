// Dashboard statistics page handler
// Requirements: 6.1 - Display dashboard with job statistics

use axum::{extract::State, http::HeaderMap, response::Html};
use chrono::{Duration, Utc};
use tera::Context;

use crate::handlers::ErrorResponse;
use crate::state::AppState;
use crate::templates::TEMPLATES;

/// Dashboard index page with statistics
#[tracing::instrument(skip(state, headers))]
pub async fn dashboard_index(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Html<String>, ErrorResponse> {
    let mut context = Context::new();
    context.insert("active_page", "dashboard");

    let is_htmx = headers.get("HX-Request").is_some();
    context.insert("is_htmx", &is_htmx);

    // Get dashboard statistics
    let total_jobs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs")
        .fetch_one(state.db_pool.pool())
        .await
        .unwrap_or(0);

    let enabled_jobs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM jobs WHERE enabled = true")
        .fetch_one(state.db_pool.pool())
        .await
        .unwrap_or(0);

    let running_executions: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM job_executions WHERE status = 'running'")
            .fetch_one(state.db_pool.pool())
            .await
            .unwrap_or(0);

    // Get 24h statistics
    let twenty_four_hours_ago = Utc::now() - Duration::hours(24);
    let total_executions_24h: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM job_executions WHERE created_at >= $1")
            .bind(twenty_four_hours_ago)
            .fetch_one(state.db_pool.pool())
            .await
            .unwrap_or(0);

    let successful_executions_24h: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM job_executions WHERE status = 'success' AND created_at >= $1",
    )
    .bind(twenty_four_hours_ago)
    .fetch_one(state.db_pool.pool())
    .await
    .unwrap_or(0);

    let failed_executions_24h: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM job_executions WHERE status IN ('failed', 'timeout', 'dead_letter') AND created_at >= $1",
    )
    .bind(twenty_four_hours_ago)
    .fetch_one(state.db_pool.pool())
    .await
    .unwrap_or(0);

    let success_rate = if total_executions_24h > 0 {
        (successful_executions_24h as f64 / total_executions_24h as f64) * 100.0
    } else {
        0.0
    };

    let mut stats = tera::Map::new();
    stats.insert(
        "total_jobs".to_string(),
        tera::to_value(total_jobs).unwrap(),
    );
    stats.insert(
        "enabled_jobs".to_string(),
        tera::to_value(enabled_jobs).unwrap(),
    );
    stats.insert(
        "running_executions".to_string(),
        tera::to_value(running_executions).unwrap(),
    );
    stats.insert(
        "total_executions_24h".to_string(),
        tera::to_value(total_executions_24h).unwrap(),
    );
    stats.insert(
        "successful_executions_24h".to_string(),
        tera::to_value(successful_executions_24h).unwrap(),
    );
    stats.insert(
        "failed_executions_24h".to_string(),
        tera::to_value(failed_executions_24h).unwrap(),
    );
    stats.insert(
        "success_rate".to_string(),
        tera::to_value(success_rate).unwrap(),
    );
    context.insert("stats", &stats);

    // Get recent executions
    let execution_repo = common::db::repositories::ExecutionRepository::new(state.db_pool.clone());
    let filter = common::db::repositories::ExecutionFilter {
        job_id: None,
        status: None,
        trigger_source: None,
        limit: Some(10),
    };
    let recent_executions = execution_repo
        .find_with_filter(filter)
        .await
        .unwrap_or_default();
    context.insert("recent_executions", &recent_executions);

    // Get active jobs (enabled jobs)
    let job_repo = common::db::repositories::JobRepository::new(state.db_pool.clone());
    let all_jobs = job_repo.find_all().await.unwrap_or_default();
    let active_jobs: Vec<serde_json::Value> = all_jobs
        .iter()
        .filter(|job| job.enabled)
        .take(5)
        .map(|job| {
            serde_json::json!({
                "id": job.id,
                "name": job.name,
                "description": job.description,
                "enabled": job.enabled,
            })
        })
        .collect();
    context.insert("active_jobs", &active_jobs);

    // If HTMX request, return only the content partial
    // Otherwise, return the full page with layout
    let template = if is_htmx {
        "_dashboard_content.html"
    } else {
        "dashboard.html"
    };

    let html = TEMPLATES
        .render(template, &context)
        .map_err(|e| ErrorResponse::new("template_error", &format!("Template error: {}", e)))?;

    Ok(Html(html))
}
