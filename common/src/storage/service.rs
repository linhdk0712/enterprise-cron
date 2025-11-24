// MinIO service trait and implementation for job definitions and context
// Requirements: 13.2, 13.3, 13.7 - Store and load job definitions and execution context
// RECC 2025: No unwrap(), use #[tracing::instrument], proper error handling

use crate::errors::StorageError;
use crate::models::JobContext;
use crate::storage::MinioClient;
use async_trait::async_trait;
use serde_json;
use tracing::{debug, error, info, instrument};
use uuid::Uuid;

/// MinIO service trait for job definitions and execution context
#[async_trait]
pub trait MinIOService: Send + Sync {
    /// Store job definition to MinIO
    /// Path format: jobs/{job_id}/definition.json
    /// Requirements: 13.2, 13.3
    async fn store_job_definition(
        &self,
        job_id: Uuid,
        definition: &str,
    ) -> Result<String, StorageError>;

    /// Load job definition from MinIO
    /// Requirements: 13.2
    async fn load_job_definition(&self, job_id: Uuid) -> Result<String, StorageError>;

    /// Store job context to MinIO
    /// Path format: jobs/{job_id}/executions/{execution_id}/context.json
    /// Requirements: 13.7
    async fn store_context(&self, context: &JobContext) -> Result<String, StorageError>;

    /// Load job context from MinIO
    /// Requirements: 13.7, 13.8
    async fn load_context(
        &self,
        job_id: Uuid,
        execution_id: Uuid,
    ) -> Result<JobContext, StorageError>;

    /// Store arbitrary file to MinIO
    /// Requirements: 13.7
    async fn store_file(&self, path: &str, data: &[u8]) -> Result<String, StorageError>;

    /// Load arbitrary file from MinIO
    /// Requirements: 13.7
    async fn load_file(&self, path: &str) -> Result<Vec<u8>, StorageError>;
}

/// MinIO service implementation
pub struct MinIOServiceImpl {
    client: MinioClient,
}

impl MinIOServiceImpl {
    /// Create a new MinIO service instance
    pub fn new(client: MinioClient) -> Self {
        Self { client }
    }

    /// Generate path for job definition
    /// Format: jobs/{job_id}/definition.json
    /// Requirements: 13.3 - MinIO path format for job definitions
    fn job_definition_path(job_id: Uuid) -> String {
        format!("jobs/{}/definition.json", job_id)
    }

    /// Generate path for job context
    /// Format: jobs/{job_id}/executions/{execution_id}/context.json
    /// Requirements: 13.7 - Job Context path format
    fn job_context_path(job_id: Uuid, execution_id: Uuid) -> String {
        format!("jobs/{}/executions/{}/context.json", job_id, execution_id)
    }
}

#[async_trait]
impl MinIOService for MinIOServiceImpl {
    /// Store job definition to MinIO
    /// Requirements: 13.2 - Store job definition in MinIO
    /// Property 77: MinIO job definition persistence
    /// Property 78: MinIO path format for job definitions
    #[instrument(skip(self, definition), fields(job_id = %job_id, size = definition.len()))]
    async fn store_job_definition(
        &self,
        job_id: Uuid,
        definition: &str,
    ) -> Result<String, StorageError> {
        info!(job_id = %job_id, "Storing job definition to MinIO");

        // Validate JSON before storing
        serde_json::from_str::<serde_json::Value>(definition).map_err(|e| {
            error!(error = %e, job_id = %job_id, "Invalid JSON in job definition");
            StorageError::MinioError(format!("Invalid JSON in job definition: {}", e))
        })?;

        let path = Self::job_definition_path(job_id);
        self.client.put_object(&path, definition.as_bytes()).await?;

        info!(job_id = %job_id, path = %path, "Job definition stored successfully");
        Ok(path)
    }

    /// Load job definition from MinIO
    /// Requirements: 13.2 - Load job definition from MinIO
    /// Property 77: MinIO job definition persistence (round-trip)
    #[instrument(skip(self), fields(job_id = %job_id))]
    async fn load_job_definition(&self, job_id: Uuid) -> Result<String, StorageError> {
        debug!(job_id = %job_id, "Loading job definition from MinIO");

        let path = Self::job_definition_path(job_id);
        let data = self.client.get_object(&path).await?;

        let definition = String::from_utf8(data).map_err(|e| {
            error!(error = %e, job_id = %job_id, "Failed to decode job definition as UTF-8");
            StorageError::MinioError(format!("Failed to decode job definition: {}", e))
        })?;

        // Validate JSON
        serde_json::from_str::<serde_json::Value>(&definition).map_err(|e| {
            error!(error = %e, job_id = %job_id, "Invalid JSON in stored job definition");
            StorageError::MinioError(format!("Invalid JSON in stored job definition: {}", e))
        })?;

        debug!(job_id = %job_id, size = definition.len(), "Job definition loaded successfully");
        Ok(definition)
    }

    /// Store job context to MinIO
    /// Requirements: 13.7 - Persist Job Context to MinIO after each step
    /// Property 82: Job Context persistence to MinIO
    /// Property 83: Job Context path format
    #[instrument(skip(self, context), fields(job_id = %context.job_id, execution_id = %context.execution_id))]
    async fn store_context(&self, context: &JobContext) -> Result<String, StorageError> {
        info!(
            job_id = %context.job_id,
            execution_id = %context.execution_id,
            "Storing job context to MinIO"
        );

        let json = serde_json::to_string_pretty(context).map_err(|e| {
            error!(
                error = %e,
                job_id = %context.job_id,
                execution_id = %context.execution_id,
                "Failed to serialize job context"
            );
            StorageError::MinioError(format!("Failed to serialize job context: {}", e))
        })?;

        let path = Self::job_context_path(context.job_id, context.execution_id);
        self.client.put_object(&path, json.as_bytes()).await?;

        info!(
            job_id = %context.job_id,
            execution_id = %context.execution_id,
            path = %path,
            "Job context stored successfully"
        );
        Ok(path)
    }

    /// Load job context from MinIO
    /// Requirements: 13.7, 13.8 - Load Job Context for subsequent steps
    /// Property 82: Job Context persistence to MinIO (round-trip)
    /// Property 84: Job Context loading for subsequent steps
    #[instrument(skip(self), fields(job_id = %job_id, execution_id = %execution_id))]
    async fn load_context(
        &self,
        job_id: Uuid,
        execution_id: Uuid,
    ) -> Result<JobContext, StorageError> {
        debug!(
            job_id = %job_id,
            execution_id = %execution_id,
            "Loading job context from MinIO"
        );

        let path = Self::job_context_path(job_id, execution_id);
        let data = self.client.get_object(&path).await?;

        let json = String::from_utf8(data).map_err(|e| {
            error!(
                error = %e,
                job_id = %job_id,
                execution_id = %execution_id,
                "Failed to decode job context as UTF-8"
            );
            StorageError::MinioError(format!("Failed to decode job context: {}", e))
        })?;

        let context: JobContext = serde_json::from_str(&json).map_err(|e| {
            error!(
                error = %e,
                job_id = %job_id,
                execution_id = %execution_id,
                "Failed to deserialize job context"
            );
            StorageError::MinioError(format!("Failed to deserialize job context: {}", e))
        })?;

        debug!(
            job_id = %job_id,
            execution_id = %execution_id,
            steps_count = context.steps.len(),
            "Job context loaded successfully"
        );
        Ok(context)
    }

    /// Store arbitrary file to MinIO
    /// Requirements: 13.7 - Store files in MinIO
    #[instrument(skip(self, data), fields(path = %path, size = data.len()))]
    async fn store_file(&self, path: &str, data: &[u8]) -> Result<String, StorageError> {
        debug!(path = %path, size = data.len(), "Storing file to MinIO");

        self.client.put_object(path, data).await?;

        debug!(path = %path, "File stored successfully");
        Ok(path.to_string())
    }

    /// Load arbitrary file from MinIO
    /// Requirements: 13.7 - Load files from MinIO
    #[instrument(skip(self), fields(path = %path))]
    async fn load_file(&self, path: &str) -> Result<Vec<u8>, StorageError> {
        debug!(path = %path, "Loading file from MinIO");

        let data = self.client.get_object(path).await?;

        debug!(path = %path, size = data.len(), "File loaded successfully");
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_job_definition_path_format() {
        let job_id = Uuid::new_v4();
        let path = MinIOServiceImpl::job_definition_path(job_id);
        assert_eq!(path, format!("jobs/{}/definition.json", job_id));
    }

    #[test]
    fn test_job_context_path_format() {
        let job_id = Uuid::new_v4();
        let execution_id = Uuid::new_v4();
        let path = MinIOServiceImpl::job_context_path(job_id, execution_id);
        assert_eq!(
            path,
            format!("jobs/{}/executions/{}/context.json", job_id, execution_id)
        );
    }

    #[test]
    fn test_job_context_serialization() {
        let context = JobContext {
            execution_id: Uuid::new_v4(),
            job_id: Uuid::new_v4(),
            variables: HashMap::new(),
            steps: HashMap::new(),
            webhook: None,
            files: Vec::new(),
        };

        let json = serde_json::to_string(&context).unwrap();
        let deserialized: JobContext = serde_json::from_str(&json).unwrap();

        assert_eq!(context.execution_id, deserialized.execution_id);
        assert_eq!(context.job_id, deserialized.job_id);
    }

    #[test]
    fn test_invalid_json_detection() {
        let invalid_json = "{ invalid json }";
        let result = serde_json::from_str::<serde_json::Value>(invalid_json);
        assert!(result.is_err());
    }
}
