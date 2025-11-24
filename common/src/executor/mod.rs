// Executor module for job execution
// Provides trait and implementations for different job types

pub mod database;
pub mod file;
pub mod http;
pub mod sftp;

use crate::errors::ExecutionError;
use crate::models::{JobContext, JobStep, StepOutput};
use async_trait::async_trait;

/// JobExecutor trait defines the interface for executing job steps
#[async_trait]
pub trait JobExecutor: Send + Sync {
    /// Execute a job step and return the output
    async fn execute(
        &self,
        step: &JobStep,
        context: &mut JobContext,
    ) -> Result<StepOutput, ExecutionError>;
}
