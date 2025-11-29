// Executions list handler
// Requirements: 6.4 - Display execution history with filtering

use axum::{extract::{Query, State}, http::HeaderMap, response::Html};
use chrono::{DateTime, Utc};
use tera::Context;
use uuid::Uuid;

use crate::handlers::ErrorResponse;
use crate::state::AppState;
use crate::templates::TEMPLATES;
use super::ExecutionQueryParams;
use super::shared_utils::{setup_htmx_context, calculate_pagination, db_error};

/// Executions partial (HTMX)
#[tracing::instrument(skip(state, headers))]
pub async fn executions_partial(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ExecutionQueryParams>,
) -> Result<Html<String>, ErrorResponse> {
    let mut context = Context::new();
    context.insert("active_page", "executions");

    let limit = params.limit.unwrap_or(20);
    let offset = params.offset.unwrap_or(0);

    // Build count query with filters
    let mut count_query = String::from(
        r#"
        SELECT COUNT(*)
        FROM job_executions je
        LEFT JOIN jobs j ON je.job_id = j.id
        WHERE je.created_at >= NOW() - INTERVAL '30 days'
        "#,
    );

    if let Some(job_id) = params.job_id {
        count_query.push_str(&format!(" AND je.job_id = '{}'", job_id));
    }

    if let Some(status) = &params.status {
        if !status.is_empty() {
            count_query.push_str(&format!(" AND je.status = '{}'", status.to_lowercase()));
        }
    }

    if let Some(trigger_source) = &params.trigger_source {
        if !trigger_source.is_empty() {
            count_query.push_str(&format!(" AND je.trigger_source = '{}'", trigger_source.to_lowercase()));
        }
    }

    if let Some(job_name) = &params.job_name {
        if !job_name.is_empty() {
            count_query.push_str(&format!(" AND j.name ILIKE '%{}%'", job_name.replace("'", "''")));
        }
    }

    let total_count: i64 = sqlx::query_scalar(&count_query)
        .fetch_one(state.db_pool.pool())
        .await
        .unwrap_or(0);

    let (page, total_pages) = calculate_pagination(offset, limit, total_count);

    // Build query with JOIN to get job name and filters
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
        if !status.is_empty() {
            query.push_str(&format!(" AND je.status = '{}'", status.to_lowercase()));
        }
    }

    if let Some(trigger_source) = &params.trigger_source {
        if !trigger_source.is_empty() {
            query.push_str(&format!(" AND je.trigger_source = '{}'", trigger_source.to_lowercase()));
        }
    }

    if let Some(job_name) = &params.job_name {
        if !job_name.is_empty() {
            query.push_str(&format!(" AND j.name ILIKE '%{}%'", job_name.replace("'", "''")));
        }
    }

    query.push_str(" ORDER BY je.created_at DESC");
    query.push_str(&format!(" LIMIT {} OFFSET {}", limit, offset));

    // Execute raw query to get executions with job names
    let rows = sqlx::query(&query)
        .fetch_all(state.db_pool.pool())
        .await
        .map_err(db_error)?;

    // Map rows to JSON for template
    let executions: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            use sqlx::Row;
            let id: Uuid = row.get("id");
            let job_id: Uuid = row.get("job_id");
            let status: String = row.get("status");
            let trigger_source: String = row.get("trigger_source");
            let attempt: i32 = row.get("attempt");
            let started_at: Option<DateTime<Utc>> = row.get("started_at");
            let completed_at: Option<DateTime<Utc>> = row.get("completed_at");
            let created_at: DateTime<Utc> = row.get("created_at");
            let job_name: Option<String> = row.get("job_name");

            // Calculate duration in seconds if both timestamps exist
            let duration_seconds = if let (Some(start), Some(end)) = (started_at, completed_at) {
                Some((end - start).num_seconds())
            } else {
                None
            };

            serde_json::json!({
                "id": id.to_string(),
                "job_id": job_id.to_string(),
                "status": status,
                "trigger_source": trigger_source,
                "attempt": attempt,
                "started_at": started_at.map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
                "completed_at": completed_at.map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
                "created_at": created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                "job_name": job_name,
                "duration_seconds": duration_seconds,
            })
        })
        .collect();

    context.insert("executions", &executions);
    context.insert("limit", &limit);
    context.insert("offset", &offset);
    context.insert("page", &page);
    context.insert("total_pages", &total_pages);
    context.insert("total_count", &total_count);
    context.insert("status_filter", &params.status.clone().unwrap_or_default());
    context.insert("trigger_source_filter", &params.trigger_source.clone().unwrap_or_default());
    context.insert("job_name_filter", &params.job_name.clone().unwrap_or_default());
    context.insert("job_id_filter", &params.job_id.map(|id| id.to_string()).unwrap_or_default());
    
    // Check if this is embedded in job details (has job_id filter)
    let is_embedded = params.job_id.is_some();
    context.insert("is_embedded", &is_embedded);

    // Setup HTMX context and determine template using shared utility
    let template = setup_htmx_context(&mut context, &headers, "_executions_content.html", "executions.html");

    let html = TEMPLATES.render(template, &context).map_err(|e| {
        tracing::error!(error = %e, "Template rendering failed");
        ErrorResponse::new(
            "template_error",
            &format!("Failed to render '{}'", template),
        )
    })?;

    Ok(Html(html))
}
