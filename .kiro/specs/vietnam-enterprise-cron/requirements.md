# Requirements Document

## Introduction

The Vietnam Enterprise Cron System is a production-ready, enterprise-grade scheduled job and cron job management system built in Rust. This system is designed to replace existing Java Quartz + Spring Batch implementations in Vietnamese enterprises (banking, telco, e-commerce sectors). The system provides distributed job scheduling with exactly-once execution guarantees, comprehensive observability, and a real-time administrative dashboard. 

Jobs are defined as JSON documents and stored in MinIO object storage, supporting complex multi-step workflows where each step can perform different types of operations. Each job execution maintains its own context object in MinIO, allowing steps to pass data between each other and enabling sophisticated data processing pipelines.

**Supported Job Types:**
1. **File Processing Jobs**: Read and write Excel (XLSX) and CSV files with data transformation capabilities
2. **Database Jobs**: Execute CRUD operations across Oracle 19c, PostgreSQL, and MySQL databases
3. **API Integration Jobs**: Send and receive data from external HTTP/REST APIs with various authentication methods
4. **SFTP Jobs**: Connect to SFTP servers to download or upload files for secure file transfer operations

**Trigger Methods:**
1. **Scheduled Triggers**: Automatic execution based on cron expressions, fixed delay, or fixed rate configurations
2. **Manual Triggers**: On-demand execution through dashboard or API by authorized users
3. **Webhook Triggers**: Event-driven execution triggered by external systems via HTTP webhooks

The system supports full timezone support for Vietnamese operations (default: Asia/Ho_Chi_Minh) and provides comprehensive data passing between job steps through the Job Context mechanism.

## Glossary

- **Scheduler**: The component responsible for detecting when jobs should execute and publishing them to the job queue
- **Worker**: The component that consumes jobs from the queue and executes them
- **Job**: A scheduled task definition with execution parameters and schedule configuration stored in the System Database
- **JobExecution**: A single execution instance of a Job with status, timing, and result information stored in the System Database
- **System Database**: The PostgreSQL database used by the system to store job definitions, execution history, and metadata
- **Target Database**: An external database (PostgreSQL, Oracle 19c, or MySQL) that a database job connects to for executing business queries
- **Distributed Lock**: A Redis-based RedLock mechanism ensuring only one scheduler node triggers a job
- **Idempotency Key**: A unique identifier ensuring duplicate job executions produce the same result
- **Dead Letter Queue**: A storage mechanism for jobs that have exhausted all retry attempts
- **Circuit Breaker**: A pattern that prevents execution attempts when external systems are unavailable
- **HTMX**: A frontend library enabling dynamic UI updates without full page reloads
- **Server-Sent Events**: A unidirectional communication protocol for real-time server-to-client updates
- **Cron Expression**: A time-based scheduling pattern using Quartz syntax with second precision
- **Fixed Delay**: A scheduling pattern where the next execution starts X time after the previous execution completes
- **Fixed Rate**: A scheduling pattern where executions start at fixed intervals regardless of completion time
- **Graceful Shutdown**: A process termination pattern that completes in-flight work before stopping
- **Keycloak**: An optional open-source identity and access management system providing authentication and authorization
- **RBAC**: Role-Based Access Control, a method of regulating access based on user roles
- **JWT**: JSON Web Token, a compact token format used for authentication and authorization claims
- **Authentication Mode**: The configured authentication strategy, either "keycloak" for external identity provider or "database" for local user management
- **User**: An entity stored in the System Database with credentials and role assignments (used in database authentication mode)
- **Job Variable**: A key-value pair that can be referenced by jobs during execution, supporting templating and dynamic values
- **Global Variable**: A variable available to all jobs in the system
- **Job-Specific Variable**: A variable scoped to a particular job, overriding global variables with the same name
- **MinIO**: An S3-compatible object storage system used to store job definitions and job execution data
- **Job Definition**: A JSON document stored in MinIO that defines a job's schedule, steps, and configuration
- **Job Step**: An individual unit of work within a job that performs a specific action (HTTP request, database query, or file processing)
- **Job Context**: A data object owned by a job execution that stores intermediate results from each step, persisted to MinIO
- **Job Export**: The process of downloading a job definition as a JSON file for backup, sharing, or version control
- **Job Import**: The process of uploading and creating a job from a JSON file definition
- **Visual Job Builder**: A dashboard UI component that provides a form-based interface for creating and editing jobs without writing JSON manually
- **Step Output**: The data returned from executing a step (API response, database query result, or processed file data) that is stored in the Job Context
- **File Processing Job**: A job type that reads, processes, and writes files in formats such as Excel (XLSX), CSV, or other structured data formats
- **Webhook Trigger**: An HTTP endpoint that external systems can call to trigger job execution on-demand
- **Manual Trigger**: A user-initiated job execution through the dashboard or API
- **Scheduled Trigger**: An automatic job execution based on cron expression, fixed delay, or fixed rate configuration
- **SFTP**: SSH File Transfer Protocol, a secure network protocol for transferring files between systems
- **SFTP Job**: A job type that connects to SFTP servers to download or upload files using SSH authentication
- **SFTP Server**: A remote server that supports SFTP protocol for secure file transfer operations
## Requirements

### Requirement 1

**User Story:** As a system administrator, I want to define jobs with flexible scheduling options, so that I can automate business processes according to Vietnamese enterprise requirements.

#### Acceptance Criteria

1. WHEN a user creates a job with a cron expression, THE Scheduler SHALL parse the expression using Quartz syntax with second precision
2. WHEN a user specifies a timezone for a job, THE Scheduler SHALL evaluate the cron expression in that timezone using chrono_tz
3. WHERE no timezone is specified, THE Scheduler SHALL default to Asia/Ho_Chi_Minh timezone
4. WHEN a user creates a fixed delay job, THE Scheduler SHALL schedule the next execution X seconds after the previous execution completes
5. WHEN a user creates a fixed rate job, THE Scheduler SHALL schedule executions at fixed intervals regardless of execution duration
6. WHEN a user creates a one-time job with a specific datetime, THE Scheduler SHALL execute the job once at that datetime and mark it complete
7. WHEN a user creates a recurring job with an end date, THE Scheduler SHALL stop scheduling executions after the end date is reached

### Requirement 2

**User Story:** As a system administrator, I want to manage variables that jobs can reference, so that I can configure job behavior without modifying job definitions.

#### Acceptance Criteria

1. WHEN a user creates a global variable, THE System SHALL store it in the System Database and make it available to all jobs
2. WHEN a user creates a job-specific variable, THE System SHALL associate it with that job and make it available only to that job
3. WHEN a job references a variable, THE Worker SHALL resolve the variable value before executing the job
4. WHEN a job-specific variable has the same name as a global variable, THE Worker SHALL use the job-specific variable value
5. WHEN a job references a non-existent variable, THE Worker SHALL fail the execution with a clear error message
6. WHEN a user updates a variable value, THE System SHALL apply the new value to subsequent job executions
7. WHEN a variable value contains sensitive data, THE System SHALL encrypt it at rest in the System Database
8. WHEN displaying variables in the dashboard, THE System SHALL mask sensitive variable values
9. WHEN a job uses variables in HTTP request URLs, THE Worker SHALL substitute variable placeholders with actual values
10. WHEN a job uses variables in HTTP request headers or body, THE Worker SHALL substitute variable placeholders with actual values
11. WHEN a job uses variables in database connection strings, THE Worker SHALL substitute variable placeholders with actual values
12. WHEN a job uses variables in SQL queries, THE Worker SHALL substitute variable placeholders with actual values using parameterized queries to prevent SQL injection

### Requirement 3

**User Story:** As a system administrator, I want to execute different types of jobs, so that I can integrate with various enterprise systems.

#### Acceptance Criteria

1. WHEN a job type is HTTP Request, THE Worker SHALL send HTTP requests with the specified method (GET, POST, PUT)
2. WHEN an HTTP job includes headers, THE Worker SHALL include those headers in the request
3. WHEN an HTTP job includes a request body, THE Worker SHALL send the body with the request
4. WHEN an HTTP job specifies Basic authentication, THE Worker SHALL include Basic auth credentials in the Authorization header
5. WHEN an HTTP job specifies Bearer token authentication, THE Worker SHALL include the Bearer token in the Authorization header
6. WHEN an HTTP job specifies OAuth2 authentication, THE Worker SHALL obtain and include a valid OAuth2 token
7. WHEN a job type is Database Query, THE Worker SHALL connect to the specified Target Database and execute the SQL query or stored procedure
8. WHEN a database job targets Oracle 19c, THE Worker SHALL connect to the Target Database using Oracle-compatible drivers and execute the query
9. WHEN a database job targets PostgreSQL, THE Worker SHALL connect to the Target Database using PostgreSQL drivers and execute the query
10. WHEN a database job targets MySQL, THE Worker SHALL connect to the Target Database using MySQL drivers and execute the query
11. WHEN storing job definitions, THE System SHALL persist them to the System Database using PostgreSQL
12. WHEN storing execution history, THE System SHALL persist it to the System Database using PostgreSQL

### Requirement 4

**User Story:** As a platform engineer, I want zero duplicate job executions in a distributed environment, so that business operations remain consistent and correct.

#### Acceptance Criteria

1. WHEN multiple Scheduler nodes detect a job is due, THE Scheduler SHALL use Redis RedLock algorithm to ensure only one node publishes the job
2. WHEN a Worker receives a job from the queue, THE Worker SHALL process it exactly once even if multiple workers consume the same message
3. WHEN a job execution includes an idempotency key, THE Worker SHALL check for previous executions with that key before processing
4. WHERE no idempotency key is provided, THE Worker SHALL generate a unique idempotency key for the execution
5. WHEN a job execution fails, THE Worker SHALL retry up to 10 times with exponential backoff and jitter
6. WHEN calculating retry delay, THE Worker SHALL use the sequence 5s, 15s, 1m, 5m, 30m with exponential growth and random jitter
7. WHEN an external system is detected as unavailable, THE Worker SHALL activate a circuit breaker to fail fast without attempting execution
8. WHEN a job exhausts all retry attempts, THE Worker SHALL move the job to the Dead Letter Queue
9. WHEN a job execution exceeds the configured timeout, THE Worker SHALL terminate the execution and mark it as failed
10. WHEN a job is in the Dead Letter Queue, THE System SHALL prevent automatic re-execution until manual intervention

### Requirement 5

**User Story:** As a DevOps engineer, I want comprehensive observability and alerting, so that I can monitor system health and respond to issues quickly.

#### Acceptance Criteria

1. WHEN a job execution starts, THE System SHALL create a structured log entry with job_id, execution_id, and timestamp
2. WHEN a job execution completes, THE System SHALL log the duration and final status
3. WHEN a job succeeds, THE System SHALL increment the job_success_total Prometheus counter
4. WHEN a job fails, THE System SHALL increment the job_failed_total Prometheus counter
5. WHEN a job completes, THE System SHALL record the duration in the job_duration_seconds Prometheus histogram
6. WHEN jobs are queued, THE System SHALL expose the current queue size via the job_queue_size Prometheus gauge
7. WHEN a job execution starts, THE System SHALL create an OpenTelemetry trace span for the entire execution
8. WHEN a job fails 3 consecutive times, THE System SHALL trigger an alert notification
9. WHEN logging occurs, THE System SHALL use structured logging with tracing crate integration

### Requirement 6

**User Story:** As a system administrator, I want a real-time dashboard to manage and monitor jobs, so that I can operate the system efficiently without command-line tools.

#### Acceptance Criteria

1. WHEN a user accesses the dashboard, THE System SHALL display all jobs with their current status, next run time, last run time, and success rate
2. WHEN a user requests execution history, THE System SHALL display executions from the last 30 days
3. WHEN viewing execution history, THE System SHALL allow filtering by status and job identifier
4. WHEN a user triggers a job manually, THE System SHALL immediately queue the job for execution
5. WHEN a user disables a job, THE Scheduler SHALL stop scheduling future executions of that job
6. WHEN a user enables a previously disabled job, THE Scheduler SHALL resume scheduling executions
7. WHEN job status changes occur, THE System SHALL push updates to connected clients using Server-Sent Events
8. WHEN the dashboard renders, THE System SHALL use HTMX for dynamic updates without full page reloads
9. WHEN a user accesses the dashboard from a mobile device, THE System SHALL display a responsive interface optimized for the screen size

### Requirement 7

**User Story:** As a platform architect, I want high availability and dynamic configuration, so that the system can scale horizontally and adapt without downtime.

#### Acceptance Criteria

1. WHEN 100 Scheduler nodes are running, THE System SHALL ensure only one node executes the scheduling logic for each job
2. WHEN a user adds a new job, THE System SHALL store it in the database and make it available for scheduling without restarting any nodes
3. WHEN a user edits a job, THE System SHALL update the database and apply changes to future executions without restarting any nodes
4. WHEN a user deletes a job, THE System SHALL remove it from the database and stop scheduling it without restarting any nodes
5. WHEN configuration changes are detected, THE System SHALL reload the configuration without requiring a restart
6. WHEN a SIGTERM or SIGINT signal is received, THE Scheduler SHALL complete in-flight scheduling operations before terminating
7. WHEN a SIGTERM or SIGINT signal is received, THE Worker SHALL complete in-flight job executions before terminating

### Requirement 8

**User Story:** As a security engineer, I want proper error handling and no unsafe operations, so that the system is reliable and secure in production.

#### Acceptance Criteria

1. WHEN an error occurs in production code, THE System SHALL use the ? operator or explicit error handling instead of unwrap()
2. WHEN an unrecoverable error occurs, THE System SHALL log the error with full context and return a Result type
3. WHEN handling errors, THE System SHALL use thiserror for domain errors and anyhow for application errors

### Requirement 9

**User Story:** As a DevOps engineer, I want containerized deployment with infrastructure as code, so that I can deploy the system consistently across environments.

#### Acceptance Criteria

1. WHEN building the Docker image, THE System SHALL use multi-stage builds to produce a final image smaller than 50MB
2. WHEN deploying with docker-compose, THE System SHALL include PostgreSQL, Redis, and NATS services
3. WHEN deploying to Kubernetes, THE System SHALL provide Helm chart templates for all components
4. WHEN the Scheduler binary starts, THE System SHALL initialize only scheduler-specific components
5. WHEN the Worker binary starts, THE System SHALL initialize only worker-specific components

### Requirement 10

**User Story:** As a security administrator, I want flexible authentication options with RBAC authorization, so that I can control access to the system based on user roles using either Keycloak or database-based authentication.

#### Acceptance Criteria

1. WHEN authentication mode is configured as "keycloak", THE System SHALL validate JWT tokens issued by Keycloak
2. WHEN authentication mode is configured as "database", THE System SHALL validate credentials against users stored in the System Database
3. WHEN a user authenticates with database mode, THE System SHALL generate a JWT token for subsequent requests
4. WHEN a JWT token is invalid or expired, THE System SHALL reject the request with 401 Unauthorized status
5. WHEN a user attempts to view jobs, THE System SHALL verify the user has the "job:read" permission
6. WHEN a user attempts to create or edit jobs, THE System SHALL verify the user has the "job:write" permission
7. WHEN a user attempts to manually trigger a job, THE System SHALL verify the user has the "job:execute" permission
8. WHEN a user attempts to delete jobs, THE System SHALL verify the user has the "job:delete" permission
9. WHEN a user attempts to view execution history, THE System SHALL verify the user has the "execution:read" permission
10. WHEN extracting user information from JWT, THE System SHALL log the user identity with each operation for audit purposes
11. WHERE Keycloak mode is enabled and Keycloak is unavailable, THE System SHALL cache the last known public keys and continue validating tokens
12. WHERE Keycloak mode is enabled, THE System SHALL support configuring the Keycloak realm, client ID, and server URL
13. WHERE database mode is enabled, THE System SHALL store user credentials with bcrypt hashing and role assignments in the System Database

### Requirement 11

**User Story:** As a Vietnamese enterprise user, I want documentation in Vietnamese, so that local teams can operate the system effectively.

#### Acceptance Criteria

1. WHEN a user reads the README.md file, THE System SHALL provide complete setup instructions in Vietnamese language
2. WHEN a user needs configuration guidance, THE README.md SHALL document all configuration options in Vietnamese

### Requirement 12

**User Story:** As a developer, I want a well-organized codebase following Vietnamese enterprise standards, so that the system is maintainable and extensible.

#### Acceptance Criteria

1. WHEN examining the project structure, THE System SHALL organize code into config, errors, models, scheduler, worker, api, db, queue, telemetry, and web modules
2. WHEN the main.rs file is reviewed, THE System SHALL contain only application wiring and initialization logic
3. WHEN separate binaries are needed, THE System SHALL provide scheduler.rs and worker.rs in the bin directory
4. WHEN executing queries against the System Database, THE System SHALL use sqlx with compile-time query checking
5. WHEN connecting to Target Databases, THE System SHALL support runtime query execution for Oracle, PostgreSQL, and MySQL compatibility
6. WHEN initializing the System Database, THE System SHALL run migrations to create tables for jobs, executions, and system metadata

### Requirement 13

**User Story:** As a system administrator, I want to define jobs as JSON documents with multiple steps stored in MinIO, so that I can create complex workflows with data passing between steps.

#### Acceptance Criteria

1. WHEN a user creates a job, THE System SHALL accept a JSON job definition document
2. WHEN a job definition is provided, THE System SHALL store it in MinIO object storage
3. WHEN a job definition is stored in MinIO, THE System SHALL use the path format: `jobs/{job_id}/definition.json`
4. WHEN a job definition contains multiple steps, THE System SHALL execute steps sequentially in the order defined
5. WHEN a step executes an HTTP request, THE Worker SHALL store the API response in the Job Context object
6. WHEN a step executes a database query, THE Worker SHALL store the query result set in the Job Context object
7. WHEN a step completes, THE Worker SHALL persist the updated Job Context to MinIO at path: `jobs/{job_id}/executions/{execution_id}/context.json`
8. WHEN a subsequent step executes, THE Worker SHALL load the Job Context from MinIO to access previous step outputs
9. WHEN a job execution completes, THE System SHALL retain the final Job Context in MinIO for audit and debugging purposes
10. WHEN a job execution fails, THE System SHALL preserve the Job Context up to the point of failure in MinIO
11. WHEN a user queries job execution details, THE System SHALL provide a reference to the Job Context object in MinIO
12. WHEN the System Database stores job metadata, THE System SHALL store only the MinIO object path reference, not the full job definition or context data

### Requirement 14

**User Story:** As a system administrator, I want steps within a job to reference outputs from previous steps, so that I can build complex data processing workflows.

#### Acceptance Criteria

1. WHEN a step references a previous step's output, THE Worker SHALL resolve the reference from the Job Context
2. WHEN a step uses a reference like `{{steps.step1.response.data.id}}`, THE Worker SHALL extract the value from the previous step's output stored in Job Context
3. WHEN a step reference is invalid or the path does not exist, THE Worker SHALL fail the execution with a clear error message
4. WHEN a step output contains nested JSON data, THE Worker SHALL support JSONPath-style references to access nested values
5. WHEN a step needs to pass data to subsequent steps, THE Worker SHALL automatically store the step output in the Job Context without requiring explicit configuration
6. WHEN a job has conditional logic based on step outputs, THE System SHALL evaluate conditions using data from the Job Context
7. WHEN a step references data that has not been populated by a previous step, THE Worker SHALL fail the execution with a clear error message indicating the missing data path

### Requirement 15

**User Story:** As a system administrator, I want to process Excel and CSV files in jobs, so that I can automate data import/export workflows with spreadsheet data.

#### Acceptance Criteria

1. WHEN a job step type is FileProcessing with format Excel, THE Worker SHALL read XLSX files from MinIO using the specified file path
2. WHEN reading an Excel file, THE Worker SHALL parse all sheets and store the data as structured JSON in the Job Context
3. WHEN a job step type is FileProcessing with format CSV, THE Worker SHALL read CSV files from MinIO using the specified file path
4. WHEN reading a CSV file, THE Worker SHALL parse rows with configurable delimiter (comma, semicolon, tab) and store as structured JSON in the Job Context
5. WHEN a file processing step specifies a sheet name or index for Excel files, THE Worker SHALL read only that specific sheet
6. WHEN a file processing step includes data transformation rules, THE Worker SHALL apply transformations (column mapping, data type conversion, filtering) before storing in Job Context
7. WHEN a file processing step writes data to Excel format, THE Worker SHALL generate an XLSX file from Job Context data and store it in MinIO
8. WHEN a file processing step writes data to CSV format, THE Worker SHALL generate a CSV file from Job Context data and store it in MinIO
9. WHEN writing files to MinIO, THE Worker SHALL use the path format: `jobs/{job_id}/executions/{execution_id}/output/{filename}`
10. WHEN a file processing step completes, THE Worker SHALL store the MinIO file path and row count in the Job Context
11. WHEN a file processing step encounters invalid data format, THE Worker SHALL fail the execution with a clear error message indicating the parsing error
12. WHEN processing large files (>100MB), THE Worker SHALL use streaming processing to avoid memory exhaustion

### Requirement 16

**User Story:** As a system administrator, I want to trigger jobs via webhooks from external systems, so that I can integrate job execution with event-driven architectures.

#### Acceptance Criteria

1. WHEN a job is configured with webhook trigger enabled, THE System SHALL generate a unique webhook URL for that job
2. WHEN an external system sends an HTTP POST request to the webhook URL, THE System SHALL immediately queue the job for execution
3. WHEN a webhook request includes a JSON payload, THE System SHALL store the payload in the Job Context as `webhook.payload`
4. WHEN a webhook request includes query parameters, THE System SHALL store them in the Job Context as `webhook.query_params`
5. WHEN a webhook request includes custom headers, THE System SHALL store specified headers in the Job Context as `webhook.headers`
6. WHEN a job step references webhook data, THE Worker SHALL resolve references like `{{webhook.payload.user_id}}` from the Job Context
7. WHEN a webhook request is received, THE System SHALL validate the webhook signature using a configured secret key (HMAC-SHA256)
8. WHEN a webhook signature is invalid, THE System SHALL reject the request with 401 Unauthorized status
9. WHEN a webhook request is successfully received, THE System SHALL return 202 Accepted with the execution_id in the response
10. WHEN a webhook is called for a disabled job, THE System SHALL reject the request with 403 Forbidden status
11. WHEN webhook requests exceed rate limits (configurable per job), THE System SHALL reject requests with 429 Too Many Requests status
12. WHEN a webhook URL is regenerated, THE System SHALL invalidate the previous webhook URL immediately

### Requirement 17

**User Story:** As a system administrator, I want multiple trigger methods for jobs, so that I can execute jobs automatically, manually, or via external events.

#### Acceptance Criteria

1. WHEN a job is configured with scheduled trigger, THE Scheduler SHALL automatically queue the job based on the schedule configuration
2. WHEN a job is configured with manual trigger only, THE Scheduler SHALL NOT automatically queue the job
3. WHEN a user manually triggers a job through the dashboard, THE System SHALL immediately queue the job regardless of schedule configuration
4. WHEN a user manually triggers a job through the API, THE System SHALL verify the user has "job:execute" permission before queueing
5. WHEN a job is triggered via webhook, THE System SHALL queue the job immediately and include webhook data in the Job Context
6. WHEN a job supports multiple trigger methods, THE System SHALL record the trigger source (scheduled, manual, webhook) in the job execution record
7. WHEN a job execution is triggered, THE System SHALL generate a unique execution_id regardless of trigger method
8. WHEN viewing execution history, THE System SHALL display the trigger source for each execution
9. WHEN a job is triggered while a previous execution is still running, THE System SHALL queue the new execution if concurrent execution is allowed
10. WHEN a job is configured to prevent concurrent execution, THE System SHALL reject new trigger requests while an execution is in progress


### Requirement 18

**User Story:** As a system administrator, I want to create jobs through a visual interface and import/export job definitions as JSON files, so that I can easily share, backup, and version control job configurations.

#### Acceptance Criteria

1. WHEN a user creates a job through the dashboard UI, THE System SHALL provide a visual form builder for defining job steps, schedule, and configuration
2. WHEN a user completes the visual job creation, THE System SHALL generate the corresponding JSON job definition
3. WHEN a user clicks "Export Job" in the dashboard, THE System SHALL download the job definition as a JSON file with filename format: `job-{job_name}-{timestamp}.json`
4. WHEN a user exports a job, THE System SHALL include all job configuration (schedule, steps, variables, triggers, timeout, retries) in the JSON file
5. WHEN a user exports a job, THE System SHALL exclude sensitive data (passwords, API keys) from the exported JSON and replace with placeholder values
6. WHEN a user clicks "Import Job" in the dashboard, THE System SHALL provide a file upload interface accepting JSON files
7. WHEN a user uploads a JSON job definition file, THE System SHALL validate the JSON schema before importing
8. WHEN a JSON job definition is invalid, THE System SHALL display clear error messages indicating which fields are incorrect
9. WHEN a valid JSON job definition is imported, THE System SHALL create a new job with a new job_id and store the definition in MinIO
10. WHEN importing a job, THE System SHALL prompt the user to provide values for sensitive data placeholders before saving
11. WHEN a user imports a job with the same name as an existing job, THE System SHALL create a new job with a suffix (e.g., "job-name-copy-1")
12. WHEN exporting multiple jobs, THE System SHALL support bulk export as a single JSON array file or individual files in a ZIP archive
13. WHEN importing multiple jobs from a JSON array or ZIP file, THE System SHALL process each job definition and report success/failure for each
14. WHEN a job is exported, THE System SHALL include metadata (export_date, exported_by, system_version) in the JSON file for traceability


### Requirement 19

**User Story:** As a user, I want a web-based login page with a professional interface, so that I can securely authenticate to the system using my browser without requiring API clients or command-line tools.

#### Acceptance Criteria

1. WHEN a user navigates to the root URL (/), THE System SHALL display a modern, responsive login page
2. WHEN the login page is displayed, THE System SHALL show the Vietnam Enterprise Cron System branding with logo and system name
3. WHEN the login page is rendered, THE System SHALL provide a login form with username and password input fields
4. WHEN authentication mode is configured as "database", THE System SHALL display a standard login form with username and password fields
5. WHEN authentication mode is configured as "keycloak", THE System SHALL provide a button to redirect users to the Keycloak login page
6. WHEN a user enters invalid credentials and submits the login form, THE System SHALL display an error message without revealing whether the username or password was incorrect (to prevent user enumeration)
7. WHEN a user enters valid credentials and submits the login form, THE System SHALL authenticate the user and redirect to the dashboard
8. WHEN authentication is successful, THE System SHALL store the JWT token securely in the browser (using httpOnly cookies or secure localStorage)
9. WHEN a user is already authenticated and navigates to the login page, THE System SHALL redirect them directly to the dashboard
10. WHEN the login form is submitted, THE System SHALL display a loading indicator to provide user feedback
11. WHEN authentication fails due to network error, THE System SHALL display a user-friendly error message and allow retry
12. WHEN the login page is accessed from a mobile device, THE System SHALL display a mobile-optimized responsive layout
13. WHEN a user has JavaScript disabled, THE System SHALL still allow form submission and display error/success messages
14. WHEN authentication mode is "database", THE System SHALL include a "Forgot Password" link that explains password reset must be done by system administrator
15. WHEN the login page loads, THE System SHALL display the current system version and health status (optional)
16. WHEN a user clicks "Login" with empty fields, THE System SHALL display field validation errors without making an API call
17. WHEN a user session expires, THE System SHALL redirect to the login page with a message indicating session expiration
18. WHEN the login page is displayed, THE System SHALL use consistent styling with the rest of the dashboard (matching colors, fonts, layout patterns)
19. WHEN a user successfully logs in, THE System SHALL log the login event with timestamp, username, IP address, and user agent for audit purposes
20. WHEN a user fails to log in, THE System SHALL log the failed attempt with timestamp, attempted username, IP address, and failure reason
21. WHEN the login page is displayed in database authentication mode, THE System SHALL show the default credentials hint for initial setup: "Default: admin / admin123"
22. WHEN the login form detects multiple failed login attempts from the same IP (5+ failures in 15 minutes), THE System SHALL implement temporary rate limiting with appropriate user messaging
23. WHEN the login page loads, THE System SHALL include CSRF protection tokens for form submission security
24. WHEN the login page is served, THE System SHALL include appropriate security headers (Content-Security-Policy, X-Frame-Options, X-Content-Type-Options)

### Requirement 20

**User Story:** As a system administrator, I want to connect to SFTP servers to download and upload files, so that I can automate secure file transfer operations with external systems.

#### Acceptance Criteria

1. WHEN a job step type is SFTP with operation download, THE Worker SHALL connect to the specified SFTP server and download files to MinIO
2. WHEN a job step type is SFTP with operation upload, THE Worker SHALL upload files from MinIO to the specified SFTP server
3. WHEN connecting to an SFTP server, THE Worker SHALL support password-based authentication with username and password
4. WHEN connecting to an SFTP server, THE Worker SHALL support SSH key-based authentication with private key file
5. WHEN downloading files via SFTP, THE Worker SHALL support wildcard patterns to download multiple files matching a pattern (e.g., `*.csv`, `report-*.xlsx`)
6. WHEN downloading files via SFTP, THE Worker SHALL store downloaded files in MinIO at path: `jobs/{job_id}/executions/{execution_id}/sftp/downloads/{filename}`
7. WHEN uploading files via SFTP, THE Worker SHALL read files from MinIO and transfer them to the specified remote path on the SFTP server
8. WHEN an SFTP download completes, THE Worker SHALL store file metadata (filename, size, download_time, remote_path) in the Job Context
9. WHEN an SFTP upload completes, THE Worker SHALL store upload metadata (filename, size, upload_time, remote_path) in the Job Context
10. WHEN an SFTP operation fails due to connection error, THE Worker SHALL retry with exponential backoff as configured
11. WHEN an SFTP operation fails due to authentication error, THE Worker SHALL fail immediately without retry
12. WHEN an SFTP operation fails due to file not found, THE Worker SHALL fail immediately without retry
13. WHEN downloading files via SFTP, THE Worker SHALL support recursive directory download to download entire directory structures
14. WHEN uploading files via SFTP, THE Worker SHALL create remote directories if they do not exist
15. WHEN an SFTP step references files from previous steps, THE Worker SHALL resolve file paths from the Job Context (e.g., `{{steps.step1.output_files[0].path}}`)
16. WHEN an SFTP connection is established, THE Worker SHALL verify the SFTP server host key to prevent man-in-the-middle attacks
17. WHEN SFTP operations involve large files (>100MB), THE Worker SHALL use streaming transfer to avoid memory exhaustion
18. WHEN an SFTP step completes, THE Worker SHALL close the SFTP connection and clean up resources properly
