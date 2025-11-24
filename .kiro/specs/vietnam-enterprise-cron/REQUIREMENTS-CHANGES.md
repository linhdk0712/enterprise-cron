# Requirements Changes - Multi-Step Jobs with MinIO

## Tổng quan thay đổi (Change Overview)

Đã bổ sung 2 requirements mới (Requirement 13 và 14) để hỗ trợ:
1. **Job definitions được lưu trữ dưới dạng JSON trong MinIO**
2. **Multi-step jobs** với khả năng truyền dữ liệu giữa các steps
3. **Job Context** - một object thuộc về job execution để lưu trữ kết quả từ mỗi step

## Thay đổi chi tiết (Detailed Changes)

### 1. Glossary - Các thuật ngữ mới

Đã thêm các định nghĩa mới:

- **MinIO**: S3-compatible object storage system để lưu job definitions và execution data
- **Job Definition**: JSON document trong MinIO định nghĩa schedule, steps, và configuration của job
- **Job Step**: Một đơn vị công việc trong job thực hiện một hành động cụ thể (HTTP request hoặc database query)
- **Job Context**: Data object thuộc về job execution, lưu trữ kết quả trung gian từ mỗi step, được persist vào MinIO
- **Step Output**: Dữ liệu trả về từ việc thực thi step (API response hoặc database query result) được lưu trong Job Context

### 2. Introduction - Cập nhật mô tả hệ thống

**Trước:**
> The system provides distributed job scheduling with exactly-once execution guarantees, comprehensive observability, and a real-time administrative dashboard. It supports multiple job execution types including HTTP requests, database queries...

**Sau:**
> The system provides distributed job scheduling with exactly-once execution guarantees, comprehensive observability, and a real-time administrative dashboard. 
>
> **Jobs are defined as JSON documents and stored in MinIO object storage, supporting complex multi-step workflows where each step can perform HTTP requests or database queries. Each job execution maintains its own context object in MinIO, allowing steps to pass data between each other and enabling sophisticated data processing pipelines.** The system supports multiple job execution types...

### 3. Requirement 13 - Job Definitions và Multi-Step Execution

**User Story:** As a system administrator, I want to define jobs as JSON documents with multiple steps stored in MinIO, so that I can create complex workflows with data passing between steps.

**Acceptance Criteria:**

1. **JSON Job Definition**: System accepts JSON job definition documents
2. **MinIO Storage**: Job definitions stored in MinIO at path `jobs/{job_id}/definition.json`
3. **Path Format**: Standardized path structure for job definitions
4. **Sequential Execution**: Steps execute sequentially in defined order
5. **HTTP Step Output**: API responses stored in Job Context
6. **Database Step Output**: Query results stored in Job Context
7. **Context Persistence**: Job Context persisted to MinIO at `jobs/{job_id}/executions/{execution_id}/context.json`
8. **Context Loading**: Subsequent steps load Job Context to access previous outputs
9. **Context Retention**: Final Job Context retained for audit/debugging
10. **Failure Preservation**: Job Context preserved up to point of failure
11. **Context Reference**: Execution details include MinIO context path reference
12. **Metadata Only**: System Database stores only MinIO path references, not full data

### 4. Requirement 14 - Step References và Data Passing

**User Story:** As a system administrator, I want steps within a job to reference outputs from previous steps, so that I can build complex data processing workflows.

**Acceptance Criteria:**

1. **Reference Resolution**: Worker resolves step references from Job Context
2. **JSONPath Syntax**: Support syntax like `{{steps.step1.response.data.id}}`
3. **Invalid Reference Handling**: Clear error messages for invalid references
4. **Nested Data Support**: JSONPath-style access to nested JSON values
5. **Automatic Storage**: Step outputs automatically stored in Job Context
6. **Conditional Logic**: Evaluate conditions using Job Context data
7. **Missing Data Handling**: Clear error for references to unpopulated data

## Kiến trúc mới (New Architecture)

### Data Storage Strategy

```
┌─────────────────────────────────────────────────────────────┐
│                     PostgreSQL (System DB)                   │
│  - Job metadata (id, name, schedule, enabled)               │
│  - MinIO path references ONLY                               │
│  - Execution status and timing                              │
│  - NO large data objects                                    │
└─────────────────────────────────────────────────────────────┘
                              │
                              │ References
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     MinIO (Object Storage)                   │
│  - Full job definitions (JSON)                              │
│  - Job Context objects (step outputs)                       │
│  - Large response payloads                                  │
│  - Audit trail of all executions                            │
└─────────────────────────────────────────────────────────────┘
```

### Job Definition Structure

```json
{
  "name": "user-data-sync",
  "schedule": {
    "type": "Cron",
    "expression": "0 0 2 * * *",
    "timezone": "Asia/Ho_Chi_Minh"
  },
  "steps": [
    {
      "id": "step1",
      "name": "fetch-users",
      "type": "HttpRequest",
      "config": {
        "method": "GET",
        "url": "https://api.example.com/users?status=active",
        "headers": {
          "Authorization": "Bearer {{api_token}}"
        }
      }
    },
    {
      "id": "step2",
      "name": "process-users",
      "type": "DatabaseQuery",
      "config": {
        "database_type": "PostgreSQL",
        "connection_string": "postgresql://{{db_user}}:{{db_pass}}@{{db_host}}/{{db_name}}",
        "query": "INSERT INTO users (id, name, email) VALUES {{steps.step1.response.data}}"
      }
    },
    {
      "id": "step3",
      "name": "send-notification",
      "type": "HttpRequest",
      "config": {
        "method": "POST",
        "url": "https://notification.example.com/send",
        "body": {
          "message": "Synced {{steps.step1.response.total}} users",
          "inserted": "{{steps.step2.rows_affected}}"
        }
      }
    }
  ],
  "timeout_seconds": 600,
  "max_retries": 3
}
```

### Job Context Structure

```json
{
  "job_id": "550e8400-e29b-41d4-a716-446655440000",
  "execution_id": "exec-uuid",
  "started_at": "2025-01-20T02:00:00Z",
  "completed_at": "2025-01-20T02:00:05Z",
  "status": "Success",
  "variables": {
    "api_token": "***",
    "db_user": "app_user",
    "db_pass": "***",
    "db_host": "prod-db.example.com",
    "db_name": "production"
  },
  "steps": {
    "step1": {
      "id": "step1",
      "name": "fetch-users",
      "status": "Success",
      "started_at": "2025-01-20T02:00:01Z",
      "completed_at": "2025-01-20T02:00:02Z",
      "response": {
        "status_code": 200,
        "headers": {
          "content-type": "application/json"
        },
        "data": [
          {
            "id": 1,
            "name": "John Doe",
            "email": "john@example.com"
          },
          {
            "id": 2,
            "name": "Jane Smith",
            "email": "jane@example.com"
          }
        ],
        "total": 2
      }
    },
    "step2": {
      "id": "step2",
      "name": "process-users",
      "status": "Success",
      "started_at": "2025-01-20T02:00:03Z",
      "completed_at": "2025-01-20T02:00:03Z",
      "result": {
        "rows_affected": 2,
        "execution_time": 0.15
      }
    },
    "step3": {
      "id": "step3",
      "name": "send-notification",
      "status": "Success",
      "started_at": "2025-01-20T02:00:04Z",
      "completed_at": "2025-01-20T02:00:05Z",
      "response": {
        "status_code": 200,
        "data": {
          "notification_id": "notif-123",
          "sent": true
        }
      }
    }
  }
}
```

## Step Reference Syntax

### Basic References

```
{{steps.step1.response.total}}
→ 2

{{steps.step1.response.data[0].name}}
→ "John Doe"

{{steps.step2.rows_affected}}
→ 2
```

### In HTTP Request Body

```json
{
  "message": "Synced {{steps.step1.response.total}} users",
  "inserted": "{{steps.step2.rows_affected}}",
  "users": "{{steps.step1.response.data}}"
}
```

Resolves to:

```json
{
  "message": "Synced 2 users",
  "inserted": 2,
  "users": [
    {"id": 1, "name": "John Doe", "email": "john@example.com"},
    {"id": 2, "name": "Jane Smith", "email": "jane@example.com"}
  ]
}
```

### In Database Query

```sql
INSERT INTO users (id, name, email) 
VALUES {{steps.step1.response.data}}
```

Resolves to parameterized query:

```sql
INSERT INTO users (id, name, email) 
VALUES ($1, $2, $3), ($4, $5, $6)
```

With parameters: `[1, "John Doe", "john@example.com", 2, "Jane Smith", "jane@example.com"]`

## Lợi ích (Benefits)

### 1. Separation of Concerns
- **PostgreSQL**: Fast metadata queries, small database size
- **MinIO**: Unlimited storage for large payloads, S3-compatible

### 2. Complex Workflows
- Multi-step jobs with data dependencies
- Build sophisticated data processing pipelines
- ETL (Extract, Transform, Load) workflows

### 3. Debugging & Audit
- Full execution context preserved in MinIO
- Easy to inspect what happened at each step
- Replay failed executions with same context

### 4. Scalability
- Database doesn't grow with job data
- MinIO scales horizontally
- Cost-effective for large datasets

### 5. Flexibility
- Job definitions as code (JSON)
- Version control for job definitions
- Easy to template and generate jobs programmatically

## Impact on Existing Components

### Components cần thay đổi:

1. **API Server**:
   - Accept JSON job definitions
   - Store definitions in MinIO
   - Store MinIO path references in PostgreSQL

2. **Worker Process**:
   - Load job definitions from MinIO
   - Execute steps sequentially
   - Manage Job Context
   - Persist context to MinIO after each step
   - Resolve step references

3. **Database Schema**:
   - Add `minio_definition_path` column to `jobs` table
   - Add `minio_context_path` column to `job_executions` table

4. **New Components**:
   - MinIO client integration
   - Job Context Manager
   - Step Executor (orchestrates multi-step execution)
   - Step Reference Resolver (JSONPath-style)

### Components không thay đổi:

- Scheduler (vẫn schedule jobs như cũ)
- Distributed Lock (Redis RedLock)
- NATS JetStream (job queue)
- Authentication & Authorization
- Observability (metrics, logs, traces)
- Dashboard (có thể thêm link download Job Context)

## Migration Path

### Phase 1: Add MinIO Support
1. Deploy MinIO cluster
2. Add MinIO client to codebase
3. Update database schema (add MinIO path columns)

### Phase 2: Support JSON Job Definitions
1. Implement JSON job definition parser
2. Store definitions in MinIO
3. Maintain backward compatibility with existing jobs

### Phase 3: Multi-Step Execution
1. Implement Job Context Manager
2. Implement Step Executor
3. Add step reference resolution

### Phase 4: Migration
1. Convert existing jobs to JSON format
2. Store in MinIO
3. Update references in database

## Testing Considerations

### New Property-Based Tests Needed:

1. **Job Definition Parsing**: Valid JSON schemas parse correctly
2. **Step Reference Resolution**: All reference syntaxes resolve correctly
3. **Job Context Persistence**: Context survives failures and retries
4. **Step Execution Order**: Steps execute in correct sequence
5. **Data Passing**: Data flows correctly between steps
6. **MinIO Integration**: Objects stored and retrieved correctly

### Integration Tests:

1. End-to-end multi-step job execution
2. Failure recovery with context preservation
3. Large payload handling (>10MB responses)
4. Concurrent job executions with separate contexts

## Additional Requirements (Latest Update)

### Requirement 15: File Processing Jobs

**New Job Type**: File Processing (Excel and CSV)

**Capabilities:**
- Read Excel (XLSX) files with sheet selection
- Read CSV files with configurable delimiters
- Apply data transformations (column mapping, filtering, type conversion)
- Write Excel and CSV files
- Streaming processing for large files (>100MB)
- Store file paths and metadata in Job Context

**Use Cases:**
- Data import from spreadsheets
- Report generation
- ETL workflows
- Data validation and cleansing

### Requirement 16: Webhook Triggers

**New Trigger Method**: Webhook-based job execution

**Capabilities:**
- Unique webhook URL per job
- HMAC-SHA256 signature validation
- Webhook payload stored in Job Context
- Rate limiting (configurable per job)
- Webhook URL regeneration
- Support for custom headers and query parameters

**Use Cases:**
- Event-driven job execution
- Integration with external systems
- Real-time data processing
- Microservices communication

### Requirement 17: Multiple Trigger Methods

**Trigger Types:**
1. **Scheduled**: Automatic execution (cron, fixed delay, fixed rate)
2. **Manual**: User-initiated via dashboard or API
3. **Webhook**: External system triggered via HTTP

**Features:**
- Record trigger source in execution history
- Concurrent execution control
- Trigger-specific data in Job Context

## Updated Job Types Summary

### 1. File Processing Jobs
```json
{
  "type": "FileProcessing",
  "config": {
    "operation": "read|write",
    "format": "excel|csv",
    "source_path": "files/input/data.xlsx",
    "sheet": "Sheet1",
    "transformations": [...]
  }
}
```

### 2. Database Jobs (CRUD)
```json
{
  "type": "DatabaseQuery",
  "config": {
    "database_type": "PostgreSQL|MySQL|Oracle",
    "connection_string": "...",
    "query": "INSERT INTO ..."
  }
}
```

### 3. API Integration Jobs
```json
{
  "type": "HttpRequest",
  "config": {
    "method": "GET|POST|PUT",
    "url": "https://api.example.com/...",
    "auth": {...}
  }
}
```

## Updated Trigger Methods Summary

### 1. Scheduled Trigger (Automatic)
```json
{
  "schedule": {
    "type": "Cron",
    "expression": "0 0 2 * * *",
    "timezone": "Asia/Ho_Chi_Minh"
  }
}
```

### 2. Manual Trigger
- Dashboard: Click "Run Now" button
- API: `POST /api/jobs/{job_id}/trigger`
- Requires `job:execute` permission

### 3. Webhook Trigger
```bash
POST https://api.example.com/webhooks/{webhook_id}
X-Webhook-Signature: sha256=abc123...
Content-Type: application/json

{
  "event": "user.created",
  "data": {...}
}
```

### Requirement 18: Job Import/Export

**New Feature**: Visual job builder with import/export capabilities

**Capabilities:**
- Visual form builder for creating jobs (no JSON knowledge required)
- Export job definitions as JSON files
- Import jobs from JSON files
- Sensitive data redaction on export
- Sensitive data input prompts on import
- Bulk export/import (multiple jobs)
- JSON schema validation
- Duplicate name handling (auto-suffix)
- Export metadata (traceability)

**Use Cases:**
- Job backup and disaster recovery
- Job sharing between environments (dev → staging → prod)
- Version control integration (Git)
- Job templates and reusability
- Collaboration and code review
- Configuration as code

### Requirement 19: SFTP Jobs

**New Job Type**: SFTP (Secure File Transfer Protocol)

**Capabilities:**
- Connect to SFTP servers via SSH
- Download files from remote SFTP servers
- Upload files to remote SFTP servers
- Password-based authentication
- SSH key-based authentication
- Host key verification (prevent MITM attacks)
- Wildcard pattern matching (e.g., `*.csv`, `report-*.xlsx`)
- Recursive directory download
- Automatic directory creation on upload
- Streaming transfer for large files (>100MB)
- File metadata tracking in Job Context

**Use Cases:**
- Automated file exchange with partners
- Secure data import from external systems
- Report distribution to SFTP servers
- Backup file transfers
- ETL workflows with file-based data sources
- Integration with legacy systems using SFTP

### Requirement 18: Job Import/Export (continued)

**New Feature**: Visual job builder with import/export capabilities

**Capabilities:**
- Visual form builder for creating jobs (no JSON knowledge required)
- Export job definitions as JSON files
- Import jobs from JSON files
- Sensitive data redaction on export
- Sensitive data input prompts on import
- Bulk export/import (multiple jobs)
- JSON schema validation
- Duplicate name handling (auto-suffix)
- Export metadata (traceability)

**Use Cases:**
- Job backup and disaster recovery
- Job sharing between environments (dev → staging → prod)
- Version control integration (Git)
- Job templates and reusability
- Collaboration and code review
- Configuration as code

## New Sequence Diagrams

- [x] `sequence-10-file-processing-job.puml` - Excel/CSV processing
- [x] `sequence-11-webhook-trigger.puml` - Webhook-triggered execution
- [x] `sequence-12-job-import-export.puml` - Visual builder and JSON import/export
- [x] `sequence-13-sftp-job.puml` - SFTP download and upload operations

## Documentation Updates Needed

- [ ] Update design.md with new architecture
- [ ] Add MinIO configuration guide
- [ ] Document JSON job definition schema
- [ ] Document step reference syntax
- [ ] Add examples of multi-step jobs
- [ ] Update deployment guide (add MinIO)
- [ ] Document file processing capabilities
- [ ] Document webhook configuration and security
- [ ] Document job import/export workflow
- [ ] Add examples for all job types
- [ ] Create visual job builder UI mockups
- [x] Create sequence diagram for multi-step execution
- [x] Create sequence diagram for file processing
- [x] Create sequence diagram for webhook trigger
- [x] Create sequence diagram for job import/export
- [x] Update requirements.md with Requirements 15, 16, 17, 18, 19

## Job Import/Export Examples

### Export Job Example

**Request:**
```bash
GET /api/jobs/550e8400-e29b-41d4-a716-446655440000/export
Authorization: Bearer {token}
```

**Response (JSON file):**
```json
{
  "export_metadata": {
    "export_date": "2025-01-20T10:00:00Z",
    "exported_by": "admin",
    "system_version": "1.0.0",
    "job_id": "550e8400-e29b-41d4-a716-446655440000",
    "original_created_at": "2025-01-15T08:00:00Z"
  },
  "job_definition": {
    "name": "Daily User Sync",
    "description": "Sync users from API to database",
    "schedule": {
      "type": "Cron",
      "expression": "0 0 2 * * *",
      "timezone": "Asia/Ho_Chi_Minh"
    },
    "triggers": {
      "scheduled": true,
      "manual": true,
      "webhook": true
    },
    "steps": [
      {
        "id": "step1",
        "name": "fetch-users",
        "type": "HttpRequest",
        "config": {
          "method": "GET",
          "url": "https://api.example.com/users",
          "headers": {
            "Authorization": "Bearer ***REDACTED***"
          }
        }
      },
      {
        "id": "step2",
        "name": "insert-users",
        "type": "DatabaseQuery",
        "config": {
          "database_type": "PostgreSQL",
          "connection_string": "postgresql://***REDACTED***:***REDACTED***@{{db_host}}/{{db_name}}",
          "query": "INSERT INTO users VALUES {{steps.step1.response.data}}"
        }
      }
    ],
    "timeout_seconds": 600,
    "max_retries": 3,
    "concurrent_execution": false
  }
}
```

### Import Job Example

**Request:**
```bash
POST /api/jobs/import
Authorization: Bearer {token}
Content-Type: multipart/form-data

file: job-daily-user-sync-20250120.json
```

**Response (requires sensitive data):**
```json
{
  "validation": "success",
  "sensitive_fields": [
    "steps[0].config.headers.Authorization",
    "steps[1].config.connection_string.user",
    "steps[1].config.connection_string.password"
  ],
  "requires_input": true
}
```

**Confirm Import with Sensitive Data:**
```bash
POST /api/jobs/import/confirm
Authorization: Bearer {token}
Content-Type: application/json

{
  "job_definition": {...},
  "sensitive_values": {
    "steps[0].config.headers.Authorization": "Bearer real_token_xyz",
    "steps[1].config.connection_string.user": "app_user",
    "steps[1].config.connection_string.password": "secure_password"
  }
}
```

**Response:**
```json
{
  "job_id": "new-job-uuid",
  "name": "Daily User Sync (Copy 1)",
  "message": "Job imported successfully"
}
```

### Bulk Export Example

**Request:**
```bash
POST /api/jobs/export/bulk
Authorization: Bearer {token}
Content-Type: application/json

{
  "job_ids": [
    "550e8400-e29b-41d4-a716-446655440000",
    "660e8400-e29b-41d4-a716-446655440001",
    "770e8400-e29b-41d4-a716-446655440002"
  ],
  "format": "zip"
}
```

**Response:**
- Content-Type: application/zip
- Content-Disposition: attachment; filename="jobs-export-20250120.zip"
- ZIP contains:
  - job-daily-user-sync-20250120.json
  - job-weekly-report-20250120.json
  - job-data-cleanup-20250120.json
  - README.txt (import instructions)

### Version Control Workflow

```bash
# 1. Export all jobs
curl -X POST https://api.example.com/api/jobs/export/bulk \
  -H "Authorization: Bearer token" \
  -o jobs-backup.zip

# 2. Extract to Git repository
unzip jobs-backup.zip -d jobs/
cd jobs/

# 3. Commit to Git
git add .
git commit -m "Backup jobs - 2025-01-20"
git push origin main

# 4. Later: Import from Git (e.g., to staging environment)
curl -X POST https://staging.example.com/api/jobs/import/bulk \
  -H "Authorization: Bearer staging_token" \
  -F "file=@jobs-backup.zip"
```

## SFTP Job Examples

### SFTP Download Example

```json
{
  "name": "Download Partner Reports",
  "schedule": {
    "type": "Cron",
    "expression": "0 0 6 * * *"
  },
  "steps": [
    {
      "id": "step1",
      "name": "download-reports",
      "type": "SFTP",
      "config": {
        "operation": "download",
        "host": "sftp.partner.com",
        "port": 22,
        "username": "{{sftp_user}}",
        "auth_type": "password",
        "password": "{{sftp_password}}",
        "remote_path": "/incoming/reports/*.csv",
        "verify_host_key": true,
        "host_key_fingerprint": "SHA256:abc123..."
      }
    },
    {
      "id": "step2",
      "name": "process-reports",
      "type": "FileProcessing",
      "config": {
        "operation": "read",
        "format": "csv",
        "source_files": "{{steps.step1.downloaded_files}}"
      }
    }
  ]
}
```

### SFTP Upload Example

```json
{
  "name": "Upload Processed Data",
  "steps": [
    {
      "id": "step1",
      "name": "generate-report",
      "type": "FileProcessing",
      "config": {
        "operation": "write",
        "format": "excel",
        "data_source": "{{database_query_result}}",
        "filename": "monthly-report-{{execution_id}}.xlsx"
      }
    },
    {
      "id": "step2",
      "name": "upload-to-sftp",
      "type": "SFTP",
      "config": {
        "operation": "upload",
        "host": "sftp.client.com",
        "port": 22,
        "username": "{{sftp_user}}",
        "auth_type": "ssh_key",
        "private_key_path": "{{sftp_private_key}}",
        "local_files": "{{steps.step1.output_files}}",
        "remote_path": "/reports/monthly/",
        "create_directories": true
      }
    }
  ]
}
```

### SFTP with SSH Key Authentication

```json
{
  "config": {
    "operation": "download",
    "host": "sftp.example.com",
    "port": 22,
    "username": "automation_user",
    "auth_type": "ssh_key",
    "private_key_path": "{{sftp_private_key}}",
    "private_key_passphrase": "{{sftp_key_passphrase}}",
    "remote_path": "/data/exports/*.json",
    "verify_host_key": true,
    "host_key_fingerprint": "SHA256:xyz789..."
  }
}
```

### SFTP Recursive Directory Download

```json
{
  "config": {
    "operation": "download",
    "host": "sftp.example.com",
    "port": 22,
    "username": "{{sftp_user}}",
    "auth_type": "password",
    "password": "{{sftp_password}}",
    "remote_path": "/archive/2025/**/*",
    "recursive": true,
    "preserve_directory_structure": true
  }
}
```

## Updated Job Types Summary (Final)

### 1. File Processing Jobs
```json
{
  "type": "FileProcessing",
  "config": {
    "operation": "read|write",
    "format": "excel|csv",
    "source_path": "files/input/data.xlsx"
  }
}
```

### 2. Database Jobs (CRUD)
```json
{
  "type": "DatabaseQuery",
  "config": {
    "database_type": "PostgreSQL|MySQL|Oracle",
    "connection_string": "...",
    "query": "INSERT INTO ..."
  }
}
```

### 3. API Integration Jobs
```json
{
  "type": "HttpRequest",
  "config": {
    "method": "GET|POST|PUT",
    "url": "https://api.example.com/...",
    "auth": {...}
  }
}
```

### 4. SFTP Jobs (NEW)
```json
{
  "type": "SFTP",
  "config": {
    "operation": "download|upload",
    "host": "sftp.example.com",
    "port": 22,
    "username": "{{sftp_user}}",
    "auth_type": "password|ssh_key",
    "remote_path": "/path/to/files/*.csv"
  }
}
```

---

**Last Updated**: 2025-01-20
**Version**: 4.0
**Author**: Vietnam Enterprise Cron Team
