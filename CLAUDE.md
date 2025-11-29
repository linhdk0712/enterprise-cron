# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Vietnam Enterprise Cron System - A distributed job scheduling platform built in Rust to replace Java Quartz + Spring Batch in Vietnamese enterprises (banking, telecom, e-commerce). This is a production-ready microservices system with high reliability requirements.

## Build & Development Commands

### Building

```bash
# Build all binaries in release mode
cargo build --release

# Build specific binary
cargo build --release --bin api
cargo build --release --bin scheduler
cargo build --release --bin worker

# Debug build (faster compilation)
cargo build --bin api
```

### Running Tests

```bash
# All tests
cargo test --workspace

# Unit tests only
cargo test --lib

# Property-based tests (100+ iterations per test)
cargo test property_

# Integration tests
cargo test --test '*_integration'

# Specific test
cargo test test_name

# Run tests with logging
RUST_LOG=debug cargo test test_name -- --nocapture
```

### Local Development

```bash
# Quick start - automated (recommended)
./start-services-local.sh    # Start infrastructure + apps
./check-services.sh          # Verify all services running
./stop-services.sh           # Stop application services

# Manual start infrastructure only
docker-compose up -d postgres redis nats

# Create file storage directory
mkdir -p ./data/files
chmod 755 ./data/files

# Run individual services (in separate terminals)
RUST_LOG=info cargo run --bin scheduler
RUST_LOG=info cargo run --bin worker
RUST_LOG=info cargo run --bin api
```

### Database Migrations

```bash
# Install sqlx-cli if not present
cargo install sqlx-cli --no-default-features --features postgres

# Set database URL
export DATABASE_URL="postgresql://cronuser:cronpass@localhost:5432/vietnam_cron"

# Run migrations
sqlx migrate run

# Create new migration
sqlx migrate add migration_name

# Revert last migration
sqlx migrate revert
```

### Access Points (Local Development)

- Dashboard: http://localhost:8080 (admin/admin123)
- API: http://localhost:8080/api
- Health: http://localhost:8080/health
- Metrics: http://localhost:9090/metrics
- NATS Monitor: http://localhost:8222

### Deployment

```bash
# Docker build
docker build -t vietnam-cron:latest .

# Docker Compose
docker-compose up -d
docker-compose --profile monitoring up -d  # With Prometheus/Grafana

# Kubernetes/Helm
helm install my-cron ./charts/vietnam-enterprise-cron \
  --namespace cron-system --create-namespace
```

## Architecture & Design Patterns

### Workspace Structure

This is a Cargo workspace with three main binaries and one shared library:

- **common/** - Shared library with all business logic
- **api/** - REST API server (Axum) + HTMX dashboard + webhooks
- **scheduler/** - Detects jobs due for execution, publishes to queue
- **worker/** - Consumes jobs from queue, executes multi-step workflows
- **integration-tests/** - End-to-end tests with testcontainers

### Data Flow

```
Scheduler → polls PostgreSQL for due jobs
         → acquires Redis RedLock
         → publishes message to NATS JetStream queue
         → releases lock

Worker → consumes message from NATS queue
       → loads job definition from PostgreSQL (with Redis cache)
       → executes steps sequentially (HTTP/DB/File/SFTP)
       → stores Job Context in PostgreSQL (with Redis cache) between steps
       → updates execution status in PostgreSQL
       → on failure: retries with exponential backoff → Dead Letter Queue

API → serves HTMX dashboard
    → handles webhook triggers (HMAC-SHA256 validation)
    → provides REST endpoints for job CRUD
    → streams real-time updates via Server-Sent Events
```

### Key Patterns & Infrastructure

**Repository Pattern**: All database access through `*Repository` traits in `common/src/db/`:
- `JobRepository` - Job definitions
- `ExecutionRepository` - Execution history
- `VariableRepository` - Global/job variables
- `UserRepository` - Authentication (database mode)
- `WebhookRepository` - Webhook configs

**Strategy Pattern**: Job executors implement `JobExecutor` trait:
- `HttpExecutor` - HTTP requests with OAuth2/Basic/Bearer auth
- `DatabaseExecutor` - PostgreSQL/MySQL/Oracle 19c queries
- `FileExecutor` - Excel/CSV read/write with transformations
- `SftpExecutor` - SFTP upload/download

**Multi-Step Execution**: Jobs defined as JSON with sequential steps:
- Job Context stored in PostgreSQL (`job_executions.context` JSONB column)
- Redis cache for fast access (TTL: 30 days)
- Step output references: `{{steps.step1.output.field}}`
- Variable substitution: `${VAR_NAME}`
- Webhook data: `{{webhook.payload.field}}`

**Exactly-Once Execution**:
- Redis RedLock for distributed locking (3+ replicas in production)
- Idempotency keys (execution_id) for all operations
- NATS JetStream with acknowledgments

**Reliability Features**:
- Circuit Breaker pattern for external systems (`common/src/circuit_breaker.rs`)
- Exponential backoff with jitter (`common/src/retry.rs`)
- Dead Letter Queue for failed jobs (`common/src/dlq.rs`)
- Graceful shutdown on SIGTERM/Ctrl+C

### Storage Layer

- **PostgreSQL** - Job metadata, execution history, users, variables, job definitions, execution context
  - `jobs.definition` (JSONB) - Job definitions
  - `job_executions.context` (JSONB) - Execution context
  - `job_executions.trigger_metadata` (JSONB) - Trigger metadata
- **Redis** - Distributed locks (RedLock), rate limiting, cache for definitions/context
  - `storage:job_def:{job_id}` - Cached job definitions (TTL: 7 days)
  - `storage:job_ctx:{job_id}:{execution_id}` - Cached execution context (TTL: 30 days)
- **NATS JetStream** - Job queue with exactly-once delivery
- **Filesystem** - Uploaded/processed files
  - `./data/files/jobs/{job_id}/executions/{execution_id}/output/*.xlsx`
  - `./data/files/jobs/{job_id}/executions/{execution_id}/sftp/*.csv`

## Critical Coding Standards (RECC 2025)

**These rules from `.kiro/steering/implments-rules.md` are MANDATORY:**

### Absolute Requirements (No Exceptions)

1. **NO `unwrap()` or `expect()` in production code** - Only allowed in `main()` when crash is acceptable or in tests. Always use `?` operator with custom errors.

2. **All async functions MUST have tracing**:
   ```rust
   #[tracing::instrument(skip(pool, redis, state))]
   async fn my_function(pool: &PgPool, redis: &RedisPool) -> Result<T, E>
   ```

3. **Error handling with `thiserror`**:
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

5. **Graceful shutdown required** on SIGTERM and Ctrl+C

6. **Prefer channels over `Arc<Mutex<T>>`** - Use `tokio::sync::mpsc` or `broadcast`

7. **SQLx compile-time verification**:
   ```rust
   sqlx::query_as!(Job, "SELECT * FROM jobs WHERE id = $1", id)
   ```

8. **All spawned tasks must handle errors**:
   ```rust
   tokio::spawn(async move {
       if let Err(e) = process().await {
           tracing::error!(error = %e, "Task failed");
       }
   });
   ```

### Standard Patterns

**Public structs**:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Job { ... }
```

**Timeouts on external calls**:
```rust
reqwest::Client::builder()
    .timeout(Duration::from_secs(30))
    .build()?
```

**Retry with jitter**:
```rust
use backoff::ExponentialBackoffBuilder;
let backoff = ExponentialBackoffBuilder::new()
    .with_jitter()
    .build();
```

**Idempotency keys**:
```rust
let execution_id = Uuid::new_v4();  // Use as idempotency key
```

**Layered configuration** (`config/default.toml` → `config/local.toml` → env vars):
```rust
Config::builder()
    .add_source(File::with_name("config/default"))
    .add_source(File::with_name("config/local").required(false))
    .add_source(Environment::with_prefix("APP"))
    .build()?
```

## Common Development Patterns

### Adding a New Job Executor Type

1. Create new file in `common/src/executor/` (e.g., `grpc.rs`)
2. Implement `JobExecutor` trait:
   ```rust
   #[async_trait]
   impl JobExecutor for GrpcExecutor {
       #[tracing::instrument(skip(self, context))]
       async fn execute(
           &self,
           step: &JobStep,
           context: &mut JobContext,
       ) -> Result<StepOutput, ExecutionError> {
           // Implementation
       }
   }
   ```
3. Add executor to `common/src/executor/mod.rs`
4. Update `JobType` enum in `common/src/models.rs`
5. Wire up in `worker/src/main.rs`
6. Add property-based tests

### Adding Database Migration

1. Create migration: `sqlx migrate add add_feature_x`
2. Edit `migrations/{timestamp}_add_feature_x.sql`
3. Run: `sqlx migrate run`
4. Update corresponding model in `common/src/models.rs`
5. Update repository in `common/src/db/`
6. Regenerate offline sqlx cache: `cargo sqlx prepare`

### Adding API Endpoint

1. Add handler in `api/src/handlers/{module}.rs`
2. Add `#[tracing::instrument(skip(...))]` to handler
3. Add route in `api/src/main.rs`
4. Add RBAC permission check via middleware
5. Update HTMX template if needed (in `api/templates/`)

### Working with Storage Service (PostgreSQL + Redis + Filesystem)

```rust
use common::storage::StorageService;

// Store job definition (PostgreSQL + Redis cache)
storage.store_job_definition(job_id, &serde_json::to_string(&job)?).await?;

// Load job definition (Redis cache → PostgreSQL fallback)
let definition = storage.load_job_definition(job_id).await?;
let job: Job = serde_json::from_str(&definition)?;

// Store execution context (PostgreSQL + Redis cache)
storage.store_context(&context).await?;

// Load execution context (Redis cache → PostgreSQL fallback)
let context = storage.load_context(job_id, execution_id).await?;

// Store file to filesystem
storage.store_file("jobs/xxx/executions/yyy/output/file.xlsx", &data).await?;

// Load file from filesystem
let data = storage.load_file("jobs/xxx/executions/yyy/output/file.xlsx").await?;
```

### Template Substitution

Variables and step outputs are substituted in job configs:
```rust
use common::substitution::substitute_variables;

let config_str = r#"{"url": "${API_URL}/users/{{steps.step1.output.user_id}}"}"#;
let substituted = substitute_variables(config_str, &variables, &context)?;
```

## Configuration

Default config is in `config/default.toml`. Create `config/local.toml` for local overrides (git-ignored).

Environment variables use `APP__` prefix with double underscores for nesting:
```bash
export APP__DATABASE__URL="postgresql://..."
export APP__REDIS__URL="redis://..."
export APP__AUTH__JWT_SECRET="secret"
```

## Observability

- **Structured Logging**: JSON format with trace context
- **Metrics**: Prometheus endpoint at `:9090/metrics`
- **Tracing**: OpenTelemetry OTLP export to configured endpoint
- **Health Checks**: `/health` endpoint returns service status

All async functions should be instrumented with `#[tracing::instrument(skip(...))]` for distributed tracing.

## Testing Philosophy

- **Unit tests** for business logic in `common/src/`
- **Property-based tests** for invariants (20 test suites, 100+ iterations each)
- **Integration tests** with testcontainers for full stack
- **No `unwrap()` in tests** - Use `assert!(result.is_ok())` or `?` in test functions

## Security Considerations

- JWT tokens for API authentication (24h expiration)
- Bcrypt for password hashing (cost: 12)
- HMAC-SHA256 for webhook signature validation
- Variable encryption for sensitive values
- Rate limiting on webhook endpoints
- RBAC permissions: `job:read`, `job:write`, `job:execute`, `job:delete`, `execution:read`
- No secrets in code - use environment variables or config files (git-ignored)

## Documentation

- Main README: Vietnamese language, comprehensive
- `DEPLOYMENT.md`: Production deployment with Kubernetes/Helm
- `LOCAL-DEVELOPMENT.md`: Local setup and development workflow
- `.kiro/specs/`: Technical specs and design docs
- `examples/`: 13 example job definitions (JSON)
