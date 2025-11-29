# Testing & API Documentation

## Overview
This document consolidates all testing and API documentation.

## API Testing

### Quick Start

#### Using Postman (Recommended)
1. Import `Vietnam_Cron_API.postman_collection.json`
2. Run "Authentication → Login (Admin)"
3. Token is automatically saved
4. Test other endpoints

#### Using Curl
```bash
# Login and save token
TOKEN=$(curl -s -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"admin123"}' | \
  jq -r '.data.token')

# List jobs
curl -X GET http://localhost:8080/api/jobs \
  -H "Authorization: Bearer $TOKEN" | jq '.'
```

### Default Credentials
- **Username**: admin
- **Password**: admin123
- **Role**: Admin (18 permissions)

### Key Endpoints

#### Authentication
- `POST /api/auth/login` - Login
- `POST /api/auth/refresh` - Refresh token

#### Jobs
- `GET /api/jobs` - List jobs
- `POST /api/jobs` - Create job
- `GET /api/jobs/:id` - Get job details
- `PUT /api/jobs/:id` - Update job
- `DELETE /api/jobs/:id` - Delete job
- `POST /api/jobs/:id/trigger` - Trigger job manually
- `PUT /api/jobs/:id/enable` - Enable job
- `PUT /api/jobs/:id/disable` - Disable job

#### Executions
- `GET /api/executions` - List executions
- `GET /api/executions/:id` - Get execution details
- `POST /api/executions/:id/stop` - Stop execution (graceful or force)

#### Variables
- `GET /api/variables` - List variables
- `POST /api/variables` - Create variable
- `PUT /api/variables/:id` - Update variable
- `DELETE /api/variables/:id` - Delete variable

#### Users (Admin Only)
- `GET /api/users` - List users
- `POST /api/users` - Create user
- `GET /api/users/:id` - Get user
- `PUT /api/users/:id/roles` - Assign roles
- `DELETE /api/users/:id` - Delete user

#### System
- `GET /health` - Health check
- `GET /api/info` - API info
- `GET /metrics` - Prometheus metrics

### RBAC Permissions

#### Admin Role (18 permissions)
- job:read, job:write, job:execute, job:delete
- job:import, job:export
- execution:read, execution:stop
- variable:read, variable:write, variable:encrypt
- webhook:read, webhook:write
- user:manage, role:assign
- system:config, system:audit
- dashboard:admin

#### Regular User Role (5 permissions)
- job:read, job:execute
- execution:read
- variable:read
- dashboard:user

## Integration Testing

### Test Suite Location
- **Directory**: `integration-tests/`
- **Test File**: `integration-tests/tests/integration_tests.rs`

### Test Coverage

#### 1. Multi-Step Job Execution
- Job definition storage in PostgreSQL
- Job record creation
- Multi-step configuration
- Step output references
- Job Context persistence
- Sequential execution

#### 2. Webhook Trigger Flow
- Webhook URL generation
- Secret key configuration
- Rate limiting
- Webhook data references
- Webhook record creation

#### 3. File Processing Flow
- CSV file upload to filesystem
- File processing configuration
- Data transformations
- File path references
- File metadata storage

#### 4. SFTP Operations
- SFTP connection configuration
- Password authentication
- Remote path configuration
- Operation options
- Job definition structure

#### 5. Job Import/Export
- Export with complete configuration
- Sensitive data masking
- Export metadata
- Import round-trip consistency
- Configuration preservation

### Running Tests

```bash
# All integration tests
cargo test --test integration_tests -- --ignored --test-threads=1

# Individual test
cargo test --test integration_tests test_multi_step_job_execution -- --ignored

# With services
docker-compose up -d
cargo test --test integration_tests -- --ignored --test-threads=1
```

### Test Requirements
- PostgreSQL running on localhost:5432
- Redis running on localhost:6379
- NATS running on localhost:4222
- Filesystem writable at ./data/files

## Property-Based Testing

### Location
- API: `api/tests/api_property_tests.rs`
- Common: `common/tests/job_context_property_tests.rs`

### Properties Tested

#### API Properties
- Property 48: Job listing completeness
- Property 49: Execution history time window
- Property 50: Execution history filtering
- Property 51: Manual trigger queueing
- Property 52: Job disable effect
- Property 53: Job enable effect
- Property 15: Sensitive variable masking

#### Job Context Properties
- Property 80: HTTP response storage
- Property 81: Database result storage
- Property 84: Context loading for subsequent steps
- Property 85: Context retention after completion
- Property 86: Context preservation on failure
- Property 93: Automatic step output storage

### Running Property Tests

```bash
# All property tests
cargo test property_

# Specific property test
cargo test property_48_job_listing_completeness

# With verbose output
cargo test property_ -- --nocapture
```

### Test Configuration
- **Iterations**: 100 per property (RECC 2025 standard)
- **Framework**: proptest
- **Tagging**: `// Feature: vietnam-enterprise-cron, Property N: <description>`

## Manual Testing Checklist

### Basic Functionality
- [ ] Create a new job
- [ ] View job details
- [ ] Update job definition
- [ ] Delete job
- [ ] Execute job manually
- [ ] View execution history

### Multi-Step Jobs
- [ ] Create multi-step job
- [ ] Execute with step output references
- [ ] Verify context persistence
- [ ] Check step execution order

### File Processing
- [ ] Upload CSV file
- [ ] Process with transformations
- [ ] Verify output file
- [ ] Check file metadata

### SFTP Operations
- [ ] Configure SFTP job
- [ ] Test download operation
- [ ] Test upload operation
- [ ] Verify file transfer

### Webhook Triggers
- [ ] Create webhook-triggered job
- [ ] Send webhook POST
- [ ] Verify signature validation
- [ ] Check rate limiting

### Job Import/Export
- [ ] Export job to JSON
- [ ] Verify sensitive data masked
- [ ] Import job from JSON
- [ ] Verify configuration preserved

## Debugging

### Check Service Status
```bash
./check-services.sh
```

### View Logs
```bash
# API logs
docker logs vietnam-cron-api -f

# Worker logs
docker logs vietnam-cron-worker-1 -f

# Scheduler logs
docker logs vietnam-cron-scheduler -f
```

### Database Queries
```bash
# Check job status
docker exec vietnam-cron-postgres psql -U cronuser -d vietnam_cron \
  -c "SELECT id, name, enabled FROM jobs;"

# Check execution status
docker exec vietnam-cron-postgres psql -U cronuser -d vietnam_cron \
  -c "SELECT id, status, started_at, completed_at FROM job_executions ORDER BY created_at DESC LIMIT 5;"
```

### Common Issues

#### 401 Unauthorized
- Token expired (24h) - login again
- Invalid token - check JWT secret

#### 403 Forbidden
- Insufficient permissions - check user role
- Decode JWT to see permissions

#### 429 Too Many Requests
- Rate limit exceeded - wait or clear Redis

#### Services Not Running
```bash
# Check status
docker-compose ps

# Restart services
docker-compose restart

# View logs
docker-compose logs -f
```

## Performance Testing

### Metrics to Monitor
- Job execution duration
- Queue depth
- Database query performance
- Redis cache hit rate
- Filesystem I/O

### Load Testing
```bash
# Create multiple jobs
for i in {1..10}; do
  curl -X POST http://localhost:8080/api/jobs \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d "{\"name\":\"Test Job $i\", ...}"
done

# Trigger all jobs
for job_id in $(curl -s http://localhost:8080/api/jobs -H "Authorization: Bearer $TOKEN" | jq -r '.data[].id'); do
  curl -X POST http://localhost:8080/api/jobs/$job_id/trigger \
    -H "Authorization: Bearer $TOKEN"
done
```

## Summary

Comprehensive testing coverage including:
- ✅ API endpoint testing (Postman + curl)
- ✅ Integration tests (5 major workflows)
- ✅ Property-based tests (13 properties)
- ✅ Manual testing checklist
- ✅ Debugging guides
- ✅ Performance testing

---

**Last Updated**: 2025-01-28
**Status**: Complete
