// SFTP executor module
// Requirements: 19.1-19.17 - SFTP operations
// RECC 2025: Module organization - max 100 lines for mod.rs

mod auth;
mod connection;
mod operations;

pub use auth::authenticate_session;
pub use connection::SftpConnection;
pub use operations::{download_file, list_files, upload_file};

use crate::errors::ExecutionError;
use crate::executor::JobExecutor;
use crate::models::{JobContext, JobStep, StepOutput};
use crate::storage::StorageService;
use crate::worker::reference::ReferenceResolver;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::instrument;

/// SftpExecutor executes SFTP operations (download/upload)
pub struct SftpExecutor {
    storage_service: Arc<dyn StorageService>,
    reference_resolver: Arc<ReferenceResolver>,
    timeout_seconds: u64,
}

impl SftpExecutor {
    /// Create a new SftpExecutor
    pub fn new(storage_service: Arc<dyn StorageService>, timeout_seconds: u64) -> Self {
        Self {
            storage_service,
            reference_resolver: Arc::new(ReferenceResolver::new()),
            timeout_seconds,
        }
    }

    /// Create a new SftpExecutor with custom reference resolver
    pub fn with_resolver(
        storage_service: Arc<dyn StorageService>,
        reference_resolver: Arc<ReferenceResolver>,
        timeout_seconds: u64,
    ) -> Self {
        Self {
            storage_service,
            reference_resolver,
            timeout_seconds,
        }
    }
}

#[async_trait]
impl JobExecutor for SftpExecutor {
    #[instrument(skip(self, step, context), fields(step_id = %step.id, step_name = %step.name))]
    async fn execute(
        &self,
        step: &JobStep,
        context: &mut JobContext,
    ) -> Result<StepOutput, ExecutionError> {
        // Implementation will be in operations.rs
        operations::execute_sftp_step(
            step,
            context,
            &self.storage_service,
            &self.reference_resolver,
            self.timeout_seconds,
        )
        .await
    }
}
