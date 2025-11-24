# Technology Stack

> **Last Updated**: January 2025  
> **Rust Version**: 1.75+ (2021 Edition)  
> **Update Policy**: Use latest stable versions, update quarterly

## Language & Runtime

- **Rust 1.75+** (2021 Edition): Type-safe, high-performance systems programming
  - Minimum version: 1.75 for latest async features
  - Use stable channel, not nightly
- **Tokio 1.35+**: Async runtime for concurrent operations
  - Full feature set: `features = ["full"]`
  - Stable and production-ready

## Core Dependencies

### Web & API
- **Axum 0.7**: Web framework for REST API and dashboard
  - Latest stable: 0.7.x
  - Excellent performance and ergonomics
- **Tower 0.4** / **Tower-HTTP 0.5**: Middleware and service abstractions
  - Industry standard for middleware
- **Tera 1.19+**: Template engine for HTMX dashboard
  - Stable and feature-complete
- **Reqwest 0.12**: HTTP client for job execution
  - Updated from 0.11 for better async support
  - Use `rustls-tls` feature for pure Rust TLS

### Data Storage
- **PostgreSQL 14+**: System database for job definitions, execution history, variables, and users
  - Recommended: PostgreSQL 15 or 16 for best performance
  - Via sqlx with compile-time query checking
- **Redis 7.0+**: Distributed locking (RedLock algorithm) and rate limiting
  - Stable and production-ready
  - Support for Redis Cluster
- **NATS 2.10+**: Job queue with exactly-once delivery guarantees
  - NATS JetStream for persistence
  - Recommended: NATS 2.10+ for latest features
- **MinIO RELEASE.2024-01+**: Object storage for job definitions and execution context
  - S3-compatible API
  - Self-hosted or cloud

### Database Drivers
- **sqlx 0.8**: PostgreSQL (system database and target database)
  - Updated from 0.7 for better performance
  - Features: `runtime-tokio-rustls`, `postgres`, `uuid`, `chrono`, `json`
- **mysql_async 0.34**: MySQL target database support
  - Updated from 0.32 for MySQL 8.0+ compatibility
- **oracle 0.6**: Oracle 19c+ target database support
  - Updated from 0.5 for better error handling

### Object Storage
- **rust-s3 0.34**: MinIO/S3 client
  - Updated from 0.33 for better async support
  - Pure Rust implementation

### File Processing
- **calamine 0.24**: Excel file reading (XLSX)
  - Updated from 0.22 for better performance
- **rust_xlsxwriter 0.65**: Excel file writing (XLSX)
  - Updated from 0.56 for more features
- **csv 1.3**: CSV file processing
  - Stable and feature-complete

### SFTP & SSH
- **ssh2 0.9**: SFTP operations
  - Stable libssh2 bindings
  - Support for password and key-based auth

### Time & Scheduling
- **chrono 0.4**: Date/time handling
  - Industry standard, stable
- **chrono-tz 0.9**: Timezone support
  - Updated from 0.8 for latest timezone data
- **cron 0.12**: Quartz-syntax cron expression parsing
  - Stable and feature-complete

### Authentication & Security
- **jsonwebtoken 9.3**: JWT token handling
  - Updated from 9.2 for security fixes
- **bcrypt 0.15**: Password hashing for database authentication mode
  - Stable and secure
- **hmac 0.12** / **sha2 0.10**: HMAC-SHA256 for webhook signatures
  - Pure Rust cryptography

### JSON & Serialization
- **serde 1.0**: Serialization framework
  - Industry standard
- **serde_json 1.0**: JSON serialization
  - Fast and reliable
- **serde_json_path 0.6**: JSONPath for nested data access
  - For step output references

### Observability
- **tracing 0.1**: Structured logging framework
  - Industry standard for Rust
- **tracing-subscriber 0.3**: Logging subscriber with JSON output
  - Features: `json`, `env-filter`
- **tracing-opentelemetry 0.23**: OpenTelemetry integration
  - Updated from 0.22 for latest OTLP support
- **opentelemetry 0.22**: OpenTelemetry SDK
  - Updated from 0.21
- **opentelemetry-otlp 0.15**: OTLP exporter
  - Updated from 0.14
- **metrics 0.22**: Metrics facade
  - Updated from 0.21
- **metrics-exporter-prometheus 0.15**: Prometheus metrics exporter
  - Updated from 0.13

### Configuration
- **config 0.14**: Layered configuration management
  - Updated from 0.13 for better TOML support

### Error Handling
- **thiserror 1.0**: Domain-specific errors
  - Stable and feature-complete
- **anyhow 1.0**: Application error propagation
  - Stable and widely used

### Async Utilities
- **async-trait 0.1**: Async trait support
  - Required for async trait methods
- **futures 0.3**: Future utilities
  - Standard async utilities

### UUID
- **uuid 1.7**: UUID generation and parsing
  - Updated from 1.6
  - Features: `v4`, `serde`

### Testing
- **proptest 1.4**: Property-based testing (minimum 100 iterations per property)
  - Stable and feature-complete
- **mockall 0.12**: Mocking for unit tests
  - Latest stable version
- **testcontainers 0.17**: Integration testing with PostgreSQL, Redis, NATS
  - Updated from 0.15 for better Docker support
- **criterion 0.5**: Performance benchmarking
  - Industry standard for Rust benchmarks

## Version Update Policy

### Quarterly Updates (Every 3 Months)
- Review all dependencies for security updates
- Update to latest stable minor versions
- Test thoroughly before updating production

### Security Updates (Immediate)
- Apply security patches as soon as available
- Monitor GitHub Security Advisories
- Use `cargo audit` regularly

### Major Version Updates (Carefully)
- Evaluate breaking changes
- Update design document if APIs change
- Full regression testing required

## Dependency Management

### Cargo.toml Best Practices

```toml
[dependencies]
# Use specific minor versions, allow patch updates
axum = "0.7"              # Allows 0.7.x
tokio = { version = "1.35", features = ["full"] }
sqlx = { version = "0.8", features = ["runtime-tokio-rustls", "postgres", "uuid", "chrono", "json"] }

# Pin exact versions for critical security dependencies
jsonwebtoken = "9.3"
bcrypt = "0.15"

# Use workspace dependencies for consistency
[workspace.dependencies]
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
```

### Security Scanning

```bash
# Install cargo-audit
cargo install cargo-audit

# Run security audit
cargo audit

# Update dependencies
cargo update

# Check for outdated dependencies
cargo outdated
```

## Minimum Supported Rust Version (MSRV)

**MSRV: 1.75.0**

Reasons:
- Async trait improvements
- Better error messages
- Performance improvements
- Latest stable features

Update MSRV policy:
- Review every 6 months
- Only update if new features are needed
- Document breaking changes

## Project Structure

```
src/
├── main.rs                 # API server entry point
├── bin/
│   ├── scheduler.rs        # Scheduler binary (separate process)
│   └── worker.rs           # Worker binary (separate process)
├── config/                 # Configuration management
├── errors/                 # Domain and application errors
├── models/                 # Job, Execution, Variable, User models
├── scheduler/              # Trigger detection, locking, publishing
├── worker/                 # Job consumption and execution
│   └── executor/           # HTTP and database executors
├── api/                    # REST API routes and handlers
│   ├── handlers/           # Job, execution, variable, auth handlers
│   └── middleware/         # JWT validation and RBAC
├── db/                     # Database layer
│   ├── migrations/         # SQL migrations
│   └── repositories/       # Job, execution, variable, user repos
├── queue/                  # NATS JetStream integration
├── telemetry/              # Logging, metrics, tracing
└── web/                    # HTMX templates
    └── templates/
```

## Common Commands

### Development
```bash
# Run database migrations
sqlx migrate run

# Start scheduler
cargo run --bin scheduler

# Start worker
cargo run --bin worker

# Start API server
cargo run

# Run all tests
cargo test

# Run property tests
cargo test --test property_tests

# Run integration tests (requires Docker)
cargo test --test integration_tests
```

### Testing
```bash
# Unit tests
cargo test --lib

# Property-based tests (100+ iterations)
cargo test property_

# Integration tests with testcontainers
cargo test --test '*_integration'

# Benchmarks
cargo bench
```

### Build & Deploy
```bash
# Build release binaries
cargo build --release

# Build Docker image (multi-stage, <50MB target)
docker build -t vietnam-cron:latest .

# Run with docker-compose
docker-compose up -d

# Deploy to Kubernetes with Helm
helm install vietnam-cron ./charts/vietnam-enterprise-cron
```

## Configuration

Layered configuration (precedence: CLI args > env vars > config file > defaults):

- **Config file**: `config.toml` (TOML format)
- **Environment variables**: Prefix with `APP_` (e.g., `APP_DATABASE__URL`)
- **CLI arguments**: Override all other sources

Key configuration sections:
- `[server]`: API server host/port
- `[database]`: PostgreSQL connection and pool settings
- `[redis]`: Redis connection for distributed locking
- `[nats]`: NATS JetStream configuration
- `[auth]`: Authentication mode (database/keycloak), JWT settings
- `[scheduler]`: Poll interval, lock TTL
- `[worker]`: Concurrency, retries, timeouts
- `[observability]`: Log level, metrics port, tracing endpoint

## Code Quality Standards

- **No `unwrap()` or `expect()` in production code**: Use `?` operator or explicit error handling
- **Compile-time query checking**: All SQL queries validated at compile time with sqlx
- **Property-based testing**: Minimum 100 iterations per property test
- **Test tagging**: Property tests tagged with `// Feature: vietnam-enterprise-cron, Property N: <description>`
- **Structured logging**: All logs in JSON format with trace context
- **Graceful shutdown**: Handle SIGTERM/SIGINT, complete in-flight work before exit
