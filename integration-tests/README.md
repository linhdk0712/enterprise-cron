# Integration Tests

This directory contains end-to-end integration tests for the Vietnam Enterprise Cron System.

## Overview

These tests verify complete workflows across all system components:
- Multi-step job execution with Job Context
- Webhook trigger flows with signature validation
- File processing (Excel/CSV) operations
- SFTP download/upload operations
- Job import/export with sensitive data masking

## Prerequisites

Before running integration tests, ensure the following services are running:

### Using Docker Compose (Recommended)

```bash
# Start all required services
docker-compose up -d postgres redis nats minio

# Wait for services to be healthy
docker-compose ps

# Run migrations
sqlx migrate run
```

### Manual Setup

If not using Docker Compose, ensure these services are available:

1. **PostgreSQL 16+** on `localhost:5432`
   - Database: `vietnam_cron`
   - User: `cronuser`
   - Password: `cronpass`

2. **Redis 7+** on `localhost:6379`
   - Password: `redispass`

3. **NATS 2.10+** with JetStream on `localhost:4222`

4. **MinIO** on `localhost:9000`
   - Access Key: `minioadmin`
   - Secret Key: `minioadmin`
   - Bucket: `vietnam-cron-test`

## Running Tests

### Run All Integration Tests

```bash
# Run all integration tests (requires services to be running)
cargo test --test integration_tests -- --ignored --test-threads=1
```

### Run Specific Test

```bash
# Run only multi-step job test
cargo test --test integration_tests test_multi_step_job_execution -- --ignored

# Run only webhook test
cargo test --test integration_tests test_webhook_trigger_flow -- --ignored

# Run only file processing test
cargo test --test integration_tests test_file_processing_flow -- --ignored

# Run only SFTP test
cargo test --test integration_tests test_sftp_operations -- --ignored

# Run only import/export test
cargo test --test integration_tests test_job_import_export -- --ignored
```

### Environment Variables

You can override default connection settings:

```bash
export DATABASE_URL="postgresql://cronuser:cronpass@localhost:5432/vietnam_cron"
export MINIO_ENDPOINT="localhost:9000"
export MINIO_ACCESS_KEY="minioadmin"
export MINIO_SECRET_KEY="minioadmin"
export MINIO_BUCKET="vietnam-cron-test"

cargo test --test integration_tests -- --ignored
```

## Test Coverage

### Task 39.1: Multi-Step Job Execution
- **Requirements**: 13.4, 13.8, 14.1
- **Verifies**:
  - Job definition storage in MinIO
  - Job record creation in database
  - Sequential step execution
  - Job Context persistence
  - Step output references

### Task 39.2: Webhook Trigger Flow
- **Requirements**: 16.2, 16.7, 16.9
- **Verifies**:
  - Webhook URL generation
  - Webhook configuration storage
  - HMAC-SHA256 signature validation
  - Job execution with webhook data
  - Rate limiting

### Task 39.3: File Processing Flow
- **Requirements**: 15.1, 15.3, 15.6, 15.7
- **Verifies**:
  - Excel/CSV file upload to MinIO
  - File parsing and data transformation
  - Output file generation
  - File metadata storage in Job Context

### Task 39.4: SFTP Operations
- **Requirements**: 19.1, 19.2, 19.5, 19.14
- **Verifies**:
  - SFTP connection configuration
  - Download with wildcard patterns
  - Upload with directory creation
  - File storage in MinIO
  - Authentication methods (password/SSH key)

### Task 39.5: Job Import/Export
- **Requirements**: 18.4, 18.5, 18.9
- **Verifies**:
  - Job export with metadata
  - Sensitive data masking
  - JSON schema validation
  - Import round-trip consistency
  - Job configuration preservation

## Test Architecture

### Test Structure

```
tests/
├── integration_tests.rs    # Main integration test suite
└── README.md               # This file
```

### Helper Functions

- `setup_test_db()`: Establishes PostgreSQL connection
- `setup_storage()`: Initializes MinIO storage service
- `wait_for_execution_completion()`: Polls for job execution completion

### Test Isolation

Each test:
1. Creates unique job IDs to avoid conflicts
2. Stores test data in MinIO with unique paths
3. Cleans up resources after completion
4. Can run independently

## Running with Worker

For complete end-to-end testing with actual job execution:

```bash
# Terminal 1: Start services
docker-compose up -d

# Terminal 2: Start worker
cargo run --bin worker

# Terminal 3: Run integration tests
cargo test --test integration_tests -- --ignored --test-threads=1
```

The tests will:
1. Create job definitions
2. Trigger job executions
3. Wait for worker to process them
4. Verify results in database and MinIO

## Troubleshooting

### Tests Fail to Connect to Services

```bash
# Check service status
docker-compose ps

# View service logs
docker-compose logs postgres
docker-compose logs redis
docker-compose logs nats
docker-compose logs minio

# Restart services
docker-compose restart
```

### Database Migration Issues

```bash
# Run migrations manually
sqlx migrate run

# Reset database (WARNING: deletes all data)
docker-compose down -v
docker-compose up -d postgres
sqlx migrate run
```

### MinIO Bucket Not Found

```bash
# Create bucket manually using MinIO client
mc alias set local http://localhost:9000 minioadmin minioadmin
mc mb local/vietnam-cron-test
```

### Worker Not Processing Jobs

```bash
# Check worker logs
cargo run --bin worker

# Verify NATS connection
docker-compose logs nats

# Check job queue
# (Use NATS CLI or monitoring tools)
```

## CI/CD Integration

For automated testing in CI/CD pipelines:

```yaml
# Example GitHub Actions workflow
name: Integration Tests

on: [push, pull_request]

jobs:
  integration-tests:
    runs-on: ubuntu-latest
    
    services:
      postgres:
        image: postgres:16-alpine
        env:
          POSTGRES_DB: vietnam_cron
          POSTGRES_USER: cronuser
          POSTGRES_PASSWORD: cronpass
        ports:
          - 5432:5432
      
      redis:
        image: redis:7-alpine
        ports:
          - 6379:6379
      
      nats:
        image: nats:2.10-alpine
        ports:
          - 4222:4222
      
      minio:
        image: minio/minio:latest
        env:
          MINIO_ROOT_USER: minioadmin
          MINIO_ROOT_PASSWORD: minioadmin
        ports:
          - 9000:9000
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Run migrations
        run: sqlx migrate run
      
      - name: Run integration tests
        run: cargo test --test integration_tests -- --ignored --test-threads=1
```

## Notes

- Integration tests are marked with `#[ignore]` to prevent them from running during normal `cargo test`
- Tests require external services and may take longer to execute
- Use `--test-threads=1` to run tests sequentially and avoid resource conflicts
- Tests create and clean up their own test data
- For production testing, use separate test database and MinIO bucket

## Related Documentation

- [Requirements Document](../.kiro/specs/vietnam-enterprise-cron/requirements.md)
- [Design Document](../.kiro/specs/vietnam-enterprise-cron/design.md)
- [Task List](../.kiro/specs/vietnam-enterprise-cron/tasks.md)
- [Docker Compose Configuration](../docker-compose.yml)
