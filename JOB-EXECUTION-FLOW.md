# 🔄 Job Execution Flow - Chi tiết đầy đủ

## 📊 Tổng quan Architecture

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Scheduler  │────▶│    NATS     │────▶│   Worker    │
│   Service   │     │ JetStream   │     │   Service   │
└─────────────┘     └─────────────┘     └─────────────┘
       │                                        │
       │                                        │
       ▼                                        ▼
┌─────────────┐                        ┌─────────────┐
│ PostgreSQL  │◀───────────────────────│    MinIO    │
│  Database   │                        │   Storage   │
└─────────────┘                        └─────────────┘
       ▲                                        ▲
       │                                        │
       └────────────────┬───────────────────────┘
                        │
                  ┌─────────────┐
                  │     API     │
                  │   Service   │
                  └─────────────┘
```

---

## 🎬 Flow 1: Job Creation (Tạo Job)

### Step 1: User tạo job qua API/Dashboard

**API Endpoint**: `POST /api/jobs`

**Handler**: `api/src/handlers/jobs.rs::create_job()`

```rust
pub async fn create_job(
    State(state): State<AppState>,
    Json(request): Json<CreateJobRequest>,
) -> Result<Json<SuccessResponse<Uuid>>, ErrorResponse>
```

**Flow**:
```
1. Validate input (name, schedule, steps, etc.)
   ├─ Check duplicate name
   ├─ Validate cron expression (if scheduled)
   └─ Validate step configuration

2. Create Job struct
   ├─ Generate UUID
   ├─ Set default values (timeout, max_retries)
   └─ Set trigger config (scheduled, manual, webhook)

3. Save to PostgreSQL
   ├─ JobRepository::create()
   └─ INSERT INTO jobs (id, name, enabled, trigger_config, ...)

4. Save job definition to MinIO
   ├─ MinIOService::store_job_definition()
   ├─ Serialize Job to JSON
   └─ Upload to: jobs/{job_id}/definition.json

5. Return job_id to user
```

**Database Changes**:
```sql
-- PostgreSQL: jobs table
INSERT INTO jobs (
    id, name, description, enabled, 
    timeout_seconds, max_retries, allow_concurrent,
    minio_definition_path, trigger_config,
    created_at, updated_at
) VALUES (
    'uuid', 'Job Name', 'Description', true,
    300, 10, false,
    'jobs/uuid/definition.json', 
    '{"scheduled": true, "manual": true, "webhook": null}',
    NOW(), NOW()
);
```

**MinIO Changes**:
```
Upload file: jobs/{job_id}/definition.json
Content: Full job definition JSON (steps, schedule, etc.)
```

---

## 🔄 Flow 2: Scheduled Job Execution (Job chạy theo schedule)

### Phase 1: SCHEDULER - Trigger Detection

**Binary**: `scheduler/src/main.rs`

**Main Loop**: `common/src/scheduler/engine.rs::SchedulerEngine::start()`

```rust
pub async fn start(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
```

**Flow**:
```
┌─────────────────────────────────────────────────────────────┐
│ SCHEDULER POLLING LOOP (Every 10 seconds)                   │
└─────────────────────────────────────────────────────────────┘

1. Poll for jobs due
   ├─ JobRepository::find_jobs_due(now)
   ├─ SELECT * FROM jobs WHERE enabled = true
   └─ Filter jobs with scheduled trigger enabled

2. For each job due:
   ├─ Check trigger conditions
   │  ├─ Scheduled trigger enabled?
   │  ├─ Cron expression matches current time?
   │  └─ Next execution time <= now?
   │
   ├─ Check concurrent execution
   │  ├─ ExecutionRepository::has_running_execution(job_id)
   │  ├─ SELECT COUNT(*) FROM job_executions 
   │  │  WHERE job_id = ? AND status IN ('running', 'pending')
   │  └─ If allow_concurrent = false AND has_running → Skip
   │
   ├─ Acquire distributed lock (Redis RedLock)
   │  ├─ DistributedLock::acquire("schedule:job:{job_id}", 30s)
   │  ├─ SET NX EX schedule:job:{job_id} {lock_value} 30
   │  └─ If lock failed → Skip (another scheduler is processing)
   │
   ├─ Create execution record
   │  ├─ Generate execution_id (UUID)
   │  ├─ Generate idempotency_key: "{job_id}:{uuid}"
   │  ├─ ExecutionRepository::create()
   │  └─ INSERT INTO job_executions (
   │       id, job_id, idempotency_key, status='pending',
   │       trigger_source='scheduled', created_at
   │     )
   │
   ├─ Publish message to NATS
   │  ├─ JobPublisher::publish(execution)
   │  ├─ Create JobMessage { execution_id, job_id, idempotency_key }
   │  ├─ Serialize to JSON
   │  ├─ Publish to subject: "jobs.job_stream.{job_id}"
   │  ├─ Headers: Nats-Msg-Id = idempotency_key (deduplication)
   │  └─ Wait for ACK from NATS
   │
   ├─ Update job stats
   │  └─ JobRepository::update_stats(job_id, success=true)
   │
   └─ Release distributed lock
      └─ DEL schedule:job:{job_id}
```

**Database State After Scheduler**:
```sql
-- job_executions table
id                  | job_id | status  | idempotency_key        | created_at
--------------------|--------|---------|------------------------|------------
execution-uuid-123  | job-1  | pending | job-1:unique-uuid-456  | 2025-11-26 08:00:00
```

**NATS State After Scheduler**:
```
Stream: job_stream
Subject: jobs.job_stream.{job_id}
Message: {
  "execution_id": "execution-uuid-123",
  "job_id": "job-1",
  "idempotency_key": "job-1:unique-uuid-456",
  "attempt": 1,
  "published_at": "2025-11-26T08:00:00Z"
}
Headers: {
  "Nats-Msg-Id": "job-1:unique-uuid-456"
}
```

---

### Phase 2: WORKER - Job Consumption & Execution

**Binary**: `worker/src/main.rs`

**Consumer**: `common/src/queue/consumer.rs::NatsJobConsumer::start()`

**Handler**: `common/src/worker/consumer.rs::WorkerJobConsumer::process_job()`

```rust
async fn process_job(
    job_message: JobMessage,
    job_repo: Arc<JobRepository>,
    execution_repo: Arc<ExecutionRepository>,
    // ... other dependencies
) -> Result<(), anyhow::Error>
```

**Flow**:
```
┌─────────────────────────────────────────────────────────────┐
│ WORKER - MESSAGE CONSUMPTION                                 │
└─────────────────────────────────────────────────────────────┘

1. Receive message from NATS
   ├─ NatsJobConsumer::start() → messages.next()
   ├─ Deserialize JobMessage from JSON
   └─ Log: "Processing message, stream_sequence: X"

2. Check idempotency (Exactly-once execution)
   ├─ ExecutionRepository::find_by_idempotency_key(key)
   ├─ SELECT * FROM job_executions WHERE idempotency_key = ?
   ├─ If found AND status IN (success, failed, timeout, dead_letter):
   │  ├─ Log: "Job already completed, skipping"
   │  ├─ ACK message
   │  └─ Return OK (skip processing)
   └─ If found AND status IN (pending, running):
      └─ Log: "Found existing execution in progress, will process it"

3. Load job metadata from PostgreSQL
   ├─ JobRepository::find_by_id(job_id)
   ├─ SELECT * FROM jobs WHERE id = ?
   └─ Get: name, timeout, max_retries, minio_definition_path

4. Load full job definition from MinIO
   ├─ MinIOService::load_job_definition(job_id)
   ├─ GET jobs/{job_id}/definition.json from MinIO
   ├─ Parse JSON to Job struct
   └─ Get: steps[], schedule, triggers

5. Update execution status to RUNNING
   ├─ execution.status = Running
   ├─ execution.started_at = NOW()
   ├─ ExecutionRepository::update(execution)
   ├─ UPDATE job_executions SET status='running', started_at=NOW()
   └─ Publish status change event to NATS (for SSE)

6. Initialize or load Job Context
   ├─ Try load existing context from MinIO
   ├─ GET jobs/{job_id}/executions/{execution_id}/context.json
   ├─ If not found: Create new JobContext
   └─ JobContext { execution_id, job_id, steps: {}, variables: {} }

7. Execute job steps sequentially
   └─ For each step in job.steps:
      │
      ├─ Update current_step in execution
      │  └─ UPDATE job_executions SET current_step = ?
      │
      ├─ Check step condition (if any)
      │  └─ Evaluate condition expression
      │
      ├─ Route to appropriate executor
      │  ├─ HttpRequest → HttpExecutor
      │  ├─ DatabaseQuery → DatabaseExecutor
      │  ├─ FileProcessing → FileProcessingExecutor
      │  └─ Sftp → SftpExecutor
      │
      ├─ Execute step with retry logic
      │  ├─ Attempt 1: executor.execute(step, context)
      │  ├─ If failed: Wait with exponential backoff
      │  ├─ Attempt 2: executor.execute(step, context)
      │  └─ ... up to max_retries
      │
      ├─ Store step output in context
      │  ├─ context.set_step_output(step_id, output)
      │  └─ Output: { status, data, headers, timing }
      │
      ├─ Persist context to MinIO after each step
      │  ├─ MinIOService::store_context(context)
      │  ├─ Serialize context to JSON
      │  └─ PUT jobs/{job_id}/executions/{execution_id}/context.json
      │
      └─ If step failed AND on_failure = "stop":
         └─ Break loop, mark execution as failed

8. Update final execution status
   ├─ If all steps succeeded:
   │  ├─ execution.status = Success
   │  ├─ execution.completed_at = NOW()
   │  ├─ execution.result = "Job completed successfully"
   │  └─ UPDATE job_executions SET status='success', completed_at=NOW()
   │
   └─ If any step failed:
      ├─ execution.status = Failed
      ├─ execution.completed_at = NOW()
      ├─ execution.error = error_message
      └─ UPDATE job_executions SET status='failed', error=?, completed_at=NOW()

9. Save final context to MinIO
   ├─ MinIOService::store_context(context)
   └─ PUT jobs/{job_id}/executions/{execution_id}/context.json

10. Publish final status change event
    └─ NATS publish: status.execution.{execution_id}

11. ACK message to NATS
    ├─ message.ack()
    └─ Message removed from stream (WorkQueue retention)

12. Update job stats
    ├─ JobRepository::update_stats(job_id, success)
    └─ UPDATE job_stats SET total_executions++, ...
```

---

## 🔍 Chi tiết Step Execution (HTTP Request Example)

**Executor**: `common/src/executor/http.rs::HttpExecutor::execute()`

```rust
pub async fn execute(
    &self,
    step: &JobStep,
    context: &mut JobContext,
) -> Result<StepOutput, ExecutorError>
```

**Flow**:
```
┌─────────────────────────────────────────────────────────────┐
│ HTTP EXECUTOR - Execute HTTP Request Step                    │
└─────────────────────────────────────────────────────────────┘

1. Extract HTTP configuration from step
   ├─ step.step_type = JobType::HttpRequest { url, method, headers, body, auth }
   └─ Resolve variables in URL/headers/body using context

2. Build HTTP request
   ├─ reqwest::Client::new()
   ├─ Set method (GET/POST/PUT/DELETE)
   ├─ Set URL (with variable substitution)
   ├─ Set headers
   ├─ Set body (if POST/PUT)
   └─ Set authentication (Basic/Bearer/OAuth2)

3. Execute request with timeout
   ├─ timeout(step.timeout_seconds, client.send(request))
   └─ If timeout → Return ExecutorError::Timeout

4. Process response
   ├─ Get status code
   ├─ Get headers
   ├─ Read body as text/json
   └─ Calculate duration

5. Create StepOutput
   └─ StepOutput {
        status: "success",
        data: response_body,
        metadata: {
          "status_code": 200,
          "headers": {...},
          "duration_ms": 123
        }
      }

6. Return output (will be stored in context)
```

---

## 🎯 Flow 3: Manual Job Trigger (User click "Trigger" button)

**API Endpoint**: `POST /api/jobs/{id}/trigger`

**Handler**: `api/src/handlers/jobs.rs::trigger_job()`

```rust
pub async fn trigger_job(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SuccessResponse<Uuid>>, ErrorResponse>
```

**Flow**:
```
┌─────────────────────────────────────────────────────────────┐
│ MANUAL TRIGGER FLOW                                          │
└─────────────────────────────────────────────────────────────┘

1. Validate job exists
   ├─ JobRepository::find_by_id(id)
   └─ If not found → Return 404

2. Check concurrent execution
   ├─ If allow_concurrent = false:
   ├─ ExecutionRepository::has_running_execution(job_id)
   └─ If has_running → Return 500 "concurrent_execution_not_allowed"

3. Create execution record
   ├─ Generate execution_id
   ├─ Generate idempotency_key: "manual-{job_id}-{execution_id}"
   ├─ ExecutionRepository::create()
   └─ INSERT INTO job_executions (
        id, job_id, idempotency_key, status='pending',
        trigger_source='manual', created_at
      )

4. Publish to NATS
   ├─ JobPublisher::publish(execution)
   ├─ Subject: jobs.job_stream.{job_id}
   └─ Wait for ACK

5. Return execution_id to user
   └─ Response: { "data": "execution-uuid" }

6. Worker picks up message (same as scheduled flow)
   └─ See Phase 2 above
```

---

## 📊 Database Schema & State Transitions

### jobs table
```sql
CREATE TABLE jobs (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE,
    description TEXT,
    enabled BOOLEAN DEFAULT true,
    timeout_seconds INTEGER DEFAULT 300,
    max_retries INTEGER DEFAULT 10,
    allow_concurrent BOOLEAN DEFAULT false,
    minio_definition_path VARCHAR(500) NOT NULL,
    trigger_config JSONB NOT NULL,  -- {"scheduled": bool, "manual": bool, "webhook": {...}}
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);
```

### job_executions table
```sql
CREATE TABLE job_executions (
    id UUID PRIMARY KEY,
    job_id UUID NOT NULL REFERENCES jobs(id),
    idempotency_key VARCHAR(255) NOT NULL UNIQUE,
    status VARCHAR(50) NOT NULL,  -- pending, running, success, failed, timeout, dead_letter
    attempt INTEGER DEFAULT 1,
    trigger_source VARCHAR(50) NOT NULL,  -- scheduled, manual, webhook
    trigger_metadata JSONB,
    current_step VARCHAR(255),
    minio_context_path VARCHAR(500) NOT NULL,
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    result TEXT,
    error TEXT,
    created_at TIMESTAMP NOT NULL
);

CREATE INDEX idx_job_executions_job_id ON job_executions(job_id);
CREATE INDEX idx_job_executions_status ON job_executions(status);
CREATE INDEX idx_job_executions_idempotency_key ON job_executions(idempotency_key);
```

### Execution Status Transitions
```
pending → running → success
                 → failed
                 → timeout
                 → dead_letter (after max retries)
```

---

## 🗄️ MinIO Storage Structure

```
vietnam-cron/
├── jobs/
│   └── {job_id}/
│       ├── definition.json          # Job definition (steps, schedule, etc.)
│       └── executions/
│           └── {execution_id}/
│               ├── context.json     # Job context (step outputs, variables)
│               └── files/           # Uploaded/processed files
│                   ├── input/
│                   └── output/
```

### Job Definition JSON (definition.json)
```json
{
  "id": "job-uuid",
  "name": "Job Name",
  "description": "Description",
  "schedule": {
    "Cron": {
      "expression": "0 */10 * * * *",
      "timezone": "Asia/Ho_Chi_Minh"
    }
  },
  "steps": [
    {
      "id": "step-1",
      "name": "Fetch Data",
      "type": {
        "HttpRequest": {
          "url": "https://api.example.com/data",
          "method": "GET",
          "headers": {},
          "body": null,
          "auth": null
        }
      },
      "condition": null
    }
  ],
  "triggers": {
    "scheduled": true,
    "manual": true,
    "webhook": null
  },
  "enabled": true,
  "timeout_seconds": 300,
  "max_retries": 10,
  "allow_concurrent": false
}
```

### Job Context JSON (context.json)
```json
{
  "execution_id": "execution-uuid",
  "job_id": "job-uuid",
  "steps": {
    "step-1": {
      "status": "success",
      "data": "{\"temperature\": 25}",
      "metadata": {
        "status_code": 200,
        "duration_ms": 123
      },
      "completed_at": "2025-11-26T08:00:01Z"
    }
  },
  "variables": {
    "API_KEY": "secret-value"
  },
  "created_at": "2025-11-26T08:00:00Z",
  "updated_at": "2025-11-26T08:00:01Z"
}
```

---

## 🔄 NATS JetStream Configuration

### Stream Configuration
```
Name: job_stream
Subjects: jobs.>
Retention: WorkQueue (messages deleted after ACK)
Max Age: 24 hours
Max Messages: 1,000,000
```

### Consumer Configuration
```
Name: worker-consumer
Durable: true
Ack Policy: Explicit (manual ACK required)
Max Deliver: 10 (max retry attempts)
Ack Wait: 5 minutes (timeout before redelivery)
```

### Message Flow
```
1. Scheduler publishes → Stream stores message
2. Worker pulls message → Stream marks as "delivered"
3. Worker processes job
4. Worker ACKs message → Stream deletes message (WorkQueue)

If Worker NAKs or timeout:
→ Stream redelivers message (up to Max Deliver times)
→ After Max Deliver: Message goes to dead letter
```

---

## 🔐 Redis Distributed Lock (RedLock)

**Purpose**: Ensure only one scheduler processes each job

**Flow**:
```
1. Scheduler tries to acquire lock
   ├─ SET NX EX schedule:job:{job_id} {random_value} 30
   └─ If SET returns 1 → Lock acquired

2. Scheduler processes job
   └─ Create execution, publish to NATS

3. Scheduler releases lock
   ├─ Check lock value matches (prevent releasing other's lock)
   └─ DEL schedule:job:{job_id}

If scheduler crashes:
→ Lock expires after 30 seconds (TTL)
→ Another scheduler can acquire lock
```

---

## 📈 Monitoring & Observability

### Logs (JSON format)
```json
{
  "timestamp": "2025-11-26T08:00:00Z",
  "level": "INFO",
  "message": "Processing job",
  "span": {
    "execution_id": "uuid",
    "job_id": "uuid",
    "name": "process_job"
  }
}
```

### Metrics (Prometheus)
```
# Job execution metrics
job_executions_total{status="success|failed|timeout"}
job_execution_duration_seconds{job_id="..."}
job_queue_depth{stream="job_stream"}

# System metrics
scheduler_poll_duration_seconds
worker_message_processing_duration_seconds
```

### Tracing (OpenTelemetry)
```
Trace: Job Execution
├─ Span: scheduler.process_job
│  ├─ Span: db.find_jobs_due
│  ├─ Span: lock.acquire
│  └─ Span: queue.publish
└─ Span: worker.process_job
   ├─ Span: db.load_job
   ├─ Span: minio.load_definition
   ├─ Span: executor.execute_step
   └─ Span: minio.store_context
```

---

## 🎯 Summary: Complete Flow Diagram

```
┌──────────────────────────────────────────────────────────────────────┐
│                         JOB EXECUTION FLOW                            │
└──────────────────────────────────────────────────────────────────────┘

TIME    SCHEDULER              POSTGRESQL         NATS          WORKER              MINIO
─────   ─────────              ──────────         ────          ──────              ─────
T+0s    Poll jobs due ────────▶ SELECT jobs
        │                       WHERE enabled=true
        │                       AND scheduled=true
        │◀─────────────────────
        │
T+1s    Check concurrent ─────▶ SELECT COUNT(*)
        execution               FROM job_executions
        │◀─────────────────────
        │
T+2s    Acquire Redis lock
        (RedLock)
        │
T+3s    Create execution ─────▶ INSERT INTO
        │                       job_executions
        │                       (status=pending)
        │◀─────────────────────
        │
T+4s    Publish message ───────────────────────▶ Store in
        │                                        job_stream
        │                                        │
        │                                        │
T+5s    Release lock                             │
                                                 │
                                                 │
T+6s                                             │ Pull msg ──▶ Receive
                                                 │              message
                                                 │              │
T+7s                                             │              Check ──▶ SELECT *
                                                 │              idempo-  FROM
                                                 │              tency    job_exec
                                                 │              │◀───────
                                                 │              │
T+8s                                             │              Load ───▶ SELECT *
                                                 │              job      FROM jobs
                                                 │              │◀───────
                                                 │              │
T+9s                                             │              Load ───────────────▶ GET
                                                 │              defini-              definition
                                                 │              tion                 .json
                                                 │              │◀──────────────────
                                                 │              │
T+10s                                            │              Update ─▶ UPDATE
                                                 │              status   job_exec
                                                 │              =running SET status
                                                 │              │◀───────
                                                 │              │
T+11s                                            │              Execute
                                                 │              Step 1
                                                 │              (HTTP)
                                                 │              │
T+12s                                            │              Store ──────────────▶ PUT
                                                 │              context              context
                                                 │              │                    .json
                                                 │              │◀──────────────────
                                                 │              │
T+13s                                            │              Execute
                                                 │              Step 2
                                                 │              ...
                                                 │              │
T+14s                                            │              Update ─▶ UPDATE
                                                 │              status   job_exec
                                                 │              =success SET status
                                                 │              │◀───────
                                                 │              │
T+15s                                            │              ACK msg
                                                 │◀─────────────
                                                 │
                                                 Delete msg
                                                 (WorkQueue)

```

---

**Tạo bởi**: Kiro AI Assistant  
**Ngày**: 2025-11-26  
**Version**: 1.0
