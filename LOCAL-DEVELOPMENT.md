# Local Development Guide

This guide explains how to run the Vietnam Enterprise Cron System locally for development.

## Prerequisites

- Rust 1.75+ (2021 Edition)
- Docker and Docker Compose
- PostgreSQL client tools (optional, for manual DB access)

## Quick Start

### Option 1: Automated Start (Recommended)

```bash
# Start all services (infrastructure + application)
./start-services-local.sh

# Check service status
./check-services.sh

# Stop services when done
./stop-services.sh
```

### Option 2: Manual Start

#### Step 1: Start Infrastructure Services (Docker)

```bash
# Start PostgreSQL, Redis, NATS
docker-compose up -d postgres redis nats

# Wait for services to be healthy (about 10 seconds)
docker-compose ps
```

#### Step 2: Build Application Binaries

```bash
# Build all binaries in release mode
cargo build --release --bin api --bin scheduler --bin worker
```

#### Step 3: Start Application Services

```bash
# Create logs directory
mkdir -p logs

# Start Scheduler
RUST_LOG=info ./target/release/scheduler > logs/scheduler.log 2>&1 &

# Start Worker
RUST_LOG=info ./target/release/worker > logs/worker.log 2>&1 &

# Start API
RUST_LOG=info ./target/release/api > logs/api.log 2>&1 &
```

## Configuration

The system uses layered configuration:

1. **Default config**: `config/default.toml`
2. **Local overrides**: `config/local.toml` (created automatically)
3. **Environment variables**: Prefix with `APP__` (e.g., `APP__SERVER__PORT=8080`)

### Local Configuration File

The `config/local.toml` file is automatically created with the following settings:

```toml
[database]
url = "postgresql://cronuser:cronpass@localhost:5432/vietnam_cron"

[redis]
url = "redis://:redispass@localhost:6379"

[nats]
url = "nats://localhost:4222"

[storage]
file_base_path = "./data/files"
```

## Access Points

Once all services are running:

- **Dashboard**: http://localhost:8080
- **API**: http://localhost:8080/api
- **Health Check**: http://localhost:8080/health
- **Prometheus Metrics**: http://localhost:9090/metrics
- **NATS Monitoring**: http://localhost:8222

## Default Credentials

For database authentication mode:

- **Username**: admin
- **Password**: admin123

## Viewing Logs

### Application Logs (Local Services)

```bash
# Scheduler logs
tail -f logs/scheduler.log

# Worker logs
tail -f logs/worker.log

# API logs
tail -f logs/api.log
```

### Infrastructure Logs (Docker Services)

```bash
# PostgreSQL logs
docker logs -f vietnam-cron-postgres

# Redis logs
docker logs -f vietnam-cron-redis

# NATS logs
docker logs -f vietnam-cron-nats
```

## Checking Service Status

```bash
# Run the status check script
./check-services.sh
```

This will show:
- ✓ Running services (green)
- ✗ Stopped services (red)
- ⚠ Services with issues (yellow)

## Stopping Services

```bash
# Stop application services (keeps infrastructure running)
./stop-services.sh

# Stop everything (including infrastructure)
docker-compose down
```

## Development Workflow

### Making Code Changes

1. Make your changes to the source code
2. Rebuild the affected binary:
   ```bash
   cargo build --release --bin api    # For API changes
   cargo build --release --bin worker # For worker changes
   cargo build --release --bin scheduler # For scheduler changes
   ```
3. Stop and restart the affected service:
   ```bash
   pkill -f "./target/release/api"
   RUST_LOG=info ./target/release/api > logs/api.log 2>&1 &
   ```

### Running Tests

```bash
# Run all tests
cargo test

# Run property-based tests
cargo test --test property_tests

# Run specific test
cargo test test_name
```

### Database Migrations

Migrations are automatically applied when the API starts. To run them manually:

```bash
# Set database URL
export DATABASE_URL="postgresql://cronuser:cronpass@localhost:5432/vietnam_cron"

# Run migrations
sqlx migrate run
```

## Troubleshooting

### Services Won't Start

1. Check if ports are already in use:
   ```bash
   lsof -i :8080  # API
   lsof -i :5432  # PostgreSQL
   lsof -i :6379  # Redis
   lsof -i :4222  # NATS
   ```

2. Check logs for errors:
   ```bash
   tail -f logs/api.log
   tail -f logs/scheduler.log
   tail -f logs/worker.log
   ```

3. Verify infrastructure services are healthy:
   ```bash
   docker-compose ps
   ```

### Database Connection Issues

1. Verify PostgreSQL is running:
   ```bash
   docker ps | grep postgres
   ```

2. Test connection manually:
   ```bash
   psql postgresql://cronuser:cronpass@localhost:5432/vietnam_cron
   ```

3. Check database logs:
   ```bash
   docker logs vietnam-cron-postgres
   ```

### Redis Connection Issues

1. Verify Redis is running:
   ```bash
   docker ps | grep redis
   ```

2. Test connection:
   ```bash
   redis-cli -a redispass ping
   ```

### NATS Connection Issues

1. Verify NATS is running:
   ```bash
   docker ps | grep nats
   ```

2. Check NATS monitoring:
   ```bash
   curl http://localhost:8222/varz
   ```

## Performance Tips

### Development Mode

For faster compilation during development:

```bash
# Use debug build (faster compilation, slower runtime)
cargo build --bin api
./target/debug/api
```

### Release Mode

For production-like performance:

```bash
# Use release build (slower compilation, faster runtime)
cargo build --release --bin api
./target/release/api
```

## IDE Integration

### VS Code

Recommended extensions:
- rust-analyzer
- CodeLLDB (for debugging)
- Even Better TOML

### Debugging

1. Build with debug symbols:
   ```bash
   cargo build --bin api
   ```

2. Use your IDE's debugger or lldb:
   ```bash
   lldb ./target/debug/api
   ```

## Additional Resources

- [Main README](README.md) - Project overview
- [Deployment Guide](DEPLOYMENT.md) - Production deployment
- [Architecture Documentation](.kiro/specs/vietnam-enterprise-cron/design.md)
- [Requirements](.kiro/specs/vietnam-enterprise-cron/requirements.md)

## Support

If you encounter issues:

1. Check the logs
2. Verify all services are running with `./check-services.sh`
3. Review the troubleshooting section above
4. Check the GitHub issues for similar problems
