// Database job executor module
// Requirements: 3.7, 3.8, 3.9, 3.10
// Tách theo RECC 2025 rules - Mỗi database type một file riêng

mod postgresql;
mod mysql;
mod oracle;

use crate::errors::ExecutionError;
use crate::executor::JobExecutor;
use crate::models::{DatabaseType, JobContext, JobStep, JobType, QueryType, StepOutput};
use crate::worker::reference::ReferenceResolver;
use async_trait::async_trait;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;

pub use postgresql::PostgreSQLExecutor;
pub use mysql::MySQLExecutor;
pub use oracle::OracleExecutor;

/// DatabaseExecutor executes database query jobs
pub struct DatabaseExecutor {
    timeout: Duration,
    reference_resolver: Arc<ReferenceResolver>,
}

impl DatabaseExecutor {
    /// Create a new DatabaseExecutor with the specified timeout
    pub fn new(timeout_seconds: u64) -> Self {
        Self {
            timeout: Duration::from_secs(timeout_seconds),
            reference_resolver: Arc::new(ReferenceResolver::new()),
        }
    }

    /// Create a new DatabaseExecutor with a custom reference resolver
    pub fn with_resolver(timeout_seconds: u64, reference_resolver: Arc<ReferenceResolver>) -> Self {
        Self {
            timeout: Duration::from_secs(timeout_seconds),
            reference_resolver,
        }
    }

    /// Get the timeout duration
    pub fn timeout(&self) -> Duration {
        self.timeout
    }
}

#[async_trait]
impl JobExecutor for DatabaseExecutor {
    #[tracing::instrument(skip(self, step, context), fields(step_id = %step.id, step_name = %step.name))]
    async fn execute(
        &self,
        step: &JobStep,
        context: &mut JobContext,
    ) -> Result<StepOutput, ExecutionError> {
        let started_at = Utc::now();

        // Extract database query details from step
        let (database_type, connection_string, query, query_type) = match &step.step_type {
            JobType::DatabaseQuery {
                database_type,
                connection_string,
                query,
                query_type,
            } => (database_type, connection_string, query, query_type),
            _ => {
                return Err(ExecutionError::InvalidJobDefinition(
                    "DatabaseExecutor can only execute DatabaseQuery job types".to_string(),
                ));
            }
        };

        // Requirement 14.6: Resolve references in database connection strings and queries
        let resolved_connection_string = self
            .reference_resolver
            .resolve(connection_string, context)
            .map_err(|e| {
                ExecutionError::InvalidJobDefinition(format!(
                    "Failed to resolve connection string references: {}",
                    e
                ))
            })?;

        let resolved_query = self
            .reference_resolver
            .resolve(query, context)
            .map_err(|e| {
                ExecutionError::InvalidJobDefinition(format!(
                    "Failed to resolve query references: {}",
                    e
                ))
            })?;

        // Resolve query type references (for stored procedure parameters)
        let resolved_query_type = match query_type {
            QueryType::RawSql => QueryType::RawSql,
            QueryType::StoredProcedure {
                procedure_name,
                parameters,
            } => {
                let resolved_procedure_name = self
                    .reference_resolver
                    .resolve(procedure_name, context)
                    .map_err(|e| {
                        ExecutionError::InvalidJobDefinition(format!(
                            "Failed to resolve procedure name references: {}",
                            e
                        ))
                    })?;

                let mut resolved_parameters = Vec::new();
                for param in parameters {
                    let resolved_param =
                        self.reference_resolver
                            .resolve(param, context)
                            .map_err(|e| {
                                ExecutionError::InvalidJobDefinition(format!(
                                    "Failed to resolve parameter '{}': {}",
                                    param, e
                                ))
                            })?;
                    resolved_parameters.push(resolved_param);
                }

                QueryType::StoredProcedure {
                    procedure_name: resolved_procedure_name,
                    parameters: resolved_parameters,
                }
            }
        };

        // Execute the database query based on database type
        // Requirement 3.7: Support PostgreSQL, MySQL, and Oracle
        let output = match database_type {
            DatabaseType::PostgreSQL => {
                let executor = PostgreSQLExecutor::new(self.timeout);
                executor
                    .execute_query(&resolved_connection_string, &resolved_query, &resolved_query_type)
                    .await?
            }
            DatabaseType::MySQL => {
                let executor = MySQLExecutor::new(self.timeout);
                executor
                    .execute_query(&resolved_connection_string, &resolved_query, &resolved_query_type)
                    .await?
            }
            DatabaseType::Oracle => {
                let executor = OracleExecutor::new(self.timeout);
                executor
                    .execute_query(&resolved_connection_string, &resolved_query, &resolved_query_type)
                    .await?
            }
        };

        let completed_at = Utc::now();

        Ok(StepOutput {
            step_id: step.id.clone(),
            status: "success".to_string(),
            output,
            started_at,
            completed_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_database_executor_creation() {
        let executor = DatabaseExecutor::new(30);
        assert_eq!(executor.timeout, Duration::from_secs(30));
    }

    #[tokio::test]
    async fn test_invalid_job_type() {
        use crate::models::HttpMethod;

        let executor = DatabaseExecutor::new(30);
        let step = JobStep {
            id: "step1".to_string(),
            name: "Test Step".to_string(),
            step_type: JobType::HttpRequest {
                method: HttpMethod::Get,
                url: "http://example.com".to_string(),
                headers: std::collections::HashMap::new(),
                body: None,
                auth: None,
            },
            condition: None,
            on_failure: None,
            timeout_seconds: None,
            retry_count: None,
        };

        let mut context = JobContext {
            execution_id: Uuid::new_v4(),
            job_id: Uuid::new_v4(),
            variables: std::collections::HashMap::new(),
            steps: std::collections::HashMap::new(),
            webhook: None,
            files: Vec::new(),
        };

        let result = executor.execute(&step, &mut context).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ExecutionError::InvalidJobDefinition(_)
        ));
    }
}
