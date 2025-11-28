// MySQL executor implementation
// Requirements: 3.10 - Execute MySQL queries and stored procedures

use crate::errors::ExecutionError;
use crate::models::QueryType;
use serde_json::json;
use std::time::Duration;

/// MySQL query executor
pub struct MySQLExecutor {
    timeout: Duration,
}

impl MySQLExecutor {
    /// Create a new MySQL executor
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    /// Execute MySQL query
    #[tracing::instrument(skip(self, connection_string, query), fields(database_type = "mysql"))]
    pub async fn execute_query(
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
            QueryType::RawSql => self.execute_raw_sql(&mut conn, query).await?,
            QueryType::StoredProcedure {
                procedure_name,
                parameters,
            } => self.execute_stored_procedure(&mut conn, procedure_name, parameters).await?,
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
    async fn execute_raw_sql(
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
    async fn execute_stored_procedure(
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
}
