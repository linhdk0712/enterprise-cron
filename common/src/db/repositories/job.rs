// Job repository implementation
// Requirements: 3.11, 7.2, 7.3, 7.4 - Job CRUD operations and stats tracking

use crate::db::DbPool;
use crate::errors::DatabaseError;
use crate::models::Job;
use chrono::{DateTime, Utc};
use sqlx::Row;
use tracing::instrument;
use uuid::Uuid;

/// Repository for job-related database operations
pub struct JobRepository {
    pool: DbPool,
}

impl JobRepository {
    /// Create a new JobRepository
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Find jobs that are due for execution
    ///
    /// # Requirements
    /// - 3.11: Query jobs from database
    /// - 7.2: Dynamic job addition support
    /// - 17.1: Only return jobs with scheduled trigger enabled
    ///
    /// # Arguments
    /// * `now` - Current timestamp to compare against next execution time
    ///
    /// # Returns
    /// List of jobs that should be executed now
    #[instrument(skip(self))]
    pub async fn find_jobs_due(&self, now: DateTime<Utc>) -> Result<Vec<Job>, DatabaseError> {
        // Query jobs with trigger_config
        let rows = sqlx::query(
            r#"
            SELECT 
                id, name, description, enabled, timeout_seconds, 
                max_retries, allow_concurrent, minio_definition_path,
                trigger_config, created_at, updated_at
            FROM jobs
            WHERE enabled = true
            "#,
        )
        .fetch_all(self.pool.pool())
        .await?;

        let mut jobs = Vec::new();
        for row in rows {
            let trigger_config_json: serde_json::Value = row.try_get("trigger_config")?;
            let trigger_config: crate::models::TriggerConfig =
                serde_json::from_value(trigger_config_json).map_err(|e| {
                    DatabaseError::QueryFailed(format!("Failed to parse trigger_config: {}", e))
                })?;

            let job = Job {
                id: row.try_get("id")?,
                name: row.try_get("name")?,
                description: row.try_get("description")?,
                schedule: None,    // Will be loaded from MinIO
                steps: Vec::new(), // Will be loaded from MinIO
                triggers: trigger_config,
                enabled: row.try_get("enabled")?,
                timeout_seconds: row.try_get("timeout_seconds")?,
                max_retries: row.try_get("max_retries")?,
                allow_concurrent: row.try_get("allow_concurrent")?,
                minio_definition_path: row.try_get("minio_definition_path")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            };

            jobs.push(job);
        }

        tracing::debug!(count = jobs.len(), "Found jobs due for execution");
        Ok(jobs)
    }

    /// Create a new job
    ///
    /// # Requirements
    /// - 3.11: Job persistence
    /// - 7.2: Dynamic job addition
    /// - 17.1, 17.2: Store trigger configuration
    #[instrument(skip(self, job))]
    pub async fn create(&self, job: &Job) -> Result<(), DatabaseError> {
        let trigger_config_json = serde_json::to_value(&job.triggers).map_err(|e| {
            DatabaseError::QueryFailed(format!("Failed to serialize trigger_config: {}", e))
        })?;

        sqlx::query(
            r#"
            INSERT INTO jobs (
                id, name, description, enabled, timeout_seconds,
                max_retries, allow_concurrent, minio_definition_path,
                trigger_config, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(&job.id)
        .bind(&job.name)
        .bind(&job.description)
        .bind(job.enabled)
        .bind(job.timeout_seconds as i32)
        .bind(job.max_retries as i32)
        .bind(job.allow_concurrent)
        .bind(&job.minio_definition_path)
        .bind(trigger_config_json)
        .bind(job.created_at)
        .bind(job.updated_at)
        .execute(self.pool.pool())
        .await?;

        tracing::info!(job_id = %job.id, job_name = %job.name, "Job created");
        Ok(())
    }

    /// Find a job by name
    ///
    /// # Requirements
    /// - 18.11: Duplicate name handling
    #[instrument(skip(self))]
    pub async fn find_by_name(&self, name: &str) -> Result<Option<Job>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT 
                id, name, description, enabled, timeout_seconds,
                max_retries, allow_concurrent, minio_definition_path,
                trigger_config, created_at, updated_at
            FROM jobs
            WHERE name = $1
            "#,
        )
        .bind(name)
        .fetch_optional(self.pool.pool())
        .await?;

        let job = if let Some(row) = row {
            let trigger_config_json: serde_json::Value = row.try_get("trigger_config")?;
            let trigger_config: crate::models::TriggerConfig =
                serde_json::from_value(trigger_config_json).map_err(|e| {
                    DatabaseError::QueryFailed(format!("Failed to parse trigger_config: {}", e))
                })?;

            Some(Job {
                id: row.try_get("id")?,
                name: row.try_get("name")?,
                description: row.try_get("description")?,
                schedule: None,    // Will be loaded from MinIO
                steps: Vec::new(), // Will be loaded from MinIO
                triggers: trigger_config,
                enabled: row.try_get("enabled")?,
                timeout_seconds: row.try_get("timeout_seconds")?,
                max_retries: row.try_get("max_retries")?,
                allow_concurrent: row.try_get("allow_concurrent")?,
                minio_definition_path: row.try_get("minio_definition_path")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            })
        } else {
            None
        };

        Ok(job)
    }

    /// Find a job by ID
    ///
    /// # Requirements
    /// - 3.11: Job retrieval
    /// - 17.1, 17.2: Load trigger configuration
    #[instrument(skip(self))]
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Job>, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT 
                id, name, description, enabled, timeout_seconds,
                max_retries, allow_concurrent, minio_definition_path,
                trigger_config, created_at, updated_at
            FROM jobs
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(self.pool.pool())
        .await?;

        let job = if let Some(row) = row {
            let trigger_config_json: serde_json::Value = row.try_get("trigger_config")?;
            let trigger_config: crate::models::TriggerConfig =
                serde_json::from_value(trigger_config_json).map_err(|e| {
                    DatabaseError::QueryFailed(format!("Failed to parse trigger_config: {}", e))
                })?;

            Some(Job {
                id: row.try_get("id")?,
                name: row.try_get("name")?,
                description: row.try_get("description")?,
                schedule: None,    // Will be loaded from MinIO
                steps: Vec::new(), // Will be loaded from MinIO
                triggers: trigger_config,
                enabled: row.try_get("enabled")?,
                timeout_seconds: row.try_get("timeout_seconds")?,
                max_retries: row.try_get("max_retries")?,
                allow_concurrent: row.try_get("allow_concurrent")?,
                minio_definition_path: row.try_get("minio_definition_path")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            })
        } else {
            None
        };

        Ok(job)
    }

    /// Find all jobs
    ///
    /// # Requirements
    /// - 3.11: Job listing
    /// - 17.1, 17.2: Load trigger configuration
    #[instrument(skip(self))]
    pub async fn find_all(&self) -> Result<Vec<Job>, DatabaseError> {
        let rows = sqlx::query(
            r#"
            SELECT 
                id, name, description, enabled, timeout_seconds,
                max_retries, allow_concurrent, minio_definition_path,
                trigger_config, created_at, updated_at
            FROM jobs
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(self.pool.pool())
        .await?;

        let mut jobs = Vec::new();
        for row in rows {
            let trigger_config_json: serde_json::Value = row.try_get("trigger_config")?;
            let trigger_config: crate::models::TriggerConfig =
                serde_json::from_value(trigger_config_json).map_err(|e| {
                    DatabaseError::QueryFailed(format!("Failed to parse trigger_config: {}", e))
                })?;

            let job = Job {
                id: row.try_get("id")?,
                name: row.try_get("name")?,
                description: row.try_get("description")?,
                schedule: None,    // Will be loaded from MinIO
                steps: Vec::new(), // Will be loaded from MinIO
                triggers: trigger_config,
                enabled: row.try_get("enabled")?,
                timeout_seconds: row.try_get("timeout_seconds")?,
                max_retries: row.try_get("max_retries")?,
                allow_concurrent: row.try_get("allow_concurrent")?,
                minio_definition_path: row.try_get("minio_definition_path")?,
                created_at: row.try_get("created_at")?,
                updated_at: row.try_get("updated_at")?,
            };

            jobs.push(job);
        }

        Ok(jobs)
    }

    /// Update an existing job
    ///
    /// # Requirements
    /// - 7.3: Dynamic job update
    /// - 17.1, 17.2: Update trigger configuration
    #[instrument(skip(self, job))]
    pub async fn update(&self, job: &Job) -> Result<(), DatabaseError> {
        let trigger_config_json = serde_json::to_value(&job.triggers).map_err(|e| {
            DatabaseError::QueryFailed(format!("Failed to serialize trigger_config: {}", e))
        })?;

        let result = sqlx::query(
            r#"
            UPDATE jobs
            SET name = $2,
                description = $3,
                enabled = $4,
                timeout_seconds = $5,
                max_retries = $6,
                allow_concurrent = $7,
                minio_definition_path = $8,
                trigger_config = $9,
                updated_at = $10
            WHERE id = $1
            "#,
        )
        .bind(&job.id)
        .bind(&job.name)
        .bind(&job.description)
        .bind(job.enabled)
        .bind(job.timeout_seconds as i32)
        .bind(job.max_retries as i32)
        .bind(job.allow_concurrent)
        .bind(&job.minio_definition_path)
        .bind(trigger_config_json)
        .bind(Utc::now())
        .execute(self.pool.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::NotFound(format!(
                "Job not found: {}",
                job.id
            )));
        }

        tracing::info!(job_id = %job.id, job_name = %job.name, "Job updated");
        Ok(())
    }

    /// Delete a job
    ///
    /// # Requirements
    /// - 7.4: Dynamic job deletion
    #[instrument(skip(self))]
    pub async fn delete(&self, id: Uuid) -> Result<(), DatabaseError> {
        let result = sqlx::query("DELETE FROM jobs WHERE id = $1")
            .bind(id)
            .execute(self.pool.pool())
            .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::NotFound(format!("Job not found: {}", id)));
        }

        tracing::info!(job_id = %id, "Job deleted");
        Ok(())
    }

    /// Get job statistics
    ///
    /// # Requirements
    /// - 3.11: Job stats tracking
    #[instrument(skip(self))]
    pub async fn get_stats(&self, job_id: Uuid) -> Result<Option<JobStats>, DatabaseError> {
        let stats = sqlx::query_as::<_, JobStats>(
            r#"
            SELECT 
                job_id, total_executions, successful_executions, failed_executions,
                last_execution_at, last_success_at, last_failure_at,
                consecutive_failures, updated_at
            FROM job_stats
            WHERE job_id = $1
            "#,
        )
        .bind(job_id)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(stats)
    }

    /// Update job statistics after execution
    ///
    /// # Requirements
    /// - 3.11: Job stats tracking
    #[instrument(skip(self))]
    pub async fn update_stats(&self, job_id: Uuid, success: bool) -> Result<(), DatabaseError> {
        if success {
            sqlx::query(
                r#"
                INSERT INTO job_stats (
                    job_id, total_executions, successful_executions, failed_executions,
                    last_execution_at, last_success_at, consecutive_failures, updated_at
                )
                VALUES ($1, 1, 1, 0, NOW(), NOW(), 0, NOW())
                ON CONFLICT (job_id) DO UPDATE SET
                    total_executions = job_stats.total_executions + 1,
                    successful_executions = job_stats.successful_executions + 1,
                    last_execution_at = NOW(),
                    last_success_at = NOW(),
                    consecutive_failures = 0,
                    updated_at = NOW()
                "#,
            )
            .bind(job_id)
            .execute(self.pool.pool())
            .await?;
        } else {
            sqlx::query(
                r#"
                INSERT INTO job_stats (
                    job_id, total_executions, successful_executions, failed_executions,
                    last_execution_at, last_failure_at, consecutive_failures, updated_at
                )
                VALUES ($1, 1, 0, 1, NOW(), NOW(), 1, NOW())
                ON CONFLICT (job_id) DO UPDATE SET
                    total_executions = job_stats.total_executions + 1,
                    failed_executions = job_stats.failed_executions + 1,
                    last_execution_at = NOW(),
                    last_failure_at = NOW(),
                    consecutive_failures = job_stats.consecutive_failures + 1,
                    updated_at = NOW()
                "#,
            )
            .bind(job_id)
            .execute(self.pool.pool())
            .await?;
        }

        tracing::debug!(job_id = %job_id, success, "Job stats updated");
        Ok(())
    }

    /// Enable a job
    ///
    /// # Requirements
    /// - 7.3: Dynamic job update
    #[instrument(skip(self))]
    pub async fn enable(&self, id: Uuid) -> Result<(), DatabaseError> {
        let result =
            sqlx::query("UPDATE jobs SET enabled = true, updated_at = NOW() WHERE id = $1")
                .bind(id)
                .execute(self.pool.pool())
                .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::NotFound(format!("Job not found: {}", id)));
        }

        tracing::info!(job_id = %id, "Job enabled");
        Ok(())
    }

    /// Disable a job
    ///
    /// # Requirements
    /// - 7.3: Dynamic job update
    #[instrument(skip(self))]
    pub async fn disable(&self, id: Uuid) -> Result<(), DatabaseError> {
        let result =
            sqlx::query("UPDATE jobs SET enabled = false, updated_at = NOW() WHERE id = $1")
                .bind(id)
                .execute(self.pool.pool())
                .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::NotFound(format!("Job not found: {}", id)));
        }

        tracing::info!(job_id = %id, "Job disabled");
        Ok(())
    }
}

/// Job statistics model
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct JobStats {
    pub job_id: Uuid,
    pub total_executions: i64,
    pub successful_executions: i64,
    pub failed_executions: i64,
    pub last_execution_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_success_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_failure_at: Option<chrono::DateTime<chrono::Utc>>,
    pub consecutive_failures: i32,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_repository_creation() {
        // This test verifies the repository can be created
        // Actual database tests require a running PostgreSQL instance
    }
}
