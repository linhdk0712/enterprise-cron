// PostgreSQL executor implementation
// Requirements: 3.9 - Execute PostgreSQL queries and stored procedures

use crate::errors::ExecutionError;
use crate::models::QueryType;
use chrono::Utc;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Column, PgPool, Row};
use std::time::Duration;

/// PostgreSQL query executor
pub struct PostgreSQLExecutor {
    timeout: Duration,
}

impl PostgreSQLExecutor {
    /// Create a new PostgreSQL executor
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }

    /// Execute PostgreSQL query
    #[tracing::instrument(
        skip(self, connection_string, query),
        fields(database_type = "postgresql")
    )]
    pub async fn execute_query(
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
            QueryType::RawSql => self.execute_raw_sql(&pool, query).await?,
            QueryType::StoredProcedure {
                procedure_name,
                parameters,
            } => {
                self.execute_stored_procedure(&pool, procedure_name, parameters)
                    .await?
            }
        };

        // Close the pool
        pool.close().await;

        Ok(result)
    }

    /// Execute raw SQL query on PostgreSQL
    #[tracing::instrument(skip(self, pool, query))]
    async fn execute_raw_sql(
        &self,
        pool: &PgPool,
        query: &str,
    ) -> Result<serde_json::Value, ExecutionError> {
        tracing::debug!("Executing raw SQL query");

        let rows = sqlx::query(query).fetch_all(pool).await.map_err(|e| {
            ExecutionError::DatabaseQueryFailed(format!("PostgreSQL query failed: {}", e))
        })?;

        let mut result_rows = Vec::new();
        for row in rows {
            let mut row_map = serde_json::Map::new();

            for (i, column) in row.columns().iter().enumerate() {
                let column_name = column.name();

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
    async fn execute_stored_procedure(
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
}
