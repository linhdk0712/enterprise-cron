use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::handlers::{ErrorResponse, SuccessResponse};
use crate::state::AppState;
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

/// Get execution details by ID
///
/// # Requirements
/// - 6.2: Display execution details
#[tracing::instrument(skip(state))]
pub async fn get_execution(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SuccessResponse<JobExecution>>, ErrorResponse> {
    let repo = ExecutionRepository::new(state.db_pool.clone());

    let execution = repo.find_by_id(id).await.map_err(|e| {
        tracing::error!(error = %e, execution_id = %id, "Failed to get execution");
        ErrorResponse::new("database_error", "Failed to retrieve execution")
    })?;

    match execution {
        Some(exec) => {
            tracing::info!(execution_id = %id, "Retrieved execution details");
            Ok(Json(SuccessResponse::new(exec)))
        }
        None => {
            tracing::warn!(execution_id = %id, "Execution not found");
            Err(ErrorResponse::new(
                "not_found",
                format!("Execution not found: {}", id),
            ))
        }
    }
}
