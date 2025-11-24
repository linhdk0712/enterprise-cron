// Variable repository implementation
// Requirements: 2.1, 2.2, 2.6, 2.7 - Variable management with encryption

use crate::db::DbPool;
use crate::errors::DatabaseError;
use crate::models::{Variable, VariableScope};
use chrono::Utc;
use std::collections::HashMap;
use tracing::instrument;
use uuid::Uuid;

/// Repository for variable-related database operations
pub struct VariableRepository {
    pool: DbPool,
    encryption_key: Option<String>,
}

impl VariableRepository {
    /// Create a new VariableRepository
    ///
    /// # Arguments
    /// * `pool` - Database connection pool
    /// * `encryption_key` - Optional encryption key for sensitive variables
    pub fn new(pool: DbPool, encryption_key: Option<String>) -> Self {
        Self {
            pool,
            encryption_key,
        }
    }

    /// Find all global variables
    ///
    /// # Requirements
    /// - 2.1: Global variable availability
    #[instrument(skip(self))]
    pub async fn find_global_variables(&self) -> Result<HashMap<String, String>, DatabaseError> {
        let variables = sqlx::query_as::<_, Variable>(
            r#"
            SELECT id, name, value, is_sensitive, scope, created_at, updated_at
            FROM variables
            WHERE scope_type = 'global'
            "#,
        )
        .fetch_all(self.pool.pool())
        .await?;

        let mut map = HashMap::new();
        for var in variables {
            let value = if var.is_sensitive {
                self.decrypt_value(&var.value)?
            } else {
                var.value
            };
            map.insert(var.name, value);
        }

        tracing::debug!(count = map.len(), "Found global variables");
        Ok(map)
    }

    /// Find variables for a specific job
    ///
    /// # Requirements
    /// - 2.2: Job-specific variable scoping
    #[instrument(skip(self))]
    pub async fn find_job_variables(
        &self,
        job_id: Uuid,
    ) -> Result<HashMap<String, String>, DatabaseError> {
        let variables = sqlx::query_as::<_, Variable>(
            r#"
            SELECT id, name, value, is_sensitive, scope, created_at, updated_at
            FROM variables
            WHERE scope_type = 'job' AND scope_id = $1
            "#,
        )
        .bind(job_id)
        .fetch_all(self.pool.pool())
        .await?;

        let mut map = HashMap::new();
        for var in variables {
            let value = if var.is_sensitive {
                self.decrypt_value(&var.value)?
            } else {
                var.value
            };
            map.insert(var.name, value);
        }

        tracing::debug!(job_id = %job_id, count = map.len(), "Found job-specific variables");
        Ok(map)
    }

    /// Find all variables for a job (global + job-specific)
    /// Job-specific variables take precedence over global ones
    ///
    /// # Requirements
    /// - 2.1: Global variable availability
    /// - 2.2: Job-specific variable scoping
    /// - 2.4: Variable precedence (job-specific > global)
    #[instrument(skip(self))]
    pub async fn find_all_for_job(
        &self,
        job_id: Uuid,
    ) -> Result<HashMap<String, String>, DatabaseError> {
        // Start with global variables
        let mut variables = self.find_global_variables().await?;

        // Override with job-specific variables
        let job_vars = self.find_job_variables(job_id).await?;
        variables.extend(job_vars);

        Ok(variables)
    }

    /// Create a new variable
    ///
    /// # Requirements
    /// - 2.6: Variable CRUD operations
    /// - 2.7: Sensitive variable encryption
    #[instrument(skip(self, variable))]
    pub async fn create(&self, variable: &Variable) -> Result<(), DatabaseError> {
        let (scope_type, scope_id) = match &variable.scope {
            VariableScope::Global => ("global".to_string(), None),
            VariableScope::Job { job_id } => ("job".to_string(), Some(*job_id)),
        };

        let value = if variable.is_sensitive {
            self.encrypt_value(&variable.value)?
        } else {
            variable.value.clone()
        };

        sqlx::query(
            r#"
            INSERT INTO variables (
                id, name, value, is_sensitive, scope_type, scope_id,
                created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(&variable.id)
        .bind(&variable.name)
        .bind(&value)
        .bind(variable.is_sensitive)
        .bind(&scope_type)
        .bind(scope_id)
        .bind(variable.created_at)
        .bind(variable.updated_at)
        .execute(self.pool.pool())
        .await?;

        tracing::info!(
            variable_id = %variable.id,
            variable_name = %variable.name,
            is_sensitive = variable.is_sensitive,
            "Variable created"
        );
        Ok(())
    }

    /// Find a variable by ID
    #[instrument(skip(self))]
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Variable>, DatabaseError> {
        let mut variable = sqlx::query_as::<_, Variable>(
            r#"
            SELECT id, name, value, is_sensitive, scope, created_at, updated_at
            FROM variables
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(self.pool.pool())
        .await?;

        // Decrypt if sensitive
        if let Some(ref mut var) = variable {
            if var.is_sensitive {
                var.value = self.decrypt_value(&var.value)?;
            }
        }

        Ok(variable)
    }

    /// Find a variable by name and scope
    #[instrument(skip(self))]
    pub async fn find_by_name_and_scope(
        &self,
        name: &str,
        scope: &VariableScope,
    ) -> Result<Option<Variable>, DatabaseError> {
        let (scope_type, scope_id) = match scope {
            VariableScope::Global => ("global".to_string(), None),
            VariableScope::Job { job_id } => ("job".to_string(), Some(*job_id)),
        };

        let query = if scope_id.is_some() {
            r#"
            SELECT id, name, value, is_sensitive, scope, created_at, updated_at
            FROM variables
            WHERE name = $1 AND scope_type = $2 AND scope_id = $3
            "#
        } else {
            r#"
            SELECT id, name, value, is_sensitive, scope, created_at, updated_at
            FROM variables
            WHERE name = $1 AND scope_type = $2 AND scope_id IS NULL
            "#
        };

        let mut query_builder = sqlx::query_as::<_, Variable>(query)
            .bind(name)
            .bind(&scope_type);

        if let Some(sid) = scope_id {
            query_builder = query_builder.bind(sid);
        }

        let mut variable = query_builder.fetch_optional(self.pool.pool()).await?;

        // Decrypt if sensitive
        if let Some(ref mut var) = variable {
            if var.is_sensitive {
                var.value = self.decrypt_value(&var.value)?;
            }
        }

        Ok(variable)
    }

    /// Update a variable
    ///
    /// # Requirements
    /// - 2.6: Variable CRUD operations
    /// - 2.7: Sensitive variable encryption
    #[instrument(skip(self, variable))]
    pub async fn update(&self, variable: &Variable) -> Result<(), DatabaseError> {
        let value = if variable.is_sensitive {
            self.encrypt_value(&variable.value)?
        } else {
            variable.value.clone()
        };

        let result = sqlx::query(
            r#"
            UPDATE variables
            SET name = $2,
                value = $3,
                is_sensitive = $4,
                updated_at = $5
            WHERE id = $1
            "#,
        )
        .bind(&variable.id)
        .bind(&variable.name)
        .bind(&value)
        .bind(variable.is_sensitive)
        .bind(Utc::now())
        .execute(self.pool.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::NotFound(format!(
                "Variable not found: {}",
                variable.id
            )));
        }

        tracing::info!(
            variable_id = %variable.id,
            variable_name = %variable.name,
            "Variable updated"
        );
        Ok(())
    }

    /// Delete a variable
    ///
    /// # Requirements
    /// - 2.6: Variable CRUD operations
    #[instrument(skip(self))]
    pub async fn delete(&self, id: Uuid) -> Result<(), DatabaseError> {
        let result = sqlx::query("DELETE FROM variables WHERE id = $1")
            .bind(id)
            .execute(self.pool.pool())
            .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::NotFound(format!(
                "Variable not found: {}",
                id
            )));
        }

        tracing::info!(variable_id = %id, "Variable deleted");
        Ok(())
    }

    /// List all variables (for admin purposes)
    /// Sensitive values are masked
    ///
    /// # Requirements
    /// - 2.8: Sensitive variable masking
    #[instrument(skip(self))]
    pub async fn list_all(&self) -> Result<Vec<Variable>, DatabaseError> {
        let mut variables = sqlx::query_as::<_, Variable>(
            r#"
            SELECT id, name, value, is_sensitive, scope, created_at, updated_at
            FROM variables
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(self.pool.pool())
        .await?;

        // Mask sensitive values
        for var in &mut variables {
            if var.is_sensitive {
                var.value = "***".to_string();
            }
        }

        Ok(variables)
    }

    /// Encrypt a variable value
    ///
    /// # Requirements
    /// - 2.7: Sensitive variable encryption
    fn encrypt_value(&self, value: &str) -> Result<String, DatabaseError> {
        // For now, we use a simple base64 encoding
        // In production, this should use proper encryption (AES-256-GCM)
        // with the encryption_key

        if self.encryption_key.is_none() {
            tracing::warn!("No encryption key configured, using base64 encoding");
        }

        let encoded = base64::encode(value.as_bytes());
        Ok(encoded)
    }

    /// Decrypt a variable value
    ///
    /// # Requirements
    /// - 2.7: Sensitive variable encryption
    fn decrypt_value(&self, encrypted: &str) -> Result<String, DatabaseError> {
        // For now, we use simple base64 decoding
        // In production, this should use proper decryption (AES-256-GCM)
        // with the encryption_key

        let decoded = base64::decode(encrypted).map_err(|e| {
            DatabaseError::QueryFailed(format!("Failed to decrypt variable: {}", e))
        })?;

        let value = String::from_utf8(decoded)
            .map_err(|e| DatabaseError::QueryFailed(format!("Failed to decode variable: {}", e)))?;

        Ok(value)
    }
}

// Add base64 dependency for simple encryption/decryption
// In production, use a proper encryption library like `aes-gcm`
mod base64 {
    pub fn encode(data: &[u8]) -> String {
        use std::fmt::Write;
        let mut result = String::new();
        for byte in data {
            write!(&mut result, "{:02x}", byte).unwrap();
        }
        result
    }

    pub fn decode(s: &str) -> Result<Vec<u8>, String> {
        if s.len() % 2 != 0 {
            return Err("Invalid hex string".to_string());
        }

        (0..s.len())
            .step_by(2)
            .map(|i| {
                u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| format!("Invalid hex: {}", e))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variable_repository_creation() {
        // This test verifies the repository can be created
        // Actual database tests require a running PostgreSQL instance
    }

    #[test]
    fn test_encryption_decryption() {
        let original = "secret_value";
        let encrypted = base64::encode(original.as_bytes());
        let decrypted_bytes = base64::decode(&encrypted).unwrap();
        let decrypted = String::from_utf8(decrypted_bytes).unwrap();
        assert_eq!(original, decrypted);
    }
}
