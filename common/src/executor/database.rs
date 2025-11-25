// Database job executor implementation
// Requirements: 3.7, 3.8, 3.9, 3.10

use crate::errors::ExecutionError;
use crate::executor::JobExecutor;
use crate::models::{DatabaseType, JobContext, JobStep, JobType, QueryType, StepOutput};
use crate::worker::reference::ReferenceResolver;
use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Column, PgPool, Row};
use std::sync::Arc;
use std::time::Duration;

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

    /// Execute PostgreSQL query
    #[tracing::instrument(
        skip(self, connection_string, query),
        fields(database_type = "postgresql")
    )]
    async fn execute_postgresql(
        &self,
        connection_string: &str,
        query: &str,
        query_type: &QueryType,
    ) -> Result<serde_json::Value, ExecutionError> {
        tracing::info!("Connecting to PostgreSQL database");

        // Create connection pool with timeout
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(self.timeout)
            .connect(connection_string)
            .await
            .map_err(|e| {
                ExecutionError::DatabaseConnectionFailed(format!(
                    "Failed to connect to PostgreSQL: {}",
                    e
                ))
            })?;

        let result = match query_type {
            QueryType::RawSql => {
                // Requirement 3.9: Execute raw SQL queries
                self.execute_raw_sql_postgres(&pool, query).await?
            }
            QueryType::StoredProcedure {
                procedure_name,
                parameters,
            } => {
                // Requirement 3.9: Execute stored procedures
                self.execute_stored_procedure_postgres(&pool, procedure_name, parameters)
                    .await?
            }
        };

        // Close the pool
        pool.close().await;

        Ok(result)
    }

    /// Execute raw SQL query on PostgreSQL
    #[tracing::instrument(skip(self, pool, query))]
    async fn execute_raw_sql_postgres(
        &self,
        pool: &PgPool,
        query: &str,
    ) -> Result<serde_json::Value, ExecutionError> {
        tracing::debug!("Executing raw SQL query");

        // Execute query
        let rows = sqlx::query(query).fetch_all(pool).await.map_err(|e| {
            ExecutionError::DatabaseQueryFailed(format!("PostgreSQL query failed: {}", e))
        })?;

        // Convert rows to JSON
        let mut result_rows = Vec::new();
        for row in rows {
            let mut row_map = serde_json::Map::new();

            for (i, column) in row.columns().iter().enumerate() {
                let column_name = column.name();

                // Try to get value as different types
                let value: serde_json::Value = if let Ok(v) = row.try_get::<String, _>(i) {
                    json!(v)
                } else if let Ok(v) = row.try_get::<i32, _>(i) {
                    json!(v)
                } else if let Ok(v) = row.try_get::<i64, _>(i) {
                    json!(v)
                } else if let Ok(v) = row.try_get::<f64, _>(i) {
                    json!(v)
                } else if let Ok(v) = row.try_get::<bool, _>(i) {
                    json!(v)
                } else if let Ok(v) = row.try_get::<chrono::NaiveDateTime, _>(i) {
                    json!(v.to_string())
                } else if let Ok(v) = row.try_get::<chrono::DateTime<Utc>, _>(i) {
                    json!(v.to_rfc3339())
                } else if let Ok(v) = row.try_get::<serde_json::Value, _>(i) {
                    v
                } else {
                    // If we can't determine the type, try to get as string or null
                    row.try_get::<Option<String>, _>(i)
                        .ok()
                        .flatten()
                        .map(|s| json!(s))
                        .unwrap_or(json!(null))
                };

                row_map.insert(column_name.to_string(), value);
            }

            result_rows.push(serde_json::Value::Object(row_map));
        }

        let result = json!({
            "rows": result_rows,
            "row_count": result_rows.len(),
        });

        tracing::info!("PostgreSQL query returned {} rows", result_rows.len());

        Ok(result)
    }

    /// Execute stored procedure on PostgreSQL
    #[tracing::instrument(skip(self, pool, procedure_name, parameters))]
    async fn execute_stored_procedure_postgres(
        &self,
        pool: &PgPool,
        procedure_name: &str,
        parameters: &[String],
    ) -> Result<serde_json::Value, ExecutionError> {
        tracing::debug!(
            "Executing stored procedure: {} with {} parameters",
            procedure_name,
            parameters.len()
        );

        // Build CALL statement with placeholders
        let placeholders: Vec<String> = (1..=parameters.len()).map(|i| format!("${}", i)).collect();
        let call_statement = format!("CALL {}({})", procedure_name, placeholders.join(", "));

        tracing::debug!("Call statement: {}", call_statement);

        // Build query with parameters
        let mut query = sqlx::query(&call_statement);
        for param in parameters {
            query = query.bind(param);
        }

        // Execute the stored procedure
        query.execute(pool).await.map_err(|e| {
            ExecutionError::DatabaseQueryFailed(format!(
                "PostgreSQL stored procedure execution failed: {}",
                e
            ))
        })?;

        let result = json!({
            "procedure": procedure_name,
            "parameters": parameters,
            "status": "success",
        });

        tracing::info!("PostgreSQL stored procedure executed successfully");

        Ok(result)
    }

    /// Execute MySQL query
    #[tracing::instrument(skip(self, connection_string, query), fields(database_type = "mysql"))]
    async fn execute_mysql(
        &self,
        connection_string: &str,
        query: &str,
        query_type: &QueryType,
    ) -> Result<serde_json::Value, ExecutionError> {

        tracing::info!("Connecting to MySQL database");

        // Parse connection string
        let opts = mysql_async::Opts::from_url(connection_string).map_err(|e| {
            ExecutionError::DatabaseConnectionFailed(format!(
                "Invalid MySQL connection string: {}",
                e
            ))
        })?;

        // Create connection pool
        let pool = mysql_async::Pool::new(opts);

        // Get connection
        let mut conn = pool.get_conn().await.map_err(|e| {
            ExecutionError::DatabaseConnectionFailed(format!("Failed to connect to MySQL: {}", e))
        })?;

        let result = match query_type {
            QueryType::RawSql => {
                // Requirement 3.10: Execute raw SQL queries on MySQL
                self.execute_raw_sql_mysql(&mut conn, query).await?
            }
            QueryType::StoredProcedure {
                procedure_name,
                parameters,
            } => {
                // Requirement 3.10: Execute stored procedures on MySQL
                self.execute_stored_procedure_mysql(&mut conn, procedure_name, parameters)
                    .await?
            }
        };

        // Disconnect
        drop(conn);
        pool.disconnect().await.map_err(|e| {
            ExecutionError::DatabaseQueryFailed(format!("Failed to disconnect from MySQL: {}", e))
        })?;

        Ok(result)
    }

    /// Execute raw SQL query on MySQL
    #[tracing::instrument(skip(self, conn, query))]
    async fn execute_raw_sql_mysql(
        &self,
        conn: &mut mysql_async::Conn,
        query: &str,
    ) -> Result<serde_json::Value, ExecutionError> {
        use mysql_async::prelude::*;

        tracing::debug!("Executing raw SQL query on MySQL");

        // Execute query and fetch results
        let result: Vec<mysql_async::Row> = conn.query(query).await.map_err(|e| {
            ExecutionError::DatabaseQueryFailed(format!("MySQL query failed: {}", e))
        })?;

        // Convert rows to JSON
        let mut result_rows = Vec::new();
        for row in result {
            let mut row_map = serde_json::Map::new();

            // Get column names from the row
            let columns = row.columns_ref();

            for (i, column) in columns.iter().enumerate() {
                let column_name = column.name_str();

                // Try to get value as different types
                let value: serde_json::Value = if let Some(v) = row.get::<Option<String>, _>(i) {
                    json!(v)
                } else if let Some(v) = row.get::<Option<i64>, _>(i) {
                    json!(v)
                } else if let Some(v) = row.get::<Option<f64>, _>(i) {
                    json!(v)
                } else if let Some(v) = row.get::<Option<bool>, _>(i) {
                    json!(v)
                } else {
                    // For other types (dates, times, etc.), default to null
                    // In production, you might want to handle more types
                    json!(null)
                };

                row_map.insert(column_name.to_string(), value);
            }

            result_rows.push(serde_json::Value::Object(row_map));
        }

        let result = json!({
            "rows": result_rows,
            "row_count": result_rows.len(),
        });

        tracing::info!("MySQL query returned {} rows", result_rows.len());

        Ok(result)
    }

    /// Execute stored procedure on MySQL
    #[tracing::instrument(skip(self, conn, procedure_name, parameters))]
    async fn execute_stored_procedure_mysql(
        &self,
        conn: &mut mysql_async::Conn,
        procedure_name: &str,
        parameters: &[String],
    ) -> Result<serde_json::Value, ExecutionError> {
        use mysql_async::prelude::*;

        tracing::debug!(
            "Executing stored procedure: {} with {} parameters",
            procedure_name,
            parameters.len()
        );

        // Build CALL statement with placeholders
        let placeholders: Vec<String> = parameters.iter().map(|_| "?".to_string()).collect();
        let call_statement = format!("CALL {}({})", procedure_name, placeholders.join(", "));

        tracing::debug!("Call statement: {}", call_statement);

        // Execute the stored procedure
        conn.exec_drop(&call_statement, parameters.to_vec())
            .await
            .map_err(|e| {
                ExecutionError::DatabaseQueryFailed(format!(
                    "MySQL stored procedure execution failed: {}",
                    e
                ))
            })?;

        let result = json!({
            "procedure": procedure_name,
            "parameters": parameters,
            "status": "success",
        });

        tracing::info!("MySQL stored procedure executed successfully");

        Ok(result)
    }

    /// Execute Oracle query
    #[tracing::instrument(skip(self, connection_string, query), fields(database_type = "oracle"))]
    async fn execute_oracle(
        &self,
        connection_string: &str,
        query: &str,
        query_type: &QueryType,
    ) -> Result<serde_json::Value, ExecutionError> {
        tracing::info!("Connecting to Oracle database");

        // Parse connection string (format: username/password@host:port/service_name)
        let parts: Vec<&str> = connection_string.split('@').collect();
        if parts.len() != 2 {
            return Err(ExecutionError::DatabaseConnectionFailed(
                "Invalid Oracle connection string format. Expected: username/password@host:port/service_name".to_string(),
            ));
        }

        let credentials = parts[0];
        let connect_string = parts[1];

        let cred_parts: Vec<&str> = credentials.split('/').collect();
        if cred_parts.len() != 2 {
            return Err(ExecutionError::DatabaseConnectionFailed(
                "Invalid Oracle credentials format. Expected: username/password".to_string(),
            ));
        }

        let username = cred_parts[0];
        let password = cred_parts[1];

        // Create connection
        let conn =
            oracle::Connection::connect(username, password, connect_string).map_err(|e| {
                ExecutionError::DatabaseConnectionFailed(format!(
                    "Failed to connect to Oracle: {}",
                    e
                ))
            })?;

        let result = match query_type {
            QueryType::RawSql => {
                // Requirement 3.8: Execute raw SQL queries on Oracle
                self.execute_raw_sql_oracle(&conn, query).await?
            }
            QueryType::StoredProcedure {
                procedure_name,
                parameters,
            } => {
                // Requirement 3.8: Execute stored procedures on Oracle
                self.execute_stored_procedure_oracle(&conn, procedure_name, parameters)
                    .await?
            }
        };

        // Close connection
        conn.close().map_err(|e| {
            ExecutionError::DatabaseQueryFailed(format!("Failed to close Oracle connection: {}", e))
        })?;

        Ok(result)
    }

    /// Execute raw SQL query on Oracle
    #[tracing::instrument(skip(self, conn, query))]
    async fn execute_raw_sql_oracle(
        &self,
        conn: &oracle::Connection,
        query: &str,
    ) -> Result<serde_json::Value, ExecutionError> {
        tracing::debug!("Executing raw SQL query on Oracle");

        // Execute query
        let rows = conn.query(query, &[]).map_err(|e| {
            ExecutionError::DatabaseQueryFailed(format!("Oracle query failed: {}", e))
        })?;

        // Convert rows to JSON
        let mut result_rows = Vec::new();
        for row_result in rows {
            let row = row_result.map_err(|e| {
                ExecutionError::DatabaseQueryFailed(format!("Failed to fetch Oracle row: {}", e))
            })?;

            let mut row_map = serde_json::Map::new();

            // Get column info
            let column_info = row.column_info();

            for (i, col_info) in column_info.iter().enumerate() {
                let column_name = col_info.name();

                // Try to get value as different types
                let value: serde_json::Value = if let Ok(v) = row.get::<usize, String>(i) {
                    json!(v)
                } else if let Ok(v) = row.get::<usize, i64>(i) {
                    json!(v)
                } else if let Ok(v) = row.get::<usize, f64>(i) {
                    json!(v)
                } else {
                    // For other types, default to null
                    json!(null)
                };

                row_map.insert(column_name.to_string(), value);
            }

            result_rows.push(serde_json::Value::Object(row_map));
        }

        let result = json!({
            "rows": result_rows,
            "row_count": result_rows.len(),
        });

        tracing::info!("Oracle query returned {} rows", result_rows.len());

        Ok(result)
    }

    /// Execute stored procedure on Oracle
    #[tracing::instrument(skip(self, conn, procedure_name, parameters))]
    async fn execute_stored_procedure_oracle(
        &self,
        conn: &oracle::Connection,
        procedure_name: &str,
        parameters: &[String],
    ) -> Result<serde_json::Value, ExecutionError> {
        tracing::debug!(
            "Executing stored procedure: {} with {} parameters",
            procedure_name,
            parameters.len()
        );

        // Build CALL statement
        let placeholders: Vec<String> = (1..=parameters.len()).map(|i| format!(":{}", i)).collect();
        let call_statement = format!(
            "BEGIN {}({}); END;",
            procedure_name,
            placeholders.join(", ")
        );

        tracing::debug!("Call statement: {}", call_statement);

        // Prepare statement
        let mut stmt = conn.statement(&call_statement).build().map_err(|e| {
            ExecutionError::DatabaseQueryFailed(format!(
                "Failed to prepare Oracle statement: {}",
                e
            ))
        })?;

        // Bind parameters
        for (i, param) in parameters.iter().enumerate() {
            stmt.bind(i + 1, param).map_err(|e| {
                ExecutionError::DatabaseQueryFailed(format!(
                    "Failed to bind Oracle parameter {}: {}",
                    i + 1,
                    e
                ))
            })?;
        }

        // Execute the stored procedure
        stmt.execute(&[]).map_err(|e| {
            ExecutionError::DatabaseQueryFailed(format!(
                "Oracle stored procedure execution failed: {}",
                e
            ))
        })?;

        let result = json!({
            "procedure": procedure_name,
            "parameters": parameters,
            "status": "success",
        });

        tracing::info!("Oracle stored procedure executed successfully");

        Ok(result)
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
        // Resolve connection string references
        let resolved_connection_string = self
            .reference_resolver
            .resolve(connection_string, context)
            .map_err(|e| {
                ExecutionError::InvalidJobDefinition(format!(
                    "Failed to resolve connection string references: {}",
                    e
                ))
            })?;

        // Resolve query references
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
                self.execute_postgresql(
                    &resolved_connection_string,
                    &resolved_query,
                    &resolved_query_type,
                )
                .await?
            }
            DatabaseType::MySQL => {
                self.execute_mysql(
                    &resolved_connection_string,
                    &resolved_query,
                    &resolved_query_type,
                )
                .await?
            }
            DatabaseType::Oracle => {
                self.execute_oracle(
                    &resolved_connection_string,
                    &resolved_query,
                    &resolved_query_type,
                )
                .await?
            }
        };

        let completed_at = Utc::now();

        // Create step output
        let step_output = StepOutput {
            step_id: step.id.clone(),
            status: "success".to_string(),
            output,
            started_at,
            completed_at,
        };

        Ok(step_output)
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
