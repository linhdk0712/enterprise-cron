use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::handlers::{ErrorResponse, SuccessResponse};
use crate::state::{AppState, SseEvent};
use common::db::repositories::execution::{ExecutionFilter, ExecutionRepository};
use common::models::{ExecutionStatus, JobExecution};

/// Query parameters for listing executions
///
/// # Requirements
/// - 6.2: Display executions from the last 30 days
/// - 6.3: Allow filtering by status and job identifier
#[derive(Debug, Deserialize)]
pub struct ListExecutionsQuery {
    /// Filter by job ID
    pub job_id: Option<Uuid>,
    /// Filter by execution status
    pub status: Option<String>,
    /// Filter by trigger source (scheduled, manual, webhook)
    pub trigger_source: Option<String>,
    /// Limit the number of results
    pub limit: Option<i64>,
}

/// List executions with filters
///
/// # Requirements
/// - 6.2: Display executions from the last 30 days
/// - 6.3: Allow filtering by status and job identifier
///
/// # Correctness Properties
/// - Property 49: Execution history time window - Only executions within last 30 days
/// - Property 50: Execution history filtering - Filter by status and job_id
#[tracing::instrument(skip(state))]
pub async fn list_executions(
    State(state): State<AppState>,
    Query(query): Query<ListExecutionsQuery>,
) -> Result<Json<SuccessResponse<Vec<JobExecution>>>, ErrorResponse> {
    // Parse status if provided
    let status = if let Some(status_str) = query.status {
        match status_str.parse::<ExecutionStatus>() {
            Ok(s) => Some(s),
            Err(_) => {
                return Err(ErrorResponse::new(
                    "validation_error",
                    format!("Invalid status value: {}", status_str),
                ));
            }
        }
    } else {
        None
    };

    // Build filter
    let filter = ExecutionFilter {
        job_id: query.job_id,
        status,
        trigger_source: query.trigger_source,
        limit: query.limit,
    };

    // Query executions
    let repo = ExecutionRepository::new(state.db_pool.clone());
    let executions = repo.find_with_filter(filter).await.map_err(|e| {
        tracing::error!(error = %e, "Failed to list executions");
        ErrorResponse::new("database_error", "Failed to retrieve executions")
    })?;

    tracing::info!(count = executions.len(), "Listed executions");
    Ok(Json(SuccessResponse::new(executions)))
}

/// Get execution details by ID (HTML for modal)
///
/// # Requirements
/// - 6.2: Display execution details
/// - 13.8: Load Job Context from MinIO to display step outputs
#[tracing::instrument(skip(state))]
pub async fn get_execution(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<axum::response::Html<String>, ErrorResponse> {
    use crate::templates::TEMPLATES;
    use tera::Context;

    let repo = ExecutionRepository::new(state.db_pool.clone());

    let execution = repo.find_by_id(id).await.map_err(|e| {
        tracing::error!(error = %e, execution_id = %id, "Failed to get execution");
        ErrorResponse::new("database_error", "Failed to retrieve execution")
    })?;

    let execution = match execution {
        Some(exec) => exec,
        None => {
            tracing::warn!(execution_id = %id, "Execution not found");
            return Err(ErrorResponse::new(
                "not_found",
                format!("Execution not found: {}", id),
            ));
        }
    };

    // Get job name
    let job_repo = common::db::repositories::JobRepository::new(state.db_pool.clone());
    let job = job_repo.find_by_id(execution.job_id).await.ok().flatten();
    let job_name = job
        .as_ref()
        .map(|j| j.name.clone())
        .unwrap_or_else(|| "Unknown".to_string());

    // Calculate duration
    let duration_seconds =
        if let (Some(start), Some(end)) = (execution.started_at, execution.completed_at) {
            Some((end - start).num_seconds())
        } else {
            None
        };

    // Load JobContext from storage to get step outputs
    // Requirements: 13.8 - Load Job Context to display detailed step results
    let step_outputs = {
        tracing::info!(
            execution_id = %id,
            job_id = %execution.job_id,
            "Loading JobContext from storage"
        );

        // Load context using Storage service (with Redis cache)
        match state
            .storage_service
            .load_context(execution.job_id, execution.id)
            .await
        {
            Ok(context) => {
                tracing::info!(
                    execution_id = %id,
                    steps_count = context.steps.len(),
                    "JobContext loaded successfully"
                );

                // Convert step outputs to JSON for template
                let mut steps = Vec::new();
                for (step_id, step_output) in context.steps.iter() {
                    let duration = (step_output.completed_at - step_output.started_at)
                        .num_milliseconds() as f64
                        / 1000.0;

                    steps.push(serde_json::json!({
                        "step_id": step_id,
                        "status": step_output.status,
                        "output": serde_json::to_string_pretty(&step_output.output).unwrap_or_else(|_| "{}".to_string()),
                        "started_at": step_output.started_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                        "completed_at": step_output.completed_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                        "duration_seconds": duration,
                    }));
                }

                Some(steps)
            }
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    execution_id = %id,
                    "Failed to load JobContext from MinIO"
                );
                None
            }
        }
    };

    // Parse result as JSON if possible for pretty display
    let result_display = execution.result.as_ref().map(|r| {
        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(r) {
            serde_json::to_string_pretty(&json_value).unwrap_or_else(|_| r.clone())
        } else {
            r.clone()
        }
    });

    // Prepare execution data for template
    let execution_data = serde_json::json!({
        "id": execution.id.to_string(),
        "job_id": execution.job_id.to_string(),
        "job_name": job_name,
        "status": execution.status.to_string(),
        "trigger_source": execution.trigger_source.to_string(),
        "attempt": execution.attempt,
        "current_step": execution.current_step,
        "started_at": execution.started_at.map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
        "completed_at": execution.completed_at.map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string()),
        "created_at": execution.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
        "duration_seconds": duration_seconds,
        "result": result_display,
        "error": execution.error,
        "idempotency_key": execution.idempotency_key,
        "step_outputs": step_outputs,
    });

    let mut context = Context::new();
    context.insert("execution", &execution_data);

    let html = TEMPLATES
        .render("_execution_details_modal_content.html", &context)
        .map_err(|e| {
            tracing::error!(error = %e, execution_id = %id, "Template rendering failed");
            ErrorResponse::new("template_error", "Failed to render execution details")
        })?;

    tracing::info!(execution_id = %id, "Retrieved execution details");
    Ok(axum::response::Html(html))
}

/// Query parameters for stop execution
#[derive(Debug, Deserialize)]
pub struct StopExecutionQuery {
    /// Force stop immediately (true) or graceful stop (false)
    #[serde(default)]
    pub force: bool,
}

/// Stop a running execution
///
/// This endpoint allows stopping a running job execution with two modes:
/// - Graceful stop (force=false): Wait for current step to complete, then stop
/// - Force stop (force=true): Terminate immediately without waiting
///
/// # Requirements
/// - New feature: Stop job execution with graceful and force modes
///
/// # Correctness Properties
/// - Only running executions can be stopped
/// - Graceful stop sets status to Cancelling, force stop sets to Cancelled
/// - Worker checks cancellation status before each step
#[tracing::instrument(skip(state))]
pub async fn stop_execution(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<StopExecutionQuery>,
) -> Result<Json<SuccessResponse<()>>, ErrorResponse> {
    let repo = ExecutionRepository::new(state.db_pool.clone());

    // Get execution
    let execution = repo.find_by_id(id).await.map_err(|e| {
        tracing::error!(error = %e, execution_id = %id, "Failed to get execution");
        ErrorResponse::new("database_error", "Failed to retrieve execution")
    })?;

    let mut execution = match execution {
        Some(exec) => exec,
        None => {
            tracing::warn!(execution_id = %id, "Execution not found");
            return Err(ErrorResponse::new(
                "not_found",
                format!("Execution not found: {}", id),
            ));
        }
    };

    // Check if execution is running
    if execution.status != ExecutionStatus::Running {
        return Err(ErrorResponse::new(
            "invalid_state",
            format!(
                "Cannot stop execution with status: {}. Only running executions can be stopped.",
                execution.status
            ),
        ));
    }

    // Update status based on force flag
    let new_status = if query.force {
        ExecutionStatus::Cancelled
    } else {
        ExecutionStatus::Cancelling
    };

    execution.status = new_status.clone();
    execution.completed_at = if query.force {
        Some(chrono::Utc::now())
    } else {
        None
    };
    execution.error = Some(format!(
        "Execution stopped by user ({})",
        if query.force { "force" } else { "graceful" }
    ));

    // Update execution in database
    repo.update(&execution).await.map_err(|e| {
        tracing::error!(error = %e, execution_id = %id, "Failed to update execution");
        ErrorResponse::new("database_error", "Failed to stop execution")
    })?;

    // Broadcast SSE event
    state.broadcast_event(SseEvent::ExecutionStatusChanged {
        execution_id: id,
        job_id: execution.job_id,
        status: new_status.to_string(),
    });

    tracing::info!(
        execution_id = %id,
        force = query.force,
        "Execution stop requested"
    );

    Ok(Json(SuccessResponse::new(())))
}
