// Oracle executor implementation
// Requirements: 3.8 - Execute Oracle queries and stored procedures

use crate::errors::ExecutionError;
use crate::models::QueryType;
use serde_json::json;
use std::time::Duration;

/// Oracle query executor
pub struct OracleExecutor {
    _timeout: Duration,
}

impl OracleExecutor {
    /// Create a new Oracle executor
    pub fn new(timeout: Duration) -> Self {
        Self { _timeout: timeout }
    }

    /// Execute Oracle query
    #[tracing::instrument(skip(self, connection_string, query), fields(database_type = "oracle"))]
    pub async fn execute_query(
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
            QueryType::RawSql => self.execute_raw_sql(&conn, query).await?,
            QueryType::StoredProcedure {
                procedure_name,
                parameters,
            } => {
                self.execute_stored_procedure(&conn, procedure_name, parameters)
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
    async fn execute_raw_sql(
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
    async fn execute_stored_procedure(
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
