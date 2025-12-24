// Jobs list handler
// Requirements: 6.2 - Display paginated job list

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::Html,
};
use tera::Context;

use super::shared_utils::{
    calculate_pagination, get_job_type_str, get_next_run_display, get_schedule_type_str,
    load_job_from_storage, setup_htmx_context,
};
use super::ExecutionQueryParams;
use crate::handlers::ErrorResponse;
use crate::state::AppState;
use crate::templates::TEMPLATES;

/// Jobs list partial (HTMX)
#[tracing::instrument(skip(state, headers))]
pub async fn jobs_partial(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ExecutionQueryParams>,
) -> Result<Html<String>, ErrorResponse> {
    let mut context = Context::new();
    context.insert("active_page", "jobs");

    let limit = params.limit.unwrap_or(20);
    let offset = params.offset.unwrap_or(0);

    // Fetch jobs with pagination
    let job_repo = common::db::repositories::JobRepository::new(state.db_pool.clone());
    let all_jobs = job_repo.find_all().await.unwrap_or_default();

    // Apply pagination
    let total_jobs = all_jobs.len() as i64;
    let start = offset as usize;
    let end = std::cmp::min(start + limit as usize, all_jobs.len());
    let paginated_jobs = &all_jobs[start..end];

    // Convert jobs to JSON for template with full details
    let mut jobs_json: Vec<serde_json::Value> = Vec::new();

    for job in paginated_jobs {
        // Load full job definition from storage (Redis cache → PostgreSQL) using shared utility
        let (schedule_type, next_run_time, job_type) = if let Some(full_job) =
            load_job_from_storage(state.storage_service.as_ref(), job.id).await
        {
            let sched_type = get_schedule_type_str(&full_job.schedule);
            let next_run = get_next_run_display(&full_job.schedule, job.enabled);
            let jtype = get_job_type_str(&full_job.steps);
            (sched_type, next_run, jtype)
        } else {
            (None, None, None)
        };

        // Get job statistics
        let stats = job_repo.get_stats(job.id).await.ok().flatten();
        let last_exec = stats.as_ref().and_then(|s| s.last_execution_at);
        let total_execs = stats.as_ref().map(|s| s.total_executions).unwrap_or(0);
        let success_execs = stats.as_ref().map(|s| s.successful_executions).unwrap_or(0);

        jobs_json.push(serde_json::json!({
            "id": job.id.to_string(),
            "name": job.name,
            "description": job.description,
            "enabled": job.enabled,
            "schedule_type": schedule_type,
            "next_run_time": next_run_time,
            "job_type": job_type,
            "last_execution_at": last_exec.map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
            "total_executions": total_execs,
            "successful_executions": success_execs,
            "timeout_seconds": job.timeout_seconds,
            "max_retries": job.max_retries,
            "allow_concurrent": job.allow_concurrent,
            "created_at": job.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            "updated_at": job.updated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        }));
    }

    // Calculate pagination using shared utility
    let total_count = total_jobs;
    let (page, total_pages) = calculate_pagination(offset, limit, total_count);

    context.insert("jobs", &jobs_json);
    context.insert("limit", &limit);
    context.insert("offset", &offset);
    context.insert("page", &page);
    context.insert("total_pages", &total_pages);
    context.insert("total_count", &total_count);

    // Setup HTMX context and determine template using shared utility
    let template = setup_htmx_context(&mut context, &headers, "_jobs_content.html", "jobs.html");

    let html = TEMPLATES
        .render(template, &context)
        .map_err(|e| ErrorResponse::new("template_error", &format!("Template error: {}", e)))?;

    Ok(Html(html))
}
