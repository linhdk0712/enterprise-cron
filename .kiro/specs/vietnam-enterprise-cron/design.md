# Design Document

## Overview

The Vietnam Enterprise Cron System is a distributed, production-ready job scheduling and execution platform built in Rust. The system replaces Java Quartz + Spring Batch implementations with a modern, high-performance architecture designed for Vietnamese enterprise requirements.

### Key Design Principles

1. **Distributed-First**: All components designed for horizontal scalability with no single point of failure
2. **Exactly-Once Semantics**: Leveraging Redis RedLock and idempotency keys to prevent duplicate executions
3. **Observable by Default**: Comprehensive structured logging, Prometheus metrics, and OpenTelemetry tracing
4. **Type-Safe**: Leveraging Rust's type system and sqlx compile-time checking for correctness
5. **Separation of Concerns**: Clear boundaries between scheduler, worker, API, and storage layers

### System Architecture

The system consists of six main components:

1. **Scheduler**: Detects when jobs are due and publishes them to the job queue
2. **Worker**: Consumes jobs from the queue and executes multi-step jobs with various job types (HTTP, Database, FileProcessing, SFTP)
3. **API Server**: Provides REST API, serves the HTMX dashboard, and handles webhook triggers
4. **Storage Layer**: 
   - PostgreSQL for job metadata and execution records
   - Redis for distributed locks and rate limiting
   - NATS JetStream for job queue with exactly-once delivery
   - MinIO for job definitions, execution context, and file storage
5. **Job Executors**: Specialized executors for HTTP requests, database queries, file processing (Excel/CSV), and SFTP operations
6. **Webhook Handler**: Receives webhook requests, validates signatures, and queues jobs with webhook data

## Architecture

### Component Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                         Load Balancer                            │
└────────────────────────────┬────────────────────────────────────┘
                             │
                ┌────────────┴────────────┐
                │                         │
        ┌───────▼────────┐       ┌───────▼────────┐
        │  API Server 1  │       │  API Server N  │
        │  (Axum + HTMX) │       │  (Axum + HTMX) │
        │  + Webhooks    │       │  + Webhooks    │
        └───────┬────────┘       └───────┬────────┘
                │                         │
                └────────────┬────────────┘
                             │
        ┌────────────────────┼────────────────────┐
        │                    │                    │
┌───────▼────────┐  ┌────────▼────────┐  ┌───────▼────────┐
│  Scheduler 1   │  │  Scheduler N    │  │   Worker 1-N   │
│  (Distributed  │  │  (Distributed   │  │  (Multi-Step   │
│   Locking)     │  │   Locking)      │  │   Execution)   │
└───────┬────────┘  └────────┬────────┘  └───────┬────────┘
        │                    │                    │
        └────────────────────┼────────────────────┘
                             │
        ┌────────────────────┼────────────────────┬────────────┐
        │                    │                    │            │
┌───────▼────────┐  ┌────────▼────────┐  ┌───────▼────────┐ │
│   PostgreSQL   │  │     Redis       │  │ NATS JetStream │ │
│  (Metadata)    │  │  (Dist Lock +   │  │  (Job Queue)   │ │
│                │  │   Rate Limit)   │  │                │ │
└────────────────┘  └─────────────────┘  └────────────────┘ │
                                                             │
                                                    ┌────────▼────────┐
                                                    │     MinIO       │
                                                    │  (Job Defs +    │
                                                    │   Context +     │
                                                    │   Files)        │
                                                    └─────────────────┘
```

### Data Flow

1. **Job Scheduling Flow**:
   - Scheduler polls database for jobs due for execution
   - Acquires distributed lock via Redis RedLock
   - Loads job definition from MinIO
   - Publishes job to NATS JetStream queue
   - Releases lock

2. **Multi-Step Job Execution Flow**:
   - Worker consumes job from NATS queue
   - Checks idempotency key in database
   - Loads job definition from MinIO
   - Initializes Job Context object
   - For each step in sequence:
     - Loads current Job Context from MinIO
     - Resolves variable and step output references
     - Executes step (HTTP, Database, FileProcessing, or SFTP)
     - Stores step output in Job Context
     - Persists updated Job Context to MinIO
   - Records final execution result in database
   - Acknowledges message in NATS

3. **Webhook Trigger Flow**:
   - External system sends POST to webhook URL
   - API validates HMAC signature
   - API checks rate limits in Redis
   - API stores webhook payload/headers/params in Job Context
   - API queues job execution with webhook data
   - API returns 202 Accepted with execution_id

4. **API Flow**:
   - User authenticates via Keycloak or database JWT
   - API validates token and checks RBAC permissions
   - API reads/writes to PostgreSQL
   - API reads/writes job definitions to MinIO
   - API pushes updates via Server-Sent Events

5. **Job Import/Export Flow**:
   - Export: API reads job definition from MinIO, masks sensitive data, returns JSON
   - Import: API validates JSON schema, prompts for sensitive data, stores to MinIO, creates job record

## Components and Interfaces


### Scheduler Component

**Responsibilities**:
- Poll database for jobs that are due for execution
- Acquire distributed locks to prevent duplicate scheduling
- Calculate next execution time based on schedule type
- Publish jobs to the queue

**Key Interfaces**:
```rust
trait ScheduleTrigger {
    fn next_execution_time(&self, last_execution: Option<DateTime<Tz>>) -> Option<DateTime<Tz>>;
    fn is_complete(&self) -> bool;
}

trait DistributedLock {
    async fn acquire(&self, resource: &str, ttl: Duration) -> Result<LockGuard>;
}

trait JobPublisher {
    async fn publish(&self, job: &JobExecution) -> Result<()>;
}
```

### Worker Component

**Responsibilities**:
- Consume jobs from the queue
- Check idempotency to prevent duplicate execution
- Load job definitions from MinIO
- Execute multi-step jobs sequentially
- Manage Job Context (load, update, persist to MinIO)
- Resolve variable and step output references
- Execute jobs based on type (HTTP, Database, FileProcessing, SFTP)
- Handle retries with exponential backoff
- Implement circuit breaker pattern
- Record execution results

**Key Interfaces**:
```rust
trait JobExecutor {
    async fn execute(&self, step: &JobStep, context: &mut JobContext) -> Result<StepOutput>;
}

trait ContextManager {
    async fn load_context(&self, execution_id: Uuid) -> Result<JobContext>;
    async fn save_context(&self, context: &JobContext) -> Result<()>;
}

trait ReferenceResolver {
    fn resolve(&self, template: &str, context: &JobContext) -> Result<String>;
    fn resolve_json_path(&self, path: &str, data: &serde_json::Value) -> Result<serde_json::Value>;
}

trait RetryStrategy {
    fn next_delay(&self, attempt: u32) -> Option<Duration>;
}

trait CircuitBreaker {
    async fn call<F, T>(&self, f: F) -> Result<T>
    where
        F: Future<Output = Result<T>>;
}
```

### API Component

**Responsibilities**:
- Serve REST API for job management
- Handle webhook requests with signature validation
- Validate JWT tokens from Keycloak or database
- Enforce RBAC permissions
- Manage job import/export with sensitive data masking
- Serve HTMX dashboard with visual job builder
- Push real-time updates via Server-Sent Events
- Enforce rate limiting for webhooks

**Key Interfaces**:
```rust
trait AuthMiddleware {
    async fn validate_token(&self, token: &str) -> Result<UserClaims>;
    async fn check_permission(&self, user: &UserClaims, permission: &str) -> Result<bool>;
}

trait JobService {
    async fn list_jobs(&self, filter: JobFilter) -> Result<Vec<JobWithStats>>;
    async fn create_job(&self, job: CreateJobRequest) -> Result<Job>;
    async fn update_job(&self, id: Uuid, job: UpdateJobRequest) -> Result<Job>;
    async fn delete_job(&self, id: Uuid) -> Result<()>;
    async fn trigger_job(&self, id: Uuid, trigger_source: TriggerSource) -> Result<JobExecution>;
    async fn export_job(&self, id: Uuid) -> Result<String>;
    async fn import_job(&self, json: &str) -> Result<Job>;
}

trait WebhookHandler {
    async fn handle_webhook(&self, job_id: Uuid, payload: WebhookRequest) -> Result<Uuid>;
    async fn validate_signature(&self, payload: &[u8], signature: &str, secret: &str) -> Result<bool>;
    async fn check_rate_limit(&self, job_id: Uuid) -> Result<bool>;
}

trait MinIOService {
    async fn store_job_definition(&self, job_id: Uuid, definition: &str) -> Result<String>;
    async fn load_job_definition(&self, job_id: Uuid) -> Result<String>;
    async fn store_context(&self, execution_id: Uuid, context: &JobContext) -> Result<String>;
    async fn load_context(&self, execution_id: Uuid) -> Result<JobContext>;
    async fn store_file(&self, path: &str, data: &[u8]) -> Result<String>;
    async fn load_file(&self, path: &str) -> Result<Vec<u8>>;
}
```

### Storage Component

**Responsibilities**:
- Persist job definitions and execution history
- Manage variables (global and job-specific)
- Provide query interfaces for all data access

**Key Interfaces**:
```rust
trait JobRepository {
    async fn find_jobs_due(&self, now: DateTime<Utc>) -> Result<Vec<Job>>;
    async fn save_job(&self, job: &Job) -> Result<()>;
    async fn find_by_id(&self, id: Uuid) -> Result<Option<Job>>;
}

trait ExecutionRepository {
    async fn create_execution(&self, execution: &JobExecution) -> Result<()>;
    async fn update_execution(&self, execution: &JobExecution) -> Result<()>;
    async fn find_by_idempotency_key(&self, key: &str) -> Result<Option<JobExecution>>;
}

trait VariableRepository {
    async fn find_global_variables(&self) -> Result<HashMap<String, String>>;
    async fn find_job_variables(&self, job_id: Uuid) -> Result<HashMap<String, String>>;
}
```

## Data Models


### Core Models

```rust
struct Job {
    id: Uuid,
    name: String,
    description: Option<String>,
    schedule: Option<Schedule>,
    steps: Vec<JobStep>,
    triggers: TriggerConfig,
    enabled: bool,
    timeout_seconds: u32,
    max_retries: u32,
    allow_concurrent: bool,
    minio_definition_path: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

struct JobStep {
    id: String,
    name: String,
    step_type: JobType,
    condition: Option<String>,
}

struct TriggerConfig {
    scheduled: bool,
    manual: bool,
    webhook: Option<WebhookConfig>,
}

struct WebhookConfig {
    enabled: bool,
    url: String,
    secret_key: String,
    rate_limit: Option<RateLimit>,
}

struct RateLimit {
    max_requests: u32,
    window_seconds: u32,
}

enum Schedule {
    Cron {
        expression: String,
        timezone: Tz,
        end_date: Option<DateTime<Tz>>,
    },
    FixedDelay {
        delay_seconds: u32,
    },
    FixedRate {
        interval_seconds: u32,
    },
    OneTime {
        execute_at: DateTime<Tz>,
    },
}

enum JobType {
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

enum FileOperation {
    Read,
    Write,
}

enum FileFormat {
    Excel,
    Csv { delimiter: char },
}

struct FileProcessingOptions {
    sheet_name: Option<String>,
    sheet_index: Option<usize>,
    transformations: Vec<DataTransformation>,
    streaming: bool,
}

enum DataTransformation {
    ColumnMapping { from: String, to: String },
    TypeConversion { column: String, target_type: String },
    Filter { condition: String },
}

enum SftpOperation {
    Download,
    Upload,
}

enum SftpAuth {
    Password { username: String, password: String },
    SshKey { username: String, private_key_path: String },
}

struct SftpOptions {
    wildcard_pattern: Option<String>,
    recursive: bool,
    create_directories: bool,
    verify_host_key: bool,
}

enum HttpAuth {
    Basic { username: String, password: String },
    Bearer { token: String },
    OAuth2 { client_id: String, client_secret: String, token_url: String },
}

enum DatabaseType {
    PostgreSQL,
    MySQL,
    Oracle,
}

enum QueryType {
    RawSql,
    StoredProcedure { procedure_name: String, parameters: Vec<String> },
}

struct JobExecution {
    id: Uuid,
    job_id: Uuid,
    idempotency_key: String,
    status: ExecutionStatus,
    attempt: u32,
    trigger_source: TriggerSource,
    current_step: Option<String>,
    minio_context_path: String,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    result: Option<String>,
    error: Option<String>,
    created_at: DateTime<Utc>,
}

enum TriggerSource {
    Scheduled,
    Manual { user_id: String },
    Webhook { webhook_url: String },
}

struct JobContext {
    execution_id: Uuid,
    job_id: Uuid,
    variables: HashMap<String, serde_json::Value>,
    steps: HashMap<String, StepOutput>,
    webhook: Option<WebhookData>,
    files: Vec<FileMetadata>,
}

struct StepOutput {
    step_id: String,
    status: String,
    output: serde_json::Value,
    started_at: DateTime<Utc>,
    completed_at: DateTime<Utc>,
}

struct WebhookData {
    payload: serde_json::Value,
    query_params: HashMap<String, String>,
    headers: HashMap<String, String>,
}

struct FileMetadata {
    path: String,
    filename: String,
    size: u64,
    mime_type: Option<String>,
    row_count: Option<usize>,
    created_at: DateTime<Utc>,
}

enum ExecutionStatus {
    Pending,
    Running,
    Success,
    Failed,
    Timeout,
    DeadLetter,
}

struct Variable {
    id: Uuid,
    name: String,
    value: String,
    is_sensitive: bool,
    scope: VariableScope,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

enum VariableScope {
    Global,
    Job { job_id: Uuid },
}
```

### Database Schema

```sql
CREATE TABLE jobs (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    schedule_type VARCHAR(50),
    schedule_config JSONB,
    trigger_config JSONB NOT NULL,
    minio_definition_path VARCHAR(500) NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    timeout_seconds INTEGER NOT NULL DEFAULT 300,
    max_retries INTEGER NOT NULL DEFAULT 10,
    allow_concurrent BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE job_executions (
    id UUID PRIMARY KEY,
    job_id UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    idempotency_key VARCHAR(255) NOT NULL UNIQUE,
    status VARCHAR(50) NOT NULL,
    attempt INTEGER NOT NULL DEFAULT 1,
    trigger_source VARCHAR(50) NOT NULL,
    trigger_metadata JSONB,
    current_step VARCHAR(255),
    minio_context_path VARCHAR(500) NOT NULL,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    result TEXT,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    INDEX idx_job_executions_job_id (job_id),
    INDEX idx_job_executions_status (status),
    INDEX idx_job_executions_created_at (created_at),
    INDEX idx_job_executions_trigger_source (trigger_source)
);

CREATE TABLE variables (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    value TEXT NOT NULL,
    is_sensitive BOOLEAN NOT NULL DEFAULT false,
    scope_type VARCHAR(50) NOT NULL,
    scope_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (name, scope_type, scope_id)
);

CREATE TABLE job_stats (
    job_id UUID PRIMARY KEY REFERENCES jobs(id) ON DELETE CASCADE,
    total_executions BIGINT NOT NULL DEFAULT 0,
    successful_executions BIGINT NOT NULL DEFAULT 0,
    failed_executions BIGINT NOT NULL DEFAULT 0,
    last_execution_at TIMESTAMPTZ,
    last_success_at TIMESTAMPTZ,
    last_failure_at TIMESTAMPTZ,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

## Correctness Properties

*A property is a characteristic or behavior that should hold true across all valid executions of a system-essentially, a formal statement about what the system should do. Properties serve as the bridge between human-readable specifications and machine-verifiable correctness guarantees.*


### Scheduling Properties

**Property 1: Cron expression parsing validity**
*For any* valid Quartz-syntax cron expression with second precision, the parser should successfully parse it without error, and for any invalid expression, the parser should return an error.
**Validates: Requirements 1.1**

**Property 2: Timezone-aware scheduling**
*For any* job with a specified timezone and cron expression, the next execution time calculated should be correct for that timezone, accounting for daylight saving time transitions.
**Validates: Requirements 1.2**

**Property 3: Default timezone application**
*For any* job created without a timezone specification, the system should use Asia/Ho_Chi_Minh as the default timezone for all schedule calculations.
**Validates: Requirements 1.3**

**Property 4: Fixed delay timing**
*For any* fixed delay job with delay D seconds, if the previous execution completed at time T, the next execution should be scheduled at time T + D seconds.
**Validates: Requirements 1.4**

**Property 5: Fixed rate timing**
*For any* fixed rate job with interval I seconds and start time T, the Nth execution should be scheduled at time T + (N * I) seconds, regardless of execution duration.
**Validates: Requirements 1.5**

**Property 6: One-time job completion**
*For any* one-time job that has been executed, the system should mark it as complete and not schedule any future executions.
**Validates: Requirements 1.6**

**Property 7: End date enforcement**
*For any* recurring job with an end date E, no executions should be scheduled for times after E.
**Validates: Requirements 1.7**

### Variable Management Properties

**Property 8: Global variable availability**
*For any* global variable created in the system, it should be retrievable and usable by all jobs.
**Validates: Requirements 2.1**

**Property 9: Job-specific variable scoping**
*For any* job-specific variable associated with job J, it should only be accessible when executing job J and not accessible to other jobs.
**Validates: Requirements 2.2**

**Property 10: Variable resolution**
*For any* job configuration containing variable placeholders, all placeholders should be replaced with actual variable values before execution.
**Validates: Requirements 2.3**

**Property 11: Variable precedence**
*For any* job J with a job-specific variable V and a global variable with the same name V, the job-specific value should be used when executing job J.
**Validates: Requirements 2.4**

**Property 12: Undefined variable handling**
*For any* job referencing a non-existent variable, the execution should fail with a clear error message indicating which variable is undefined.
**Validates: Requirements 2.5**

**Property 13: Variable update propagation**
*For any* variable that is updated at time T, all job executions starting after time T should use the new value.
**Validates: Requirements 2.6**

**Property 14: Sensitive variable encryption**
*For any* variable marked as sensitive, its value should be encrypted in the database and never stored in plaintext.
**Validates: Requirements 2.7**

**Property 15: Sensitive variable masking**
*For any* API response containing sensitive variables, the values should be masked (e.g., "***") and not exposed in plaintext.
**Validates: Requirements 2.8**

**Property 16: Variable substitution in URLs**
*For any* HTTP job with variables in the URL template, all variable placeholders should be replaced with actual values before making the request.
**Validates: Requirements 2.9**

**Property 17: Variable substitution in headers and body**
*For any* HTTP job with variables in headers or body, all variable placeholders should be replaced with actual values before making the request.
**Validates: Requirements 2.10**

**Property 18: Variable substitution in connection strings**
*For any* database job with variables in the connection string, all variable placeholders should be replaced with actual values before connecting.
**Validates: Requirements 2.11**

**Property 19: Parameterized query substitution**
*For any* database job with variables in SQL queries, the system should use parameterized queries to substitute values, preventing SQL injection attacks.
**Validates: Requirements 2.12**

### Job Execution Properties

**Property 20: HTTP method correctness**
*For any* HTTP job with method M (GET, POST, PUT), the actual HTTP request sent should use method M.
**Validates: Requirements 3.1**

**Property 21: HTTP header inclusion**
*For any* HTTP job with headers H, all headers in H should be present in the actual HTTP request.
**Validates: Requirements 3.2**

**Property 22: HTTP body inclusion**
*For any* HTTP job with a request body B, the actual HTTP request should include body B.
**Validates: Requirements 3.3**

**Property 23: Basic authentication formatting**
*For any* HTTP job with Basic authentication credentials (username U, password P), the Authorization header should contain "Basic " followed by base64(U:P).
**Validates: Requirements 3.4**

**Property 24: Bearer token formatting**
*For any* HTTP job with Bearer token T, the Authorization header should contain "Bearer " followed by T.
**Validates: Requirements 3.5**

**Property 25: OAuth2 token acquisition**
*For any* HTTP job with OAuth2 configuration, the system should obtain a valid access token from the token endpoint before making the request.
**Validates: Requirements 3.6**

**Property 26: Database query execution**
*For any* database job with query Q and target database D, the system should execute query Q against database D.
**Validates: Requirements 3.7**

**Property 27: Job persistence**
*For any* job created through the API, it should be persisted to the System Database and retrievable by its ID.
**Validates: Requirements 3.11**

**Property 28: Execution history persistence**
*For any* job execution, its status, timing, and result should be persisted to the System Database.
**Validates: Requirements 3.12**

### Reliability Properties

**Property 29: Distributed lock exclusivity**
*For any* job J that is due for execution, when multiple scheduler nodes attempt to schedule it, only one node should successfully acquire the lock and publish the job.
**Validates: Requirements 4.1**

**Property 30: Exactly-once execution**
*For any* job execution with idempotency key K, even if the message is delivered multiple times, the job should be executed exactly once.
**Validates: Requirements 4.2**

**Property 31: Idempotency key checking**
*For any* job execution with an explicit idempotency key K, if a previous execution with key K exists, the new execution should be skipped.
**Validates: Requirements 4.3**

**Property 32: Idempotency key generation**
*For any* job execution without an explicit idempotency key, the system should generate a unique key that is different from all other execution keys.
**Validates: Requirements 4.4**

**Property 33: Retry limit enforcement**
*For any* failed job execution, the system should retry up to 10 times, and after the 10th failure, no more retries should occur.
**Validates: Requirements 4.5**

**Property 34: Exponential backoff with jitter**
*For any* retry attempt N (where N ≤ 10), the delay before retry should follow the exponential backoff sequence (5s, 15s, 1m, 5m, 30m, ...) with random jitter added.
**Validates: Requirements 4.6**

**Property 35: Circuit breaker activation**
*For any* external system that has failed F consecutive times (where F exceeds the threshold), the circuit breaker should open and subsequent calls should fail fast without attempting execution.
**Validates: Requirements 4.7**

**Property 36: Dead letter queue placement**
*For any* job execution that has exhausted all 10 retry attempts, it should be moved to the Dead Letter Queue with status DeadLetter.
**Validates: Requirements 4.8**

**Property 37: Timeout enforcement**
*For any* job execution that runs longer than its configured timeout T seconds, the system should terminate it and mark it as failed with status Timeout.
**Validates: Requirements 4.9**

**Property 38: Dead letter queue isolation**
*For any* job execution in the Dead Letter Queue, it should not be automatically retried or re-executed without manual intervention.
**Validates: Requirements 4.10**


### Observability Properties

**Property 39: Execution start logging**
*For any* job execution that starts, a structured log entry should be created containing job_id, execution_id, and timestamp.
**Validates: Requirements 5.1**

**Property 40: Execution completion logging**
*For any* job execution that completes, a structured log entry should be created containing the duration and final status.
**Validates: Requirements 5.2**

**Property 41: Success metric increment**
*For any* job execution that completes with status Success, the job_success_total Prometheus counter should be incremented by 1.
**Validates: Requirements 5.3**

**Property 42: Failure metric increment**
*For any* job execution that completes with status Failed, Timeout, or DeadLetter, the job_failed_total Prometheus counter should be incremented by 1.
**Validates: Requirements 5.4**

**Property 43: Duration metric recording**
*For any* job execution that completes, the duration (completed_at - started_at) should be recorded in the job_duration_seconds Prometheus histogram.
**Validates: Requirements 5.5**

**Property 44: Queue size metric**
*For any* point in time, the job_queue_size Prometheus gauge should reflect the current number of jobs in the queue.
**Validates: Requirements 5.6**

**Property 45: Trace span creation**
*For any* job execution, an OpenTelemetry trace span should be created with attributes including job_id, execution_id, and job_type.
**Validates: Requirements 5.7**

**Property 46: Consecutive failure alerting**
*For any* job that fails 3 consecutive times, an alert notification should be triggered.
**Validates: Requirements 5.8**

**Property 47: Structured logging format**
*For any* log entry created by the system, it should be structured (JSON format) and include trace context (trace_id, span_id).
**Validates: Requirements 5.9**

### Dashboard and API Properties

**Property 48: Job listing completeness**
*For any* request to list jobs, the response should include all jobs with their current status, next run time, last run time, and success rate.
**Validates: Requirements 6.1**

**Property 49: Execution history time window**
*For any* request for execution history, only executions with created_at within the last 30 days should be returned.
**Validates: Requirements 6.2**

**Property 50: Execution history filtering**
*For any* execution history request with status filter S and job filter J, only executions matching both filters should be returned.
**Validates: Requirements 6.3**

**Property 51: Manual trigger queueing**
*For any* manual trigger request for job J, a new job execution should be created and added to the queue immediately.
**Validates: Requirements 6.4**

**Property 52: Job disable effect**
*For any* job that is disabled at time T, no new executions should be scheduled for times after T.
**Validates: Requirements 6.5**

**Property 53: Job enable effect**
*For any* previously disabled job that is enabled at time T, new executions should be scheduled according to its schedule starting from time T.
**Validates: Requirements 6.6**

**Property 54: Real-time status updates**
*For any* job status change, a Server-Sent Event should be pushed to all connected dashboard clients within 1 second.
**Validates: Requirements 6.7**

### High Availability Properties

**Property 55: Single scheduler execution**
*For any* job J and time T when J is due, even with 100 scheduler nodes running, only one node should publish J to the queue.
**Validates: Requirements 7.1**

**Property 56: Dynamic job addition**
*For any* new job created at time T, it should be available for scheduling by all scheduler nodes without requiring a restart.
**Validates: Requirements 7.2**

**Property 57: Dynamic job update**
*For any* job updated at time T, the changes should be applied to all future executions without requiring a restart.
**Validates: Requirements 7.3**

**Property 58: Dynamic job deletion**
*For any* job deleted at time T, it should stop being scheduled by all scheduler nodes without requiring a restart.
**Validates: Requirements 7.4**

**Property 59: Configuration hot reload**
*For any* configuration change detected at time T, the new configuration should be applied without requiring a process restart.
**Validates: Requirements 7.5**

**Property 60: Scheduler graceful shutdown**
*For any* SIGTERM or SIGINT signal received by a scheduler, all in-flight scheduling operations should complete before the process terminates.
**Validates: Requirements 7.6**

**Property 61: Worker graceful shutdown**
*For any* SIGTERM or SIGINT signal received by a worker, all in-flight job executions should complete before the process terminates.
**Validates: Requirements 7.7**

### Error Handling Properties

**Property 62: Error logging with context**
*For any* unrecoverable error, the system should log the error with full context including the operation being performed and relevant identifiers.
**Validates: Requirements 8.2**

### Authentication and Authorization Properties

**Property 63: Keycloak JWT validation**
*For any* API request when authentication mode is "keycloak", the system should validate the JWT token signature and expiration against Keycloak's public keys.
**Validates: Requirements 10.1**

**Property 64: Database authentication**
*For any* login request when authentication mode is "database", the system should validate credentials against bcrypt-hashed passwords in the System Database and issue a JWT token on success.
**Validates: Requirements 10.2, 10.3**

**Property 65: Invalid token rejection**
*For any* API request with an invalid or expired JWT token, the system should return HTTP 401 Unauthorized.
**Validates: Requirements 10.4**

**Property 66: Read permission enforcement**
*For any* request to view jobs, the system should verify the user has "job:read" permission, and reject requests without it.
**Validates: Requirements 10.5**

**Property 67: Write permission enforcement**
*For any* request to create or edit jobs, the system should verify the user has "job:write" permission, and reject requests without it.
**Validates: Requirements 10.6**

**Property 68: Execute permission enforcement**
*For any* request to manually trigger a job, the system should verify the user has "job:execute" permission, and reject requests without it.
**Validates: Requirements 10.7**

**Property 69: Delete permission enforcement**
*For any* request to delete jobs, the system should verify the user has "job:delete" permission, and reject requests without it.
**Validates: Requirements 10.8**

**Property 70: Execution read permission enforcement**
*For any* request to view execution history, the system should verify the user has "execution:read" permission, and reject requests without it.
**Validates: Requirements 10.9**

**Property 71: Audit logging with user identity**
*For any* API operation, the system should log the user identity extracted from the JWT token for audit purposes.
**Validates: Requirements 10.10**

**Property 72: Keycloak resilience**
*For any* JWT validation request when authentication mode is "keycloak" and Keycloak is unavailable, the system should use cached public keys to validate tokens.
**Validates: Requirements 10.11**

**Property 73: Keycloak configuration**
*For any* Keycloak integration when authentication mode is "keycloak", the system should support configuration of realm, client ID, and server URL.
**Validates: Requirements 10.12**

**Property 74: Database user storage**
*For any* user created when authentication mode is "database", the password should be hashed with bcrypt and stored with role assignments in the System Database.
**Validates: Requirements 10.13**

### Component Initialization Properties

**Property 73: Scheduler component isolation**
*For any* scheduler binary startup, only scheduler-specific components (trigger detection, lock acquisition, job publisher) should be initialized, and worker components should not be initialized.
**Validates: Requirements 9.4**

**Property 74: Worker component isolation**
*For any* worker binary startup, only worker-specific components (job consumer, executors, retry logic) should be initialized, and scheduler components should not be initialized.
**Validates: Requirements 9.5**

**Property 75: Database migration execution**
*For any* system initialization, database migrations should run and create all required tables (jobs, job_executions, variables, job_stats) if they don't exist.
**Validates: Requirements 12.6**

### Multi-Step Job and MinIO Properties

**Property 76: JSON job definition acceptance**
*For any* valid JSON job definition document, the system should accept it, and for any invalid JSON, the system should reject it with a clear error.
**Validates: Requirements 13.1**

**Property 77: MinIO job definition persistence**
*For any* job definition stored in MinIO, retrieving it should return the same definition (round-trip consistency).
**Validates: Requirements 13.2**

**Property 78: MinIO path format for job definitions**
*For any* job_id, the MinIO path for the job definition should be `jobs/{job_id}/definition.json`.
**Validates: Requirements 13.3**

**Property 79: Sequential step execution**
*For any* job with N steps, step i should complete before step i+1 starts, maintaining sequential order.
**Validates: Requirements 13.4**

**Property 80: HTTP response storage in Job Context**
*For any* HTTP step execution, the API response should be present in the Job Context after the step completes.
**Validates: Requirements 13.5**

**Property 81: Database result storage in Job Context**
*For any* database query step execution, the query result set should be present in the Job Context after the step completes.
**Validates: Requirements 13.6**

**Property 82: Job Context persistence to MinIO**
*For any* Job Context persisted to MinIO, retrieving it should return the same context (round-trip consistency).
**Validates: Requirements 13.7**

**Property 83: Job Context path format**
*For any* job_id and execution_id, the MinIO path for Job Context should be `jobs/{job_id}/executions/{execution_id}/context.json`.
**Validates: Requirements 13.7**

**Property 84: Job Context loading for subsequent steps**
*For any* multi-step job, step N should have access to outputs from all previous steps (1..N-1) via the Job Context.
**Validates: Requirements 13.8**

**Property 85: Job Context retention after completion**
*For any* completed job execution, the final Job Context should remain retrievable from MinIO.
**Validates: Requirements 13.9**

**Property 86: Job Context preservation on failure**
*For any* failed job execution, the Job Context up to the point of failure should be persisted and retrievable from MinIO.
**Validates: Requirements 13.10**

**Property 87: Job Context reference in execution details**
*For any* execution query, the response should include the MinIO path reference to the Job Context.
**Validates: Requirements 13.11**

**Property 88: Database stores only MinIO path references**
*For any* job record in the database, it should contain only the MinIO path string, not the full job definition or context data.
**Validates: Requirements 13.12**

### Step Output Reference Properties

**Property 89: Step output reference resolution**
*For any* valid step output reference, the Worker should successfully resolve it from the Job Context.
**Validates: Requirements 14.1**

**Property 90: Template reference extraction**
*For any* valid template reference like `{{steps.step1.response.data.id}}`, the Worker should extract the correct value from the Job Context.
**Validates: Requirements 14.2**

**Property 91: Invalid reference error handling**
*For any* invalid step reference or non-existent path, the Worker should fail the execution with a clear error message.
**Validates: Requirements 14.3**

**Property 92: JSONPath nested value access**
*For any* nested JSON structure in step output, JSONPath-style references should correctly extract nested values.
**Validates: Requirements 14.4**

**Property 93: Automatic step output storage**
*For any* step execution, the step output should be automatically stored in the Job Context without explicit configuration.
**Validates: Requirements 14.5**

**Property 94: Conditional logic evaluation**
*For any* conditional expression in a job, it should be evaluated using data from the Job Context.
**Validates: Requirements 14.6**

**Property 95: Missing data reference error**
*For any* step reference to data not populated by a previous step, the Worker should fail with a clear error indicating the missing data path.
**Validates: Requirements 14.7**

### File Processing Properties

**Property 96: Excel file reading**
*For any* valid XLSX file in MinIO, the Worker should successfully read and parse it.
**Validates: Requirements 15.1**

**Property 97: Excel data structure preservation**
*For any* Excel file parsed to JSON, the structure (sheets, rows, columns) should be preserved in the Job Context.
**Validates: Requirements 15.2**

**Property 98: CSV file reading**
*For any* valid CSV file in MinIO, the Worker should successfully read and parse it.
**Validates: Requirements 15.3**

**Property 99: CSV delimiter support**
*For any* CSV file with delimiter D (comma, semicolon, tab), parsing with delimiter D should correctly parse all rows.
**Validates: Requirements 15.4**

**Property 100: Excel sheet selection**
*For any* Excel file and sheet selector (name or index), only that sheet's data should be present in the output.
**Validates: Requirements 15.5**

**Property 101: Data transformation application**
*For any* transformation rule applied to file data, the output in Job Context should reflect the transformation.
**Validates: Requirements 15.6**

**Property 102: Excel write round-trip**
*For any* data written to Excel format then read back, the data should be preserved (round-trip consistency).
**Validates: Requirements 15.7**

**Property 103: CSV write round-trip**
*For any* data written to CSV format then read back, the data should be preserved (round-trip consistency).
**Validates: Requirements 15.8**

**Property 104: File output path format**
*For any* file written to MinIO, the path should follow the format `jobs/{job_id}/executions/{execution_id}/output/{filename}`.
**Validates: Requirements 15.9**

**Property 105: File metadata storage**
*For any* file processing step completion, the MinIO file path and row count should be present in the Job Context.
**Validates: Requirements 15.10**

**Property 106: Invalid file format error handling**
*For any* invalid file format encountered, the Worker should fail with a clear error message indicating the parsing error.
**Validates: Requirements 15.11**

### Webhook Trigger Properties

**Property 107: Unique webhook URL generation**
*For any* job with webhook trigger enabled, a unique webhook URL should be generated.
**Validates: Requirements 16.1**

**Property 108: Webhook POST queueing**
*For any* valid HTTP POST to a webhook URL, a job execution should be queued immediately.
**Validates: Requirements 16.2**

**Property 109: Webhook payload storage**
*For any* webhook request with JSON payload, the payload should be accessible at `webhook.payload` in the Job Context.
**Validates: Requirements 16.3**

**Property 110: Webhook query parameters storage**
*For any* webhook request with query parameters, they should be accessible at `webhook.query_params` in the Job Context.
**Validates: Requirements 16.4**

**Property 111: Webhook headers storage**
*For any* webhook request with custom headers, they should be accessible at `webhook.headers` in the Job Context.
**Validates: Requirements 16.5**

**Property 112: Webhook data reference resolution**
*For any* valid webhook data reference like `{{webhook.payload.user_id}}`, the Worker should resolve it from the Job Context.
**Validates: Requirements 16.6**

**Property 113: Webhook signature validation**
*For any* webhook request with valid HMAC-SHA256 signature, it should be accepted; invalid signatures should be rejected.
**Validates: Requirements 16.7**

**Property 114: Invalid webhook signature rejection**
*For any* webhook request with invalid signature, the system should return 401 Unauthorized.
**Validates: Requirements 16.8**

**Property 115: Successful webhook response**
*For any* successfully received webhook, the system should return 202 Accepted with the execution_id in the response.
**Validates: Requirements 16.9**

**Property 116: Disabled job webhook rejection**
*For any* webhook call to a disabled job, the system should return 403 Forbidden.
**Validates: Requirements 16.10**

**Property 117: Webhook rate limiting**
*For any* webhook that exceeds its configured rate limit, subsequent requests should return 429 Too Many Requests.
**Validates: Requirements 16.11**

**Property 118: Webhook URL invalidation**
*For any* webhook URL regeneration, the previous webhook URL should be immediately invalidated and no longer work.
**Validates: Requirements 16.12**

### Multiple Trigger Method Properties

**Property 119: Manual-only job non-scheduling**
*For any* job configured with manual trigger only, the Scheduler should not automatically queue it.
**Validates: Requirements 17.2**

**Property 120: Dashboard manual trigger queueing**
*For any* manual trigger through the dashboard, a job execution should be queued immediately regardless of schedule.
**Validates: Requirements 17.3**

**Property 121: Trigger source recording**
*For any* job execution, the trigger source (scheduled, manual, webhook) should be recorded in the execution record.
**Validates: Requirements 17.6**

**Property 122: Unique execution ID generation**
*For any* job execution triggered by any method, a unique execution_id should be generated.
**Validates: Requirements 17.7**

**Property 123: Trigger source display**
*For any* execution history query, the trigger source should be included for each execution.
**Validates: Requirements 17.8**

**Property 124: Concurrent execution allowance**
*For any* job allowing concurrent execution, multiple executions should be possible simultaneously.
**Validates: Requirements 17.9**

**Property 125: Concurrent execution prevention**
*For any* job configured to prevent concurrent execution, new trigger requests should be rejected while an execution is in progress.
**Validates: Requirements 17.10**

### Job Import/Export Properties

**Property 126: Visual job creation JSON generation**
*For any* job created through the visual interface, a valid JSON job definition should be generated.
**Validates: Requirements 18.2**

**Property 127: Export filename format**
*For any* job export, the filename should follow the format `job-{job_name}-{timestamp}.json`.
**Validates: Requirements 18.3**

**Property 128: Export completeness**
*For any* exported job, all configuration fields (schedule, steps, variables, triggers, timeout, retries) should be present in the JSON.
**Validates: Requirements 18.4**

**Property 129: Sensitive data masking on export**
*For any* exported job, sensitive fields (passwords, API keys) should be masked with placeholder values.
**Validates: Requirements 18.5**

**Property 130: Import JSON schema validation**
*For any* JSON job definition upload, schema validation should occur before importing.
**Validates: Requirements 18.7**

**Property 131: Invalid JSON error messages**
*For any* invalid JSON job definition, clear error messages should indicate which fields are incorrect.
**Validates: Requirements 18.8**

**Property 132: Import round-trip**
*For any* valid JSON job definition imported, it should create a job equivalent to the original (export then import preserves job configuration).
**Validates: Requirements 18.9**

**Property 133: Duplicate name handling**
*For any* imported job with the same name as an existing job, the new job should have a unique name with a suffix (e.g., "job-name-copy-1").
**Validates: Requirements 18.11**

**Property 134: Bulk export format**
*For any* bulk export of multiple jobs, the output should be either a JSON array file or individual files in a ZIP archive.
**Validates: Requirements 18.12**

**Property 135: Bulk import processing**
*For any* bulk import from JSON array or ZIP, each job definition should be processed independently with success/failure reported for each.
**Validates: Requirements 18.13**

**Property 136: Export metadata inclusion**
*For any* exported job, metadata fields (export_date, exported_by, system_version) should be present in the JSON.
**Validates: Requirements 18.14**

### SFTP Operation Properties

**Property 137: SFTP download to MinIO**
*For any* SFTP download step, files should be successfully transferred from the SFTP server to MinIO.
**Validates: Requirements 19.1**

**Property 138: SFTP upload from MinIO**
*For any* SFTP upload step, files should be successfully transferred from MinIO to the SFTP server.
**Validates: Requirements 19.2**

**Property 139: SFTP password authentication**
*For any* SFTP connection with valid password credentials, the connection should succeed.
**Validates: Requirements 19.3**

**Property 140: SFTP key-based authentication**
*For any* SFTP connection with valid SSH key, the connection should succeed.
**Validates: Requirements 19.4**

**Property 141: SFTP wildcard pattern matching**
*For any* wildcard pattern in SFTP download, all files matching the pattern should be downloaded.
**Validates: Requirements 19.5**

**Property 142: SFTP download path format**
*For any* SFTP download, files should be stored in MinIO at path `jobs/{job_id}/executions/{execution_id}/sftp/downloads/{filename}`.
**Validates: Requirements 19.6**

**Property 143: SFTP upload round-trip**
*For any* file uploaded via SFTP then downloaded, the file content should be preserved (round-trip consistency).
**Validates: Requirements 19.7**

**Property 144: SFTP download metadata storage**
*For any* SFTP download completion, file metadata (filename, size, download_time, remote_path) should be in the Job Context.
**Validates: Requirements 19.8**

**Property 145: SFTP upload metadata storage**
*For any* SFTP upload completion, upload metadata (filename, size, upload_time, remote_path) should be in the Job Context.
**Validates: Requirements 19.9**

**Property 146: SFTP authentication error no-retry**
*For any* SFTP operation failing due to authentication error, no retry should occur.
**Validates: Requirements 19.11**

**Property 147: SFTP file not found no-retry**
*For any* SFTP operation failing due to file not found, no retry should occur.
**Validates: Requirements 19.12**

**Property 148: SFTP recursive directory download**
*For any* recursive SFTP download, all files in nested directories should be downloaded.
**Validates: Requirements 19.13**

**Property 149: SFTP remote directory creation**
*For any* SFTP upload to a non-existent directory, the remote directory should be created automatically.
**Validates: Requirements 19.14**

**Property 150: SFTP file path reference resolution**
*For any* valid file path reference from previous steps like `{{steps.step1.output_files[0].path}}`, the Worker should resolve it correctly.
**Validates: Requirements 19.15**

**Property 151: SFTP host key verification**
*For any* SFTP connection establishment, host key verification should occur to prevent man-in-the-middle attacks.
**Validates: Requirements 19.16**

## Error Handling


### Error Handling Strategy

The system uses a layered error handling approach:

1. **Domain Errors** (using `thiserror`):
   - `ScheduleError`: Invalid cron expressions, timezone issues
   - `ExecutionError`: Job execution failures, timeout errors
   - `AuthError`: Authentication and authorization failures
   - `ValidationError`: Input validation failures

2. **Application Errors** (using `anyhow`):
   - Used for error propagation in application logic
   - Provides context wrapping for better debugging
   - Never uses `unwrap()` or `expect()` in production code

3. **Error Response Format**:
```rust
struct ErrorResponse {
    error: String,
    message: String,
    details: Option<serde_json::Value>,
    trace_id: String,
}
```

4. **Retry Strategy**:
   - Transient errors (network, database connection): Retry with exponential backoff
   - Permanent errors (validation, auth): Fail immediately without retry
   - Circuit breaker for external system failures

### Authentication Architecture

The system supports two authentication modes configured via environment variable `AUTH_MODE`:

#### Keycloak Mode (`AUTH_MODE=keycloak`)

```
┌─────────┐         ┌──────────┐         ┌─────────┐
│ Client  │────────>│ Keycloak │────────>│  System │
└─────────┘  Login  └──────────┘  JWT    └─────────┘
                                   Token
```

- Users authenticate with Keycloak
- Keycloak issues JWT tokens with role claims
- System validates JWT signature using Keycloak's public keys
- Roles and permissions extracted from JWT claims
- Public keys cached with TTL for resilience

#### Database Mode (`AUTH_MODE=database`)

```
┌─────────┐         ┌─────────────────┐
│ Client  │────────>│  System (Auth)  │
└─────────┘  Login  └─────────────────┘
              ↓              ↓
         Validate      Generate JWT
         Password      with claims
              ↓              ↓
         ┌──────────────────┐
         │ System Database  │
         │  (users table)   │
         └──────────────────┘
```

- Users stored in `users` table with bcrypt-hashed passwords
- System validates credentials and issues JWT tokens
- Roles and permissions stored in database
- JWT tokens signed with system's private key

**Database Schema for Users**:
```sql
CREATE TABLE users (
    id UUID PRIMARY KEY,
    username VARCHAR(255) NOT NULL UNIQUE,
    password_hash VARCHAR(255) NOT NULL,
    email VARCHAR(255),
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE roles (
    id UUID PRIMARY KEY,
    name VARCHAR(100) NOT NULL UNIQUE,
    permissions TEXT[] NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE user_roles (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, role_id)
);

CREATE TABLE webhooks (
    id UUID PRIMARY KEY,
    job_id UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    url_path VARCHAR(255) NOT NULL UNIQUE,
    secret_key VARCHAR(255) NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    rate_limit_max_requests INTEGER,
    rate_limit_window_seconds INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    INDEX idx_webhooks_job_id (job_id),
    INDEX idx_webhooks_url_path (url_path)
);
```

**Common JWT Claims Structure**:
```json
{
  "sub": "user-id",
  "username": "admin",
  "permissions": ["job:read", "job:write", "job:execute", "job:delete", "execution:read"],
  "exp": 1234567890,
  "iat": 1234567890
}
```

## Testing Strategy

### Unit Testing

Unit tests will cover individual components and functions:

- **Schedule parsing**: Test cron expression parsing, timezone handling
- **Variable resolution**: Test template substitution, precedence rules
- **Authentication**: Test JWT validation, permission checking
- **Retry logic**: Test exponential backoff calculation
- **Circuit breaker**: Test state transitions (closed → open → half-open)

**Testing Framework**: `tokio::test` for async tests, `mockall` for mocking external dependencies

### Property-Based Testing

Property-based tests will verify universal properties across many inputs using **proptest** library:

- **Minimum 100 iterations** per property test to ensure thorough coverage
- Each property test tagged with comment: `// Feature: vietnam-enterprise-cron, Property N: <description>`
- Properties test invariants that should hold for all valid inputs

**Key Property Tests**:
- Schedule calculations (cron, fixed delay, fixed rate)
- Variable substitution and precedence
- Idempotency key uniqueness
- Retry backoff timing
- Permission enforcement
- Distributed lock exclusivity

### Integration Testing

Integration tests will verify component interactions:

- **Database integration**: Test repository operations against real PostgreSQL
- **Queue integration**: Test job publishing and consumption with NATS
- **Redis integration**: Test distributed locking with Redis
- **HTTP executor**: Test HTTP requests against mock servers
- **End-to-end flows**: Test complete job scheduling and execution cycles

**Test Containers**: Use `testcontainers` crate for PostgreSQL, Redis, and NATS

### Load Testing

Performance tests to validate scalability:

- **Scheduler throughput**: 1000+ jobs scheduled per second
- **Worker throughput**: 500+ jobs executed per second per worker
- **Distributed lock contention**: 100 scheduler nodes competing for locks
- **Database query performance**: Sub-100ms for job lookups

**Tools**: `criterion` for benchmarking, custom load test harness

## Deployment Architecture

### Docker Deployment

**Multi-stage Dockerfile**:
```dockerfile
# Stage 1: Build
FROM rust:1.75-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /app
COPY . .
RUN cargo build --release

# Stage 2: Runtime
FROM alpine:3.19
RUN apk add --no-cache ca-certificates
COPY --from=builder /app/target/release/scheduler /usr/local/bin/
COPY --from=builder /app/target/release/worker /usr/local/bin/
COPY --from=builder /app/target/release/api /usr/local/bin/
USER nobody
```

**Target image size**: < 50MB

### Kubernetes Deployment

**Components**:
- **Scheduler Deployment**: 3+ replicas with anti-affinity
- **Worker Deployment**: Auto-scaling based on queue depth
- **API Deployment**: 2+ replicas behind load balancer
- **PostgreSQL StatefulSet**: Primary + read replicas
- **Redis Cluster**: 6 nodes (3 masters, 3 replicas)
- **NATS JetStream**: 3-node cluster

**Helm Chart Structure**:
```
charts/vietnam-enterprise-cron/
├── Chart.yaml
├── values.yaml
├── templates/
│   ├── scheduler-deployment.yaml
│   ├── worker-deployment.yaml
│   ├── api-deployment.yaml
│   ├── postgresql-statefulset.yaml
│   ├── redis-cluster.yaml
│   ├── nats-cluster.yaml
│   ├── configmap.yaml
│   ├── secrets.yaml
│   ├── service.yaml
│   ├── ingress.yaml
│   ├── rbac.yaml
│   └── hpa.yaml
```

**Resource Requests/Limits**:
- Scheduler: 100m CPU / 128Mi memory
- Worker: 200m CPU / 256Mi memory
- API: 200m CPU / 256Mi memory

### Configuration Management

**Layered Configuration** (using `config` crate):

1. **Default values** (embedded in binary)
2. **Config file** (`config.toml`)
3. **Environment variables** (override config file)
4. **Command-line arguments** (override everything)

**Configuration Structure**:
```toml
[server]
host = "0.0.0.0"
port = 8080

[database]
url = "postgresql://user:pass@localhost/cron"
max_connections = 20
min_connections = 5

[redis]
url = "redis://localhost:6379"
pool_size = 10

[nats]
url = "nats://localhost:4222"
stream_name = "jobs"

[minio]
endpoint = "localhost:9000"
access_key = "minioadmin"
secret_key = "minioadmin"
bucket_name = "vietnam-cron"
use_ssl = false

[auth]
mode = "database"  # or "keycloak"
jwt_secret = "secret"  # for database mode
jwt_expiry_hours = 24

[auth.keycloak]  # optional, for keycloak mode
server_url = "https://keycloak.example.com"
realm = "enterprise"
client_id = "cron-system"

[scheduler]
poll_interval_seconds = 10
lock_ttl_seconds = 30

[worker]
concurrency = 10
max_retries = 10
timeout_seconds = 300

[observability]
log_level = "info"
metrics_port = 9090
tracing_endpoint = "http://jaeger:4317"
```

### Monitoring and Alerting

**Prometheus Metrics**:
- `job_success_total{job_id, job_name}`
- `job_failed_total{job_id, job_name, reason}`
- `job_duration_seconds{job_id, job_name}`
- `job_queue_size{}`
- `scheduler_lock_acquisitions_total{}`
- `worker_executions_active{}`
- `circuit_breaker_state{target}`

**Grafana Dashboards**:
- Job execution overview (success rate, duration, throughput)
- System health (queue depth, worker utilization, error rates)
- Distributed lock contention
- Database and Redis performance

**Alert Rules**:
- Job failure rate > 10% for 5 minutes
- Job consecutive failures >= 3
- Queue depth > 1000 for 10 minutes
- Worker execution time > timeout threshold
- Database connection pool exhaustion
- Redis lock acquisition failures

### Security Considerations

1. **Secrets Management**:
   - Database credentials stored in Kubernetes secrets
   - JWT signing keys rotated regularly
   - Variable encryption keys managed via sealed secrets

2. **Network Security**:
   - TLS for all external communication
   - mTLS between internal services (optional)
   - Network policies to restrict pod-to-pod communication

3. **RBAC**:
   - Principle of least privilege for service accounts
   - Separate roles for read-only vs. admin operations
   - Audit logging for all privileged operations

4. **Input Validation**:
   - SQL injection prevention via parameterized queries
   - URL validation for HTTP jobs
   - Cron expression validation before storage

## Technology Stack

### Core Dependencies

```toml
[dependencies]
# Web framework
axum = "0.7"
tower = "0.4"
tower-http = "0.5"
hyper = "1.0"

# Async runtime
tokio = { version = "1.35", features = ["full"] }

# Database
sqlx = { version = "0.8", features = ["runtime-tokio-rustls", "postgres", "uuid", "chrono", "json"] }  # Updated from 0.7

# Queue
async-nats = "0.33"

# Redis
redis = { version = "0.24", features = ["tokio-comp", "connection-manager"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Time handling
chrono = "0.4"
chrono-tz = "0.9"  # Updated from 0.8 for latest timezone data
cron = "0.12"

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# Observability
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
tracing-opentelemetry = "0.23"  # Updated from 0.22 for latest OTLP support
opentelemetry = "0.22"  # Updated from 0.21
opentelemetry-otlp = "0.15"  # Updated from 0.14
metrics = "0.22"  # Updated from 0.21
metrics-exporter-prometheus = "0.15"  # Updated from 0.13

# Configuration
config = "0.14"  # Updated from 0.13 for better TOML support

# UUID
uuid = { version = "1.7", features = ["v4", "serde"] }  # Updated from 1.6

# HTTP client
reqwest = { version = "0.12", features = ["rustls-tls", "json"] }  # Updated from 0.11

# Authentication
jsonwebtoken = "9.3"  # Updated from 9.2 for security fixes
bcrypt = "0.15"

# Template engine
tera = "1.19"

# Database drivers for target databases
oracle = "0.6"  # Updated from 0.5 for better error handling
mysql_async = "0.34"  # Updated from 0.32 for MySQL 8.0+ compatibility

# MinIO / S3 client
rust-s3 = "0.34"  # Updated from 0.33 for better async support

# File processing
calamine = "0.24"  # Updated from 0.22 for better performance
rust_xlsxwriter = "0.65"  # Updated from 0.56 for more features
csv = "1.3"  # Stable

# SFTP client
ssh2 = "0.9"  # Stable

# HMAC for webhook signature validation
hmac = "0.12"
sha2 = "0.10"

# JSONPath for nested data access
serde_json_path = "0.6"

[dev-dependencies]
proptest = "1.4"
mockall = "0.12"
testcontainers = "0.17"  # Updated from 0.15 for better Docker support
criterion = "0.5"
```

### Module Organization

```
src/
├── main.rs                 # API server entry point
├── bin/
│   ├── scheduler.rs        # Scheduler binary
│   └── worker.rs           # Worker binary
├── config/
│   ├── mod.rs
│   └── settings.rs         # Configuration loading
├── errors/
│   ├── mod.rs
│   ├── domain.rs           # Domain errors (thiserror)
│   └── app.rs              # Application errors
├── models/
│   ├── mod.rs
│   ├── job.rs              # Job model
│   ├── execution.rs        # JobExecution model
│   ├── variable.rs         # Variable model
│   └── user.rs             # User model (for database auth)
├── scheduler/
│   ├── mod.rs
│   ├── trigger.rs          # Schedule trigger detection
│   ├── lock.rs             # Distributed locking
│   └── publisher.rs        # Job queue publisher
├── worker/
│   ├── mod.rs
│   ├── consumer.rs         # Queue consumer
│   ├── context.rs          # Job Context management
│   ├── reference.rs        # Reference resolver (variables, step outputs)
│   ├── executor/
│   │   ├── mod.rs
│   │   ├── http.rs         # HTTP executor
│   │   ├── database.rs     # Database executor
│   │   ├── file.rs         # File processing executor (Excel/CSV)
│   │   └── sftp.rs         # SFTP executor
│   ├── retry.rs            # Retry strategy
│   └── circuit_breaker.rs  # Circuit breaker
├── api/
│   ├── mod.rs
│   ├── routes.rs           # Route definitions
│   ├── handlers/
│   │   ├── mod.rs
│   │   ├── jobs.rs         # Job CRUD handlers
│   │   ├── executions.rs   # Execution history handlers
│   │   ├── variables.rs    # Variable management handlers
│   │   ├── auth.rs         # Authentication handlers
│   │   ├── webhook.rs      # Webhook handlers
│   │   ├── import_export.rs # Job import/export handlers
│   │   └── sse.rs          # Server-Sent Events
│   └── middleware/
│       ├── mod.rs
│       ├── auth.rs         # JWT validation
│       ├── rbac.rs         # Permission checking
│       └── rate_limit.rs   # Rate limiting for webhooks
├── db/
│   ├── mod.rs
│   ├── migrations/         # SQL migrations
│   ├── repositories/
│   │   ├── mod.rs
│   │   ├── job.rs          # Job repository
│   │   ├── execution.rs    # Execution repository
│   │   ├── variable.rs     # Variable repository
│   │   └── user.rs         # User repository
│   └── pool.rs             # Connection pool
├── queue/
│   ├── mod.rs
│   ├── nats.rs             # NATS JetStream client
│   └── message.rs          # Message types
├── storage/
│   ├── mod.rs
│   ├── minio.rs            # MinIO client for object storage
│   └── file_processor.rs   # File processing utilities
├── telemetry/
│   ├── mod.rs
│   ├── logging.rs          # Structured logging setup
│   ├── metrics.rs          # Prometheus metrics
│   └── tracing.rs          # OpenTelemetry tracing
└── web/
    ├── mod.rs
    └── templates/          # HTMX templates
        ├── layout.html
        ├── jobs.html
        ├── executions.html
        └── variables.html
```

## Implementation Phases

The implementation will be broken down into manageable phases:

**Phase 1: Foundation**
- Project setup and module structure
- Configuration management
- Database schema and migrations
- Error handling framework

**Phase 2: Core Scheduling**
- Schedule parsing (cron, fixed delay, fixed rate, one-time)
- Timezone handling
- Job repository and persistence

**Phase 3: Distributed Coordination**
- Redis distributed locking
- NATS JetStream integration
- Scheduler component

**Phase 4: Job Execution**
- Worker component and queue consumer
- HTTP executor with authentication
- Database executor (PostgreSQL, MySQL, Oracle)
- Retry logic and circuit breaker

**Phase 5: Variable Management**
- Variable storage and encryption
- Template substitution engine
- Variable precedence rules

**Phase 6: Observability**
- Structured logging
- Prometheus metrics
- OpenTelemetry tracing
- Alert triggers

**Phase 7: API and Dashboard**
- REST API with Axum
- Authentication (database and Keycloak modes)
- RBAC middleware
- HTMX dashboard
- Server-Sent Events

**Phase 8: Production Readiness**
- Graceful shutdown
- Docker images
- Helm charts
- Documentation (Vietnamese)
- Load testing

