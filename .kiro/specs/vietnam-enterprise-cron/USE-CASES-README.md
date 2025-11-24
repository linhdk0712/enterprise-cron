# Vietnam Enterprise Cron System - Use Case Diagrams

## Tổng quan (Overview)

Bộ tài liệu này chứa các sơ đồ use case chi tiết cho hệ thống Vietnam Enterprise Cron, được thiết kế theo định dạng PlantUML. Các sơ đồ mô tả tương tác giữa các actor (người dùng và hệ thống) với các chức năng của hệ thống.

This documentation contains detailed use case diagrams for the Vietnam Enterprise Cron System, designed in PlantUML format. The diagrams describe interactions between actors (users and systems) with system functionalities.

## Danh sách các sơ đồ (Diagram List)

### 1. `use-cases.puml` - Tổng quan hệ thống (System Overview)
**Mô tả:** Sơ đồ tổng quan hiển thị tất cả các use case chính và mối quan hệ giữa các actor với hệ thống.

**Nội dung chính:**
- 7 nhóm chức năng chính (packages)
- 40+ use cases
- 4 loại actor người dùng (System Admin, Platform Engineer, DevOps, Security Admin)
- 2 loại actor hệ thống (Scheduler, Worker)
- 4 hệ thống bên ngoài (Keycloak, Target DB, External API, Monitoring)

**Actors:**
- **System Administrator**: Quản lý jobs, variables, và thực thi thủ công
- **Platform Engineer**: Giám sát hệ thống và đảm bảo exactly-once execution
- **DevOps Engineer**: Theo dõi metrics, logs, traces và alerts
- **Security Administrator**: Quản lý authentication, authorization, users và roles
- **Scheduler Process**: Tự động phát hiện và lên lịch jobs
- **Worker Process**: Thực thi jobs từ queue

### 2. `use-cases-job-management.puml` - Quản lý Jobs (Job Management)
**Mô tả:** Chi tiết các use case liên quan đến quản lý job definitions.

**Chức năng chính:**
- **Create Job**: Tạo job mới với các loại schedule
  - Cron (Quartz syntax với second precision)
  - Fixed Delay (delay sau khi hoàn thành)
  - Fixed Rate (interval cố định)
  - One-Time (thực thi một lần)
- **Update Job**: Cập nhật cấu hình job (không cần restart)
- **Delete Job**: Xóa job (không cần restart)
- **Enable/Disable Job**: Bật/tắt scheduling
- **View Job List**: Xem danh sách jobs với statistics
- **Trigger Job Manually**: Kích hoạt job thủ công
- **Configure Schedule**: Cấu hình timezone (mặc định: Asia/Ho_Chi_Minh)

**Validates Requirements:** 1.1-1.7, 6.1-6.6, 7.3-7.4, 12.6

### 3. `use-cases-job-execution.puml` - Thực thi Jobs (Job Execution)
**Mô tả:** Chi tiết quy trình thực thi jobs với exactly-once guarantee.

**Chức năng chính:**
- **Schedule Job for Execution**: Scheduler phát hiện job đến hạn và publish vào queue
  - Acquire distributed lock (Redis RedLock)
  - Generate idempotency key
  - Publish to NATS JetStream
- **Consume Job from Queue**: Worker nhận và xử lý jobs
  - Check idempotency key
  - Prevent duplicate execution
- **Execute HTTP Request Job**: Thực thi HTTP requests
  - Support GET, POST, PUT methods
  - Basic, Bearer, OAuth2 authentication
  - Variable substitution in URL/headers/body
- **Execute Database Query Job**: Thực thi database queries
  - Support Oracle 19c, PostgreSQL, MySQL
  - Parameterized queries (prevent SQL injection)
  - Variable substitution in connection strings and queries
- **Handle Execution Failure**: Xử lý lỗi với retry logic
  - Exponential backoff with jitter (5s, 15s, 1m, 5m, 30m...)
  - Max 10 retries
  - Dead Letter Queue sau khi hết retries
- **Handle Timeout**: Terminate execution khi vượt quá timeout
- **Activate Circuit Breaker**: Fail fast khi external systems down

**Validates Requirements:** 3.1-3.12, 4.1-4.10

### 4. `use-cases-variable-management.puml` - Quản lý Variables (Variable Management)
**Mô tả:** Chi tiết quản lý variables với encryption và precedence rules.

**Chức năng chính:**
- **Create Global Variable**: Tạo variable có sẵn cho tất cả jobs
- **Create Job-Specific Variable**: Tạo variable chỉ cho một job cụ thể
- **Update Variable Value**: Cập nhật giá trị (áp dụng cho executions sau)
- **Mark Variable as Sensitive**: Đánh dấu và encrypt sensitive data
  - Encryption: AES-256-GCM
  - Masked in UI/API responses (show ***)
  - Decrypted only during execution
- **Resolve Variables in Job**: Worker resolve variables trước khi execute
  - Load global variables
  - Load job-specific variables
  - Apply precedence (job-specific > global)
  - Substitute placeholders {{variable_name}}
  - Validate all variables exist
- **Variable Substitution**:
  - HTTP URLs: `https://api.{{env}}.example.com`
  - HTTP Headers: `X-API-Key: {{api_key}}`
  - HTTP Body: `{"user": "{{username}}"}`
  - Connection Strings: `postgresql://{{db_user}}:{{db_pass}}@{{db_host}}/{{db_name}}`
  - SQL Queries: Parameterized queries (prevent SQL injection)

**Validates Requirements:** 2.1-2.12

### 5. `use-cases-authentication.puml` - Authentication & Authorization
**Mô tả:** Chi tiết hai chế độ authentication và RBAC authorization.

**Chức năng chính:**
- **Two Authentication Modes**:
  1. **Keycloak Mode** (External Identity Provider)
     - Users authenticate with Keycloak
     - Keycloak issues JWT tokens
     - System validates JWT signature
     - Public key caching for resilience
  2. **Database Mode** (Local User Management)
     - Users stored in system database
     - Passwords hashed with bcrypt
     - System generates JWT tokens
     - Roles and permissions in database
- **Validate JWT Token**: Verify signature, expiration, claims
- **Check RBAC Permissions**: Enforce role-based access control
  - `job:read` - View jobs
  - `job:write` - Create/update jobs
  - `job:execute` - Trigger jobs manually
  - `job:delete` - Delete jobs
  - `execution:read` - View execution history
- **Manage Users**: Create, update, disable users (database mode)
- **Manage Roles**: Create roles with permissions (database mode)
- **Audit Logging**: Log all operations with user identity

**Validates Requirements:** 10.1-10.13

### 6. `use-cases-monitoring.puml` - Monitoring & Observability
**Mô tả:** Chi tiết observability với logs, metrics, traces và alerts.

**Chức năng chính:**
- **View Execution History**: Query executions (last 30 days)
  - Filter by job ID, status
  - Pagination support
- **View Job Statistics**: Per-job statistics
  - Total/successful/failed executions
  - Success rate
  - Last execution times
  - Consecutive failures
  - Average duration
- **View Real-time Updates**: Server-Sent Events (SSE)
  - Push updates within 1 second
  - No polling required
- **Generate Structured Logs**: JSON format with trace context
  - Log execution start/completion
  - Include job_id, execution_id, trace_id, span_id
- **Export Prometheus Metrics**:
  - Counters: `job_success_total`, `job_failed_total`
  - Histograms: `job_duration_seconds`
  - Gauges: `job_queue_size`
- **Generate OpenTelemetry Traces**: Distributed tracing
  - Trace spans for entire execution
  - Parent-child span relationships
- **Trigger Alerts**: Alert on conditions
  - 3 consecutive failures
  - Queue depth threshold
  - Timeout exceeded
  - Circuit breaker open
- **Visualize in Grafana**: Dashboards for monitoring
- **Query Traces in Jaeger**: Trace analysis

**Validates Requirements:** 5.1-5.9, 6.1-6.3, 6.7

### 7. `use-cases-distributed-coordination.puml` - Distributed Coordination
**Mô tả:** Chi tiết cơ chế distributed coordination với exactly-once guarantee.

**Chức năng chính:**
- **Acquire Distributed Lock**: Redis RedLock algorithm
  - Requires 3+ Redis nodes
  - Acquire lock on majority of nodes
  - TTL: 30 seconds (configurable)
  - Prevents duplicate scheduling
- **Ensure Single Scheduler Execution**: Only 1 of N schedulers schedules each job
  - All schedulers poll database
  - Only one acquires lock
  - Winner publishes to queue
  - Losers skip this job
- **Publish Job to Queue**: NATS JetStream
  - Create JobExecution record
  - Generate idempotency key
  - Publish with acknowledgment
  - Release lock
- **Consume Job from Queue**: Workers consume with exactly-once
  - Subscribe to NATS stream
  - Check idempotency key
  - Process if not duplicate
  - Acknowledge on success
- **Ensure Exactly-Once Execution**: Multi-layer protection
  - Layer 1: Distributed lock (scheduler)
  - Layer 2: Idempotency key (worker)
  - Layer 3: Database unique constraint
  - Layer 4: NATS acknowledgment
- **Activate Circuit Breaker**: Prevent cascading failures
  - States: Closed → Open → Half-Open
  - Track failure rate
  - Fail fast when open
- **Graceful Shutdown**: Complete in-flight work
  - Scheduler: Complete lock acquisitions and publishes
  - Worker: Complete in-flight executions
  - Timeout: 30 seconds max

**Validates Requirements:** 4.1-4.10, 7.1, 7.6-7.7

### 8. `use-cases-actors.puml` - Actor Interactions (System Context)
**Mô tả:** Sơ đồ tổng quan về tương tác giữa actors và các components của hệ thống.

**Nội dung:**
- **Human Actors**: Admin, Engineer, DevOps, Security
- **System Actors**: Scheduler, Worker, API Server
- **External Systems**: Keycloak, Target DBs, External APIs, Monitoring
- **Infrastructure**: PostgreSQL, Redis Cluster, NATS JetStream
- **System Layers**:
  - API Layer (REST, Dashboard, SSE, JWT, RBAC)
  - Scheduler Layer (Detector, Lock Manager, Publisher)
  - Worker Layer (Consumer, Executors, Retry, Circuit Breaker)
  - Data Layer (Repositories)
  - Observability Layer (Logger, Metrics, Traces)

## Cách xem sơ đồ (How to View Diagrams)

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
plantuml use-cases.puml

# Generate SVG
plantuml -tsvg use-cases.puml
```

### Option 4: IntelliJ IDEA
1. Cài đặt plugin "PlantUML integration"
2. Mở file `.puml`
3. View diagram trong editor

## Ký hiệu trong sơ đồ (Diagram Notation)

### Relationships
- `-->` : Association (actor uses use case)
- `..>` : Dependency
- `<<include>>` : Include relationship (always executed)
- `<<extend>>` : Extend relationship (conditionally executed)
- `<<uses>>` : Uses external system

### Actors
- Human actors: Người dùng thực tế
- System actors: Processes/services
- External systems: Hệ thống bên ngoài

### Use Case Structure
```
usecase "Use Case Name" as UC1 {
  --
  **Primary Flow:**
  1. Step 1
  2. Step 2
  --
  **Validates:** Requirements X.Y
}
```

## Mapping với Requirements

Mỗi use case được tag với requirements mà nó validate:
- **Validates: Req 1.1** - Cron expression parsing
- **Validates: Req 4.1** - Distributed lock
- **Validates: Req 10.1** - Keycloak authentication

Xem file `requirements.md` để biết chi tiết về từng requirement.

## Mapping với Design Properties

Các use cases implement các correctness properties trong `design.md`:
- Property 1-7: Scheduling properties
- Property 8-19: Variable management properties
- Property 20-28: Job execution properties
- Property 29-38: Reliability properties
- Property 39-47: Observability properties
- Property 48-54: Dashboard and API properties
- Property 55-61: High availability properties
- Property 62: Error handling properties
- Property 63-74: Authentication and authorization properties
- Property 75: Component initialization properties

## Tài liệu tham khảo (References)

- **Requirements Document**: `requirements.md`
- **Design Document**: `design.md`
- **PlantUML Documentation**: https://plantuml.com/use-case-diagram
- **Redis RedLock**: https://redis.io/docs/manual/patterns/distributed-locks/
- **NATS JetStream**: https://docs.nats.io/nats-concepts/jetstream

## Ghi chú (Notes)

### Exactly-Once Execution
Hệ thống đảm bảo exactly-once execution thông qua 4 layers:
1. Distributed lock (Redis RedLock) - Prevents duplicate scheduling
2. Idempotency key - Prevents duplicate execution
3. Database unique constraint - Prevents duplicate records
4. NATS acknowledgment - Prevents message redelivery

### Security
- Passwords: bcrypt hashing
- Sensitive variables: AES-256-GCM encryption
- JWT tokens: RS256 signature
- SQL injection: Parameterized queries
- Audit logging: All operations logged with user identity

### High Availability
- Scheduler: Multiple nodes with distributed locking
- Worker: Horizontal scaling based on queue depth
- Redis: Cluster mode (3+ nodes)
- NATS: Cluster mode (3 nodes)
- PostgreSQL: Primary + read replicas

### Observability
- Logs: Structured JSON with trace context
- Metrics: Prometheus (counters, histograms, gauges)
- Traces: OpenTelemetry (distributed tracing)
- Alerts: Triggered on failure conditions
- Dashboards: Grafana visualizations

---

**Last Updated**: 2025-01-20
**Version**: 1.0
**Author**: Vietnam Enterprise Cron Team
