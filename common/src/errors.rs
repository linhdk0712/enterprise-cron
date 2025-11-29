// Error handling framework
// Requirements: 8.1, 8.2, 8.3

use thiserror::Error;

/// Schedule-related errors
#[derive(Error, Debug)]
pub enum ScheduleError {
    #[error("Invalid cron expression '{expression}': {reason}")]
    InvalidCronExpression { expression: String, reason: String },

    #[error("Invalid timezone: {0}")]
    InvalidTimezone(String),

    #[error("Invalid schedule configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Schedule calculation failed: {0}")]
    CalculationFailed(String),

    #[error("No next execution time available for {schedule_type} schedule")]
    NoNextExecution { schedule_type: String },
}

/// Job execution errors
#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("Job not found: {0}")]
    JobNotFound(String),

    #[error("Execution timeout after {0} seconds")]
    Timeout(u64),

    #[error("Maximum retries ({0}) exceeded")]
    MaxRetriesExceeded(u32),

    #[error("Idempotency key conflict: {0}")]
    IdempotencyKeyConflict(String),

    #[error("HTTP request failed: {0}")]
    HttpRequestFailed(String),

    #[error("Database connection failed: {0}")]
    DatabaseConnectionFailed(String),

    #[error("Database query failed: {0}")]
    DatabaseQueryFailed(String),

    #[error("File processing failed: {0}")]
    FileProcessingFailed(String),

    #[error("Invalid job type: {0}")]
    InvalidJobType(String),

    #[error("SFTP operation failed: {0}")]
    SftpOperationFailed(String),

    #[error("SFTP connection failed: {0}")]
    SftpConnectionFailed(String),

    #[error("SFTP authentication failed: {0}")]
    SftpAuthenticationFailed(String),

    #[error("SFTP file not found: {0}")]
    SftpFileNotFound(String),

    #[error("Storage operation failed: {0}")]
    StorageFailed(String),

    #[error("Variable resolution failed: {0}")]
    VariableResolutionFailed(String),

    #[error("Step output reference not found: {0}")]
    StepOutputNotFound(String),

    #[error("Invalid job definition: {0}")]
    InvalidJobDefinition(String),

    #[error("Circuit breaker open for: {0}")]
    CircuitBreakerOpen(String),

    #[error("Failed to load job context: {0}")]
    ContextLoadFailed(String),

    #[error("Failed to save job context: {0}")]
    ContextSaveFailed(String),
}

/// Authentication and authorization errors
#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials,

    #[error("Invalid JWT token: {0}")]
    InvalidToken(String),

    #[error("Token expired")]
    TokenExpired,

    #[error("Insufficient permissions: required {0}")]
    InsufficientPermissions(String),

    #[error("User not found: {0}")]
    UserNotFound(String),

    #[error("Keycloak error: {0}")]
    KeycloakError(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
}

/// Validation errors
#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid field value for {field}: {reason}")]
    InvalidFieldValue { field: String, reason: String },

    #[error("Invalid JSON: {0}")]
    InvalidJson(String),

    #[error("Schema validation failed: {0}")]
    SchemaValidationFailed(String),

    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),
}

/// Database-specific errors
#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("Database connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Database health check failed: {0}")]
    HealthCheckFailed(String),

    #[error("Query execution failed: {0}")]
    QueryFailed(String),

    #[error("Transaction failed: {0}")]
    TransactionFailed(String),

    #[error("Record not found: {0}")]
    NotFound(String),

    #[error("Duplicate key violation: {0}")]
    DuplicateKey(String),

    #[error("Foreign key violation: {0}")]
    ForeignKeyViolation(String),

    #[error("Migration failed: {0}")]
    MigrationFailed(String),
}

/// Storage errors
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Redis error: {0}")]
    RedisError(String),

    #[error("NATS error: {0}")]
    NatsError(String),

    #[error("MinIO error: {0}")]
    MinioError(String),

    #[error("Filesystem error: {0}")]
    FileSystemError(String),

    #[error("Invalid JSON: {0}")]
    InvalidJson(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Query failed: {0}")]
    QueryFailed(String),
}

/// Webhook errors
#[derive(Error, Debug)]
pub enum WebhookError {
    #[error("Invalid webhook signature")]
    InvalidSignature,

    #[error("Webhook not found: {0}")]
    WebhookNotFound(String),

    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    #[error("Webhook disabled")]
    WebhookDisabled,

    #[error("Invalid webhook payload: {0}")]
    InvalidPayload(String),
}

/// Queue-related errors
#[derive(Error, Debug)]
pub enum QueueError {
    #[error("Failed to connect to queue: {0}")]
    Connection(String),

    #[error("Failed to create stream: {0}")]
    StreamCreation(String),

    #[error("Stream not found: {0}")]
    StreamNotFound(String),

    #[error("Failed to create consumer: {0}")]
    ConsumerCreation(String),

    #[error("Failed to publish message: {0}")]
    PublishFailed(String),

    #[error("Failed to consume message: {0}")]
    ConsumeFailed(String),

    #[error("Failed to acknowledge message: {0}")]
    AckFailed(String),

    #[error("Message serialization failed: {0}")]
    SerializationFailed(String),

    #[error("Message deserialization failed: {0}")]
    DeserializationFailed(String),

    #[error("Health check failed: {0}")]
    HealthCheck(String),

    #[error("Queue operation timeout: {0}")]
    Timeout(String),
}

/// Variable substitution errors
#[derive(Error, Debug)]
pub enum SubstitutionError {
    #[error("Undefined variable(s) in template: {variables:?}. Template: {template}")]
    UndefinedVariable {
        variables: Vec<String>,
        template: String,
    },

    #[error("Regex compilation error: {0}")]
    RegexError(String),

    #[error("Substitution failed: {0}")]
    SubstitutionFailed(String),
}

/// API response error type for HTTP responses
#[derive(Debug, serde::Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }
}

impl From<ScheduleError> for ApiError {
    fn from(err: ScheduleError) -> Self {
        ApiError::new("SCHEDULE_ERROR", err.to_string())
    }
}

impl From<ExecutionError> for ApiError {
    fn from(err: ExecutionError) -> Self {
        ApiError::new("EXECUTION_ERROR", err.to_string())
    }
}

impl From<AuthError> for ApiError {
    fn from(err: AuthError) -> Self {
        let code = match err {
            AuthError::InvalidCredentials
            | AuthError::InvalidToken(_)
            | AuthError::TokenExpired => "UNAUTHORIZED",
            AuthError::InsufficientPermissions(_) => "FORBIDDEN",
            _ => "AUTH_ERROR",
        };
        ApiError::new(code, err.to_string())
    }
}

impl From<ValidationError> for ApiError {
    fn from(err: ValidationError) -> Self {
        ApiError::new("VALIDATION_ERROR", err.to_string())
    }
}

impl From<StorageError> for ApiError {
    fn from(err: StorageError) -> Self {
        ApiError::new("STORAGE_ERROR", err.to_string())
    }
}

impl From<WebhookError> for ApiError {
    fn from(err: WebhookError) -> Self {
        let code = match err {
            WebhookError::InvalidSignature => "UNAUTHORIZED",
            WebhookError::RateLimitExceeded => "RATE_LIMIT_EXCEEDED",
            WebhookError::WebhookDisabled => "FORBIDDEN",
            _ => "WEBHOOK_ERROR",
        };
        ApiError::new(code, err.to_string())
    }
}

impl From<SubstitutionError> for ApiError {
    fn from(err: SubstitutionError) -> Self {
        ApiError::new("SUBSTITUTION_ERROR", err.to_string())
    }
}

impl From<QueueError> for ApiError {
    fn from(err: QueueError) -> Self {
        ApiError::new("QUEUE_ERROR", err.to_string())
    }
}

// Implement From for common external errors
impl From<sqlx::Error> for DatabaseError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => DatabaseError::NotFound("Record not found".to_string()),
            sqlx::Error::Database(db_err) => {
                // Check for specific database error codes
                if let Some(code) = db_err.code() {
                    match code.as_ref() {
                        "23505" => DatabaseError::DuplicateKey(db_err.message().to_string()),
                        "23503" => DatabaseError::ForeignKeyViolation(db_err.message().to_string()),
                        _ => DatabaseError::QueryFailed(db_err.message().to_string()),
                    }
                } else {
                    DatabaseError::QueryFailed(db_err.message().to_string())
                }
            }
            _ => DatabaseError::QueryFailed(err.to_string()),
        }
    }
}

impl From<sqlx::Error> for StorageError {
    fn from(err: sqlx::Error) -> Self {
        StorageError::DatabaseError(err.to_string())
    }
}

impl From<DatabaseError> for StorageError {
    fn from(err: DatabaseError) -> Self {
        StorageError::DatabaseError(err.to_string())
    }
}

impl From<redis::RedisError> for StorageError {
    fn from(err: redis::RedisError) -> Self {
        StorageError::RedisError(err.to_string())
    }
}

impl From<serde_json::Error> for ValidationError {
    fn from(err: serde_json::Error) -> Self {
        ValidationError::InvalidJson(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schedule_error_display() {
        let err = ScheduleError::InvalidCronExpression {
            expression: "* * * *".to_string(),
            reason: "invalid format".to_string(),
        };
        assert!(err.to_string().contains("Invalid cron expression"));
    }

    #[test]
    fn test_execution_error_timeout() {
        let err = ExecutionError::Timeout(300);
        assert!(err.to_string().contains("300 seconds"));
    }

    #[test]
    fn test_auth_error_to_api_error() {
        let err = AuthError::InvalidCredentials;
        let api_err: ApiError = err.into();
        assert_eq!(api_err.code, "UNAUTHORIZED");
    }

    #[test]
    fn test_api_error_with_details() {
        let err = ApiError::new("TEST_ERROR", "Test message")
            .with_details(serde_json::json!({"field": "value"}));
        assert!(err.details.is_some());
    }
}
