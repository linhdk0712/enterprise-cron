// Shared utilities for dashboard handlers
// RECC 2025: Descriptive file name, single responsibility
// Purpose: Eliminate code duplication across dashboard handlers

use axum::http::HeaderMap;
use common::models::{Job, JobStep, JobType, Schedule};
use common::storage::StorageService;
use tera::Context;
use uuid::Uuid;

use crate::handlers::ErrorResponse;

/// Load full job definition from MinIO with Redis cache fallback
/// Returns None if path is empty or loading fails
///
/// This function consolidates the MinIO loading logic used in:
/// - job_details.rs (load_job_definition)
/// - jobs_list.rs (inline loading in loop)
#[tracing::instrument(skip(storage_service))]
pub async fn load_job_from_storage(
    storage_service: &dyn StorageService,
    job_id: Uuid,
) -> Option<Job> {
    match storage_service.load_job_definition(job_id).await {
        Ok(json_str) => match serde_json::from_str::<Job>(&json_str) {
            Ok(full_job) => Some(full_job),
            Err(e) => {
                tracing::warn!(
                    job_id = %job_id,
                    error = %e,
                    "Failed to parse job definition from MinIO"
                );
                None
            }
        },
        Err(e) => {
            tracing::warn!(
                job_id = %job_id,
                error = %e,
                "Failed to load job definition from storage (Redis/MinIO)"
            );
            None
        }
    }
}

/// Extract schedule type as display string
///
/// Consolidates schedule type extraction from:
/// - job_details.rs (inline match)
/// - jobs_list.rs (get_schedule_type function)
pub fn get_schedule_type_str(schedule: &Option<Schedule>) -> Option<&'static str> {
    schedule.as_ref().map(|s| match s {
        Schedule::Cron { .. } => "Cron",
        Schedule::FixedDelay { .. } => "FixedDelay",
        Schedule::FixedRate { .. } => "FixedRate",
        Schedule::OneTime { .. } => "OneTime",
    })
}

/// Get human-readable next run time from schedule
/// Returns None if job is disabled
///
/// Consolidates logic from jobs_list.rs (get_next_run_time)
pub fn get_next_run_display(schedule: &Option<Schedule>, enabled: bool) -> Option<String> {
    if !enabled {
        return None;
    }

    schedule.as_ref().and_then(|s| match s {
        Schedule::Cron { .. } => Some("Scheduled".to_string()),
        Schedule::FixedDelay { delay_seconds } => Some(format!("Every {}s", delay_seconds)),
        Schedule::FixedRate { interval_seconds } => Some(format!("Every {}s", interval_seconds)),
        Schedule::OneTime { execute_at } => {
            Some(execute_at.format("%Y-%m-%d %H:%M:%S").to_string())
        }
    })
}

/// Extract job type from first step
///
/// Consolidates logic from jobs_list.rs (get_job_type)
pub fn get_job_type_str(steps: &[JobStep]) -> Option<&'static str> {
    steps.first().map(|step| match &step.step_type {
        JobType::HttpRequest { .. } => "HTTP",
        JobType::DatabaseQuery { .. } => "Database",
        JobType::Sftp { .. } => "SFTP",
        JobType::FileProcessing { .. } => "File",
    })
}

/// Check if request is HTMX and setup context accordingly
/// Returns (is_htmx, content_template, full_template)
///
/// Consolidates HTMX detection pattern from all dashboard handlers
pub fn setup_htmx_context(
    context: &mut Context,
    headers: &HeaderMap,
    content_template: &'static str,
    full_template: &'static str,
) -> &'static str {
    let is_htmx = headers.get("HX-Request").is_some();
    context.insert("is_htmx", &is_htmx);

    if is_htmx {
        content_template
    } else {
        full_template
    }
}

/// Convert database error to ErrorResponse
/// Consolidates repeated error mapping pattern
pub fn db_error(e: impl std::fmt::Display) -> ErrorResponse {
    ErrorResponse::new("database_error", &format!("Database error: {}", e))
}

/// Convert template error to ErrorResponse
/// Consolidates repeated error mapping pattern
pub fn _template_error(template_name: &str, e: impl std::fmt::Display) -> ErrorResponse {
    tracing::error!(error = %e, template = template_name, "Template rendering failed");
    ErrorResponse::new(
        "template_error",
        &format!("Failed to render '{}': {}", template_name, e),
    )
}

/// Calculate pagination metadata
/// Returns (page, total_pages)
pub fn calculate_pagination(offset: i64, limit: i64, total_count: i64) -> (i64, i64) {
    let page = (offset / limit) + 1;
    let total_pages = ((total_count as f64) / (limit as f64)).ceil() as i64;
    (page, total_pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_schedule_type_extraction() {
        let cron_schedule = Some(Schedule::Cron {
            expression: "0 0 * * *".to_string(),
            timezone: chrono_tz::Asia::Ho_Chi_Minh,
            next_run: None,
        });
        assert_eq!(get_schedule_type_str(&cron_schedule), Some("Cron"));

        let fixed_delay = Some(Schedule::FixedDelay {
            delay_seconds: 60,
            last_completed_at: None,
        });
        assert_eq!(get_schedule_type_str(&fixed_delay), Some("FixedDelay"));

        assert_eq!(get_schedule_type_str(&None), None);
    }

    #[test]
    fn test_next_run_display_disabled() {
        let schedule = Some(Schedule::FixedRate {
            interval_seconds: 300,
            last_triggered_at: None,
        });
        assert_eq!(get_next_run_display(&schedule, false), None);
    }

    #[test]
    fn test_next_run_display_enabled() {
        let schedule = Some(Schedule::FixedRate {
            interval_seconds: 300,
            last_triggered_at: None,
        });
        assert_eq!(
            get_next_run_display(&schedule, true),
            Some("Every 300s".to_string())
        );
    }

    #[test]
    fn test_pagination_calculation() {
        assert_eq!(calculate_pagination(0, 20, 100), (1, 5));
        assert_eq!(calculate_pagination(20, 20, 100), (2, 5));
        assert_eq!(calculate_pagination(0, 20, 95), (1, 5));
        assert_eq!(calculate_pagination(0, 20, 0), (1, 0));
    }
}
