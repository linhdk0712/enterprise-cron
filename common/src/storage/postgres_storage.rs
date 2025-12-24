// Storage service using PostgreSQL + Redis cache (replaces MinIO)
// Requirements: 13.2, 13.3, 13.7 - Store and load job definitions and execution context
// RECC 2025: No unwrap(), use #[tracing::instrument], proper error handling

use crate::errors::StorageError;
use crate::models::JobContext;
use async_trait::async_trait;
use redis::AsyncCommands;
use serde_json;
use sqlx::PgPool;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

/// Storage service trait for job definitions and execution context
#[async_trait]
pub trait StorageService: Send + Sync {
    /// Store job definition to PostgreSQL
    async fn store_job_definition(
        &self,
        job_id: Uuid,
        definition: &str,
    ) -> Result<(), StorageError>;

    /// Load job definition from PostgreSQL (with Redis cache)
    async fn load_job_definition(&self, job_id: Uuid) -> Result<String, StorageError>;

    /// Store job context to PostgreSQL
    async fn store_context(&self, context: &JobContext) -> Result<(), StorageError>;

    /// Load job context from PostgreSQL (with Redis cache)
    async fn load_context(
        &self,
        job_id: Uuid,
        execution_id: Uuid,
    ) -> Result<JobContext, StorageError>;

    /// Store file to filesystem
    async fn store_file(&self, path: &str, data: &[u8]) -> Result<String, StorageError>;

    /// Load file from filesystem
    async fn load_file(&self, path: &str) -> Result<Vec<u8>, StorageError>;

    /// Delete file from filesystem
    async fn delete_file(&self, path: &str) -> Result<(), StorageError>;

    /// List files in filesystem with prefix
    async fn list_files(&self, prefix: &str) -> Result<Vec<String>, StorageError>;
}

/// Storage service implementation using PostgreSQL + Redis cache + Filesystem
#[derive(Clone)]
pub struct StorageServiceImpl {
    db_pool: PgPool,
    redis: Arc<redis::aio::ConnectionManager>,
    file_base_path: PathBuf,
}

// Redis key prefixes
const REDIS_JOB_DEF_PREFIX: &str = "storage:job_def:";
const REDIS_JOB_CTX_PREFIX: &str = "storage:job_ctx:";

// Redis TTL
const JOB_DEF_TTL: i64 = 7 * 24 * 60 * 60; // 7 days
const JOB_CTX_TTL: i64 = 30 * 24 * 60 * 60; // 30 days

impl StorageServiceImpl {
    pub fn new(
        db_pool: PgPool,
        redis: Arc<redis::aio::ConnectionManager>,
        file_base_path: Option<PathBuf>,
    ) -> Self {
        let file_base_path = file_base_path.unwrap_or_else(|| PathBuf::from("./data/files"));
        info!(
            file_base_path = %file_base_path.display(),
            "Initializing storage service (PostgreSQL + Redis + Filesystem)"
        );
        Self {
            db_pool,
            redis,
            file_base_path,
        }
    }

    fn redis_job_def_key(job_id: Uuid) -> String {
        format!("{}{}", REDIS_JOB_DEF_PREFIX, job_id)
    }

    fn redis_job_ctx_key(job_id: Uuid, execution_id: Uuid) -> String {
        format!("{}{}:{}", REDIS_JOB_CTX_PREFIX, job_id, execution_id)
    }

    fn get_file_path(&self, path: &str) -> PathBuf {
        self.file_base_path.join(path)
    }
}

#[async_trait]
impl StorageService for StorageServiceImpl {
    #[instrument(skip(self, definition), fields(job_id = %job_id, size = definition.len()))]
    async fn store_job_definition(
        &self,
        job_id: Uuid,
        definition: &str,
    ) -> Result<(), StorageError> {
        debug!(job_id = %job_id, "Storing job definition to PostgreSQL");

        // Validate JSON
        let json_value: serde_json::Value = serde_json::from_str(definition).map_err(|e| {
            error!(error = %e, job_id = %job_id, "Invalid JSON in job definition");
            StorageError::InvalidJson(format!("Invalid JSON: {}", e))
        })?;

        // Store in PostgreSQL
        sqlx::query!(
            "UPDATE jobs SET definition = $1, updated_at = NOW() WHERE id = $2",
            json_value,
            job_id
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| {
            error!(error = %e, job_id = %job_id, "Failed to store in PostgreSQL");
            StorageError::DatabaseError(e.to_string())
        })?;

        info!(job_id = %job_id, "Job definition stored in PostgreSQL");

        // Cache in Redis
        let redis_key = Self::redis_job_def_key(job_id);
        let mut redis_conn = (*self.redis).clone();
        if let Err(e) = redis_conn
            .set_ex::<_, _, ()>(&redis_key, definition, JOB_DEF_TTL as u64)
            .await
        {
            warn!(error = %e, job_id = %job_id, "Failed to cache in Redis");
        }

        Ok(())
    }

    #[instrument(skip(self), fields(job_id = %job_id))]
    async fn load_job_definition(&self, job_id: Uuid) -> Result<String, StorageError> {
        debug!(job_id = %job_id, "Loading job definition (Redis → PostgreSQL)");

        let redis_key = Self::redis_job_def_key(job_id);

        // Try Redis first
        let mut redis_conn = (*self.redis).clone();
        match redis_conn.get::<_, Option<String>>(&redis_key).await {
            Ok(Some(definition)) => {
                debug!(job_id = %job_id, "Found in Redis cache");
                return Ok(definition);
            }
            Ok(None) => debug!(job_id = %job_id, "Not in Redis, querying PostgreSQL"),
            Err(e) => warn!(error = %e, job_id = %job_id, "Redis read failed"),
        }

        // Query PostgreSQL
        let row = sqlx::query!("SELECT definition FROM jobs WHERE id = $1", job_id)
            .fetch_optional(&self.db_pool)
            .await
            .map_err(|e| {
                error!(error = %e, job_id = %job_id, "Failed to query PostgreSQL");
                StorageError::DatabaseError(e.to_string())
            })?
            .ok_or_else(|| {
                error!(job_id = %job_id, "Job not found");
                StorageError::NotFound(format!("Job {} not found", job_id))
            })?;

        let definition_value = row.definition.ok_or_else(|| {
            error!(job_id = %job_id, "Job definition is NULL");
            StorageError::NotFound(format!("Job {} has no definition", job_id))
        })?;

        let definition = serde_json::to_string(&definition_value).map_err(|e| {
            error!(error = %e, job_id = %job_id, "Failed to serialize definition");
            StorageError::InvalidJson(e.to_string())
        })?;

        // Cache in Redis
        let mut redis_conn = (*self.redis).clone();
        if let Err(e) = redis_conn
            .set_ex::<_, _, ()>(&redis_key, &definition, JOB_DEF_TTL as u64)
            .await
        {
            warn!(error = %e, job_id = %job_id, "Failed to cache in Redis");
        }

        debug!(job_id = %job_id, "Loaded from PostgreSQL and cached");
        Ok(definition)
    }

    #[instrument(skip(self, context), fields(job_id = %context.job_id, execution_id = %context.execution_id))]
    async fn store_context(&self, context: &JobContext) -> Result<(), StorageError> {
        debug!(job_id = %context.job_id, execution_id = %context.execution_id, "Storing context");

        let json_value = serde_json::to_value(context).map_err(|e| {
            error!(error = %e, "Failed to serialize context");
            StorageError::InvalidJson(e.to_string())
        })?;

        // Store in PostgreSQL
        sqlx::query!(
            "UPDATE job_executions SET context = $1 WHERE id = $2",
            json_value,
            context.execution_id
        )
        .execute(&self.db_pool)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to store context in PostgreSQL");
            StorageError::DatabaseError(e.to_string())
        })?;

        info!(job_id = %context.job_id, execution_id = %context.execution_id, "Context stored");

        // Cache in Redis
        let redis_key = Self::redis_job_ctx_key(context.job_id, context.execution_id);
        let json_str =
            serde_json::to_string(context).map_err(|e| StorageError::InvalidJson(e.to_string()))?;
        let mut redis_conn = (*self.redis).clone();
        if let Err(e) = redis_conn
            .set_ex::<_, _, ()>(&redis_key, &json_str, JOB_CTX_TTL as u64)
            .await
        {
            warn!(error = %e, "Failed to cache context in Redis");
        }

        Ok(())
    }

    #[instrument(skip(self), fields(job_id = %job_id, execution_id = %execution_id))]
    async fn load_context(
        &self,
        job_id: Uuid,
        execution_id: Uuid,
    ) -> Result<JobContext, StorageError> {
        debug!(job_id = %job_id, execution_id = %execution_id, "Loading context");

        let redis_key = Self::redis_job_ctx_key(job_id, execution_id);

        // Try Redis first
        let mut redis_conn = (*self.redis).clone();
        match redis_conn.get::<_, Option<String>>(&redis_key).await {
            Ok(Some(json)) => {
                if let Ok(context) = serde_json::from_str::<JobContext>(&json) {
                    debug!("Found in Redis cache");
                    return Ok(context);
                }
            }
            Ok(None) => debug!("Not in Redis"),
            Err(e) => warn!(error = %e, "Redis read failed"),
        }

        // Query PostgreSQL
        let row = sqlx::query!(
            "SELECT context FROM job_executions WHERE id = $1",
            execution_id
        )
        .fetch_optional(&self.db_pool)
        .await
        .map_err(|e| StorageError::DatabaseError(e.to_string()))?
        .ok_or_else(|| StorageError::NotFound(format!("Execution {} not found", execution_id)))?;

        let context_value = row.context.ok_or_else(|| {
            StorageError::NotFound(format!("Context for execution {} not found", execution_id))
        })?;
        let context: JobContext = serde_json::from_value(context_value)
            .map_err(|e| StorageError::InvalidJson(e.to_string()))?;

        // Cache in Redis
        let json_str = serde_json::to_string(&context)
            .map_err(|e| StorageError::InvalidJson(e.to_string()))?;
        let mut redis_conn = (*self.redis).clone();
        if let Err(e) = redis_conn
            .set_ex::<_, _, ()>(&redis_key, &json_str, JOB_CTX_TTL as u64)
            .await
        {
            warn!(error = %e, "Failed to cache in Redis");
        }

        debug!("Loaded from PostgreSQL and cached");
        Ok(context)
    }

    #[instrument(skip(self, data), fields(path = %path, size = data.len()))]
    async fn store_file(&self, path: &str, data: &[u8]) -> Result<String, StorageError> {
        let file_path = self.get_file_path(path);

        // Create parent directories
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                error!(error = %e, path = %path, "Failed to create directories");
                StorageError::FileSystemError(e.to_string())
            })?;
        }

        // Write file
        fs::write(&file_path, data).await.map_err(|e| {
            error!(error = %e, path = %path, "Failed to write file");
            StorageError::FileSystemError(e.to_string())
        })?;

        info!(path = %path, size = data.len(), "File stored");
        Ok(path.to_string())
    }

    #[instrument(skip(self), fields(path = %path))]
    async fn load_file(&self, path: &str) -> Result<Vec<u8>, StorageError> {
        let file_path = self.get_file_path(path);

        let data = fs::read(&file_path).await.map_err(|e| {
            error!(error = %e, path = %path, "Failed to read file");
            StorageError::FileSystemError(e.to_string())
        })?;

        debug!(path = %path, size = data.len(), "File loaded");
        Ok(data)
    }

    #[instrument(skip(self), fields(path = %path))]
    async fn delete_file(&self, path: &str) -> Result<(), StorageError> {
        let file_path = self.get_file_path(path);

        fs::remove_file(&file_path).await.map_err(|e| {
            error!(error = %e, path = %path, "Failed to delete file");
            StorageError::FileSystemError(e.to_string())
        })?;

        info!(path = %path, "File deleted");
        Ok(())
    }

    #[instrument(skip(self), fields(prefix = %prefix))]
    async fn list_files(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
        let dir_path = self.get_file_path(prefix);

        let mut files = Vec::new();
        let mut entries = fs::read_dir(&dir_path).await.map_err(|e| {
            error!(error = %e, prefix = %prefix, "Failed to read directory");
            StorageError::FileSystemError(e.to_string())
        })?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| StorageError::FileSystemError(e.to_string()))?
        {
            if let Ok(file_name) = entry.file_name().into_string() {
                files.push(format!("{}/{}", prefix, file_name));
            }
        }

        debug!(prefix = %prefix, count = files.len(), "Files listed");
        Ok(files)
    }
}

// Implement StorageService for Arc<dyn StorageService> to allow using it with generic types
#[async_trait]
impl StorageService for Arc<dyn StorageService> {
    async fn store_job_definition(
        &self,
        job_id: Uuid,
        definition: &str,
    ) -> Result<(), StorageError> {
        (**self).store_job_definition(job_id, definition).await
    }

    async fn load_job_definition(&self, job_id: Uuid) -> Result<String, StorageError> {
        (**self).load_job_definition(job_id).await
    }

    async fn store_context(&self, context: &JobContext) -> Result<(), StorageError> {
        (**self).store_context(context).await
    }

    async fn load_context(
        &self,
        job_id: Uuid,
        execution_id: Uuid,
    ) -> Result<JobContext, StorageError> {
        (**self).load_context(job_id, execution_id).await
    }

    async fn store_file(&self, path: &str, data: &[u8]) -> Result<String, StorageError> {
        (**self).store_file(path, data).await
    }

    async fn load_file(&self, path: &str) -> Result<Vec<u8>, StorageError> {
        (**self).load_file(path).await
    }

    async fn delete_file(&self, path: &str) -> Result<(), StorageError> {
        (**self).delete_file(path).await
    }

    async fn list_files(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
        (**self).list_files(prefix).await
    }
}
