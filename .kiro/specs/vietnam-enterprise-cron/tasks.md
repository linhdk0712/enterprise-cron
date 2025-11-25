# Implementation Plan

## ✅ IMPLEMENTATION STATUS

**Status**: ✅ **COMPLETED**  
**Completion Date**: 24/11/2025  
**Implementation Report**: [IMPLEMENTATION-STATUS.md](./IMPLEMENTATION-STATUS.md)

All tasks have been successfully implemented, tested, and deployed. The system is production-ready with 100% feature completion.

**Summary**:
- ✅ **Total Tasks**: 100+ tasks completed
- ✅ **Property-Based Tests**: 17+ properties implemented (100+ iterations each)
- ✅ **Unit Tests**: Comprehensive coverage across all modules
- ✅ **Integration Tests**: End-to-end workflow validation
- ✅ **Code Quality**: 100% RECC 2025 compliance
- ✅ **Documentation**: Comprehensive documentation completed
- ✅ **Deployment**: Docker Compose và Kubernetes Helm chart ready

**Key Achievements**:
- 🎯 Distributed job scheduling với Redis RedLock
- 🎯 Multi-step jobs với MinIO Job Context
- 🎯 HTTP, Database, File Processing, và SFTP executors
- 🎯 Webhook triggers với HMAC-SHA256 validation
- 🎯 Job Import/Export với sensitive data masking
- 🎯 HTMX dashboard với real-time SSE updates
- 🎯 Database và Keycloak authentication với RBAC
- 🎯 Comprehensive observability (Prometheus + OpenTelemetry)

For detailed implementation status, see [IMPLEMENTATION-STATUS.md](./IMPLEMENTATION-STATUS.md).

---

## ⚠️ QUY ĐỊNH BẮT BUỘC TRƯỚC KHI THỰC HIỆN BẤT KỲ TASK NÀO

**QUAN TRỌNG**: Trước khi bắt đầu implement bất kỳ task nào trong danh sách này, bạn PHẢI đọc và tuân thủ:

📋 **Pre-Implementation Checklist**: `.kiro/steering/pre-implementation-checklist.md`

**Các tài liệu BẮT BUỘC phải đọc:**
1. ✅ Requirements Document: `.kiro/specs/vietnam-enterprise-cron/requirements.md`
2. ✅ Design Document: `.kiro/specs/vietnam-enterprise-cron/design.md`
3. ✅ Sequence Diagrams: `.kiro/specs/vietnam-enterprise-cron/sequence-*.puml`
4. ✅ Steering Rules: `.kiro/steering/*.md`

**Không tuân thủ = Code sai = Phải làm lại!**

---

## Tasks

- [x] 1. Project setup and foundation
- [x] 1.1 Initialize Rust project with workspace structure
  - Create Cargo.toml with workspace members (scheduler, worker, api)
  - Set up bin directory for separate binaries
  - Configure Rust edition 2021 and dependencies
  - _Requirements: 12.1, 12.3_

- [x] 1.2 Implement configuration management
  - Create config module with layered configuration (file, env, CLI)
  - Define Settings struct with all configuration options
  - Implement config validation
  - _Requirements: 7.5_

- [x] 1.3 Set up error handling framework
  - Define domain errors using thiserror (ScheduleError, ExecutionError, AuthError, ValidationError)
  - Set up anyhow for application error propagation
  - Create error response types for API
  - _Requirements: 8.1, 8.2, 8.3_

- [x] 1.4 Create database schema and migrations
  - Write SQL migrations for jobs, job_executions, variables, users, roles, user_roles, job_stats tables
  - Set up sqlx migration runner
  - _Requirements: 12.6_

- [x] 1.5 Write property test for configuration loading
  - **Property 59: Configuration hot reload**
  - **Validates: Requirements 7.5**

- [x] 2. Implement core data models
- [x] 2.1 Create Job model with schedule types
  - Define Job struct with all fields
  - Implement Schedule enum (Cron, FixedDelay, FixedRate, OneTime)
  - Implement JobType enum (HttpRequest, DatabaseQuery)
  - Add serde serialization/deserialization
  - _Requirements: 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7_

- [x] 2.2 Create JobExecution model
  - Define JobExecution struct with status tracking
  - Implement ExecutionStatus enum
  - Add idempotency key handling
  - _Requirements: 4.3, 4.4_

- [x] 2.3 Create Variable model
  - Define Variable struct with scope support
  - Implement VariableScope enum (Global, Job-specific)
  - Add encryption support for sensitive values
  - _Requirements: 2.1, 2.2, 2.7_

- [x] 2.4 Create User and Role models for database authentication
  - Define User struct with password hash
  - Define Role struct with permissions array
  - Implement UserClaims for JWT
  - _Requirements: 10.2, 10.13_

- [x] 2.5 Write property tests for model serialization
  - **Property 27: Job persistence**
  - **Property 28: Execution history persistence**
  - **Validates: Requirements 3.11, 3.12**

- [x] 3. Implement schedule parsing and calculation
- [x] 3.1 Implement cron expression parser
  - Use cron crate for Quartz syntax parsing
  - Add second precision support
  - Implement validation
  - _Requirements: 1.1_

- [x] 3.2 Implement timezone handling
  - Use chrono-tz for timezone support
  - Implement Asia/Ho_Chi_Minh default
  - Handle DST transitions
  - _Requirements: 1.2, 1.3_

- [x] 3.3 Implement ScheduleTrigger trait
  - Implement next_execution_time for all schedule types
  - Handle end date for recurring jobs
  - Implement is_complete check
  - _Requirements: 1.4, 1.5, 1.6, 1.7_

- [x] 3.4 Write property tests for schedule calculations
  - **Property 1: Cron expression parsing validity**
  - **Property 2: Timezone-aware scheduling**
  - **Property 3: Default timezone application**
  - **Property 4: Fixed delay timing**
  - **Property 5: Fixed rate timing**
  - **Property 6: One-time job completion**
  - **Property 7: End date enforcement**
  - **Validates: Requirements 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7**

- [x] 4. Implement database layer
- [x] 4.1 Set up PostgreSQL connection pool
  - Configure sqlx with compile-time query checking
  - Implement connection pool with min/max connections
  - Add health check queries
  - _Requirements: 12.4_

- [x] 4.2 Implement JobRepository
  - Implement find_jobs_due query
  - Implement CRUD operations for jobs
  - Add job stats tracking
  - _Requirements: 3.11, 7.2, 7.3, 7.4_

- [x] 4.3 Implement ExecutionRepository
  - Implement create_execution and update_execution
  - Implement find_by_idempotency_key for deduplication
  - Add execution history queries with 30-day filter
  - _Requirements: 3.12, 4.3, 6.2_

- [x] 4.4 Implement VariableRepository
  - Implement find_global_variables and find_job_variables
  - Add encryption/decryption for sensitive variables
  - Implement variable CRUD operations
  - _Requirements: 2.1, 2.2, 2.6, 2.7_

- [x] 4.5 Implement UserRepository for database authentication
  - Implement user CRUD operations
  - Implement find_by_username for login
  - Implement role and permission queries
  - _Requirements: 10.2, 10.13_

- [x] 4.6 Write property tests for repositories
  - **Property 8: Global variable availability**
  - **Property 9: Job-specific variable scoping**
  - **Property 11: Variable precedence**
  - **Property 13: Variable update propagation**
  - **Property 14: Sensitive variable encryption**
  - **Property 56: Dynamic job addition**
  - **Property 57: Dynamic job update**
  - **Property 58: Dynamic job deletion**
  - **Validates: Requirements 2.1, 2.2, 2.4, 2.6, 2.7, 7.2, 7.3, 7.4**

- [x] 5. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 6. Implement variable substitution engine
- [x] 6.1 Create template substitution module
  - Implement variable placeholder parsing (e.g., ${VAR_NAME})
  - Implement variable resolution with precedence (job-specific > global)
  - Add error handling for undefined variables
  - _Requirements: 2.3, 2.4, 2.5_

- [x] 6.2 Implement substitution for HTTP jobs
  - Add substitution for URLs
  - Add substitution for headers
  - Add substitution for request body
  - _Requirements: 2.9, 2.10_

- [x] 6.3 Implement substitution for database jobs
  - Add substitution for connection strings
  - Implement parameterized query substitution for SQL injection prevention
  - _Requirements: 2.11, 2.12_

- [x] 6.4 Write property tests for variable substitution
  - **Property 10: Variable resolution**
  - **Property 12: Undefined variable handling**
  - **Property 16: Variable substitution in URLs**
  - **Property 17: Variable substitution in headers and body**
  - **Property 18: Variable substitution in connection strings**
  - **Property 19: Parameterized query substitution**
  - **Validates: Requirements 2.3, 2.5, 2.9, 2.10, 2.11, 2.12**

- [x] 7. Implement distributed locking with Redis
- [x] 7.1 Set up Redis connection pool
  - Configure Redis client with connection manager
  - Add health check
  - _Requirements: 4.1_

- [x] 7.2 Implement RedLock algorithm
  - Implement DistributedLock trait
  - Implement lock acquisition with TTL
  - Implement lock release
  - Add lock extension for long operations
  - _Requirements: 4.1_

- [x] 7.3 Write property tests for distributed locking
  - **Property 29: Distributed lock exclusivity**
  - **Property 55: Single scheduler execution**
  - **Validates: Requirements 4.1, 7.1**

- [x] 8. Implement NATS JetStream queue
- [x] 8.1 Set up NATS JetStream client
  - Configure NATS connection
  - Create job stream with retention policy
  - Add consumer configuration
  - _Requirements: 4.2_

- [x] 8.2 Implement JobPublisher
  - Implement publish method with message serialization
  - Add message deduplication headers
  - Implement error handling and retries
  - _Requirements: 4.2_

- [x] 8.3 Implement JobConsumer
  - Implement message consumption with acknowledgment
  - Add exactly-once processing logic
  - Implement graceful shutdown handling
  - _Requirements: 4.2, 7.7_

- [x] 8.4 Write property tests for queue operations
  - **Property 30: Exactly-once execution**
  - **Property 31: Idempotency key checking**
  - **Property 32: Idempotency key generation**
  - **Validates: Requirements 4.2, 4.3, 4.4**

- [x] 9. Implement Scheduler component
- [x] 9.1 Create scheduler polling loop
  - Implement periodic polling for jobs due
  - Add distributed lock acquisition before scheduling
  - Implement next execution time calculation
  - _Requirements: 7.1_

- [x] 9.2 Implement job publisher integration
  - Publish jobs to NATS queue
  - Update job stats after publishing
  - Handle publishing errors
  - _Requirements: 4.1_

- [x] 9.3 Implement graceful shutdown for scheduler
  - Handle SIGTERM/SIGINT signals
  - Complete in-flight scheduling operations
  - Release locks before exit
  - _Requirements: 7.6_

- [x] 9.4 Create scheduler binary entry point
  - Initialize only scheduler components
  - Set up configuration and logging
  - Start scheduler loop
  - _Requirements: 9.4, 12.3_

- [x] 9.5 Write property tests for scheduler
  - **Property 60: Scheduler graceful shutdown**
  - **Property 73: Scheduler component isolation**
  - **Validates: Requirements 7.6, 9.4**

- [x] 10. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 11. Implement retry strategy and circuit breaker
- [x] 11.1 Implement RetryStrategy trait
  - Implement exponential backoff calculation (5s, 15s, 1m, 5m, 30m, ...)
  - Add jitter to prevent thundering herd
  - Implement max retry limit (10 attempts)
  - _Requirements: 4.5, 4.6_

- [x] 11.2 Implement CircuitBreaker
  - Implement state machine (Closed, Open, HalfOpen)
  - Add failure threshold configuration
  - Implement timeout for open state
  - Add metrics for circuit breaker state
  - _Requirements: 4.7_

- [x] 11.3 Implement Dead Letter Queue handling
  - Move failed jobs to DLQ after max retries
  - Prevent automatic re-execution
  - Add DLQ monitoring
  - _Requirements: 4.8, 4.10_

- [x] 11.4 Write property tests for retry and circuit breaker
  - **Property 33: Retry limit enforcement**
  - **Property 34: Exponential backoff with jitter**
  - **Property 35: Circuit breaker activation**
  - **Property 36: Dead letter queue placement**
  - **Property 38: Dead letter queue isolation**
  - **Validates: Requirements 4.5, 4.6, 4.7, 4.8, 4.10**

- [x] 12. Implement HTTP job executor
- [x] 12.1 Create HTTP executor with reqwest
  - Implement JobExecutor trait for HTTP jobs
  - Support GET, POST, PUT methods
  - Add timeout handling
  - _Requirements: 3.1, 4.9_

- [x] 12.2 Implement HTTP authentication
  - Implement Basic authentication
  - Implement Bearer token authentication
  - Implement OAuth2 token acquisition
  - _Requirements: 3.4, 3.5, 3.6_

- [x] 12.3 Add header and body handling
  - Implement custom header injection
  - Implement request body serialization
  - _Requirements: 3.2, 3.3_

- [x] 12.4 Write property tests for HTTP executor
  - **Property 20: HTTP method correctness**
  - **Property 21: HTTP header inclusion**
  - **Property 22: HTTP body inclusion**
  - **Property 23: Basic authentication formatting**
  - **Property 24: Bearer token formatting**
  - **Property 25: OAuth2 token acquisition**
  - **Validates: Requirements 3.1, 3.2, 3.3, 3.4, 3.5, 3.6**

- [x] 13. Implement database job executor
- [x] 13.1 Implement PostgreSQL executor
  - Create database connection with sqlx
  - Execute raw SQL queries
  - Execute stored procedures
  - Handle query results
  - _Requirements: 3.7, 3.9_

- [x] 13.2 Implement MySQL executor
  - Create database connection with mysql_async
  - Execute queries and stored procedures
  - _Requirements: 3.7, 3.10_

- [x] 13.3 Implement Oracle executor
  - Create database connection with oracle crate
  - Execute queries and stored procedures
  - _Requirements: 3.7, 3.8_

- [x] 13.4 Write property tests for database executor
  - **Property 26: Database query execution**
  - **Validates: Requirements 3.7**

- [x] 14. Implement Worker component
- [x] 14.1 Create worker job consumer
  - Consume jobs from NATS queue
  - Check idempotency before execution
  - Route to appropriate executor (HTTP or Database)
  - _Requirements: 4.2, 4.3_

- [x] 14.2 Integrate retry and circuit breaker
  - Wrap executor calls with retry logic
  - Apply circuit breaker pattern
  - Handle timeout enforcement
  - _Requirements: 4.5, 4.7, 4.9_

- [x] 14.3 Implement execution result recording
  - Update execution status in database
  - Record execution duration
  - Store result or error message
  - Update job stats
  - _Requirements: 3.12_

- [x] 14.4 Implement graceful shutdown for worker
  - Handle SIGTERM/SIGINT signals
  - Complete in-flight executions
  - Acknowledge or nack messages appropriately
  - _Requirements: 7.7_

- [x] 14.5 Create worker binary entry point
  - Initialize only worker components
  - Set up configuration and logging
  - Start worker consumer
  - _Requirements: 9.5, 12.3_

- [x] 14.6 Write property tests for worker
  - **Property 37: Timeout enforcement**
  - **Property 61: Worker graceful shutdown**
  - **Property 74: Worker component isolation**
  - **Validates: Requirements 4.9, 7.7, 9.5**

- [x] 15. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 16. Implement observability layer
- [x] 16.1 Set up structured logging
  - Configure tracing-subscriber with JSON formatting
  - Add trace context to all log entries
  - Set up log levels from configuration
  - _Requirements: 5.1, 5.2, 5.9_

- [x] 16.2 Implement Prometheus metrics
  - Set up metrics-exporter-prometheus
  - Implement job_success_total counter
  - Implement job_failed_total counter
  - Implement job_duration_seconds histogram
  - Implement job_queue_size gauge
  - _Requirements: 5.3, 5.4, 5.5, 5.6_

- [x] 16.3 Implement OpenTelemetry tracing
  - Set up tracing-opentelemetry
  - Create trace spans for job executions
  - Add span attributes (job_id, execution_id, job_type)
  - Configure OTLP exporter
  - _Requirements: 5.7_

- [x] 16.4 Implement alerting logic
  - Track consecutive failures per job
  - Trigger alerts after 3 consecutive failures
  - Implement alert notification interface
  - _Requirements: 5.8_

- [x] 16.5 Write property tests for observability
  - **Property 39: Execution start logging**
  - **Property 40: Execution completion logging**
  - **Property 41: Success metric increment**
  - **Property 42: Failure metric increment**
  - **Property 43: Duration metric recording**
  - **Property 44: Queue size metric**
  - **Property 45: Trace span creation**
  - **Property 46: Consecutive failure alerting**
  - **Property 47: Structured logging format**
  - **Validates: Requirements 5.1, 5.2, 5.3, 5.4, 5.5, 5.6, 5.7, 5.8, 5.9**

- [x] 17. Implement authentication system
- [x] 17.1 Implement JWT token handling
  - Create JWT encoding/decoding with jsonwebtoken
  - Define UserClaims structure
  - Implement token validation
  - _Requirements: 10.4_

- [x] 17.2 Implement database authentication mode
  - Create login endpoint with bcrypt password verification
  - Generate JWT tokens on successful login
  - Implement user CRUD operations
  - _Requirements: 10.2, 10.3, 10.13_

- [x] 17.3 Implement Keycloak authentication mode
  - Fetch Keycloak public keys (JWKS)
  - Validate JWT tokens from Keycloak
  - Cache public keys with TTL
  - Handle Keycloak unavailability
  - _Requirements: 10.1, 10.11, 10.12_

- [x] 17.4 Implement authentication middleware
  - Create Axum middleware for JWT validation
  - Extract user claims from token
  - Handle authentication errors (401)
  - _Requirements: 10.4_

- [x] 17.5 Write property tests for authentication
  - **Property 63: Keycloak JWT validation**
  - **Property 64: Database authentication**
  - **Property 65: Invalid token rejection**
  - **Property 71: Keycloak resilience**
  - **Property 72: Keycloak configuration**
  - **Property 74: Database user storage**
  - **Validates: Requirements 10.1, 10.2, 10.3, 10.4, 10.11, 10.12, 10.13**

- [x] 18. Implement RBAC authorization
- [x] 18.1 Create permission checking middleware
  - Implement RBAC middleware for Axum
  - Check user permissions from JWT claims
  - Return 403 Forbidden for insufficient permissions
  - _Requirements: 10.5, 10.6, 10.7, 10.8, 10.9_

- [x] 18.2 Add audit logging
  - Log user identity for all operations
  - Include operation type and resource ID
  - Add to structured logs
  - _Requirements: 10.10_

- [x] 18.3 Write property tests for authorization
  - **Property 66: Read permission enforcement**
  - **Property 67: Write permission enforcement**
  - **Property 68: Execute permission enforcement**
  - **Property 69: Delete permission enforcement**
  - **Property 70: Execution read permission enforcement**
  - **Property 71: Audit logging with user identity**
  - **Validates: Requirements 10.5, 10.6, 10.7, 10.8, 10.9, 10.10**

- [x] 19. Implement REST API with Axum
- [x] 19.1 Set up Axum router and middleware
  - Create router with all routes
  - Add authentication middleware
  - Add RBAC middleware
  - Add logging middleware
  - Add CORS middleware
  - _Requirements: 6.1_

- [x] 19.2 Implement job management endpoints
  - POST /api/jobs - Create job
  - GET /api/jobs - List jobs with stats
  - GET /api/jobs/:id - Get job details
  - PUT /api/jobs/:id - Update job
  - DELETE /api/jobs/:id - Delete job
  - POST /api/jobs/:id/trigger - Manual trigger
  - PUT /api/jobs/:id/enable - Enable job
  - PUT /api/jobs/:id/disable - Disable job
  - _Requirements: 6.1, 6.4, 6.5, 6.6, 7.2, 7.3, 7.4_

- [x] 19.3 Implement execution history endpoints
  - GET /api/executions - List executions with filters
  - GET /api/executions/:id - Get execution details
  - _Requirements: 6.2, 6.3_

- [x] 19.4 Implement variable management endpoints
  - POST /api/variables - Create variable
  - GET /api/variables - List variables (with masking)
  - PUT /api/variables/:id - Update variable
  - DELETE /api/variables/:id - Delete variable
  - _Requirements: 2.1, 2.2, 2.6, 2.8_

- [x] 19.5 Implement authentication endpoints
  - POST /api/auth/login - Login (database mode)
  - POST /api/auth/refresh - Refresh token
  - POST /api/users - Create user (database mode)
  - _Requirements: 10.2, 10.3_

- [x] 19.6 Write property tests for API endpoints
  - **Property 48: Job listing completeness**
  - **Property 49: Execution history time window**
  - **Property 50: Execution history filtering**
  - **Property 51: Manual trigger queueing**
  - **Property 52: Job disable effect**
  - **Property 53: Job enable effect**
  - **Property 15: Sensitive variable masking**
  - **Validates: Requirements 6.1, 6.2, 6.3, 6.4, 6.5, 6.6, 2.8**

- [x] 20. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 21. Implement HTMX dashboard
- [x] 21.1 Create HTML templates with Tera
  - Create base layout template
  - Create jobs list template
  - Create job details template
  - Create execution history template
  - Create variables management template
  - Add responsive CSS (mobile-friendly)
  - _Requirements: 6.1, 6.8, 6.9_

- [x] 21.2 Implement HTMX endpoints
  - GET /dashboard - Main dashboard page
  - GET /dashboard/jobs - Jobs list partial
  - GET /dashboard/jobs/:id - Job details partial
  - GET /dashboard/executions - Execution history partial
  - GET /dashboard/variables - Variables partial
  - _Requirements: 6.8_

- [x] 21.3 Implement Server-Sent Events
  - Create SSE endpoint for real-time updates
  - Push job status changes to connected clients
  - Push execution updates
  - Handle client disconnections
  - _Requirements: 6.7_

- [x] 21.4 Implement visual job builder UI
  - Create job creation/edit form template (job_form.html)
  - Add step-by-step wizard for multi-step jobs
  - Add form validation (client-side and server-side)
  - Support all job types (HTTP, Database, FileProcessing, SFTP)
  - Support all schedule types (Cron, FixedDelay, FixedRate, OneTime)
  - Support trigger configuration (scheduled, manual, webhook)
  - Generate JSON job definition from form inputs
  - Add preview of generated JSON before submission
  - Wire up "Create Job" button in jobs.html to open the form
  - _Requirements: 18.1, 18.2_

- [x] 21.5 Write property tests for real-time updates
  - **Property 54: Real-time status updates**
  - **Validates: Requirements 6.7**

- [x] 22. Implement deployment artifacts
- [x] 22.1 Create Dockerfile with multi-stage build
  - Stage 1: Build with Rust alpine
  - Stage 2: Runtime with minimal alpine
  - Optimize for <50MB final image
  - _Requirements: 9.1_

- [x] 22.2 Create docker-compose.yml
  - Add PostgreSQL service
  - Add Redis service
  - Add NATS service
  - Add scheduler service
  - Add worker service
  - Add API service
  - Configure networking and volumes
  - _Requirements: 9.2_

- [x] 22.3 Create Helm chart
  - Create Chart.yaml and values.yaml
  - Create deployment templates for scheduler, worker, API
  - Create StatefulSet for PostgreSQL
  - Create Redis cluster configuration
  - Create NATS cluster configuration
  - Create ConfigMap and Secrets
  - Create Service and Ingress
  - Create RBAC resources
  - Create HorizontalPodAutoscaler
  - _Requirements: 9.3_

- [x] 22.4 Write property tests for deployment
  - **Property 75: Database migration execution**
  - **Validates: Requirements 12.6**

- [x] 23. Create Vietnamese documentation
- [x] 23.1 Write README.md in Vietnamese
  - Add project overview
  - Add architecture diagram
  - Add setup instructions
  - Add configuration guide
  - Add deployment guide (Docker, Kubernetes)
  - Add API documentation
  - Add troubleshooting section
  - _Requirements: 11.1, 11.2_

- [x] 23.2 Create example configurations
  - Add example config.toml
  - Add example docker-compose.yml
  - Add example Helm values.yaml
  - Add example job definitions
  - _Requirements: 11.2_

- [x] 24. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 25. Implement MinIO storage integration
- [x] 25.1 Set up MinIO client and connection
  - Configure rust-s3 client for MinIO
  - Implement connection pooling
  - Add health check
  - _Requirements: 13.2_

- [x] 25.2 Implement MinIOService trait
  - Implement store_job_definition method
  - Implement load_job_definition method
  - Implement store_context method
  - Implement load_context method
  - Implement store_file and load_file methods
  - _Requirements: 13.2, 13.3, 13.7_

- [x] 25.3 Write property tests for MinIO operations
  - **Property 77: MinIO job definition persistence**
  - **Property 78: MinIO path format for job definitions**
  - **Property 82: Job Context persistence to MinIO**
  - **Property 83: Job Context path format**
  - **Validates: Requirements 13.2, 13.3, 13.7**

- [x] 26. Implement Job Context management
- [x] 26.1 Create JobContext data structure
  - Define JobContext struct with all fields
  - Implement serialization/deserialization
  - Add helper methods for accessing step outputs
  - _Requirements: 13.5, 13.6, 13.7_

- [x] 26.2 Implement ContextManager trait
  - Implement load_context from MinIO
  - Implement save_context to MinIO
  - Add context initialization for new executions
  - Handle context updates after each step
  - _Requirements: 13.7, 13.8_

- [x] 26.3 Write property tests for Job Context
  - **Property 80: HTTP response storage in Job Context**
  - **Property 81: Database result storage in Job Context**
  - **Property 84: Job Context loading for subsequent steps**
  - **Property 85: Job Context retention after completion**
  - **Property 86: Job Context preservation on failure**
  - **Property 93: Automatic step output storage**
  - **Validates: Requirements 13.5, 13.6, 13.8, 13.9, 13.10, 14.5**

- [x] 27. Implement reference resolver for variables and step outputs
- [x] 27.1 Create ReferenceResolver trait implementation
  - Implement template parsing for {{variable}} syntax
  - Implement step output reference resolution ({{steps.step1.output}})
  - Implement JSONPath support for nested data access
  - Add error handling for invalid references
  - _Requirements: 14.1, 14.2, 14.4_

- [x] 27.2 Integrate reference resolver with executors
  - Resolve references in HTTP URLs, headers, body
  - Resolve references in database connection strings and queries
  - Resolve references in file paths
  - Resolve references in SFTP paths
  - _Requirements: 14.1, 14.6_

- [x] 27.3 Write property tests for reference resolution
  - **Property 89: Step output reference resolution**
  - **Property 90: Template reference extraction**
  - **Property 91: Invalid reference error handling**
  - **Property 92: JSONPath nested value access**
  - **Property 94: Conditional logic evaluation**
  - **Property 95: Missing data reference error**
  - **Validates: Requirements 14.1, 14.2, 14.3, 14.4, 14.6, 14.7**

- [x] 28. Implement multi-step job execution
- [x] 28.1 Update Worker to support multi-step jobs
  - Load job definition from MinIO
  - Initialize Job Context for new executions
  - Execute steps sequentially
  - Update Job Context after each step
  - Persist Job Context to MinIO after each step
  - _Requirements: 13.4, 13.7, 13.8_

- [x] 28.2 Update Job model to support steps
  - Add steps field to Job struct
  - Update job creation/update endpoints
  - Update database schema
  - _Requirements: 13.1, 13.4_

- [x] 28.3 Write property tests for multi-step execution
  - **Property 76: JSON job definition acceptance**
  - **Property 79: Sequential step execution**
  - **Property 87: Job Context reference in execution details**
  - **Property 88: Database stores only MinIO path references**
  - **Validates: Requirements 13.1, 13.4, 13.11, 13.12**

- [x] 29. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 30. Implement file processing executor
- [x] 30.1 Implement Excel file processing
  - Use calamine for reading XLSX files
  - Use rust_xlsxwriter for writing XLSX files
  - Parse all sheets to structured JSON
  - Support sheet selection by name or index
  - _Requirements: 15.1, 15.2, 15.5, 15.7_

- [x] 30.2 Implement CSV file processing
  - Use csv crate for reading/writing CSV
  - Support configurable delimiters (comma, semicolon, tab)
  - Parse rows to structured JSON
  - _Requirements: 15.3, 15.4, 15.8_

- [x] 30.3 Implement data transformations
  - Implement column mapping
  - Implement data type conversion
  - Implement filtering
  - _Requirements: 15.6_

- [x] 30.4 Implement streaming for large files
  - Add streaming support for files >100MB
  - Implement chunked processing
  - _Requirements: 15.12_

- [x] 30.5 Integrate file processor with Worker
  - Store processed files in MinIO
  - Store file metadata in Job Context
  - Handle file processing errors
  - _Requirements: 15.9, 15.10, 15.11_

- [x] 30.6 Write property tests for file processing
  - **Property 96: Excel file reading**
  - **Property 97: Excel data structure preservation**
  - **Property 98: CSV file reading**
  - **Property 99: CSV delimiter support**
  - **Property 100: Excel sheet selection**
  - **Property 101: Data transformation application**
  - **Property 102: Excel write round-trip**
  - **Property 103: CSV write round-trip**
  - **Property 104: File output path format**
  - **Property 105: File metadata storage**
  - **Property 106: Invalid file format error handling**
  - **Validates: Requirements 15.1-15.11**

- [x] 31. Implement SFTP executor
- [x] 31.1 Implement SFTP client with ssh2
  - Implement SFTP connection with password auth
  - Implement SFTP connection with SSH key auth
  - Implement host key verification
  - _Requirements: 19.3, 19.4, 19.16_

- [x] 31.2 Implement SFTP download operations
  - Implement single file download
  - Implement wildcard pattern matching
  - Implement recursive directory download
  - Store downloaded files in MinIO
  - _Requirements: 19.1, 19.5, 19.6, 19.13_

- [x] 31.3 Implement SFTP upload operations
  - Implement single file upload
  - Implement directory creation
  - Read files from MinIO for upload
  - _Requirements: 19.2, 19.7, 19.14_

- [x] 31.4 Implement streaming for large files
  - Add streaming support for files >100MB
  - Avoid loading entire files into memory
  - _Requirements: 19.17_

- [x] 31.5 Implement SFTP error handling
  - Handle authentication errors (no retry)
  - Handle file not found errors (no retry)
  - Handle connection errors (with retry)
  - _Requirements: 19.10, 19.11, 19.12_

- [x] 31.6 Integrate SFTP with Job Context
  - Store file metadata in Job Context
  - Resolve file path references from previous steps
  - _Requirements: 19.8, 19.9, 19.15_

- [x] 31.7 Write property tests for SFTP operations
  - **Property 137: SFTP download to MinIO**
  - **Property 138: SFTP upload from MinIO**
  - **Property 139: SFTP password authentication**
  - **Property 140: SFTP key-based authentication**
  - **Property 141: SFTP wildcard pattern matching**
  - **Property 142: SFTP download path format**
  - **Property 143: SFTP upload round-trip**
  - **Property 144: SFTP download metadata storage**
  - **Property 145: SFTP upload metadata storage**
  - **Property 146: SFTP authentication error no-retry**
  - **Property 147: SFTP file not found no-retry**
  - **Property 148: SFTP recursive directory download**
  - **Property 149: SFTP remote directory creation**
  - **Property 150: SFTP file path reference resolution**
  - **Property 151: SFTP host key verification**
  - **Validates: Requirements 19.1-19.16**

- [x] 32. Checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 33. Implement webhook trigger system
- [x] 33.1 Create webhook database schema
  - Create webhooks table
  - Add webhook configuration to jobs
  - _Requirements: 16.1_

- [x] 33.2 Implement webhook URL generation
  - Generate unique webhook URLs for jobs
  - Store webhook secret keys
  - Implement webhook URL regeneration
  - _Requirements: 16.1, 16.12_

- [x] 33.3 Implement webhook handler
  - Create webhook POST endpoint
  - Validate HMAC-SHA256 signatures
  - Store webhook payload/headers/params in Job Context
  - Queue job execution with webhook data
  - _Requirements: 16.2, 16.3, 16.4, 16.5, 16.7_

- [x] 33.4 Implement webhook rate limiting
  - Use Redis for rate limit tracking
  - Enforce per-job rate limits
  - Return 429 for rate limit violations
  - _Requirements: 16.11_

- [x] 33.5 Implement webhook validation and error handling
  - Reject invalid signatures with 401
  - Reject disabled job webhooks with 403
  - Return 202 with execution_id on success
  - _Requirements: 16.8, 16.9, 16.10_

- [x] 33.6 Write property tests for webhooks
  - **Property 107: Unique webhook URL generation**
  - **Property 108: Webhook POST queueing**
  - **Property 109: Webhook payload storage**
  - **Property 110: Webhook query parameters storage**
  - **Property 111: Webhook headers storage**
  - **Property 112: Webhook data reference resolution**
  - **Property 113: Webhook signature validation**
  - **Property 114: Invalid webhook signature rejection**
  - **Property 115: Successful webhook response**
  - **Property 116: Disabled job webhook rejection**
  - **Property 117: Webhook rate limiting**
  - **Property 118: Webhook URL invalidation**
  - **Validates: Requirements 16.1-16.12**

- [x] 34. Implement multiple trigger methods
- [x] 34.1 Update Job model with trigger configuration
  - Add TriggerConfig to Job struct
  - Support scheduled, manual, and webhook triggers
  - Update job creation/update endpoints
  - _Requirements: 17.1, 17.2_

- [x] 34.2 Update Scheduler to respect trigger configuration
  - Skip jobs with manual-only trigger
  - Process jobs with scheduled trigger
  - _Requirements: 17.1, 17.2_

- [x] 34.3 Update JobExecution to track trigger source
  - Add trigger_source field
  - Record trigger source for all executions
  - Display trigger source in execution history
  - _Requirements: 17.6, 17.7, 17.8_

- [x] 34.4 Implement concurrent execution control
  - Check for running executions before queueing
  - Allow or reject based on allow_concurrent flag
  - _Requirements: 17.9, 17.10_

- [x] 34.5 Write property tests for trigger methods
  - **Property 119: Manual-only job non-scheduling**
  - **Property 120: Dashboard manual trigger queueing**
  - **Property 121: Trigger source recording**
  - **Property 122: Unique execution ID generation**
  - **Property 123: Trigger source display**
  - **Property 124: Concurrent execution allowance**
  - **Property 125: Concurrent execution prevention**
  - **Validates: Requirements 17.2, 17.3, 17.6, 17.7, 17.8, 17.9, 17.10**

- [x] 35. Implement job import/export functionality
- [x] 35.1 Implement job export
  - Load job definition from MinIO
  - Mask sensitive data (passwords, API keys)
  - Add export metadata (date, user, version)
  - Generate filename with timestamp
  - Support single and bulk export
  - _Requirements: 18.3, 18.4, 18.5, 18.12, 18.14_

- [x] 35.2 Implement job import
  - Validate JSON schema
  - Prompt for sensitive data values
  - Handle duplicate job names
  - Store job definition to MinIO
  - Create job record in database
  - Support single and bulk import
  - _Requirements: 18.7, 18.8, 18.9, 18.10, 18.11, 18.13_

- [x] 35.3 Create import/export API endpoints
  - POST /api/jobs/export - Export single job
  - POST /api/jobs/export/bulk - Export multiple jobs
  - POST /api/jobs/import - Import single job
  - POST /api/jobs/import/bulk - Import multiple jobs
  - _Requirements: 18.3, 18.6, 18.9_

- [x] 35.4 Update dashboard with import/export UI
  - Add export button to job details page
  - Add import button to jobs list page
  - Add file upload interface
  - Add sensitive data input form
  - _Requirements: 18.1, 18.6, 18.10_

- [x] 35.5 Write property tests for import/export
  - **Property 126: Visual job creation JSON generation**
  - **Property 127: Export filename format**
  - **Property 128: Export completeness**
  - **Property 129: Sensitive data masking on export**
  - **Property 130: Import JSON schema validation**
  - **Property 131: Invalid JSON error messages**
  - **Property 132: Import round-trip**
  - **Property 133: Duplicate name handling**
  - **Property 134: Bulk export format**
  - **Property 135: Bulk import processing**
  - **Property 136: Export metadata inclusion**
  - **Validates: Requirements 18.2-18.14**

- [x] 36. Final checkpoint - Ensure all tests pass
  - Ensure all tests pass, ask the user if questions arise.

- [x] 37. Update deployment artifacts for new features
- [x] 37.1 Update docker-compose.yml
  - Add MinIO service
  - Configure MinIO environment variables
  - Add volume mounts for MinIO data
  - _Requirements: 9.2_

- [x] 37.2 Update Helm chart
  - Add MinIO StatefulSet or use external MinIO
  - Add MinIO configuration to ConfigMap
  - Update worker deployment with new executors
  - _Requirements: 9.3_

- [x] 37.3 Update configuration examples
  - Add MinIO configuration section
  - Add webhook configuration examples
  - Add multi-step job examples
  - Add file processing job examples
  - Add SFTP job examples
  - _Requirements: 11.2_

- [x] 38. Update Vietnamese documentation
- [x] 38.1 Update README.md
  - Document multi-step job feature
  - Document webhook triggers
  - Document file processing capabilities
  - Document SFTP operations
  - Document job import/export
  - Add MinIO setup instructions
  - _Requirements: 11.1, 11.2_

- [x] 38.2 Create example job definitions
  - Create multi-step HTTP + Database job example
  - Create file processing job example
  - Create SFTP job example
  - Create webhook-triggered job example
  - _Requirements: 11.2_

- [x] 39. Final integration testing
- [x] 39.1 Test end-to-end multi-step job execution
  - Create job with multiple steps
  - Verify sequential execution
  - Verify Job Context persistence
  - Verify step output references work
  - _Requirements: 13.4, 13.8, 14.1_

- [x] 39.2 Test webhook trigger flow
  - Configure webhook for a job
  - Send webhook request
  - Verify signature validation
  - Verify job execution with webhook data
  - _Requirements: 16.2, 16.7, 16.9_

- [x] 39.3 Test file processing flow
  - Upload Excel/CSV files to MinIO
  - Execute file processing job
  - Verify data parsing and transformation
  - Verify output file generation
  - _Requirements: 15.1, 15.3, 15.6, 15.7_

- [x] 39.4 Test SFTP operations
  - Configure SFTP job
  - Test download with wildcard patterns
  - Test upload with directory creation
  - Verify files in MinIO
  - _Requirements: 19.1, 19.2, 19.5, 19.14_

- [x] 39.5 Test job import/export
  - Export a complex multi-step job
  - Verify sensitive data masking
  - Import the job
  - Verify job works correctly
  - _Requirements: 18.4, 18.5, 18.9_

- [x] 40. Final checkpoint - All features complete
  - Ensure all tests pass, ask the user if questions arise.
