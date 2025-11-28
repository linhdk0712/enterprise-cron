// Jobs list handler
// Requirements: 6.2 - Display paginated job list

use axum::{extract::{Query, State}, http::HeaderMap, response::Html};
use tera::Context;

use crate::handlers::ErrorResponse;
use crate::state::AppState;
use crate::templates::TEMPLATES;
use super::ExecutionQueryParams;

/// Helper function to extract schedule type from job
fn get_schedule_type(schedule: &Option<common::models::Schedule>) -> Option<&'static str> {
    schedule.as_ref().map(|s| match s {
        common::models::Schedule::Cron { .. } => "Cron",
        common::models::Schedule::FixedDelay { .. } => "FixedDelay",
        common::models::Schedule::FixedRate { .. } => "FixedRate",
        common::models::Schedule::OneTime { .. } => "OneTime",
    })
}

/// Helper function to calculate next run time
fn get_next_run_time(schedule: &Option<common::models::Schedule>, enabled: bool) -> Option<String> {
    if !enabled {
        return None;
    }
    
    schedule.as_ref().and_then(|s| match s {
        common::models::Schedule::Cron { .. } => Some("Scheduled".to_string()),
        common::models::Schedule::FixedDelay { delay_seconds } => {
            Some(format!("Every {}s", delay_seconds))
        },
        common::models::Schedule::FixedRate { interval_seconds } => {
            Some(format!("Every {}s", interval_seconds))
        },
        common::models::Schedule::OneTime { execute_at } => {
            Some(execute_at.format("%Y-%m-%d %H:%M:%S").to_string())
        },
    })
}

/// Helper function to get job type from steps
fn get_job_type(steps: &[common::models::JobStep]) -> Option<&'static str> {
    steps.first().map(|step| match &step.step_type {
        common::models::JobType::HttpRequest { .. } => "HTTP",
        common::models::JobType::DatabaseQuery { .. } => "Database",
        common::models::JobType::Sftp { .. } => "SFTP",
        common::models::JobType::FileProcessing { .. } => "File",
    })
}

/// Jobs list partial (HTMX)
#[tracing::instrument(skip(state, headers))]
pub async fn jobs_partial(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ExecutionQueryParams>,
) -> Result<Html<String>, ErrorResponse> {
    let mut context = Context::new();
    context.insert("active_page", "jobs");

    let is_htmx = headers.get("HX-Request").is_some();
    context.insert("is_htmx", &is_htmx);

    let limit = params.limit.unwrap_or(20);
    let offset = params.offset.unwrap_or(0);
    let page = (offset / limit) + 1;

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
        // Load full job definition from MinIO
        let (schedule_type, next_run_time, job_type) = if !job.minio_definition_path.is_empty() {
            match state.minio_client.get_object(&job.minio_definition_path).await {
                Ok(data) => {
                    match serde_json::from_slice::<common::models::Job>(&data) {
                        Ok(full_job) => {
                            let sched_type = get_schedule_type(&full_job.schedule);
                            let next_run = get_next_run_time(&full_job.schedule, job.enabled);
                            let jtype = get_job_type(&full_job.steps);
                            (sched_type, next_run, jtype)
                        },
                        Err(e) => {
                            tracing::warn!(job_id = %job.id, error = %e, "Failed to parse job definition");
                            (None, None, None)
                        }
                    }
                },
                Err(e) => {
                    tracing::warn!(job_id = %job.id, error = %e, "Failed to load job definition from MinIO");
                    (None, None, None)
                }
            }
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

    // Update total_count to use actual count
    let total_count = total_jobs;
    let total_pages = ((total_count as f64) / (limit as f64)).ceil() as i64;
    
    context.insert("jobs", &jobs_json);
    context.insert("limit", &limit);
    context.insert("offset", &offset);
    context.insert("page", &page);
    context.insert("total_pages", &total_pages);
    context.insert("total_count", &total_count);

    // If HTMX request, return only the content partial
    // Otherwise, return the full page with layout
    let template = if is_htmx {
        "_jobs_content.html"
    } else {
        "jobs.html"
    };

    let html = TEMPLATES
        .render(template, &context)
        .map_err(|e| ErrorResponse::new("template_error", &format!("Template error: {}", e)))?;

    Ok(Html(html))
}
