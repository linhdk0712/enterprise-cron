use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sqlx::FromRow;
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

// Helper functions for Tz serialization
fn serialize_tz<S>(tz: &Tz, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&tz.to_string())
}

fn deserialize_tz<'de, D>(deserializer: D) -> Result<Tz, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Tz::from_str(&s).map_err(serde::de::Error::custom)
}

// ============================================================================
// Job Models
// ============================================================================

/// Job represents a scheduled task definition
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Job {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    #[sqlx(skip)]
    pub schedule: Option<Schedule>,
    #[sqlx(skip)]
    pub steps: Vec<JobStep>,
    #[sqlx(skip)]
    pub triggers: TriggerConfig,
    pub enabled: bool,
    pub timeout_seconds: i32,
    pub max_retries: i32,
    pub allow_concurrent: bool,
    #[sqlx(json)]
    pub definition: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// JobStep represents a single step in a multi-step job
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStep {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub step_type: JobType,
    pub condition: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_failure: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_count: Option<i32>,
}

/// TriggerConfig defines how a job can be triggered
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TriggerConfig {
    pub scheduled: bool,
    pub manual: bool,
    pub webhook: Option<WebhookConfig>,
}

/// WebhookConfig contains webhook trigger configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookConfig {
    pub enabled: bool,
    pub url: String,
    pub secret_key: String,
    pub rate_limit: Option<RateLimit>,
}

/// RateLimit defines rate limiting for webhooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    pub max_requests: u32,
    pub window_seconds: u32,
}

/// Schedule defines when a job should execute
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Schedule {
    Cron {
        expression: String,
        #[serde(serialize_with = "serialize_tz", deserialize_with = "deserialize_tz")]
        timezone: Tz,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        end_date: Option<DateTime<Utc>>,
    },
    FixedDelay {
        delay_seconds: u32,
    },
    FixedRate {
        interval_seconds: u32,
    },
    OneTime {
        execute_at: DateTime<Utc>,
    },
}

/// JobType defines the type of operation a job step performs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum JobType {
    HttpRequest {
        method: HttpMethod,
        url: String,
        headers: HashMap<String, String>,
        body: Option<String>,
        auth: Option<HttpAuth>,
    },
    DatabaseQuery {
        database_type: DatabaseType,
        connection_string: String,
        query: String,
        query_type: QueryType,
    },
    FileProcessing {
        operation: FileOperation,
        format: FileFormat,
        source_path: Option<String>,
        destination_path: Option<String>,
        options: FileProcessingOptions,
    },
    Sftp {
        operation: SftpOperation,
        host: String,
        port: u16,
        auth: SftpAuth,
        remote_path: String,
        local_path: Option<String>,
        options: SftpOptions,
    },
}

/// HttpMethod represents HTTP request methods
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
}

/// HttpAuth represents HTTP authentication methods
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HttpAuth {
    Basic {
        username: String,
        password: String,
    },
    Bearer {
        token: String,
    },
    OAuth2 {
        client_id: String,
        client_secret: String,
        token_url: String,
    },
}

/// DatabaseType represents supported database systems
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseType {
    #[serde(rename = "postgresql")]
    PostgreSQL,
    #[serde(rename = "mysql")]
    MySQL,
    #[serde(rename = "oracle")]
    Oracle,
}

/// QueryType represents types of database queries
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QueryType {
    RawSql,
    StoredProcedure {
        procedure_name: String,
        parameters: Vec<String>,
    },
}

/// FileOperation represents file processing operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileOperation {
    Read,
    Write,
}

/// FileFormat represents supported file formats
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FileFormat {
    Excel,
    Csv { delimiter: char },
}

/// FileProcessingOptions contains options for file processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileProcessingOptions {
    pub sheet_name: Option<String>,
    pub sheet_index: Option<usize>,
    pub transformations: Vec<DataTransformation>,
    pub streaming: bool,
}

/// DataTransformation represents data transformation rules
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DataTransformation {
    ColumnMapping { from: String, to: String },
    TypeConversion { column: String, target_type: String },
    Filter { condition: String },
}

/// SftpOperation represents SFTP operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SftpOperation {
    Download,
    Upload,
}

/// SftpAuth represents SFTP authentication methods
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SftpAuth {
    Password {
        username: String,
        password: String,
    },
    SshKey {
        username: String,
        private_key_path: String,
    },
}

/// SftpOptions contains options for SFTP operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftpOptions {
    pub wildcard_pattern: Option<String>,
    pub recursive: bool,
    pub create_directories: bool,
    pub verify_host_key: bool,
}

// ============================================================================
// JobExecution Models
// ============================================================================

/// JobExecution represents a single execution instance of a job
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct JobExecution {
    pub id: Uuid,
    pub job_id: Uuid,
    pub idempotency_key: String,
    #[sqlx(try_from = "String")]
    pub status: ExecutionStatus,
    pub attempt: i32,
    #[sqlx(try_from = "String")]
    pub trigger_source: TriggerSource,
    #[sqlx(default, json)]
    pub trigger_metadata: Option<serde_json::Value>,
    pub current_step: Option<String>,
    #[sqlx(json)]
    pub context: serde_json::Value,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub result: Option<String>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl JobExecution {
    /// Create a new pending execution for scheduled trigger
    ///
    /// Requirements: 3.12, 4.3 - Create execution with idempotency key
    ///
    /// # Arguments
    /// * `job_id` - The job ID to execute
    /// * `idempotency_key` - Unique key for deduplication
    ///
    /// # Returns
    /// A new JobExecution in Pending status with Scheduled trigger source
    pub fn new_scheduled(job_id: Uuid, idempotency_key: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            job_id,
            idempotency_key,
            status: ExecutionStatus::Pending,
            attempt: 1,
            trigger_source: TriggerSource::Scheduled,
            trigger_metadata: None,
            current_step: None,
            context: serde_json::json!({}),
            started_at: None,
            completed_at: None,
            result: None,
            error: None,
            created_at: Utc::now(),
        }
    }

    /// Create a new pending execution for manual trigger
    ///
    /// Requirements: 6.4, 17.9 - Manual job triggering
    ///
    /// # Arguments
    /// * `job_id` - The job ID to execute
    /// * `user_id` - The user who triggered the job
    ///
    /// # Returns
    /// A new JobExecution in Pending status with Manual trigger source
    pub fn new_manual(job_id: Uuid, user_id: String) -> Self {
        let execution_id = Uuid::new_v4();
        let idempotency_key = format!("manual-{}-{}", job_id, execution_id);

        Self {
            id: execution_id,
            job_id,
            idempotency_key,
            status: ExecutionStatus::Pending,
            attempt: 1,
            trigger_source: TriggerSource::Manual { user_id },
            trigger_metadata: None,
            current_step: None,
            context: serde_json::json!({}),
            started_at: None,
            completed_at: None,
            result: None,
            error: None,
            created_at: Utc::now(),
        }
    }

    /// Create a new pending execution for webhook trigger
    ///
    /// Requirements: 16.1, 16.9 - Webhook-triggered job execution
    ///
    /// # Arguments
    /// * `job_id` - The job ID to execute
    /// * `webhook_url` - The webhook URL that triggered the job
    /// * `webhook_data` - Optional webhook payload and metadata
    ///
    /// # Returns
    /// A new JobExecution in Pending status with Webhook trigger source
    pub fn new_webhook(
        job_id: Uuid,
        webhook_url: String,
        webhook_data: Option<serde_json::Value>,
    ) -> Self {
        let execution_id = Uuid::new_v4();
        let idempotency_key = format!("webhook-{}-{}", job_id, execution_id);

        Self {
            id: execution_id,
            job_id,
            idempotency_key,
            status: ExecutionStatus::Pending,
            attempt: 1,
            trigger_source: TriggerSource::Webhook { webhook_url },
            trigger_metadata: webhook_data,
            current_step: None,
            context: serde_json::json!({}),
            started_at: None,
            completed_at: None,
            result: None,
            error: None,
            created_at: Utc::now(),
        }
    }

    /// Create a new execution with custom parameters (for advanced use cases)
    ///
    /// # Arguments
    /// * `job_id` - The job ID to execute
    /// * `idempotency_key` - Unique key for deduplication
    /// * `trigger_source` - How the job was triggered
    /// * `attempt` - Attempt number (default: 1)
    ///
    /// # Returns
    /// A new JobExecution with specified parameters
    pub fn new_with_params(
        job_id: Uuid,
        idempotency_key: String,
        trigger_source: TriggerSource,
        attempt: i32,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            job_id,
            idempotency_key,
            status: ExecutionStatus::Pending,
            attempt,
            trigger_source,
            trigger_metadata: None,
            current_step: None,
            context: serde_json::json!({}),
            started_at: None,
            completed_at: None,
            result: None,
            error: None,
            created_at: Utc::now(),
        }
    }
}

/// ExecutionStatus represents the status of a job execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Pending,
    Running,
    Success,
    Failed,
    Timeout,
    DeadLetter,
    Cancelling,
    Cancelled,
}

impl std::fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionStatus::Pending => write!(f, "pending"),
            ExecutionStatus::Running => write!(f, "running"),
            ExecutionStatus::Success => write!(f, "success"),
            ExecutionStatus::Failed => write!(f, "failed"),
            ExecutionStatus::Timeout => write!(f, "timeout"),
            ExecutionStatus::DeadLetter => write!(f, "dead_letter"),
            ExecutionStatus::Cancelling => write!(f, "cancelling"),
            ExecutionStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl FromStr for ExecutionStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(ExecutionStatus::Pending),
            "running" => Ok(ExecutionStatus::Running),
            "success" => Ok(ExecutionStatus::Success),
            "failed" => Ok(ExecutionStatus::Failed),
            "timeout" => Ok(ExecutionStatus::Timeout),
            "dead_letter" => Ok(ExecutionStatus::DeadLetter),
            "cancelling" => Ok(ExecutionStatus::Cancelling),
            "cancelled" => Ok(ExecutionStatus::Cancelled),
            _ => Err(format!("Invalid execution status: {}", s)),
        }
    }
}

impl TryFrom<String> for ExecutionStatus {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::from_str(&s)
    }
}

/// TriggerSource represents how a job execution was triggered
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerSource {
    Scheduled,
    Manual { user_id: String },
    Webhook { webhook_url: String },
}

impl std::fmt::Display for TriggerSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TriggerSource::Scheduled => write!(f, "scheduled"),
            TriggerSource::Manual { .. } => write!(f, "manual"),
            TriggerSource::Webhook { .. } => write!(f, "webhook"),
        }
    }
}

impl FromStr for TriggerSource {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // For database storage, we store just the type string
        // The full data is stored in a separate JSONB column
        match s {
            "scheduled" => Ok(TriggerSource::Scheduled),
            "manual" => Ok(TriggerSource::Manual {
                user_id: String::new(),
            }),
            "webhook" => Ok(TriggerSource::Webhook {
                webhook_url: String::new(),
            }),
            _ => Err(format!("Invalid trigger source: {}", s)),
        }
    }
}

impl TryFrom<String> for TriggerSource {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::from_str(&s)
    }
}

/// JobContext stores intermediate results and data for multi-step jobs
/// Requirements: 13.5, 13.6, 13.7 - Store step outputs, webhook data, and file metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobContext {
    pub execution_id: Uuid,
    pub job_id: Uuid,
    pub variables: HashMap<String, serde_json::Value>,
    pub steps: HashMap<String, StepOutput>,
    pub webhook: Option<WebhookData>,
    pub files: Vec<FileMetadata>,
}

impl JobContext {
    /// Create a new JobContext for a job execution
    /// Requirements: 13.7 - Initialize Job Context for new executions
    pub fn new(execution_id: Uuid, job_id: Uuid) -> Self {
        Self {
            execution_id,
            job_id,
            variables: HashMap::new(),
            steps: HashMap::new(),
            webhook: None,
            files: Vec::new(),
        }
    }

    /// Get step output by step ID
    /// Requirements: 13.6, 14.1 - Access step outputs for reference resolution
    pub fn get_step_output(&self, step_id: &str) -> Option<&StepOutput> {
        self.steps.get(step_id)
    }

    /// Add or update step output
    /// Requirements: 13.5, 13.6, 14.5 - Store step outputs automatically
    pub fn set_step_output(&mut self, step_id: String, output: StepOutput) {
        self.steps.insert(step_id, output);
    }

    /// Get variable value by name
    /// Requirements: 2.3 - Variable resolution from Job Context
    pub fn get_variable(&self, name: &str) -> Option<&serde_json::Value> {
        self.variables.get(name)
    }

    /// Set variable value
    /// Requirements: 2.3 - Store variables in Job Context
    pub fn set_variable(&mut self, name: String, value: serde_json::Value) {
        self.variables.insert(name, value);
    }

    /// Get webhook data
    /// Requirements: 16.3, 16.4, 16.5 - Access webhook payload, query params, headers
    pub fn get_webhook_data(&self) -> Option<&WebhookData> {
        self.webhook.as_ref()
    }

    /// Set webhook data
    /// Requirements: 16.3, 16.4, 16.5 - Store webhook data in Job Context
    pub fn set_webhook_data(&mut self, webhook_data: WebhookData) {
        self.webhook = Some(webhook_data);
    }

    /// Add file metadata
    /// Requirements: 15.10, 19.8, 19.9 - Store file metadata in Job Context
    pub fn add_file_metadata(&mut self, metadata: FileMetadata) {
        self.files.push(metadata);
    }

    /// Get all file metadata
    /// Requirements: 15.10, 19.8, 19.9 - Access file metadata
    pub fn get_files(&self) -> &[FileMetadata] {
        &self.files
    }

    /// Get the number of completed steps
    /// Requirements: 13.4 - Track sequential step execution
    pub fn completed_steps_count(&self) -> usize {
        self.steps.len()
    }

    /// Check if a step has been executed
    /// Requirements: 13.8, 14.1 - Verify step output availability
    pub fn has_step_output(&self, step_id: &str) -> bool {
        self.steps.contains_key(step_id)
    }

    /// Get all step IDs that have been executed
    /// Requirements: 13.8 - Access previous step outputs
    pub fn get_executed_step_ids(&self) -> Vec<String> {
        self.steps.keys().cloned().collect()
    }
}

/// StepOutput stores the output of a single job step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepOutput {
    pub step_id: String,
    pub status: String,
    pub output: serde_json::Value,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}

/// WebhookData stores data from webhook triggers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookData {
    pub payload: serde_json::Value,
    pub query_params: HashMap<String, String>,
    pub headers: HashMap<String, String>,
}

/// FileMetadata stores metadata about files processed or generated
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub path: String,
    pub filename: String,
    pub size: u64,
    pub mime_type: Option<String>,
    pub row_count: Option<usize>,
    pub created_at: DateTime<Utc>,
}

// ============================================================================
// Variable Models
// ============================================================================

/// Variable represents a key-value pair that can be referenced by jobs
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Variable {
    pub id: Uuid,
    pub name: String,
    pub value: String,
    pub is_sensitive: bool,
    #[sqlx(try_from = "String")]
    pub scope: VariableScope,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// VariableScope defines the scope of a variable
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VariableScope {
    Global,
    Job { job_id: Uuid },
}

impl std::fmt::Display for VariableScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VariableScope::Global => write!(f, "global"),
            VariableScope::Job { .. } => write!(f, "job"),
        }
    }
}

impl FromStr for VariableScope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // For database storage, we store just the type string
        // The full data (job_id) is stored in a separate column
        match s {
            "global" => Ok(VariableScope::Global),
            "job" => Ok(VariableScope::Job {
                job_id: Uuid::nil(),
            }),
            _ => Err(format!("Invalid variable scope: {}", s)),
        }
    }
}

impl TryFrom<String> for VariableScope {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::from_str(&s)
    }
}

// ============================================================================
// User and Authentication Models
// ============================================================================

/// User represents a user account for database authentication mode
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub email: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Role represents a role with associated permissions
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Role {
    pub id: Uuid,
    pub name: String,
    #[sqlx(json)]
    pub permissions: Vec<String>,
    pub created_at: DateTime<Utc>,
}

/// UserClaims represents JWT token claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserClaims {
    pub sub: String,              // Subject (user ID)
    pub username: String,         // Username
    pub permissions: Vec<String>, // User permissions
    pub exp: i64,                 // Expiration time (Unix timestamp)
    pub iat: i64,                 // Issued at (Unix timestamp)
}

// ============================================================================
// Webhook Models
// ============================================================================

/// Webhook represents a webhook trigger configuration for a job
/// Requirements: 16.1 - Unique webhook URL per job with secret key
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Webhook {
    pub id: Uuid,
    pub job_id: Uuid,
    pub url_path: String,
    pub secret_key: String,
    pub enabled: bool,
    pub rate_limit_max_requests: Option<i32>,
    pub rate_limit_window_seconds: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// WebhookRequest represents an incoming webhook request
/// Requirements: 16.3, 16.4, 16.5 - Webhook payload, query params, headers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookRequest {
    pub payload: serde_json::Value,
    pub query_params: HashMap<String, String>,
    pub headers: HashMap<String, String>,
}

/// WebhookResponse represents the response to a webhook request
/// Requirements: 16.9 - Return 202 Accepted with execution_id
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookResponse {
    pub execution_id: Uuid,
    pub message: String,
}
