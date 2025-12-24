// SQL query constants for repositories
// Centralizes repeated SELECT column lists to follow DRY principle
// RECC 2025: Single source of truth for database queries

/// SQL query fragments for job_executions table
pub mod execution_queries {
    /// All columns for job_executions with NULL handling via COALESCE
    ///
    /// # NULL Handling
    /// - trigger_metadata: COALESCE to 'null'::jsonb (prevents deserialization errors)
    /// - context: COALESCE to '{}'::jsonb (empty object for missing context)
    ///
    /// # Requirements
    /// - 3.12: Execution history persistence
    /// - 4.3: Idempotency key tracking
    pub const SELECT_ALL_COLUMNS: &str = r#"id, job_id, idempotency_key, status, attempt,
        trigger_source, 
        COALESCE(trigger_metadata, 'null'::jsonb) as trigger_metadata,
        current_step, 
        COALESCE(context, '{}'::jsonb) as context,
        started_at, completed_at, result, error, created_at"#;
}

/// SQL query fragments for jobs table
pub mod job_queries {
    /// All columns for jobs table
    ///
    /// # Requirements
    /// - 3.11: Job CRUD operations
    /// - 17.1, 17.2: Trigger configuration storage
    pub const SELECT_ALL_COLUMNS: &str = r#"id, name, description, enabled, timeout_seconds,
        max_retries, allow_concurrent, definition,
        trigger_config, created_at, updated_at"#;
}

/// SQL query fragments for variables table
pub mod variable_queries {
    /// All columns for variables table
    ///
    /// # Requirements
    /// - 2.1, 2.2: Variable management
    pub const SELECT_ALL_COLUMNS: &str =
        "id, name, value, is_sensitive, scope, created_at, updated_at";
}

/// SQL query fragments for users table
pub mod user_queries {
    /// All columns for users table
    ///
    /// # Requirements
    /// - 10.2, 10.13: User authentication and management
    pub const SELECT_ALL_COLUMNS: &str =
        "id, username, password_hash, email, enabled, created_at, updated_at";
}

/// SQL query fragments for roles table
pub mod role_queries {
    /// All columns for roles table
    ///
    /// # Requirements
    /// - 10.13: Role and permission management
    pub const SELECT_ALL_COLUMNS: &str = "id, name, permissions, created_at";
}
