# Vietnam Enterprise Cron System - Sequence Diagrams

## Tổng quan (Overview)

Bộ tài liệu này chứa các sequence diagram chi tiết mô tả các luồng nghiệp vụ (business flows) của hệ thống Vietnam Enterprise Cron. Các diagram được thiết kế theo định dạng PlantUML để mô tả tương tác giữa các components theo thời gian.

This documentation contains detailed sequence diagrams describing the business flows of the Vietnam Enterprise Cron System. The diagrams are designed in PlantUML format to describe interactions between components over time.

## Danh sách Sequence Diagrams

### 1. `sequence-01-job-scheduling.puml` - Job Scheduling Flow
**Mô tả:** Luồng lên lịch job với distributed coordination và exactly-once guarantee.

**Các bước chính:**
1. **Poll for Due Jobs**: Tất cả scheduler nodes poll database để tìm jobs đến hạn
2. **Acquire Distributed Lock**: Sử dụng Redis RedLock algorithm
   - Chỉ 1 trong N schedulers acquire được lock
   - TTL: 30 seconds
   - Prevents duplicate scheduling
3. **Generate Idempotency Key**: Format `{job_id}:{scheduled_time}:{uuid}`
4. **Create Execution Record**: Insert vào `job_executions` table
5. **Publish to Queue**: Publish job message vào NATS JetStream
6. **Release Lock**: Release distributed lock
7. **Calculate Next Execution Time**: Dựa trên schedule type (Cron/Fixed Delay/Fixed Rate/One-Time)

**Exactly-Once Guarantee (4 layers):**
- Layer 1: Distributed lock (Redis RedLock)
- Layer 2: Idempotency key check
- Layer 3: Database unique constraint
- Layer 4: NATS acknowledgment

**Validates Requirements:** 1.1-1.7, 4.1-4.4, 7.1

**Key Components:**
- Multiple Scheduler Nodes (distributed)
- PostgreSQL (System Database)
- Redis Cluster (3+ nodes for RedLock)
- NATS JetStream (Job Queue)

---

### 2. `sequence-02-http-job-execution.puml` - HTTP Job Execution
**Mô tả:** Luồng thực thi HTTP job với variable resolution và retry logic.

**Các bước chính:**
1. **Consume Job from Queue**: Worker subscribe NATS stream
2. **Check Idempotency**: Query database để check duplicate
3. **Update Execution Status**: Set status = 'Running'
4. **Resolve Variables**: 
   - Load global variables
   - Load job-specific variables
   - Apply precedence (job-specific > global)
   - Decrypt sensitive variables
   - Substitute placeholders in URL/headers/body
5. **Check Circuit Breaker**: Verify circuit state (CLOSED/OPEN/HALF-OPEN)
6. **Execute HTTP Request**:
   - Build request with resolved variables
   - Add authentication (Basic/Bearer/OAuth2)
   - Set timeout (configurable)
   - Send request to external API
7. **Handle Response**:
   - Success: Update status, ACK message, emit metrics
   - Failure: Update status, schedule retry with exponential backoff
   - Timeout: Mark as timeout, schedule retry

**Retry Logic:**
- Attempt 1: 5s + jitter
- Attempt 2: 15s + jitter
- Attempt 3: 1m + jitter
- Attempt 4: 5m + jitter
- Attempt 5: 30m + jitter
- Max retries: 10
- After 10 failures: Move to Dead Letter Queue

**Validates Requirements:** 2.3-2.12, 3.1-3.6, 4.2-4.10, 5.1-5.9

**Key Components:**
- Worker Process
- Variable Resolver
- HTTP Executor
- Circuit Breaker
- Retry Manager
- External API

---

### 3. `sequence-03-database-job-execution.puml` - Database Job Execution
**Mô tả:** Luồng thực thi database job với parameterized queries (SQL injection prevention).

**Các bước chính:**
1. **Consume Job from Queue**: Worker receive database job message
2. **Check Idempotency**: Prevent duplicate execution
3. **Resolve Variables**:
   - Substitute connection string variables
   - Convert query to parameterized query
   - Extract parameters safely
4. **Execute Database Query**:
   - Determine database driver (PostgreSQL/MySQL/Oracle)
   - Connect to target database
   - Begin transaction (if configured)
   - Execute parameterized query
   - Commit or rollback transaction
5. **Handle Results**:
   - Success: Record rows affected, execution time
   - Failure: Determine if retriable or not
   - Connection error: Retry with backoff

**SQL Injection Prevention:**
```
Template: UPDATE users SET status = {{status}} WHERE created_at < {{cutoff_date}}
Converted: UPDATE users SET status = $1 WHERE created_at < $2
Parameters: ["inactive", "2024-01-01"]
```

**Database Support:**
- PostgreSQL: Parameter markers `$1, $2, $3, ...`
- MySQL: Parameter markers `?, ?, ?, ...`
- Oracle 19c: Parameter markers `:1, :2, :3, ...`

**Validates Requirements:** 2.11-2.12, 3.7-3.10, 4.2-4.10

**Key Components:**
- Worker Process
- Variable Resolver
- Database Executor
- Target Database (Oracle/MySQL/PostgreSQL)

---

### 4. `sequence-04-authentication-keycloak.puml` - Authentication (Keycloak Mode)
**Mô tả:** Luồng authentication và authorization sử dụng Keycloak (external identity provider).

**Các bước chính:**
1. **Initial Login** (External to System):
   - User redirects to Keycloak login page
   - User enters credentials
   - Keycloak validates and issues JWT token
   - Browser stores token in localStorage
2. **API Request with JWT Token**:
   - Browser sends request with `Authorization: Bearer {token}`
3. **JWT Validation**:
   - Extract token from header
   - Decode JWT
   - Fetch Keycloak public keys (with caching)
   - Verify signature
   - Check expiration and issuer
   - Extract user claims (sub, username, permissions)
4. **RBAC Permission Check**:
   - Extract permissions from JWT claims
   - Check required permission (e.g., `job:write`)
   - Allow or deny request
5. **Execute Business Logic**: If authorized
6. **Audit Logging**: Log operation with user identity

**Keycloak Resilience:**
- Public keys cached with TTL (1 hour)
- If Keycloak unavailable: Use cached keys
- System continues operating during Keycloak outage

**JWT Claims Structure:**
```json
{
  "sub": "user-123",
  "username": "admin",
  "resource_access": {
    "cron-system": {
      "roles": ["job:read", "job:write", "job:execute"]
    }
  },
  "exp": 1737369600,
  "iat": 1737283200
}
```

**Validates Requirements:** 10.1, 10.4-10.12

**Key Components:**
- API Server (Axum)
- JWT Middleware
- RBAC Middleware
- Keycloak Server
- Redis Cache (for public keys)

---

### 5. `sequence-05-authentication-database.puml` - Authentication (Database Mode)
**Mô tả:** Luồng authentication và authorization sử dụng local database.

**Các bước chính:**
1. **User Login**:
   - User submits username/password
   - System queries `users` table
   - Verify password using bcrypt
   - Generate JWT token (signed with system private key)
   - Return token to client
2. **Subsequent API Requests**:
   - Client sends request with JWT token
   - System validates JWT signature
   - Check expiration and issuer
   - Extract user claims
3. **RBAC Permission Check**:
   - Extract permissions from JWT
   - Check required permission
   - Allow or deny request
4. **User Management** (Admin operations):
   - Create user with bcrypt password hash
   - Update user details
   - Assign roles to users
   - Disable/enable users
5. **Role Management**:
   - Create roles with permissions array
   - Update role permissions
   - Assign roles to users

**Database Schema:**
```sql
users:
- id (UUID)
- username (unique)
- password_hash (bcrypt)
- email
- enabled (boolean)

roles:
- id (UUID)
- name (e.g., "admin", "operator")
- permissions (text[] array)

user_roles:
- user_id (FK)
- role_id (FK)
```

**Password Security:**
- bcrypt hashing (cost factor: 12)
- Automatic salt generation
- Timing attack prevention

**Validates Requirements:** 10.2-10.3, 10.13

**Key Components:**
- API Server
- Auth Handler
- JWT Middleware
- RBAC Middleware
- PostgreSQL (System Database)

---

### 6. `sequence-06-dashboard-realtime-updates.puml` - Dashboard Real-time Updates
**Mô tả:** Luồng real-time updates sử dụng Server-Sent Events (SSE) và HTMX.

**Các bước chính:**
1. **Initial Dashboard Load**:
   - User navigates to dashboard
   - API renders HTMX template with job list
   - Browser displays dashboard
2. **Establish SSE Connection**:
   - HTMX initiates SSE connection to `/api/events/stream`
   - API validates JWT token
   - SSE handler subscribes to event broadcaster
   - Connection established (long-lived)
3. **Job Execution Starts** (Worker Event):
   - Worker updates execution status to 'Running'
   - Worker publishes event to broadcaster
   - Broadcaster pushes SSE event to all connected clients
   - HTMX receives event and updates DOM
   - User sees update (no page reload!)
4. **Job Execution Completes** (Worker Event):
   - Worker updates execution status to 'Success'
   - Worker publishes event
   - SSE pushes update to clients
   - HTMX updates DOM with new status and statistics
5. **User Triggers Manual Job**:
   - User clicks "Run Now" button
   - HTMX sends POST request
   - API creates execution and publishes event
   - All connected users see the update in real-time
6. **Connection Management**:
   - Heartbeat every 30 seconds
   - Automatic reconnection on failure
   - Event replay with `Last-Event-ID` header

**SSE Event Format:**
```
event: execution_completed
data: {"job_id": "uuid", "status": "Success", "duration": 2.3}
id: 12346

```

**Benefits of SSE:**
- Real-time updates without polling
- Efficient (single long-lived connection)
- Automatic reconnection
- Event replay capability
- Simple protocol (text-based)
- Works seamlessly with HTMX

**Validates Requirements:** 6.7, 6.8

**Key Components:**
- Browser with HTMX
- API Server
- SSE Handler
- Event Broadcaster
- Worker Process

---

### 7. `sequence-07-graceful-shutdown.puml` - Graceful Shutdown
**Mô tả:** Luồng graceful shutdown cho Scheduler và Worker processes.

**Các bước chính:**

**Scheduler Graceful Shutdown:**
1. Kubernetes sends SIGTERM signal
2. Stop polling for new jobs
3. Complete in-flight lock acquisitions
4. Publish queued jobs to NATS
5. Release all held locks
6. Close connections (Redis, NATS, PostgreSQL)
7. Exit with code 0

**Worker Graceful Shutdown:**
1. Kubernetes sends SIGTERM signal
2. Unsubscribe from NATS (stop consuming new jobs)
3. Wait for in-flight jobs to complete
4. Complete successful jobs: Update DB, ACK message
5. Failed jobs: Update DB, NACK message (requeue)
6. Handle shutdown timeout (30 seconds):
   - Force terminate if exceeded
   - NACK all in-flight messages
7. Close connections
8. Exit with code 0

**Kubernetes Configuration:**
```yaml
terminationGracePeriodSeconds: 60
strategy:
  type: RollingUpdate
  rollingUpdate:
    maxUnavailable: 1
    maxSurge: 1
```

**Zero-Downtime Guarantee:**
- Multiple scheduler/worker nodes running
- If one shuts down, others continue
- Distributed lock prevents duplicates
- Idempotency key prevents duplicate execution
- NATS message persistence (no job loss)

**Validates Requirements:** 7.6-7.7

**Key Components:**
- Kubernetes
- Scheduler Process
- Worker Process
- Signal Handler
- PostgreSQL, Redis, NATS

---

### 8. `sequence-08-circuit-breaker.puml` - Circuit Breaker Pattern
**Mô tả:** Luồng circuit breaker để prevent cascading failures.

**Các bước chính:**

**Circuit States:**
1. **CLOSED (Normal)**:
   - All requests pass through
   - Track failure rate
   - If threshold exceeded: Open circuit
2. **OPEN (Failing)**:
   - Fail fast (no external calls)
   - Save resources
   - After timeout: Transition to HALF-OPEN
3. **HALF-OPEN (Testing)**:
   - Allow limited test requests
   - If success: Close circuit
   - If failure: Reopen circuit

**State Transitions:**
```
CLOSED → OPEN (failure threshold exceeded)
OPEN → HALF-OPEN (timeout elapsed)
HALF-OPEN → CLOSED (test call succeeds)
HALF-OPEN → OPEN (test call fails)
```

**Configuration:**
```rust
CircuitBreakerConfig {
  failure_rate_threshold: 0.5,  // 50%
  consecutive_failures_threshold: 5,
  window_size: 100,  // Last 100 requests
  timeout_duration: Duration::from_secs(30),
  timeout_multiplier: 2.0,  // Double on reopen
  max_timeout: Duration::from_secs(300),
}
```

**Benefits:**
- Prevent cascading failures
- Fast failure (no waiting for timeouts)
- Automatic recovery
- Resource conservation
- Clear observability

**Validates Requirements:** 4.7

**Key Components:**
- Worker Process
- Circuit Breaker
- Failure Tracker
- External API
- Metrics & Alerts

---

### 9. `sequence-09-multi-step-job-execution.puml` - Multi-Step Job Execution with MinIO
**Mô tả:** Luồng thực thi job có nhiều steps với Job Context được lưu trong MinIO.

**Các bước chính:**

**Circuit States:**
1. **CLOSED (Normal)**:
   - All requests pass through
   - Track failure rate
   - If threshold exceeded: Open circuit
2. **OPEN (Failing)**:
   - Fail fast (no external calls)
   - Save resources
   - After timeout: Transition to HALF-OPEN
3. **HALF-OPEN (Testing)**:
   - Allow limited test requests
   - If success: Close circuit
   - If failure: Reopen circuit

**State Transitions:**
```
CLOSED → OPEN (failure threshold exceeded)
OPEN → HALF-OPEN (timeout elapsed)
HALF-OPEN → CLOSED (test call succeeds)
HALF-OPEN → OPEN (test call fails)
```

**Configuration:**
```rust
CircuitBreakerConfig {
  failure_rate_threshold: 0.5,  // 50%
  consecutive_failures_threshold: 5,
  window_size: 100,  // Last 100 requests
  timeout_duration: Duration::from_secs(30),
  timeout_multiplier: 2.0,  // Double on reopen
  max_timeout: Duration::from_secs(300),
}
```

**Benefits:**
- Prevent cascading failures
- Fast failure (no waiting for timeouts)
- Automatic recovery
- Resource conservation
- Clear observability

**Validates Requirements:** 4.7

**Key Components:**
- Worker Process
- Circuit Breaker
- Failure Tracker
- External API
- Metrics & Alerts

**Các bước chính:**
1. **Job Definition Creation**:
   - Admin submits JSON job definition with multiple steps
   - API validates JSON schema
   - API stores definition in MinIO at `jobs/{job_id}/definition.json`
   - API stores only metadata and MinIO path in PostgreSQL
2. **Job Scheduling**:
   - Scheduler detects job is due
   - Scheduler creates execution record
   - Scheduler publishes to NATS queue
3. **Multi-Step Execution**:
   - Worker loads job definition from MinIO
   - Worker initializes empty Job Context
   - **Step 1 (HTTP Request)**:
     - Execute HTTP GET to fetch users
     - Store API response in Job Context
     - Persist Job Context to MinIO
   - **Step 2 (Database Query)**:
     - Load Job Context from MinIO
     - Resolve reference `{{steps.step1.response.data}}`
     - Execute INSERT query with data from step1
     - Store query result in Job Context
     - Persist updated Job Context to MinIO
   - **Step 3 (HTTP Request)**:
     - Load Job Context from MinIO
     - Resolve references `{{steps.step1.response.total}}` and `{{steps.step2.rows_affected}}`
     - Execute HTTP POST with data from previous steps
     - Store response in Job Context
     - Persist final Job Context to MinIO
4. **Finalize Execution**:
   - Update job_executions with MinIO context path
   - ACK NATS message
   - Emit metrics and logs

**Job Definition JSON Example:**
```json
{
  "name": "user-data-sync",
  "schedule": {
    "type": "Cron",
    "expression": "0 0 2 * * *",
    "timezone": "Asia/Ho_Chi_Minh"
  },
  "steps": [
    {
      "id": "step1",
      "name": "fetch-users",
      "type": "HttpRequest",
      "config": {
        "method": "GET",
        "url": "https://api.example.com/users?status=active"
      }
    },
    {
      "id": "step2",
      "name": "process-users",
      "type": "DatabaseQuery",
      "config": {
        "query": "INSERT INTO users VALUES {{steps.step1.response.data}}"
      }
    },
    {
      "id": "step3",
      "name": "send-notification",
      "type": "HttpRequest",
      "config": {
        "method": "POST",
        "url": "https://notification.example.com/send",
        "body": {
          "message": "Synced {{steps.step1.response.total}} users",
          "inserted": "{{steps.step2.rows_affected}}"
        }
      }
    }
  ]
}
```

**Job Context Structure:**
```json
{
  "job_id": "550e8400-...",
  "execution_id": "exec-uuid",
  "started_at": "2025-01-20T02:00:00Z",
  "completed_at": "2025-01-20T02:00:05Z",
  "status": "Success",
  "steps": {
    "step1": {
      "status": "Success",
      "response": {
        "status_code": 200,
        "data": [
          {"id": 1, "name": "John", "email": "john@example.com"},
          {"id": 2, "name": "Jane", "email": "jane@example.com"}
        ],
        "total": 2
      }
    },
    "step2": {
      "status": "Success",
      "result": {
        "rows_affected": 2,
        "execution_time": 0.15
      }
    },
    "step3": {
      "status": "Success",
      "response": {
        "status_code": 200,
        "data": {"notification_id": "notif-123", "sent": true}
      }
    }
  }
}
```

**Data Storage Strategy:**
- **PostgreSQL**: Job metadata, execution status, MinIO path references only
- **MinIO**: Full job definitions, Job Context objects, large payloads
- **Benefits**: Database stays small, unlimited storage, S3-compatible, cost-effective

**Step Reference Syntax:**
- Simple field: `{{steps.step1.response.total}}` → `2`
- Nested object: `{{steps.step1.response.data[0].name}}` → `"John"`
- Array: `{{steps.step1.response.data}}` → Full array
- Multiple refs: `"Synced {{steps.step1.response.total}} users"`

**Failure Handling:**
- If step fails: Store error in Job Context
- Persist partial Job Context to MinIO
- Update execution status to 'Failed'
- On retry: Load Job Context and resume or restart

**Validates Requirements:** 13.1-13.12, 14.1-14.7

**Key Components:**
- API Server
- MinIO (Object Storage)
- PostgreSQL (System Database)
- Worker Process
- Job Context Manager
- Step Executor

---

## Cách xem Sequence Diagrams

### Option 1: PlantUML Online Server
1. Truy cập: https://www.plantuml.com/plantuml/uml/
2. Copy nội dung file `.puml` vào editor
3. Xem kết quả render

### Option 2: VS Code Extension
1. Cài đặt extension "PlantUML" trong VS Code
2. Mở file `.puml`
3. Press `Alt+D` để preview

### Option 3: Command Line
```bash
# Cài đặt PlantUML
brew install plantuml  # macOS
apt-get install plantuml  # Ubuntu

# Generate PNG
plantuml sequence-01-job-scheduling.puml

# Generate SVG
plantuml -tsvg sequence-01-job-scheduling.puml

# Generate all diagrams
plantuml sequence-*.puml
```

### Option 4: IntelliJ IDEA
1. Cài đặt plugin "PlantUML integration"
2. Mở file `.puml`
3. View diagram trong editor

## Ký hiệu trong Sequence Diagrams

### Participants
- `actor`: Human user hoặc external system
- `participant`: Internal component/service
- `database`: Database system
- `queue`: Message queue

### Arrows
- `->`: Synchronous call
- `-->`: Return/response
- `->>`: Asynchronous call
- `-->>`: Asynchronous return

### Activation
- `activate`: Component starts processing
- `deactivate`: Component finishes processing

### Control Flow
- `alt/else/end`: Alternative paths (if/else)
- `loop/end`: Loops
- `opt/end`: Optional flow
- `par/end`: Parallel execution

### Notes
- `note over`: Note spanning multiple participants
- `note left/right of`: Note next to participant

## Mapping với Requirements và Design

Mỗi sequence diagram validate các requirements cụ thể:

| Diagram | Requirements | Design Properties |
|---------|-------------|-------------------|
| 01-job-scheduling | 1.1-1.7, 4.1-4.4, 7.1 | 1-7, 29-32, 55-56 |
| 02-http-job-execution | 2.3-2.12, 3.1-3.6, 4.2-4.10, 5.1-5.9 | 8-19, 20-25, 30-38, 39-47 |
| 03-database-job-execution | 2.11-2.12, 3.7-3.10, 4.2-4.10 | 18-19, 26, 30-38 |
| 04-authentication-keycloak | 10.1, 10.4-10.12 | 63, 65-73 |
| 05-authentication-database | 10.2-10.3, 10.13 | 64-74 |
| 06-dashboard-realtime-updates | 6.7, 6.8 | 48-54 |
| 07-graceful-shutdown | 7.6-7.7 | 60-61 |
| 08-circuit-breaker | 4.7 | 35, 37 |
| 09-multi-step-job-execution | 13.1-13.12, 14.1-14.7 | TBD (new properties) |
| 10-file-processing-job | 15.1-15.12 | TBD (new properties) |
| 11-webhook-trigger | 16.1-16.12, 17.5 | TBD (new properties) |
| 12-job-import-export | 18.1-18.14 | TBD (new properties) |

## Key Patterns và Best Practices

### 1. Exactly-Once Execution
Sử dụng 4 layers of protection:
- Distributed lock (Redis RedLock)
- Idempotency key
- Database unique constraint
- Message acknowledgment

### 2. Variable Resolution
- Load global và job-specific variables
- Apply precedence rules (job-specific > global)
- Decrypt sensitive variables
- Use parameterized queries (SQL injection prevention)

### 3. Retry Logic
- Exponential backoff with jitter
- Max 10 retries
- Dead Letter Queue after exhaustion
- Circuit breaker integration

### 4. Circuit Breaker
- Prevent cascading failures
- Fail fast when system down
- Automatic recovery testing
- Per-target configuration

### 5. Graceful Shutdown
- Complete in-flight work
- Release all locks
- Close connections cleanly
- Zero-downtime deployment

### 6. Real-time Updates
- Server-Sent Events (SSE)
- HTMX for dynamic UI
- Event replay capability
- Automatic reconnection

### 7. Authentication & Authorization
- Two modes: Keycloak and Database
- JWT token validation
- RBAC permission checking
- Audit logging

### 8. Multi-Step Jobs with MinIO
- Job definitions stored as JSON in MinIO
- Sequential step execution
- Job Context for data passing between steps
- Step reference syntax (JSONPath-style)
- Persistent storage of intermediate results
- Failure recovery with context preservation

### 9. Job Import/Export
- Visual job builder UI for creating jobs
- Export jobs as JSON files (with sensitive data redaction)
- Import jobs from JSON files
- Bulk export/import (ZIP archives)
- JSON schema validation
- Version control integration (Git-friendly)

## Tài liệu tham khảo

- **Requirements Document**: `requirements.md`
- **Design Document**: `design.md`
- **Use Case Diagrams**: `use-cases-*.puml`
- **PlantUML Sequence Diagram**: https://plantuml.com/sequence-diagram
- **Redis RedLock**: https://redis.io/docs/manual/patterns/distributed-locks/
- **NATS JetStream**: https://docs.nats.io/nats-concepts/jetstream
- **Circuit Breaker Pattern**: https://martinfowler.com/bliki/CircuitBreaker.html
- **Server-Sent Events**: https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events
- **MinIO**: https://min.io/docs/minio/linux/index.html
- **JSONPath**: https://goessner.net/articles/JsonPath/

---

**Last Updated**: 2025-01-20
**Version**: 1.0
**Author**: Vietnam Enterprise Cron Team
