use axum::{
    extract::{Path, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::handlers::{ErrorResponse, SuccessResponse};
use crate::state::{AppState, SseEvent};
use common::db::repositories::execution::ExecutionRepository;
use common::db::repositories::job::JobRepository;
use common::models::{Job, JobExecution, JobStep, Schedule, TriggerConfig};

/// Request to create a new job
#[derive(Debug, Deserialize)]
pub struct CreateJobRequest {
    pub name: String,
    pub description: Option<String>,
    pub schedule: Option<Schedule>,
    pub steps: Vec<JobStep>,
    pub triggers: Option<TriggerConfig>,
    pub timeout_seconds: Option<i32>,
    pub max_retries: Option<i32>,
    pub allow_concurrent: Option<bool>,
}

/// Request to update an existing job
#[derive(Debug, Deserialize)]
pub struct UpdateJobRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub schedule: Option<Schedule>,
    pub steps: Option<Vec<JobStep>>,
    pub triggers: Option<TriggerConfig>,
    pub timeout_seconds: Option<i32>,
    pub max_retries: Option<i32>,
    pub allow_concurrent: Option<bool>,
}

/// Job with statistics for listing
#[derive(Debug, Serialize)]
pub struct JobWithStats {
    #[serde(flatten)]
    pub job: Job,
    pub stats: Option<JobStatsResponse>,
    pub next_run_time: Option<DateTime<Utc>>,
    pub last_run_time: Option<DateTime<Utc>>,
    pub success_rate: Option<f64>,
}

/// Job statistics response
#[derive(Debug, Serialize)]
pub struct JobStatsResponse {
    pub total_executions: i64,
    pub successful_executions: i64,
    pub failed_executions: i64,
    pub last_execution_at: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub last_failure_at: Option<DateTime<Utc>>,
    pub consecutive_failures: i32,
}

/// Create a new job
///
/// Requirements: 6.1, 7.2 - Job creation and dynamic job addition
#[tracing::instrument(skip(state, req))]
pub async fn create_job(
    State(state): State<AppState>,
    Json(req): Json<CreateJobRequest>,
) -> Result<Json<SuccessResponse<Uuid>>, ErrorResponse> {
    let job_id = Uuid::new_v4();
    let now = Utc::now();

    // Get triggers or default
    let triggers = req.triggers.unwrap_or_default();

    // Create job definition JSON
    let job_definition = serde_json::json!({
        "id": job_id,
        "name": req.name,
        "description": req.description,
        "schedule": req.schedule,
        "steps": req.steps,
        "triggers": triggers,
        "enabled": true,
        "timeout_seconds": req.timeout_seconds.unwrap_or(300),
        "max_retries": req.max_retries.unwrap_or(10),
        "allow_concurrent": req.allow_concurrent.unwrap_or(false),
    });

    // Store job definition in PostgreSQL
    let definition_json = serde_json::to_string_pretty(&job_definition).map_err(|e| {
        ErrorResponse::new(
            "serialization_error",
            &format!("Failed to serialize job definition: {}", e),
        )
    })?;

    let definition_value: serde_json::Value =
        serde_json::from_str(&definition_json).map_err(|e| {
            ErrorResponse::new(
                "serialization_error",
                &format!("Failed to parse job definition: {}", e),
            )
        })?;

    // Create job record in database
    let job = Job {
        id: job_id,
        name: req.name.clone(),
        description: req.description.clone(),
        schedule: req.schedule,
        steps: req.steps,
        triggers: triggers.clone(),
        enabled: true,
        timeout_seconds: req.timeout_seconds.unwrap_or(300),
        max_retries: req.max_retries.unwrap_or(10),
        allow_concurrent: req.allow_concurrent.unwrap_or(false),
        definition: Some(definition_value),
        created_at: now,
        updated_at: now,
    };

    let repo = JobRepository::new(state.db_pool.clone());
    repo.create(&job).await.map_err(|e| {
        ErrorResponse::new("database_error", &format!("Failed to create job: {}", e))
    })?;

    // Broadcast SSE event
    state.broadcast_event(SseEvent::JobCreated {
        job_id,
        name: req.name,
    });

    tracing::info!(job_id = %job_id, "Job created successfully");
    Ok(Json(SuccessResponse::new(job_id)))
}

/// List all jobs with stats
///
/// Requirements: 6.1 - Display all jobs with current status, next run time, last run time, and success rate
#[tracing::instrument(skip(state))]
pub async fn list_jobs(
    State(state): State<AppState>,
) -> Result<Json<SuccessResponse<Vec<JobWithStats>>>, ErrorResponse> {
    let repo = JobRepository::new(state.db_pool.clone());
    let execution_repo = ExecutionRepository::new(state.db_pool.clone());

    // Get all jobs
    let jobs = repo.find_all().await.map_err(|e| {
        ErrorResponse::new("database_error", &format!("Failed to fetch jobs: {}", e))
    })?;

    // Build response with stats for each job
    let mut jobs_with_stats = Vec::new();

    for job in jobs {
        // Get job stats
        let stats = repo.get_stats(job.id).await.ok().flatten();

        // Get latest execution for last_run_time
        let latest_execution = execution_repo
            .find_latest_by_job_id(job.id)
            .await
            .ok()
            .flatten();
        let last_run_time = latest_execution.as_ref().and_then(|e| e.started_at);

        // Calculate success rate
        let success_rate = stats.as_ref().and_then(|s| {
            if s.total_executions > 0 {
                Some((s.successful_executions as f64 / s.total_executions as f64) * 100.0)
            } else {
                None
            }
        });

        // Calculate next run time (simplified - would need full schedule calculation)
        let next_run_time = if job.enabled && job.triggers.scheduled {
            // For now, return None - full implementation would calculate based on schedule
            // This would require loading the full job definition from MinIO and using the scheduler logic
            None
        } else {
            None
        };

        let stats_response = stats.map(|s| JobStatsResponse {
            total_executions: s.total_executions,
            successful_executions: s.successful_executions,
            failed_executions: s.failed_executions,
            last_execution_at: s.last_execution_at,
            last_success_at: s.last_success_at,
            last_failure_at: s.last_failure_at,
            consecutive_failures: s.consecutive_failures,
        });

        jobs_with_stats.push(JobWithStats {
            job,
            stats: stats_response,
            next_run_time,
            last_run_time,
            success_rate,
        });
    }

    tracing::debug!(count = jobs_with_stats.len(), "Listed jobs with stats");
    Ok(Json(SuccessResponse::new(jobs_with_stats)))
}

/// Get job details by ID
///
/// Requirements: 6.1 - Get job details
#[tracing::instrument(skip(state))]
pub async fn get_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SuccessResponse<Job>>, ErrorResponse> {
    let repo = JobRepository::new(state.db_pool.clone());

    let job = repo
        .find_by_id(id)
        .await
        .map_err(|e| ErrorResponse::new("database_error", &format!("Failed to fetch job: {}", e)))?
        .ok_or_else(|| ErrorResponse::new("not_found", &format!("Job not found: {}", id)))?;

    // Load full job definition from PostgreSQL to get schedule and steps
    let job_definition: serde_json::Value = if let Some(def) = &job.definition {
        def.clone()
    } else {
        // Fallback: try to load from storage service
        let definition_json = state
            .storage_service
            .load_job_definition(job.id)
            .await
            .map_err(|e| {
                ErrorResponse::new(
                    "storage_error",
                    &format!("Failed to load job definition: {}", e),
                )
            })?;

        serde_json::from_str(&definition_json).map_err(|e| {
            ErrorResponse::new(
                "deserialization_error",
                &format!("Failed to parse job definition: {}", e),
            )
        })?
    };

    // Reconstruct full job with schedule and steps
    let mut full_job = job;
    if let Some(schedule) = job_definition.get("schedule") {
        full_job.schedule = serde_json::from_value(schedule.clone()).ok();
    }
    if let Some(steps) = job_definition.get("steps") {
        full_job.steps = serde_json::from_value(steps.clone()).unwrap_or_default();
    }
    if let Some(triggers) = job_definition.get("triggers") {
        full_job.triggers = serde_json::from_value(triggers.clone()).unwrap_or_default();
    }

    tracing::debug!(job_id = %id, "Retrieved job details");
    Ok(Json(SuccessResponse::new(full_job)))
}

/// Update a job
///
/// Requirements: 6.1, 7.3 - Update job and apply changes to future executions
#[tracing::instrument(skip(state, req))]
pub async fn update_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateJobRequest>,
) -> Result<Json<SuccessResponse<Job>>, ErrorResponse> {
    let repo = JobRepository::new(state.db_pool.clone());

    // Get existing job
    let mut job = repo
        .find_by_id(id)
        .await
        .map_err(|e| ErrorResponse::new("database_error", &format!("Failed to fetch job: {}", e)))?
        .ok_or_else(|| ErrorResponse::new("not_found", &format!("Job not found: {}", id)))?;

    // Load existing job definition from PostgreSQL
    let mut job_definition: serde_json::Value = if let Some(def) = &job.definition {
        def.clone()
    } else {
        // Fallback: try to load from storage service
        let definition_json = state
            .storage_service
            .load_job_definition(job.id)
            .await
            .map_err(|e| {
                ErrorResponse::new(
                    "storage_error",
                    &format!("Failed to load job definition: {}", e),
                )
            })?;

        serde_json::from_str(&definition_json).map_err(|e| {
            ErrorResponse::new(
                "deserialization_error",
                &format!("Failed to parse job definition: {}", e),
            )
        })?
    };

    // Update fields if provided
    if let Some(name) = req.name {
        job.name = name.clone();
        job_definition["name"] = serde_json::json!(name);
    }

    if let Some(description) = req.description {
        job.description = Some(description.clone());
        job_definition["description"] = serde_json::json!(description);
    }

    if let Some(schedule) = req.schedule {
        job.schedule = Some(schedule.clone());
        job_definition["schedule"] = serde_json::to_value(&schedule).map_err(|e| {
            ErrorResponse::new(
                "serialization_error",
                &format!("Failed to serialize schedule: {}", e),
            )
        })?;
    }

    if let Some(steps) = req.steps {
        job.steps = steps.clone();
        job_definition["steps"] = serde_json::to_value(&steps).map_err(|e| {
            ErrorResponse::new(
                "serialization_error",
                &format!("Failed to serialize steps: {}", e),
            )
        })?;
    }

    if let Some(triggers) = req.triggers {
        job.triggers = triggers.clone();
        job_definition["triggers"] = serde_json::to_value(&triggers).map_err(|e| {
            ErrorResponse::new(
                "serialization_error",
                &format!("Failed to serialize triggers: {}", e),
            )
        })?;
    }

    if let Some(timeout_seconds) = req.timeout_seconds {
        job.timeout_seconds = timeout_seconds;
        job_definition["timeout_seconds"] = serde_json::json!(timeout_seconds);
    }

    if let Some(max_retries) = req.max_retries {
        job.max_retries = max_retries;
        job_definition["max_retries"] = serde_json::json!(max_retries);
    }

    if let Some(allow_concurrent) = req.allow_concurrent {
        job.allow_concurrent = allow_concurrent;
        job_definition["allow_concurrent"] = serde_json::json!(allow_concurrent);
    }

    // Ensure enabled field exists (for backwards compatibility with old jobs)
    if job_definition.get("enabled").is_none() {
        job_definition["enabled"] = serde_json::json!(job.enabled);
    }

    job.updated_at = Utc::now();
    job.definition = Some(job_definition.clone());

    // Update job record in database (includes definition)
    repo.update(&job).await.map_err(|e| {
        ErrorResponse::new("database_error", &format!("Failed to update job: {}", e))
    })?;

    // Broadcast SSE event
    state.broadcast_event(SseEvent::JobStatusChanged {
        job_id: id,
        status: "updated".to_string(),
    });

    tracing::info!(job_id = %id, "Job updated successfully");
    Ok(Json(SuccessResponse::new(job)))
}

/// Delete a job
///
/// Requirements: 6.1, 7.4 - Delete job and stop scheduling it
#[tracing::instrument(skip(state))]
pub async fn delete_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SuccessResponse<()>>, ErrorResponse> {
    let repo = JobRepository::new(state.db_pool.clone());

    // Get job to verify it exists
    let _job = repo
        .find_by_id(id)
        .await
        .map_err(|e| ErrorResponse::new("database_error", &format!("Failed to fetch job: {}", e)))?
        .ok_or_else(|| ErrorResponse::new("not_found", &format!("Job not found: {}", id)))?;

    // Delete job from database (this will cascade delete executions and stats)
    // Job definition is stored in PostgreSQL, so it will be deleted automatically
    repo.delete(id).await.map_err(|e| {
        ErrorResponse::new("database_error", &format!("Failed to delete job: {}", e))
    })?;

    // Broadcast SSE event
    state.broadcast_event(SseEvent::JobDeleted { job_id: id });

    tracing::info!(job_id = %id, "Job deleted successfully");
    Ok(Json(SuccessResponse::new(())))
}

/// Manually trigger a job
///
/// Requirements:
/// - 6.4: Immediately queue job for execution
/// - 17.9: Allow concurrent execution if configured
/// - 17.10: Reject new triggers if concurrent execution not allowed
#[tracing::instrument(skip(state))]
pub async fn trigger_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SuccessResponse<Uuid>>, ErrorResponse> {
    let repo = JobRepository::new(state.db_pool.clone());
    let execution_repo = ExecutionRepository::new(state.db_pool.clone());

    // Get job to verify it exists
    let job = repo
        .find_by_id(id)
        .await
        .map_err(|e| ErrorResponse::new("database_error", &format!("Failed to fetch job: {}", e)))?
        .ok_or_else(|| ErrorResponse::new("not_found", &format!("Job not found: {}", id)))?;

    // Check if concurrent execution is allowed
    // Requirement 17.10: Reject if concurrent execution not allowed and job is running
    if !job.allow_concurrent {
        let has_running = execution_repo
            .has_running_execution(id)
            .await
            .map_err(|e| {
                ErrorResponse::new(
                    "database_error",
                    &format!("Failed to check for running executions: {}", e),
                )
            })?;

        if has_running {
            return Err(ErrorResponse::new(
                "concurrent_execution_not_allowed",
                "Job is already running and concurrent execution is not allowed",
            ));
        }
    }

    // Create execution record using factory method
    // TODO: Get user_id from JWT claims in middleware
    let execution = JobExecution::new_manual(id, "system".to_string());
    let execution_id = execution.id;

    execution_repo.create(&execution).await.map_err(|e| {
        ErrorResponse::new(
            "database_error",
            &format!("Failed to create execution: {}", e),
        )
    })?;

    // Publish job to NATS queue
    // Publish directly using JetStream
    let jetstream = async_nats::jetstream::new(state.nats_client.clone());
    let subject = format!("jobs.{}", id);

    // Create message
    let message = common::queue::publisher::JobMessage::from(&execution);
    let payload = serde_json::to_vec(&message).map_err(|e| {
        ErrorResponse::new(
            "serialization_error",
            &format!("Failed to serialize job message: {}", e),
        )
    })?;

    // Create headers for deduplication
    let mut headers = async_nats::HeaderMap::new();
    headers.insert("Nats-Msg-Id", execution.idempotency_key.as_str());
    headers.insert("Job-Id", id.to_string().as_str());
    headers.insert("Execution-Id", execution_id.to_string().as_str());

    jetstream
        .publish_with_headers(subject, headers, payload.into())
        .await
        .map_err(|e| {
            ErrorResponse::new(
                "queue_error",
                &format!("Failed to publish job to queue: {}", e),
            )
        })?
        .await
        .map_err(|e| {
            ErrorResponse::new(
                "queue_error",
                &format!("Failed to get publish acknowledgment: {}", e),
            )
        })?;

    // Broadcast SSE event
    state.broadcast_event(SseEvent::ExecutionStatusChanged {
        execution_id,
        job_id: id,
        status: "pending".to_string(),
    });

    tracing::info!(job_id = %id, execution_id = %execution_id, "Job manually triggered");
    Ok(Json(SuccessResponse::new(execution_id)))
}

/// Enable a job
///
/// Requirements: 6.6 - Resume scheduling executions
#[tracing::instrument(skip(state))]
pub async fn enable_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SuccessResponse<()>>, ErrorResponse> {
    let repo = JobRepository::new(state.db_pool.clone());

    // Get the job to update its definition
    let mut job = repo
        .find_by_id(id)
        .await
        .map_err(|e| ErrorResponse::new("database_error", &format!("Failed to fetch job: {}", e)))?
        .ok_or_else(|| ErrorResponse::new("not_found", &format!("Job not found: {}", id)))?;

    // Update enabled flag
    job.enabled = true;

    // Update definition JSON if it exists
    if let Some(mut definition) = job.definition {
        definition["enabled"] = serde_json::json!(true);
        job.definition = Some(definition);
    }

    // Save the updated job
    repo.update(&job).await.map_err(|e| {
        ErrorResponse::new("database_error", &format!("Failed to enable job: {}", e))
    })?;

    // Broadcast SSE event
    state.broadcast_event(SseEvent::JobStatusChanged {
        job_id: id,
        status: "enabled".to_string(),
    });

    tracing::info!(job_id = %id, "Job enabled");
    Ok(Json(SuccessResponse::new(())))
}

/// Disable a job
///
/// Requirements: 6.5 - Stop scheduling future executions
#[tracing::instrument(skip(state))]
pub async fn disable_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SuccessResponse<()>>, ErrorResponse> {
    let repo = JobRepository::new(state.db_pool.clone());

    // Get the job to update its definition
    let mut job = repo
        .find_by_id(id)
        .await
        .map_err(|e| ErrorResponse::new("database_error", &format!("Failed to fetch job: {}", e)))?
        .ok_or_else(|| ErrorResponse::new("not_found", &format!("Job not found: {}", id)))?;

    // Update enabled flag
    job.enabled = false;

    // Update definition JSON if it exists
    if let Some(mut definition) = job.definition {
        definition["enabled"] = serde_json::json!(false);
        job.definition = Some(definition);
    }

    // Save the updated job
    repo.update(&job).await.map_err(|e| {
        ErrorResponse::new("database_error", &format!("Failed to disable job: {}", e))
    })?;

    // Broadcast SSE event
    state.broadcast_event(SseEvent::JobStatusChanged {
        job_id: id,
        status: "disabled".to_string(),
    });

    tracing::info!(job_id = %id, "Job disabled");
    Ok(Json(SuccessResponse::new(())))
}
