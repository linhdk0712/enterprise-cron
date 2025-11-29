# DEVELOPMENT RULES
# Vietnam Enterprise Cron System - Comprehensive Development Standards

> **Purpose**: This document consolidates all development rules from `.kiro/` folder to ensure clean code, security, business requirements adherence, standard structure, and architecture.
>
> **Audience**: All developers, AI assistants (Claude, Cursor, Copilot, Cody), and code reviewers
>
> **Status**: MANDATORY - 100% Compliance Required

---

## Table of Contents

1. [Clean Code Standards](#1-clean-code-standards)
2. [Security Requirements](#2-security-requirements)
3. [Business Requirements Compliance](#3-business-requirements-compliance)
4. [Standard Project Structure](#4-standard-project-structure)
5. [Architecture Standards](#5-architecture-standards)
6. [Pre-Implementation Checklist](#6-pre-implementation-checklist)
7. [Technology Stack](#7-technology-stack)
8. [Testing Standards](#8-testing-standards)
9. [Documentation Requirements](#9-documentation-requirements)
10. [Deployment Standards](#10-deployment-standards)

---

## 1. Clean Code Standards

### 1.1 RECC 2025 - Rust Enterprise Code of Conduct (100% Mandatory)

#### Core Principles (Non-Negotiable)

1. **NO `unwrap()` or `expect()` in production code**
   - Only allowed in `main()` when crash is acceptable or in tests
   - Always use `?` operator with custom errors
   ```rust
   // ❌ WRONG
   let config = load_config().unwrap();

   // ✅ CORRECT
   let config = load_config()?;
   ```

2. **All async functions MUST have tracing**
   ```rust
   #[tracing::instrument(skip(state, pool, redis))]
   async fn my_function(pool: &PgPool, redis: &RedisPool) -> Result<T, E>
   ```

3. **Error handling with `thiserror`**
   ```rust
   #[derive(thiserror::Error, Debug)]
   pub enum AppError {
       #[error("DB error: {0}")]
       Db(#[from] sqlx::Error),
       #[error("Redis error: {0}")]
       Redis(#[from] redis::RedisError),
   }
   ```

4. **NO `println!`** - Only use `tracing::info!`, `warn!`, `error!`, `debug!`

5. **Graceful shutdown required** on SIGTERM & Ctrl+C
   ```rust
   tokio::signal::ctrl_c().await.ok();
   // Complete in-flight work before exit
   ```

6. **Prefer channels over `Arc<Mutex<T>>`**
   - Use `tokio::sync::mpsc` or `broadcast` instead

7. **NO `tokio::spawn` if queue is available**
   - Use NATS/Redis Streams for task distribution

#### 20 One-Liner Rules (Must Memorize)

```rust
// 1. All public structs
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]

// 2. All handlers & async functions
#[tracing::instrument(skip(state, pool, redis))]

// 3. DB queries - prefer compile-time checking
sqlx::query_as!(Job, "SELECT * FROM jobs WHERE id = $1", id)

// 4. Redis distributed lock - RedLock 3+ nodes
let lock = redlock_rs::RedLock::new(clients);

// 5. Spawned tasks must handle errors
tokio::spawn(async move {
    if let Err(e) = process().await {
        tracing::error!(error = %e, "Task failed");
    }
});

// 6. No unnecessary String clones
fn log(name: &str)  // Use &str, not String

// 7. Health check returns 200 immediately
get(|| async { "OK" })

// 8. Metrics endpoint required
route("/metrics", get(prometheus_handler))

// 9. Timeout all external calls
reqwest::Client::builder().timeout(Duration::from_secs(30))

// 10. Retry must have jitter
backoff::ExponentialBackoffBuilder::new().with_jitter().build()

// 11. Idempotency key required
let execution_id = Uuid::new_v4();

// 12. No secrets in commits - only .env.example

// 13. Layered configuration required
Config::builder()
    .add_source(File::with_name("config/default"))
    .add_source(File::with_name("config/local").required(false))
    .add_source(Environment::with_prefix("APP"))
    .build()?

// 14. All errors derive thiserror
#[derive(thiserror::Error, Debug)]

// 15. Graceful shutdown template
tokio::signal::ctrl_c().await.ok();

// 16. Type-safe database URLs
const DATABASE_URL: &str = env!("DATABASE_URL");

// 17. Pattern matching over unwrap
match result {
    Ok(v) => v,
    Err(e) => return Err(e.into()),
}

// 18. Structured logging with context
tracing::info!(user_id = %user_id, "User logged in");

// 19. Async traits require async-trait
#[async_trait]
trait MyTrait { async fn method(&self); }

// 20. Use workspace dependencies for consistency
[workspace.dependencies]
tokio = { version = "1.35", features = ["full"] }
```

### 1.2 Code Organization

- **main.rs**: ≤ 100 lines, only wiring
- **Functions**: ≤ 50 lines (prefer smaller)
- **Files**: ≤ 500 lines (split if larger)
- **Modules**: Single responsibility principle

### 1.3 Naming Conventions

- **Modules**: `snake_case` (e.g., `circuit_breaker.rs`)
- **Structs/Enums**: `PascalCase` (e.g., `JobExecution`, `ExecutionStatus`)
- **Functions/Variables**: `snake_case` (e.g., `find_jobs_due`, `next_execution_time`)
- **Constants**: `SCREAMING_SNAKE_CASE` (e.g., `MAX_RETRIES`, `DEFAULT_TIMEOUT`)
- **Traits**: `PascalCase` with descriptive names (e.g., `JobExecutor`, `DistributedLock`)

---

## 2. Security Requirements

### 2.1 Authentication & Authorization

1. **JWT Token Security**
   - Use `jsonwebtoken` 9.3+
   - Token expiration: 24 hours (configurable)
   - Rotate secrets regularly
   ```rust
   let jwt_secret = &config.auth.jwt_secret;
   let jwt_service = JwtService::new(jwt_secret, jwt_expiry_hours);
   ```

2. **Password Security**
   - Use `bcrypt` with cost factor 12
   - Never store plaintext passwords
   - Validate password strength
   ```rust
   let hashed = bcrypt::hash(password, 12)?;
   ```

3. **RBAC Enforcement**
   - Permissions: `job:read`, `job:write`, `job:execute`, `job:delete`, `execution:read`
   - Check permissions on every protected endpoint
   - Audit all permission checks

4. **Webhook Security**
   - HMAC-SHA256 signature validation required
   - Rate limiting per webhook endpoint
   ```rust
   let signature = hmac_sha256(&secret_key, &payload);
   ```

### 2.2 Data Protection

1. **Encryption at Rest**
   - Encrypt sensitive variables in database
   - Use AES-256-GCM or similar
   - Mask sensitive data in logs and UI

2. **SQL Injection Prevention**
   - Use parameterized queries ONLY
   - Validate all user inputs
   ```rust
   // ✅ CORRECT
   sqlx::query_as!(Job, "SELECT * FROM jobs WHERE id = $1", id)

   // ❌ WRONG
   sqlx::query(&format!("SELECT * FROM jobs WHERE id = {}", id))
   ```

3. **No Secrets in Code**
   - Use environment variables
   - Use configuration files (git-ignored)
   - Use secret management systems (Vault, Kubernetes Secrets)

4. **XSS Prevention**
   - Sanitize all HTML output
   - Use Content-Security-Policy headers
   - Validate JSON inputs

### 2.3 Network Security

1. **TLS/SSL Required**
   - Use `rustls-tls` feature in reqwest
   - Verify certificates
   - Support TLS 1.2+ only

2. **Timeout All External Calls**
   ```rust
   reqwest::Client::builder()
       .timeout(Duration::from_secs(30))
       .build()?
   ```

3. **Rate Limiting**
   - Implement per-endpoint rate limits
   - Use Redis for distributed rate limiting
   - Return 429 Too Many Requests

### 2.4 Audit & Logging

1. **Audit All Security Events**
   - Login attempts (success/failure)
   - Permission denials
   - Configuration changes
   - Job executions

2. **Structured Logging**
   - JSON format with trace context
   - Include user_id, execution_id, job_id
   - Never log sensitive data (passwords, tokens, secrets)

---

## 3. Business Requirements Compliance

### 3.1 Exactly-Once Execution (Requirement 4)

1. **Distributed Locking**
   - Use Redis RedLock algorithm
   - 3+ Redis nodes in production
   - Lock TTL: 30 seconds (configurable)
   ```rust
   let lock = distributed_lock.acquire(resource, Duration::from_secs(30)).await?;
   ```

2. **Idempotency Keys**
   - Generate `execution_id` as UUID v4
   - Check before processing
   - Store in database with unique constraint

3. **NATS JetStream**
   - Exactly-once delivery guarantees
   - Acknowledge only after successful processing
   - Dead letter queue for failures

### 3.2 Schedule Types (Requirement 1)

1. **Cron Expression**
   - Quartz syntax with second precision
   - Timezone support (default: Asia/Ho_Chi_Minh)
   - End date support
   ```rust
   Schedule::Cron {
       expression: "0 0 * * * *".to_string(),
       timezone: Tz::Asia__Ho_Chi_Minh,
       end_date: Some(end_date),
   }
   ```

2. **Fixed Delay**
   - Next execution starts X seconds after previous completes
   ```rust
   Schedule::FixedDelay { delay_seconds: 300 }
   ```

3. **Fixed Rate**
   - Executions at fixed intervals regardless of duration
   ```rust
   Schedule::FixedRate { interval_seconds: 3600 }
   ```

4. **One-Time**
   - Execute once at specific datetime
   ```rust
   Schedule::OneTime { execute_at: datetime }
   ```

### 3.3 Job Types (Requirement 3)

1. **HTTP Request Jobs**
   - Methods: GET, POST, PUT
   - Auth: Basic, Bearer, OAuth2
   - Timeout: 30 seconds default

2. **Database Query Jobs**
   - PostgreSQL, MySQL, Oracle 19c
   - Parameterized queries only
   - Connection pooling

3. **File Processing Jobs**
   - Excel (XLSX): Read/Write with calamine/rust_xlsxwriter
   - CSV: Read/Write with csv crate
   - Streaming for large files (>100MB)

4. **SFTP Jobs**
   - Upload/Download with ssh2 crate
   - Password or SSH key authentication
   - Wildcard patterns support

### 3.4 Multi-Step Jobs (Requirement 13)

1. **Job Context**
   - Store in PostgreSQL: `job_executions.context` (JSONB column)
   - Cache in Redis with TTL (30 days)
   - Load before each step
   - Save after each step

2. **Step Output References**
   - Format: `{{steps.step_id.output.field}}`
   - JSONPath support: `{{steps.step1.output.rows[0].id}}`

3. **Variable Substitution** (Requirement 2)
   - Format: `${VAR_NAME}`
   - Global and job-specific variables
   - Encryption for sensitive values

### 3.5 Observability (Requirement 5)

1. **Structured Logging**
   - JSON format with trace context
   - Log all job starts/completions
   - Include job_id, execution_id, user_id

2. **Prometheus Metrics**
   - `job_success_total` counter
   - `job_failed_total` counter
   - `job_duration_seconds` histogram
   - `job_queue_size` gauge

3. **OpenTelemetry Tracing**
   - Span per job execution
   - Distributed tracing across components

4. **Alerting**
   - Alert after 3 consecutive failures
   - Configurable alert thresholds

---

## 4. Standard Project Structure

### 4.1 Workspace Organization

```
rust-enterprise-cron/
├── Cargo.toml              # Workspace manifest
├── common/                 # Shared library
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── auth.rs         # JWT, bcrypt, Keycloak
│       ├── circuit_breaker.rs
│       ├── config.rs       # Layered configuration
│       ├── db/             # Database layer
│       │   ├── mod.rs
│       │   ├── pool.rs
│       │   ├── redis.rs
│       │   └── repositories/
│       │       ├── job.rs
│       │       ├── execution.rs
│       │       ├── variable.rs
│       │       └── user.rs
│       ├── dlq.rs          # Dead Letter Queue
│       ├── errors.rs       # thiserror errors
│       ├── executor/       # Job executors
│       │   ├── mod.rs
│       │   ├── http.rs
│       │   ├── database.rs
│       │   ├── file.rs
│       │   └── sftp.rs
│       ├── import_export.rs
│       ├── lock.rs         # Distributed locking
│       ├── middleware/
│       ├── models.rs       # Domain models
│       ├── queue/          # NATS integration
│       │   ├── mod.rs
│       │   ├── nats.rs
│       │   ├── consumer.rs
│       │   └── publisher.rs
│       ├── rate_limit.rs
│       ├── retry.rs        # Exponential backoff
│       ├── schedule.rs     # Schedule calculations
│       ├── scheduler/      # Scheduler engine
│       ├── storage/        # Storage service (PostgreSQL + Redis + Filesystem)
│       │   ├── mod.rs
│       │   ├── postgres_storage.rs
│       │   └── redis_client.rs
│       ├── substitution.rs # Variable substitution
│       ├── telemetry.rs    # Logging, metrics, tracing
│       ├── webhook.rs      # Webhook validation
│       └── worker/         # Worker logic
│           ├── mod.rs
│           ├── consumer.rs
│           ├── context.rs
│           └── reference.rs
├── scheduler/              # Scheduler binary
│   ├── Cargo.toml
│   └── src/
│       └── main.rs         # ≤ 100 lines
├── worker/                 # Worker binary
│   ├── Cargo.toml
│   └── src/
│       └── main.rs         # ≤ 100 lines
├── api/                    # API binary
│   ├── Cargo.toml
│   ├── templates/          # HTMX templates
│   │   ├── layout.html
│   │   ├── dashboard.html
│   │   ├── jobs.html
│   │   ├── executions.html
│   │   ├── variables.html
│   │   ├── job_details.html
│   │   └── job_form.html
│   └── src/
│       ├── main.rs         # ≤ 100 lines
│       ├── routes.rs
│       ├── state.rs
│       ├── handlers/
│       │   ├── mod.rs
│       │   ├── index.rs
│       │   ├── health.rs
│       │   ├── auth.rs
│       │   ├── jobs.rs
│       │   ├── executions.rs
│       │   ├── variables.rs
│       │   ├── webhooks.rs
│       │   ├── import_export.rs
│       │   ├── dashboard.rs
│       │   ├── sse.rs
│       │   └── metrics.rs
│       └── middleware/
│           ├── mod.rs
│           ├── auth.rs
│           ├── rbac.rs
│           └── rate_limit.rs
├── integration-tests/      # Integration tests
│   ├── Cargo.toml
│   └── tests/
├── migrations/             # Database migrations
│   ├── 20240101_initial_schema.sql
│   └── ...
├── config/                 # Configuration files
│   ├── default.toml
│   └── prometheus.yml
├── examples/               # Example job definitions
│   ├── http-job.json
│   ├── database-job.json
│   └── ...
├── charts/                 # Helm charts
│   └── vietnam-enterprise-cron/
├── scripts/                # Utility scripts
├── .sqlx/                  # SQLx compile-time cache
├── docker-compose.yml
├── Dockerfile
├── README.md
├── DEPLOYMENT.md
├── LOCAL-DEVELOPMENT.md
├── CLAUDE.md
└── DEVELOPMENT-RULES.md    # This file
```

### 4.2 Module Responsibilities

- **common/**: Shared library for all binaries
- **scheduler/**: Job trigger detection and queue publishing
- **worker/**: Job consumption and execution
- **api/**: REST API, dashboard, webhooks
- **integration-tests/**: End-to-end tests with testcontainers
- **migrations/**: Database schema evolution

---

## 5. Architecture Standards

### 5.1 Design Principles

1. **Distributed-First**
   - All components designed for horizontal scalability
   - No single point of failure
   - Stateless services

2. **Exactly-Once Semantics**
   - Distributed locks (Redis RedLock)
   - Idempotency keys
   - NATS JetStream acknowledgments

3. **Observable by Default**
   - Structured logging
   - Prometheus metrics
   - OpenTelemetry tracing

4. **Type-Safe**
   - Rust type system
   - SQLx compile-time checking
   - No `unwrap()` in production

5. **Separation of Concerns**
   - Clear boundaries between layers
   - Trait-based abstractions
   - Dependency injection

### 5.2 Component Architecture

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
        └───────┬────────┘       └───────┬────────┘
                │                         │
        ┌───────▼────────┐       ┌───────▼────────┐
        │  Scheduler 1-N │       │   Worker 1-N   │
        │  (Dist Lock)   │       │  (Multi-Step)  │
        └───────┬────────┘       └───────┬────────┘
                │                         │
        ┌───────┴─────────────────────────┴────────┐
        │                                           │
    ┌───▼───┐  ┌─────┐  ┌──────┐  ┌──────────┐
    │ PgSQL │  │Redis│  │ NATS │  │Filesystem│
    └───────┘  └─────┘  └──────┘  └──────────┘
```

### 5.3 Data Flow Patterns

1. **Job Scheduling Flow**
   - Scheduler polls DB → finds due jobs
   - Acquires Redis lock
   - Loads job definition from PostgreSQL (cached in Redis)
   - Publishes to NATS queue
   - Releases lock

2. **Job Execution Flow**
   - Worker consumes from NATS
   - Checks idempotency key
   - Loads job definition from PostgreSQL (cached in Redis)
   - For each step:
     - Load Job Context from PostgreSQL (cached in Redis)
     - Resolve variables/references
     - Execute step
     - Save step output to Job Context
     - Persist Job Context to PostgreSQL (update Redis cache)
   - Record execution result in DB
   - Acknowledge NATS message

3. **Webhook Trigger Flow**
   - External system POSTs to webhook URL
   - API validates HMAC signature
   - API checks rate limits in Redis
   - API stores webhook data in Job Context
   - API queues job execution
   - API returns 202 Accepted

### 5.4 Storage Strategy

- **PostgreSQL**: Job metadata, execution history, users, variables
- **Redis**: Distributed locks, rate limiting, cache (job definitions/context)
- **NATS JetStream**: Job queue with persistence
- **PostgreSQL**: Job definitions (JSONB), execution context (JSONB), metadata
  - `jobs.definition` - Job definition JSON
  - `job_executions.context` - Execution context JSON
- **Filesystem**: Uploaded/processed files
  - `./data/files/jobs/{job_id}/executions/{execution_id}/files/`
  - `jobs/{job_id}/executions/{execution_id}/output/*.xlsx`
  - `jobs/{job_id}/executions/{execution_id}/sftp/*.csv`

---

## 6. Pre-Implementation Checklist

### 6.1 MANDATORY: Read Before Coding

Before implementing ANY feature or task:

1. **✅ Read Requirements Document**
   - File: `.kiro/specs/vietnam-enterprise-cron/requirements.md`
   - Understand user story and business purpose
   - Know ALL acceptance criteria
   - Identify dependencies

2. **✅ Read Design Document**
   - File: `.kiro/specs/vietnam-enterprise-cron/design.md`
   - Understand architecture and data flow
   - Review interfaces to implement
   - Check data models and database schema
   - Read correctness properties
   - Review error handling strategy

3. **✅ View Sequence Diagrams**
   - Directory: `.kiro/specs/vietnam-enterprise-cron/`
   - Read: `SEQUENCE-DIAGRAMS-README.md`
   - View relevant diagrams:
     - `sequence-01-job-scheduling.puml`
     - `sequence-02-job-execution.puml`
     - `sequence-09-multi-step-job-execution.puml`
     - etc.

4. **✅ Review Steering Rules**
   - Files in `.kiro/steering/`:
     - `tech.md` - Technology stack
     - `structure.md` - Project organization
     - `product.md` - Product overview
     - `implments-rules.md` - RECC 2025 (100% mandatory)

5. **✅ Check Task Dependencies**
   - File: `.kiro/specs/vietnam-enterprise-cron/tasks.md`
   - Verify parent tasks completed
   - Verify prerequisite tasks completed

### 6.2 Self-Check Questions

Answer "YES" to ALL before coding:

1. ✅ I have read and understood relevant requirements?
2. ✅ I have read relevant design document sections?
3. ✅ I have viewed relevant sequence diagrams?
4. ✅ I have reviewed steering rules?
5. ✅ I know which requirement this task implements?
6. ✅ I know which correctness properties to satisfy?
7. ✅ I know which interfaces to implement?
8. ✅ I know which data models to use?
9. ✅ I know which dependencies to import?
10. ✅ I know which module to place code in?

**If ANY answer is "NO" → STOP and read documentation first!**

### 6.3 Implementation Process

1. **Pre-Implementation Review** (5-30 minutes)
   - Read requirements (5-10 min)
   - Read design sections (10-15 min)
   - View sequence diagrams (5 min)
   - Review steering rules (5 min)

2. **Implementation** (Coding time)
   - Follow 100% RECC 2025 rules
   - Follow design patterns
   - Implement defined interfaces
   - Use correct technology stack
   - Organize per structure.md

3. **Verification** (Post-coding)
   - Check RECC 2025 compliance
   - Verify design match
   - Ensure acceptance criteria met
   - Run tests
   - Update task status

---

## 7. Technology Stack

### 7.1 Core Dependencies

**Language & Runtime:**
- Rust 1.84+ (2021 Edition)
- Tokio 1.35+ (async runtime)

**Web & API:**
- Axum 0.7 (web framework)
- Tower 0.4 / Tower-HTTP 0.5 (middleware)
- Tera 1.19+ (templates)
- Reqwest 0.12 (HTTP client, `rustls-tls`)

**Data Storage:**
- PostgreSQL 14+ (system database - job definitions, execution context, metadata)
- Redis 7.0+ (distributed locking, rate limiting, cache for job definitions/context)
- NATS 2.10+ (job queue with JetStream)
- Filesystem (file storage for uploaded/processed files)

**Database Drivers:**
- sqlx 0.8 (PostgreSQL, compile-time checking)
- mysql_async 0.34 (MySQL support)
- oracle 0.6 (Oracle 19c support)

**File Processing:**
- calamine 0.24 (Excel read)
- rust_xlsxwriter 0.65 (Excel write)
- csv 1.3 (CSV processing)

**Security:**
- jsonwebtoken 9.3 (JWT)
- bcrypt 0.15 (password hashing)
- hmac 0.12 + sha2 0.10 (HMAC-SHA256)

**Observability:**
- tracing 0.1 + tracing-subscriber 0.3
- tracing-opentelemetry 0.23
- metrics 0.22 + metrics-exporter-prometheus 0.15

**Testing:**
- proptest 1.4 (property-based, 100+ iterations)
- mockall 0.12 (mocking)
- testcontainers 0.17 (integration tests)
- criterion 0.5 (benchmarks)

### 7.2 Version Policy

- **Quarterly Updates**: Review dependencies every 3 months
- **Security Updates**: Apply immediately
- **Major Versions**: Careful evaluation, full regression testing
- **MSRV**: 1.84.0 (review every 6 months)

---

## 8. Testing Standards

### 8.1 Test Coverage

- **Unit Tests**: Inline `#[cfg(test)]` modules
- **Property Tests**: Minimum 100 iterations per property
- **Integration Tests**: `tests/*_integration.rs` with testcontainers
- **Benchmarks**: `benches/` with criterion

### 8.2 Test Tagging

```rust
// Feature: vietnam-enterprise-cron, Property 1: Cron expressions parse correctly
#[test]
fn property_cron_parse() {
    // Property-based test with 100+ iterations
}
```

### 8.3 Test Organization

```bash
# Unit tests
cargo test --lib

# Property-based tests
cargo test property_

# Integration tests (requires Docker)
cargo test --test '*_integration'

# Benchmarks
cargo bench
```

### 8.4 No Unwrap in Tests

```rust
// ❌ WRONG
#[test]
fn test_something() {
    let result = function().unwrap();
    assert_eq!(result, expected);
}

// ✅ CORRECT
#[test]
fn test_something() -> Result<()> {
    let result = function()?;
    assert_eq!(result, expected);
    Ok(())
}
```

---

## 9. Documentation Requirements

### 9.1 Code Documentation

- All public APIs must have doc comments
- Include examples in doc comments
- Document panics (though should avoid in production)
- Document safety (for unsafe code - avoid if possible)

```rust
/// Executes a job step and returns the output.
///
/// # Arguments
///
/// * `step` - The job step to execute
/// * `context` - The current job execution context
///
/// # Errors
///
/// Returns `ExecutionError` if step execution fails
///
/// # Examples
///
/// ```
/// let executor = HttpExecutor::new();
/// let output = executor.execute(&step, &mut context).await?;
/// ```
#[async_trait]
pub trait JobExecutor {
    async fn execute(&self, step: &JobStep, context: &mut JobContext)
        -> Result<StepOutput, ExecutionError>;
}
```

### 9.2 Required Documentation Files

- `README.md` - Project overview, features, quick start
- `DEPLOYMENT.md` - Production deployment guide
- `LOCAL-DEVELOPMENT.md` - Local setup and development
- `CLAUDE.md` - AI assistant guidance (already created)
- `DEVELOPMENT-RULES.md` - This file

---

## 10. Deployment Standards

### 10.1 Docker

**Multi-stage Dockerfile:**
- Build stage: Rust nightly-bookworm
- Runtime stage: debian:trixie-slim
- Target size: < 50MB
- Non-root user: cronuser (UID 1000)

**docker-compose.yml:**
- PostgreSQL, Redis, NATS
- Scheduler (3 replicas recommended)
- Worker (2+ replicas with auto-scaling)
- Filesystem volume mounted at `./data/files`
- API (2+ replicas behind load balancer)

### 10.2 Kubernetes (Helm)

**Production Requirements:**
- 3+ Scheduler replicas with anti-affinity
- 2+ Worker replicas with HPA
- 2+ API replicas with HPA
- PostgreSQL with read replicas
- Redis cluster (6 nodes: 3 masters, 3 replicas)
- NATS cluster (3 nodes)

**Resources:**
- Scheduler: 0.1-0.5 CPU, 128-256MB RAM
- Worker: 0.2-1.0 CPU, 256-512MB RAM
- API: 0.2-1.0 CPU, 256-512MB RAM

### 10.3 Configuration

**Layered precedence:**
1. Default values (in binary)
2. Config file (`config/default.toml`)
3. Local config (`config/local.toml` - git-ignored)
4. Environment variables (prefix `APP__`)
5. Command-line arguments (highest priority)

**Example environment variables:**
```bash
APP__DATABASE__URL=postgresql://...
APP__REDIS__URL=redis://...
APP__NATS__URL=nats://...
APP__STORAGE__FILE_BASE_PATH=./data/files
APP__AUTH__JWT_SECRET=secret
```

---

## Summary Checklist for Developers

Before committing code, verify:

- [ ] No `unwrap()` or `expect()` in production code
- [ ] All async functions have `#[tracing::instrument]`
- [ ] All errors use `thiserror` or `anyhow`
- [ ] No `println!` (use `tracing::*!` instead)
- [ ] Graceful shutdown implemented
- [ ] All external calls have timeouts
- [ ] Retries use exponential backoff with jitter
- [ ] Idempotency keys used where required
- [ ] SQL queries use compile-time checking
- [ ] No secrets in code or commits
- [ ] Configuration is layered
- [ ] Structured logging with context
- [ ] Prometheus metrics exported
- [ ] Tests written and passing
- [ ] Documentation updated
- [ ] Code follows project structure
- [ ] Business requirements met
- [ ] Security requirements met
- [ ] Pre-implementation checklist completed

---

## For AI Assistants

When implementing a task, you MUST:

1. Use tools to read relevant documentation:
   - `.kiro/specs/vietnam-enterprise-cron/requirements.md`
   - `.kiro/specs/vietnam-enterprise-cron/design.md`
   - `.kiro/specs/vietnam-enterprise-cron/sequence-*.puml`
   - `.kiro/steering/*.md`

2. Analyze and understand requirements, design, and sequence diagrams

3. Only then begin writing code

4. In your response, state which documents you read and your understanding

5. Explain why your implementation matches requirements and design

**DO NOT skip documentation reading and go straight to code!**

---

## Rule Priority

1. **Security** - Highest priority, never compromise
2. **Business Requirements** - Must satisfy all acceptance criteria
3. **RECC 2025 Standards** - 100% compliance mandatory
4. **Architecture** - Follow established patterns
5. **Performance** - Optimize after correctness

---

**Remember**: 30 minutes reading documentation = 3 hours saved debugging

**Golden Rule**: Read first, code second, verify third
