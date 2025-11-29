// Job Context management for multi-step jobs
// Requirements: 13.7, 13.8 - Load and save Job Context from storage
// RECC 2025: No unwrap(), use #[tracing::instrument], proper error handling

use crate::errors::ExecutionError;
use crate::models::JobContext;
use crate::storage::StorageService;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, error, info, instrument};
use uuid::Uuid;

/// Context manager trait for loading and saving job contexts
/// Requirements: 13.7, 13.8 - Manage Job Context lifecycle
#[async_trait]
pub trait ContextManager: Send + Sync {
    /// Load job context from storage
    /// Requirements: 13.8 - Load Job Context for subsequent steps
    async fn load_context(
        &self,
        job_id: Uuid,
        execution_id: Uuid,
    ) -> Result<JobContext, ExecutionError>;

    /// Save job context to storage
    /// Requirements: 13.7 - Persist Job Context after each step
    async fn save_context(&self, context: &JobContext) -> Result<(), ExecutionError>;

    /// Initialize a new job context for a new execution
    /// Requirements: 13.7 - Context initialization for new executions
    async fn initialize_context(
        &self,
        job_id: Uuid,
        execution_id: Uuid,
    ) -> Result<JobContext, ExecutionError>;
}

/// Job context manager implementation using Storage service
/// Requirements: 13.7, 13.8 - Manage Job Context with storage
pub struct JobContextManager {
    storage_service: Arc<dyn StorageService>,
}

impl JobContextManager {
    /// Create a new JobContextManager with Storage service
    /// Requirements: 13.7 - Initialize context manager with storage
    pub fn new(storage_service: Arc<dyn StorageService>) -> Self {
        Self { storage_service }
    }
}

#[async_trait]
impl ContextManager for JobContextManager {
    /// Load job context from storage
    /// Requirements: 13.8 - Load Job Context for subsequent steps
    /// Property 84: Job Context loading for subsequent steps
    #[instrument(skip(self), fields(job_id = %job_id, execution_id = %execution_id))]
    async fn load_context(
        &self,
        job_id: Uuid,
        execution_id: Uuid,
    ) -> Result<JobContext, ExecutionError> {
        debug!(
            job_id = %job_id,
            execution_id = %execution_id,
            "Loading job context from storage"
        );

        self.storage_service
            .load_context(job_id, execution_id)
            .await
            .map_err(|e| {
                error!(
                    error = %e,
                    job_id = %job_id,
                    execution_id = %execution_id,
                    "Failed to load job context from storage"
                );
                ExecutionError::ContextLoadFailed(format!(
                    "Failed to load context for execution {}: {}",
                    execution_id, e
                ))
            })
    }

    /// Save job context to storage
    /// Requirements: 13.7 - Persist Job Context after each step
    /// Property 82: Job Context persistence
    #[instrument(skip(self, context), fields(job_id = %context.job_id, execution_id = %context.execution_id))]
    async fn save_context(&self, context: &JobContext) -> Result<(), ExecutionError> {
        info!(
            job_id = %context.job_id,
            execution_id = %context.execution_id,
            steps_count = context.steps.len(),
            "Saving job context to storage"
        );

        self.storage_service
            .store_context(context)
            .await
            .map_err(|e| {
                error!(
                    error = %e,
                    job_id = %context.job_id,
                    execution_id = %context.execution_id,
                    "Failed to save job context to storage"
                );
                ExecutionError::ContextSaveFailed(format!(
                    "Failed to save context for execution {}: {}",
                    context.execution_id, e
                ))
            })?;

        info!(
            job_id = %context.job_id,
            execution_id = %context.execution_id,
            "Job context saved successfully"
        );
        Ok(())
    }

    /// Initialize a new job context for a new execution
    /// Requirements: 13.7 - Context initialization for new executions
    #[instrument(skip(self), fields(job_id = %job_id, execution_id = %execution_id))]
    async fn initialize_context(
        &self,
        job_id: Uuid,
        execution_id: Uuid,
    ) -> Result<JobContext, ExecutionError> {
        info!(
            job_id = %job_id,
            execution_id = %execution_id,
            "Initializing new job context"
        );

        let context = JobContext::new(execution_id, job_id);

        // Save the initial empty context to storage
        self.save_context(&context).await?;

        debug!(
            job_id = %job_id,
            execution_id = %execution_id,
            "Job context initialized successfully"
        );
        Ok(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::StorageError;
    use crate::models::JobContext;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;

    // Mock Storage service for testing
    struct MockStorageService {
        contexts: Mutex<HashMap<(Uuid, Uuid), JobContext>>,
    }

    impl MockStorageService {
        fn new() -> Self {
            Self {
                contexts: Mutex::new(HashMap::new()),
            }
        }
    }

    #[async_trait]
    impl StorageService for MockStorageService {
        async fn store_job_definition(
            &self,
            _job_id: Uuid,
            _definition: &str,
        ) -> Result<(), StorageError> {
            Ok(())
        }

        async fn load_job_definition(&self, _job_id: Uuid) -> Result<String, StorageError> {
            Ok("{}".to_string())
        }

        async fn store_context(&self, context: &JobContext) -> Result<(), StorageError> {
            let mut contexts = self.contexts.lock().unwrap();
            contexts.insert((context.job_id, context.execution_id), context.clone());
            Ok(())
        }

        async fn load_context(
            &self,
            job_id: Uuid,
            execution_id: Uuid,
        ) -> Result<JobContext, StorageError> {
            let contexts = self.contexts.lock().unwrap();
            contexts
                .get(&(job_id, execution_id))
                .cloned()
                .ok_or_else(|| StorageError::NotFound("Context not found".to_string()))
        }

        async fn store_file(&self, _path: &str, _data: &[u8]) -> Result<String, StorageError> {
            Ok("mock_file_path".to_string())
        }

        async fn load_file(&self, _path: &str) -> Result<Vec<u8>, StorageError> {
            Ok(vec![])
        }

        async fn delete_file(&self, _path: &str) -> Result<(), StorageError> {
            Ok(())
        }

        async fn list_files(&self, _prefix: &str) -> Result<Vec<String>, StorageError> {
            Ok(vec![])
        }
    }

    #[tokio::test]
    async fn test_initialize_context() {
        let storage_service = Arc::new(MockStorageService::new());
        let manager = JobContextManager::new(storage_service);

        let job_id = Uuid::new_v4();
        let execution_id = Uuid::new_v4();

        let context = manager
            .initialize_context(job_id, execution_id)
            .await
            .unwrap();

        assert_eq!(context.job_id, job_id);
        assert_eq!(context.execution_id, execution_id);
        assert_eq!(context.steps.len(), 0);
        assert_eq!(context.variables.len(), 0);
    }

    #[tokio::test]
    async fn test_save_and_load_context() {
        let storage_service = Arc::new(MockStorageService::new());
        let manager = JobContextManager::new(storage_service);

        let job_id = Uuid::new_v4();
        let execution_id = Uuid::new_v4();

        // Initialize context
        let mut context = manager
            .initialize_context(job_id, execution_id)
            .await
            .unwrap();

        // Add some data
        context.set_variable("test_var".to_string(), serde_json::json!("test_value"));

        // Save context
        manager.save_context(&context).await.unwrap();

        // Load context
        let loaded_context = manager.load_context(job_id, execution_id).await.unwrap();

        assert_eq!(loaded_context.job_id, job_id);
        assert_eq!(loaded_context.execution_id, execution_id);
        assert_eq!(
            loaded_context.get_variable("test_var"),
            Some(&serde_json::json!("test_value"))
        );
    }

    #[tokio::test]
    async fn test_load_nonexistent_context() {
        let storage_service = Arc::new(MockStorageService::new());
        let manager = JobContextManager::new(storage_service);

        let job_id = Uuid::new_v4();
        let execution_id = Uuid::new_v4();

        let result = manager.load_context(job_id, execution_id).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ExecutionError::ContextLoadFailed(_)
        ));
    }

    #[tokio::test]
    async fn test_context_update_after_step() {
        let storage_service = Arc::new(MockStorageService::new());
        let manager = JobContextManager::new(storage_service);

        let job_id = Uuid::new_v4();
        let execution_id = Uuid::new_v4();

        // Initialize context
        let mut context = manager
            .initialize_context(job_id, execution_id)
            .await
            .unwrap();

        // Simulate step execution
        use crate::models::StepOutput;
        use chrono::Utc;

        let step_output = StepOutput {
            step_id: "step1".to_string(),
            status: "success".to_string(),
            output: serde_json::json!({"result": "data"}),
            started_at: Utc::now(),
            completed_at: Utc::now(),
        };

        context.set_step_output("step1".to_string(), step_output);

        // Save updated context
        manager.save_context(&context).await.unwrap();

        // Load and verify
        let loaded_context = manager.load_context(job_id, execution_id).await.unwrap();
        assert!(loaded_context.has_step_output("step1"));
        assert_eq!(loaded_context.completed_steps_count(), 1);
    }
}
