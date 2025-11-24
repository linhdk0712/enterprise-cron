# Project Organization

## Architecture Pattern

**Distributed Microservices**: Three separate binaries (scheduler, worker, API) sharing common library code, designed for horizontal scalability.

## Module Organization

### Binary Separation
- **`src/main.rs`**: API server (REST endpoints + HTMX dashboard)
- **`src/bin/scheduler.rs`**: Scheduler process (job trigger detection and queue publishing)
- **`src/bin/worker.rs`**: Worker process (job consumption and execution)

Each binary initializes only its required components to minimize resource usage.

### Core Modules

#### `config/`
Configuration loading with layered precedence (defaults → file → env → CLI). Single `Settings` struct for all configuration.

#### `errors/`
- **`domain.rs`**: Domain-specific errors using `thiserror` (ScheduleError, ExecutionError, AuthError, ValidationError)
- **`app.rs`**: Application-level error handling with `anyhow`

#### `models/`
Core domain models with serde serialization:
- **`job.rs`**: Job definition with Schedule enum (Cron, FixedDelay, FixedRate, OneTime) and JobType enum (HttpRequest, DatabaseQuery)
- **`execution.rs`**: JobExecution with status tracking and idempotency keys
- **`variable.rs`**: Variable with scope (Global, Job-specific) and encryption support
- **`user.rs`**: User model for database authentication mode

#### `scheduler/`
Scheduler component responsibilities:
- **`trigger.rs`**: ScheduleTrigger trait implementation for calculating next execution times
- **`lock.rs`**: DistributedLock trait with Redis RedLock implementation
- **`publisher.rs`**: JobPublisher trait for NATS queue publishing

#### `worker/`
Worker component responsibilities:
- **`consumer.rs`**: NATS queue consumer with exactly-once processing
- **`executor/http.rs`**: HTTP job executor (GET/POST/PUT with auth)
- **`executor/database.rs`**: Database job executor (PostgreSQL, MySQL, Oracle)
- **`retry.rs`**: RetryStrategy trait with exponential backoff and jitter
- **`circuit_breaker.rs`**: CircuitBreaker pattern for external system failures

#### `api/`
REST API and dashboard:
- **`routes.rs`**: Axum router configuration
- **`handlers/jobs.rs`**: Job CRUD operations
- **`handlers/executions.rs`**: Execution history queries
- **`handlers/variables.rs`**: Variable management
- **`handlers/auth.rs`**: Login and token management
- **`handlers/sse.rs`**: Server-Sent Events for real-time updates
- **`middleware/auth.rs`**: JWT validation middleware
- **`middleware/rbac.rs`**: Permission checking middleware

#### `db/`
Database layer:
- **`migrations/`**: SQL migration files (managed by sqlx)
- **`repositories/job.rs`**: JobRepository trait (find_jobs_due, CRUD operations)
- **`repositories/execution.rs`**: ExecutionRepository trait (idempotency checking, history queries)
- **`repositories/variable.rs`**: VariableRepository trait (global/job-specific variables)
- **`repositories/user.rs`**: UserRepository trait (authentication, role management)
- **`pool.rs`**: PostgreSQL connection pool management

#### `queue/`
Message queue abstraction:
- **`nats.rs`**: NATS JetStream client implementation
- **`message.rs`**: Message serialization types

#### `telemetry/`
Observability infrastructure:
- **`logging.rs`**: Structured logging setup (JSON format with trace context)
- **`metrics.rs`**: Prometheus metrics (counters, histograms, gauges)
- **`tracing.rs`**: OpenTelemetry tracing configuration

#### `web/`
Frontend templates:
- **`templates/layout.html`**: Base layout with responsive CSS
- **`templates/jobs.html`**: Job list and details
- **`templates/executions.html`**: Execution history
- **`templates/variables.html`**: Variable management

## Database Schema

### System Database (PostgreSQL)
- **`jobs`**: Job definitions with schedule and configuration (JSONB columns)
- **`job_executions`**: Execution history with status, timing, results
- **`variables`**: Global and job-specific variables with encryption
- **`job_stats`**: Aggregated statistics per job (success rate, last execution times)
- **`users`**: User accounts for database authentication mode
- **`roles`**: Role definitions with permissions array
- **`user_roles`**: Many-to-many relationship between users and roles

### Indexes
- `job_executions`: job_id, status, created_at, idempotency_key (unique)
- `variables`: (name, scope_type, scope_id) unique constraint

## Deployment Structure

### Docker
- **Multi-stage Dockerfile**: Build stage (Rust alpine) → Runtime stage (minimal alpine, <50MB)
- **docker-compose.yml**: PostgreSQL, Redis, NATS, scheduler, worker, API services

### Kubernetes (Helm Chart)
```
charts/vietnam-enterprise-cron/
├── Chart.yaml
├── values.yaml
└── templates/
    ├── scheduler-deployment.yaml      # 3+ replicas with anti-affinity
    ├── worker-deployment.yaml         # Auto-scaling based on queue depth
    ├── api-deployment.yaml            # 2+ replicas behind load balancer
    ├── postgresql-statefulset.yaml    # Primary + read replicas
    ├── redis-cluster.yaml             # 6 nodes (3 masters, 3 replicas)
    ├── nats-cluster.yaml              # 3-node cluster
    ├── configmap.yaml                 # Configuration
    ├── secrets.yaml                   # Sensitive data
    ├── service.yaml                   # Service definitions
    ├── ingress.yaml                   # External access
    ├── rbac.yaml                      # Kubernetes RBAC
    └── hpa.yaml                       # Horizontal Pod Autoscaler
```

## Design Principles

1. **Separation of Concerns**: Clear boundaries between scheduler, worker, API, and storage layers
2. **Trait-Based Abstractions**: All major components defined as traits for testability and flexibility
3. **Type Safety**: Leverage Rust's type system and sqlx compile-time checking
4. **Exactly-Once Semantics**: Distributed locks + idempotency keys + NATS acknowledgments
5. **Observable by Default**: Structured logging, metrics, and tracing built into all operations
6. **Graceful Degradation**: Circuit breakers, retries with backoff, dead letter queues
7. **Configuration as Code**: All settings externalized, hot-reloadable where possible
8. **Security First**: No plaintext secrets, RBAC enforcement, audit logging, parameterized queries

## Naming Conventions

- **Modules**: Snake_case (e.g., `circuit_breaker.rs`)
- **Structs/Enums**: PascalCase (e.g., `JobExecution`, `ExecutionStatus`)
- **Functions/Variables**: Snake_case (e.g., `find_jobs_due`, `next_execution_time`)
- **Constants**: SCREAMING_SNAKE_CASE (e.g., `MAX_RETRIES`)
- **Traits**: PascalCase with descriptive names (e.g., `JobExecutor`, `DistributedLock`)

## Testing Organization

- **Unit tests**: Inline with `#[cfg(test)]` modules in each source file
- **Property tests**: Separate `tests/property_tests.rs` with proptest (100+ iterations)
- **Integration tests**: `tests/*_integration.rs` using testcontainers
- **Benchmarks**: `benches/` directory with criterion
- **Test tagging**: Property tests include comment: `// Feature: vietnam-enterprise-cron, Property N: <description>`
