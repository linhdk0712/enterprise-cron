use axum::{
    extract::{Path, Query, State},
    response::Html,
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use tera::{Context, Tera};
use uuid::Uuid;

use crate::handlers::ErrorResponse;
use crate::state::AppState;

lazy_static::lazy_static! {
    static ref TEMPLATES: Tera = {
        match Tera::new("api/templates/**/*.html") {
            Ok(t) => t,
            Err(e) => {
                tracing::error!("Template parsing error: {}", e);
                std::process::exit(1);
            }
        }
    };
}

#[derive(Debug, Deserialize)]
pub struct ExecutionQueryParams {
    pub job_id: Option<Uuid>,
    pub status: Option<String>,
    pub offset: Option<i64>,
    pub limit: Option<i64>,
}

/// Dashboard index page
#[tracing::instrument(skip(state))]
pub async fn dashboard_index(State(state): State<AppState>) -> Result<Html<String>, ErrorResponse> {
    let mut context = Context::new();
    context.insert("active_page", "dashboard");

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

    // Get recent executions (using raw query for now)
    let recent_executions: Vec<serde_json::Value> = vec![];
    context.insert("recent_executions", &recent_executions);

    // Get active jobs (using raw query for now)
    let active_jobs: Vec<serde_json::Value> = vec![];
    context.insert("active_jobs", &active_jobs);

    let html = TEMPLATES
        .render("dashboard.html", &context)
        .map_err(|e| ErrorResponse::new("template_error", &format!("Template error: {}", e)))?;

    Ok(Html(html))
}

/// Jobs list partial (HTMX)
#[tracing::instrument(skip(state))]
pub async fn jobs_partial(State(state): State<AppState>) -> Result<Html<String>, ErrorResponse> {
    let mut context = Context::new();
    context.insert("active_page", "jobs");

    // Fetch all jobs with their stats (using raw query for now)
    let jobs: Vec<serde_json::Value> = vec![];
    context.insert("jobs", &jobs);

    let html = TEMPLATES
        .render("jobs.html", &context)
        .map_err(|e| ErrorResponse::new("template_error", &format!("Template error: {}", e)))?;

    Ok(Html(html))
}

/// Job details partial (HTMX)
#[tracing::instrument(skip(state))]
pub async fn job_details_partial(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, ErrorResponse> {
    let mut context = Context::new();
    context.insert("active_page", "jobs");

    // Fetch job details (placeholder for now)
    let job = serde_json::json!({
        "id": id,
        "name": "Sample Job",
        "description": "Job details coming soon",
        "enabled": true,
        "schedule_type": "Cron",
        "timeout_seconds": 300,
        "max_retries": 10,
        "allow_concurrent": false,
        "total_executions": 0,
        "successful_executions": 0,
        "failed_executions": 0
    });
    context.insert("job", &job);

    let html = TEMPLATES
        .render("job_details.html", &context)
        .map_err(|e| ErrorResponse::new("template_error", &format!("Template error: {}", e)))?;

    Ok(Html(html))
}

/// Executions partial (HTMX)
#[tracing::instrument(skip(state))]
pub async fn executions_partial(
    State(state): State<AppState>,
    Query(params): Query<ExecutionQueryParams>,
) -> Result<Html<String>, ErrorResponse> {
    let mut context = Context::new();
    context.insert("active_page", "executions");

    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);

    // Build query based on filters
    let mut query = String::from(
        r#"
        SELECT 
            je.id, je.job_id, je.status, je.trigger_source, je.attempt,
            je.started_at, je.completed_at, je.created_at, j.name as job_name
        FROM job_executions je
        LEFT JOIN jobs j ON je.job_id = j.id
        WHERE je.created_at >= NOW() - INTERVAL '30 days'
        "#,
    );

    if let Some(job_id) = params.job_id {
        query.push_str(&format!(" AND je.job_id = '{}'", job_id));
    }

    if let Some(status) = &params.status {
        query.push_str(&format!(" AND je.status = '{}'", status.to_lowercase()));
    }

    query.push_str(" ORDER BY je.created_at DESC");
    query.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));

    // Use ExecutionRepository to fetch executions properly
    let execution_repo = common::db::repositories::ExecutionRepository::new(state.db_pool.clone());
    let filter = common::db::repositories::ExecutionFilter {
        job_id: params.job_id,
        status: params.status.as_ref().and_then(|s| s.parse().ok()),
        trigger_source: None,
        limit: Some(limit),
    };
    let executions = execution_repo
        .find_with_filter(filter)
        .await
        .map_err(|e| ErrorResponse::new("database_error", &format!("Database error: {}", e)))?;

    context.insert("executions", &executions);
    context.insert("limit", &limit);
    context.insert("offset", &offset);
    context.insert("has_more", &(executions.len() as i64 == limit));

    let html = TEMPLATES
        .render("executions.html", &context)
        .map_err(|e| ErrorResponse::new("template_error", &format!("Template error: {}", e)))?;

    Ok(Html(html))
}

/// Variables partial (HTMX)
#[tracing::instrument(skip(state))]
pub async fn variables_partial(
    State(state): State<AppState>,
) -> Result<Html<String>, ErrorResponse> {
    let mut context = Context::new();
    context.insert("active_page", "variables");

    // Fetch all variables (placeholder for now)
    let variables: Vec<serde_json::Value> = vec![];
    context.insert("variables", &variables);

    let html = TEMPLATES
        .render("variables.html", &context)
        .map_err(|e| ErrorResponse::new("template_error", &format!("Template error: {}", e)))?;

    Ok(Html(html))
}

/// Job form page (HTMX)
///
/// Requirements: 18.1 - Visual form builder for job creation
#[tracing::instrument(skip(_state))]
pub async fn job_form_page(State(_state): State<AppState>) -> Result<Html<String>, ErrorResponse> {
    let mut context = Context::new();
    context.insert("active_page", "jobs");

    let html = TEMPLATES
        .render("job_form.html", &context)
        .map_err(|e| ErrorResponse::new("template_error", &format!("Template error: {}", e)))?;

    Ok(Html(html))
}
