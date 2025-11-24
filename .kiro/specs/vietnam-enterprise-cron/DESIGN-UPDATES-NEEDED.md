# Design and Tasks Updates Needed

## Summary

The requirements document has been significantly updated with new requirements (13-19), but the design.md and tasks.md files have NOT been updated yet. This document outlines what needs to be updated.

## New Requirements Added (Not Yet in Design/Tasks)

### ✅ Requirement 13: Multi-Step Jobs with MinIO
- Job definitions stored as JSON in MinIO
- Multiple steps per job
- Job Context for data passing between steps
- Step reference syntax (JSONPath-style)

**Design Updates Needed:**
- [ ] Update Data Models to include MinIO paths
- [ ] Add Job Context data model
- [ ] Add Step data model
- [ ] Update JobType enum to support multi-step
- [ ] Add MinIO client integration architecture
- [ ] Add Job Context Manager component
- [ ] Add Step Executor component
- [ ] Add Step Reference Resolver component

### ✅ Requirement 14: Step References and Data Passing
- Steps can reference outputs from previous steps
- JSONPath-style syntax: `{{steps.step1.response.data.id}}`
- Conditional logic based on step outputs

**Design Updates Needed:**
- [ ] Add Variable Resolution architecture for step references
- [ ] Define step reference syntax grammar
- [ ] Add error handling for invalid references

### ✅ Requirement 15: File Processing Jobs
- Read/write Excel (XLSX) files
- Read/write CSV files
- Data transformations (column mapping, filtering, type conversion)
- Streaming for large files (>100MB)

**Design Updates Needed:**
- [ ] Add FileProcessing to JobType enum
- [ ] Add File Processor component
- [ ] Define file transformation pipeline
- [ ] Add streaming architecture for large files
- [ ] Add dependencies: calamine (Excel), csv (CSV), rust_xlsxwriter

### ✅ Requirement 16: Webhook Triggers
- Unique webhook URL per job
- HMAC-SHA256 signature validation
- Webhook payload in Job Context
- Rate limiting

**Design Updates Needed:**
- [ ] Add Webhook Handler component
- [ ] Add webhook URL generation logic
- [ ] Add HMAC signature validation
- [ ] Add rate limiting architecture (Redis-based)
- [ ] Update Job model to include webhook configuration
- [ ] Add webhook_id and webhook_secret fields

### ✅ Requirement 17: Multiple Trigger Methods
- Scheduled, Manual, Webhook triggers
- Record trigger source in execution history
- Concurrent execution control

**Design Updates Needed:**
- [ ] Add trigger_source field to JobExecution model
- [ ] Add concurrent_execution_allowed field to Job model
- [ ] Add trigger method validation logic

### ✅ Requirement 18: Job Import/Export
- Visual job builder UI
- Export jobs as JSON files
- Import jobs from JSON files
- Sensitive data redaction/restoration
- Bulk import/export

**Design Updates Needed:**
- [ ] Add Job Import/Export API endpoints
- [ ] Add JSON schema validation
- [ ] Add sensitive data redaction logic
- [ ] Add visual job builder UI components
- [ ] Add bulk operations architecture

### ✅ Requirement 19: SFTP Jobs
- Connect to SFTP servers
- Download/upload files
- Password and SSH key authentication
- Host key verification
- Wildcard pattern matching
- Streaming for large files

**Design Updates Needed:**
- [ ] Add SFTP to JobType enum
- [ ] Add SFTP Executor component
- [ ] Add SFTP authentication architecture
- [ ] Add host key verification logic
- [ ] Add wildcard pattern matching
- [ ] Add dependency: ssh2 crate

## Updated System Architecture Needed

### Storage Layer Update

**Current:**
```
PostgreSQL (System DB) + Redis (Locks) + NATS (Queue)
```

**Should Be:**
```
PostgreSQL (System DB) + Redis (Locks + Rate Limiting) + NATS (Queue) + MinIO (Job Definitions + Context + Files)
```

### Component Diagram Update

**Need to Add:**
- MinIO Object Storage
- Job Context Manager
- Step Executor
- File Processor
- SFTP Executor
- Webhook Handler
- Job Import/Export Handler

### Data Models Update

**Current JobType enum:**
```rust
enum JobType {
    HttpRequest { ... },
    DatabaseQuery { ... },
}
```

**Should Be:**
```rust
enum JobType {
    HttpRequest { ... },
    DatabaseQuery { ... },
    FileProcessing { ... },  // NEW
    SFTP { ... },            // NEW
}
```

**Need to Add:**
```rust
struct JobDefinition {
    name: String,
    schedule: Schedule,
    steps: Vec<JobStep>,     // NEW: Multi-step support
    triggers: TriggerConfig, // NEW: Multiple triggers
    // ...
}

struct JobStep {
    id: String,
    name: String,
    step_type: StepType,
    config: serde_json::Value,
}

enum StepType {
    HttpRequest,
    DatabaseQuery,
    FileProcessing,
    SFTP,
}

struct JobContext {
    job_id: Uuid,
    execution_id: Uuid,
    started_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    status: ExecutionStatus,
    variables: HashMap<String, serde_json::Value>,
    steps: HashMap<String, StepOutput>,
    webhook: Option<WebhookData>,  // NEW
}

struct StepOutput {
    status: ExecutionStatus,
    started_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    data: serde_json::Value,
}

struct WebhookData {
    payload: serde_json::Value,
    headers: HashMap<String, String>,
    query_params: HashMap<String, String>,
    received_at: DateTime<Utc>,
}

struct TriggerConfig {
    scheduled: bool,
    manual: bool,
    webhook: Option<WebhookConfig>,
}

struct WebhookConfig {
    enabled: bool,
    webhook_id: String,
    webhook_secret: String,
    rate_limit: u32,  // requests per minute
}
```

**Database Schema Updates:**
```sql
-- Add to jobs table
ALTER TABLE jobs ADD COLUMN minio_definition_path VARCHAR(500);
ALTER TABLE jobs ADD COLUMN webhook_id VARCHAR(100) UNIQUE;
ALTER TABLE jobs ADD COLUMN webhook_secret VARCHAR(255);
ALTER TABLE jobs ADD COLUMN webhook_enabled BOOLEAN DEFAULT false;
ALTER TABLE jobs ADD COLUMN webhook_rate_limit INTEGER DEFAULT 100;
ALTER TABLE jobs ADD COLUMN concurrent_execution_allowed BOOLEAN DEFAULT true;

-- Add to job_executions table
ALTER TABLE job_executions ADD COLUMN minio_context_path VARCHAR(500);
ALTER TABLE job_executions ADD COLUMN trigger_source VARCHAR(50); -- 'scheduled', 'manual', 'webhook'
ALTER TABLE job_executions ADD COLUMN trigger_data JSONB;
```

## Technology Stack Updates

### New Dependencies Needed

**Cargo.toml additions:**
```toml
[dependencies]
# MinIO client
minio = "0.1"
# or use aws-sdk-s3 with MinIO compatibility

# File processing
calamine = "0.22"  # Excel reading
rust_xlsxwriter = "0.60"  # Excel writing
csv = "1.3"  # CSV processing

# SFTP
ssh2 = "0.9"  # SSH/SFTP client

# Webhook signature validation
hmac = "0.12"
sha2 = "0.10"

# Pattern matching
glob = "0.3"

# JSON schema validation
jsonschema = "0.17"
```

## Testing Strategy Updates

### New Property-Based Tests Needed

1. **Multi-Step Job Execution**
   - Property: Step execution order is preserved
   - Property: Step references resolve correctly
   - Property: Job Context is persisted after each step

2. **File Processing**
   - Property: Excel round-trip (write then read) preserves data
   - Property: CSV round-trip preserves data
   - Property: Transformations are idempotent
   - Property: Streaming handles files larger than memory

3. **Webhook Triggers**
   - Property: HMAC signature validation is correct
   - Property: Rate limiting enforces limits
   - Property: Webhook payload is stored in Job Context

4. **Job Import/Export**
   - Property: Export then import produces equivalent job
   - Property: Sensitive data is redacted on export
   - Property: JSON schema validation catches invalid jobs

5. **SFTP Operations**
   - Property: Download then upload preserves file content
   - Property: Wildcard patterns match correctly
   - Property: Host key verification prevents MITM

### New Integration Tests Needed

1. End-to-end multi-step job execution
2. File processing with large files (>100MB)
3. Webhook trigger with signature validation
4. SFTP download and upload operations
5. Job import/export with sensitive data

## Implementation Priority

### Phase 1: Foundation (MinIO + Multi-Step)
1. Add MinIO client integration
2. Update Job model to support multi-step
3. Implement Job Context Manager
4. Implement Step Executor
5. Implement step reference resolution

### Phase 2: File Processing
1. Add File Processor component
2. Implement Excel read/write
3. Implement CSV read/write
4. Implement data transformations
5. Implement streaming for large files

### Phase 3: Webhook Triggers
1. Add Webhook Handler component
2. Implement webhook URL generation
3. Implement HMAC signature validation
4. Implement rate limiting
5. Update API to handle webhook requests

### Phase 4: SFTP Jobs
1. Add SFTP Executor component
2. Implement SFTP connection and authentication
3. Implement download operations
4. Implement upload operations
5. Implement wildcard pattern matching

### Phase 5: Job Import/Export
1. Add Job Import/Export API endpoints
2. Implement JSON schema validation
3. Implement sensitive data redaction
4. Implement visual job builder UI
5. Implement bulk operations

## Action Items

- [ ] Update design.md with all new components and data models
- [ ] Update tasks.md with implementation tasks for Requirements 13-19
- [ ] Create correctness properties for new requirements
- [ ] Update deployment guide to include MinIO
- [ ] Update configuration documentation
- [ ] Create migration guide for existing jobs

## Estimated Effort

- Design updates: 2-3 days
- Implementation: 4-6 weeks
- Testing: 2-3 weeks
- Documentation: 1 week

**Total: 8-11 weeks**

---

**Created**: 2025-01-20
**Status**: Pending Review
**Author**: Vietnam Enterprise Cron Team
