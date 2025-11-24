# Task 28: Multi-Step Job Execution Implementation

## Overview
Successfully implemented multi-step job execution support with MinIO integration for job definitions and execution context storage.

## Requirements Addressed
- **13.1**: Accept JSON job definition documents
- **13.2**: Store job definitions in MinIO
- **13.3**: Use path format `jobs/{job_id}/definition.json`
- **13.4**: Execute steps sequentially in defined order
- **13.5**: Store HTTP response in Job Context
- **13.6**: Store database query results in Job Context
- **13.7**: Persist Job Context to MinIO after each step
- **13.8**: Load Job Context for subsequent steps
- **13.9**: Retain Job Context after completion
- **13.10**: Preserve Job Context on failure
- **13.11**: Provide MinIO path reference in execution details
- **13.12**: Store only MinIO path references in database

## Correctness Properties Validated
- **Property 76**: JSON job definition acceptance
- **Property 77**: MinIO job definition persistence (round-trip)
- **Property 78**: MinIO path format for job definitions
- **Property 79**: Sequential step execution
- **Property 80**: HTTP response storage in Job Context
- **Property 81**: Database result storage in Job Context
- **Property 82**: Job Context persistence to MinIO
- **Property 83**: Job Context path format
- **Property 84**: Job Context loading for subsequent steps
- **Property 85**: Job Context retention after completion
- **Property 86**: Job Context preservation on failure
- **Property 87**: Job Context reference in execution details
- **Property 88**: Database stores only MinIO path references
- **Property 93**: Automatic step output storage

## Implementation Details

### Task 28.1: Update Worker to Support Multi-Step Jobs

#### Changes to `common/src/worker/consumer.rs`:

1. **Added MinIO Service Integration**
   - Added `minio_service: Arc<dyn MinIOService>` to `WorkerJobConsumer` struct
   - Updated constructor to accept MinIO service parameter
   - Updated `process_job` method to load job definitions from MinIO

2. **Job Definition Loading**
   - Load job metadata from database (for basic info and MinIO path)
   - Load full job definition from MinIO using `minio_service.load_job_definition()`
   - Parse JSON job definition to get steps and configuration

3. **Job Context Management**
   - Initialize new Job Context using `JobContext::new()` for new executions
   - Load existing Job Context from MinIO for resumed executions
   - Persist Job Context to MinIO after each step completion
   - Save final Job Context to MinIO after job completion or failure

4. **Sequential Step Execution**
   - Execute steps in order as defined in job definition
   - Update `current_step` field in execution record
   - Store step output in Job Context using `context.set_step_output()`
   - Persist context to MinIO after each step

5. **Error Handling**
   - Proper error handling for MinIO operations
   - Context preservation on failure
   - Detailed logging with tracing instrumentation

#### Changes to `worker/src/main.rs`:

1. **MinIO Client Initialization**
   - Initialize MinIO client with configuration from settings
   - Create `MinIOServiceImpl` instance
   - Pass MinIO service to `WorkerJobConsumer`

2. **Dependencies Added**
   ```rust
   use common::storage::{MinioClient, MinIOService, MinIOServiceImpl};
   ```

### Task 28.2: Update Job Model to Support Steps

#### Verification Results:

1. **Job Model** (`common/src/models.rs`)
   - ✅ Already has `steps: Vec<JobStep>` field
   - ✅ JobStep struct already defined with id, name, step_type, condition
   - ✅ JobContext model already supports step outputs

2. **Database Schema**
   - ✅ `jobs.minio_definition_path` field exists
   - ✅ `job_executions.current_step` field exists
   - ✅ `job_executions.minio_context_path` field exists
   - ✅ No migration needed

3. **API Endpoints** (`api/src/handlers/jobs.rs`)
   - ✅ `CreateJobRequest` already accepts `steps: Vec<JobStep>`
   - ✅ `UpdateJobRequest` already accepts `steps: Option<Vec<JobStep>>`
   - ✅ Job creation stores full definition (including steps) in MinIO
   - ✅ Job update updates definition in MinIO
   - ✅ Job retrieval loads full definition from MinIO

## Architecture

### Data Flow

```
1. Job Creation:
   API → Store JSON definition in MinIO → Store metadata in DB

2. Job Execution:
   Scheduler → Queue job → Worker receives job
   Worker → Load metadata from DB → Load definition from MinIO
   Worker → Initialize/Load Job Context from MinIO
   Worker → Execute steps sequentially
   Worker → Store step output in context → Persist context to MinIO (after each step)
   Worker → Save final context to MinIO

3. Job Retrieval:
   API → Load metadata from DB → Load definition from MinIO → Return full job
```

### Storage Strategy

- **Database**: Stores job metadata and MinIO path references
- **MinIO**: Stores full job definitions and execution contexts
- **Benefits**:
  - Database remains lightweight with only references
  - Complex job definitions don't bloat database
  - Job Context can grow large without database impact
  - Easy to archive/backup job definitions and contexts

## Testing Considerations

The implementation follows RECC 2025 standards:
- ✅ No `unwrap()` or `expect()` in production code
- ✅ All async functions have `#[tracing::instrument]`
- ✅ Proper error handling with `thiserror` and `anyhow`
- ✅ Structured logging with context
- ✅ Requirements and properties documented in comments

## Files Modified

1. `common/src/worker/consumer.rs` - Updated worker to support multi-step jobs with MinIO
2. `worker/src/main.rs` - Added MinIO service initialization

## Files Verified (No Changes Needed)

1. `common/src/models.rs` - Job model already supports steps
2. `migrations/20250101000001_create_jobs_table.sql` - Schema already supports MinIO paths
3. `migrations/20250101000002_create_job_executions_table.sql` - Schema already supports context paths
4. `api/src/handlers/jobs.rs` - API already supports steps in create/update

## Next Steps

The multi-step job execution feature is now complete. The system can:
- Accept job definitions with multiple steps
- Store definitions in MinIO
- Execute steps sequentially
- Maintain execution context across steps
- Persist context after each step
- Preserve context on completion or failure

Future enhancements could include:
- Conditional step execution based on previous step outputs
- Parallel step execution for independent steps
- Step retry policies per step
- Step-level timeouts
