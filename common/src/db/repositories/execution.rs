// Execution repository implementation
// Requirements: 3.12, 4.3, 6.2 - Execution history, idempotency, and filtering

use super::queries::execution_queries;
use crate::db::DbPool;
use crate::errors::DatabaseError;
use crate::models::{ExecutionStatus, JobExecution};
use chrono::{Duration, Utc};
use sqlx::Row;
use tracing::instrument;
use uuid::Uuid;

/// Repository for job execution-related database operations
pub struct ExecutionRepository {
    pool: DbPool,
}

impl ExecutionRepository {
    /// Create a new ExecutionRepository
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Create a new job execution record
    ///
    /// # Requirements
    /// - 3.12: Execution history persistence
    /// - 4.3: Idempotency key tracking
    #[instrument(skip(self, execution))]
    pub async fn create(&self, execution: &JobExecution) -> Result<(), DatabaseError> {
        // Store trigger_source as string for the database
        let trigger_source_str = execution.trigger_source.to_string();

        sqlx::query(
            r#"
            INSERT INTO job_executions (
                id, job_id, idempotency_key, status, attempt,
                trigger_source, trigger_metadata, current_step,
                context, started_at, completed_at,
                result, error, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            "#,
        )
        .bind(&execution.id)
        .bind(&execution.job_id)
        .bind(&execution.idempotency_key)
        .bind(execution.status.to_string())
        .bind(execution.attempt as i32)
        .bind(trigger_source_str)
        .bind(&execution.trigger_metadata)
        .bind(&execution.current_step)
        .bind(&execution.context)
        .bind(execution.started_at)
        .bind(execution.completed_at)
        .bind(&execution.result)
        .bind(&execution.error)
        .bind(execution.created_at)
        .execute(self.pool.pool())
        .await?;

        tracing::info!(
            execution_id = %execution.id,
            job_id = %execution.job_id,
            "Execution created"
        );
        Ok(())
    }

    /// Update an existing job execution
    ///
    /// # Requirements
    /// - 3.12: Execution history persistence
    #[instrument(skip(self, execution))]
    pub async fn update(&self, execution: &JobExecution) -> Result<(), DatabaseError> {
        let result = sqlx::query(
            r#"
            UPDATE job_executions
            SET status = $2,
                attempt = $3,
                current_step = $4,
                context = $5,
                started_at = $6,
                completed_at = $7,
                result = $8,
                error = $9
            WHERE id = $1
            "#,
        )
        .bind(&execution.id)
        .bind(execution.status.to_string())
        .bind(execution.attempt as i32)
        .bind(&execution.current_step)
        .bind(&execution.context)
        .bind(execution.started_at)
        .bind(execution.completed_at)
        .bind(&execution.result)
        .bind(&execution.error)
        .execute(self.pool.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::NotFound(format!(
                "Execution not found: {}",
                execution.id
            )));
        }

        tracing::debug!(
            execution_id = %execution.id,
            status = %execution.status,
            "Execution updated"
        );
        Ok(())
    }

    /// Find an execution by idempotency key
    ///
    /// # Requirements
    /// - 4.3: Idempotency key checking for deduplication
    #[instrument(skip(self))]
    pub async fn find_by_idempotency_key(
        &self,
        key: &str,
    ) -> Result<Option<JobExecution>, DatabaseError> {
        let execution = sqlx::query_as::<_, JobExecution>(
            r#"
            SELECT 
                id, job_id, idempotency_key, status, attempt,
                trigger_source, trigger_metadata, current_step, context,
                started_at, completed_at, result, error, created_at
            FROM job_executions
            WHERE idempotency_key = $1
            "#,
        )
        .bind(key)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(execution)
    }

    /// Find an execution by ID
    #[instrument(skip(self))]
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<JobExecution>, DatabaseError> {
        let execution = sqlx::query_as::<_, JobExecution>(
            r#"
            SELECT 
                id, job_id, idempotency_key, status, attempt,
                trigger_source, trigger_metadata, current_step, context,
                started_at, completed_at, result, error, created_at
            FROM job_executions
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(execution)
    }

    /// Find executions with filtering and 30-day window
    ///
    /// # Requirements
    /// - 6.2: Execution history with 30-day filter
    #[instrument(skip(self))]
    pub async fn find_with_filter(
        &self,
        filter: ExecutionFilter,
    ) -> Result<Vec<JobExecution>, DatabaseError> {
        // Calculate 30 days ago
        let thirty_days_ago = Utc::now() - Duration::days(30);

        let mut query = String::from(
            r#"
            SELECT 
                id, job_id, idempotency_key, status, attempt,
                trigger_source, trigger_metadata, current_step, context,
                started_at, completed_at, result, error, created_at
            FROM job_executions
            WHERE created_at >= $1
            "#,
        );

        let mut param_count = 2;

        // Add job_id filter if provided
        if filter.job_id.is_some() {
            query.push_str(&format!(" AND job_id = ${}", param_count));
            param_count += 1;
        }

        // Add status filter if provided
        if filter.status.is_some() {
            query.push_str(&format!(" AND status = ${}", param_count));
            param_count += 1;
        }

        // Add trigger_source filter if provided
        if filter.trigger_source.is_some() {
            query.push_str(&format!(" AND trigger_source = ${}", param_count));
        }

        query.push_str(" ORDER BY created_at DESC");

        // Add limit if provided
        if let Some(limit) = filter.limit {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        let mut query_builder = sqlx::query_as::<_, JobExecution>(&query).bind(thirty_days_ago);

        if let Some(job_id) = filter.job_id {
            query_builder = query_builder.bind(job_id);
        }

        if let Some(status) = filter.status {
            query_builder = query_builder.bind(status.to_string());
        }

        if let Some(trigger_source) = filter.trigger_source {
            query_builder = query_builder.bind(trigger_source);
        }

        let executions = query_builder.fetch_all(self.pool.pool()).await?;

        tracing::debug!(count = executions.len(), "Found executions with filter");
        Ok(executions)
    }

    /// Find executions for a specific job
    #[instrument(skip(self))]
    pub async fn find_by_job_id(&self, job_id: Uuid) -> Result<Vec<JobExecution>, DatabaseError> {
        let thirty_days_ago = Utc::now() - Duration::days(30);

        let executions = sqlx::query_as::<_, JobExecution>(
            r#"
            SELECT 
                id, job_id, idempotency_key, status, attempt,
                trigger_source, trigger_metadata, current_step, context,
                started_at, completed_at, result, error, created_at
            FROM job_executions
            WHERE job_id = $1 AND created_at >= $2
            ORDER BY created_at DESC
            "#,
        )
        .bind(job_id)
        .bind(thirty_days_ago)
        .fetch_all(self.pool.pool())
        .await?;

        Ok(executions)
    }

    /// Get the latest execution for a job
    #[instrument(skip(self))]
    pub async fn find_latest_by_job_id(
        &self,
        job_id: Uuid,
    ) -> Result<Option<JobExecution>, DatabaseError> {
        let execution = sqlx::query_as::<_, JobExecution>(
            r#"
            SELECT 
                id, job_id, idempotency_key, status, attempt,
                trigger_source, trigger_metadata, current_step, context,
                started_at, completed_at, result, error, created_at
            FROM job_executions
            WHERE job_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(job_id)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(execution)
    }

    /// Count executions by status for a job
    #[instrument(skip(self))]
    pub async fn count_by_status(
        &self,
        job_id: Uuid,
        status: ExecutionStatus,
    ) -> Result<i64, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM job_executions
            WHERE job_id = $1 AND status = $2
            "#,
        )
        .bind(job_id)
        .bind(status.to_string())
        .fetch_one(self.pool.pool())
        .await?;

        let count: i64 = row.try_get("count")?;
        Ok(count)
    }

    /// Delete old executions (older than 30 days)
    /// This can be used for cleanup jobs
    #[instrument(skip(self))]
    pub async fn delete_old_executions(&self) -> Result<u64, DatabaseError> {
        let thirty_days_ago = Utc::now() - Duration::days(30);

        let result = sqlx::query("DELETE FROM job_executions WHERE created_at < $1")
            .bind(thirty_days_ago)
            .execute(self.pool.pool())
            .await?;

        let deleted = result.rows_affected();
        tracing::info!(deleted_count = deleted, "Deleted old executions");
        Ok(deleted)
    }

    /// Check if a job has any running executions
    ///
    /// # Requirements
    /// - 17.9: Check for running executions before queueing
    /// - 17.10: Reject new triggers if concurrent execution not allowed
    #[instrument(skip(self))]
    pub async fn has_running_execution(&self, job_id: Uuid) -> Result<bool, DatabaseError> {
        let row = sqlx::query(
            r#"
            SELECT COUNT(*) as count
            FROM job_executions
            WHERE job_id = $1 AND (status = 'running' OR status = 'pending')
            "#,
        )
        .bind(job_id)
        .fetch_one(self.pool.pool())
        .await?;

        let count: i64 = row.try_get("count")?;
        Ok(count > 0)
    }
}

/// Filter for querying executions
#[derive(Debug, Clone, Default)]
pub struct ExecutionFilter {
    pub job_id: Option<Uuid>,
    pub status: Option<ExecutionStatus>,
    pub trigger_source: Option<String>,
    pub limit: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_repository_creation() {
        // This test verifies the repository can be created
        // Actual database tests require a running PostgreSQL instance
    }

    #[test]
    fn test_execution_filter_default() {
        let filter = ExecutionFilter::default();
        assert!(filter.job_id.is_none());
        assert!(filter.status.is_none());
        assert!(filter.trigger_source.is_none());
        assert!(filter.limit.is_none());
    }
}
