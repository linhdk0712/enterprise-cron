-- Add storage columns to replace MinIO
-- Migration: MinIO → PostgreSQL + Redis cache
-- Requirements: 13.2, 13.3, 13.7
-- Files will be stored in filesystem, not in database

-- Add job definition column to jobs table
ALTER TABLE jobs 
    ADD COLUMN IF NOT EXISTS definition JSONB,
    DROP COLUMN IF EXISTS minio_definition_path;

-- Add context column to job_executions table
ALTER TABLE job_executions 
    ADD COLUMN IF NOT EXISTS context JSONB DEFAULT '{}'::jsonb,
    DROP COLUMN IF EXISTS minio_context_path;

-- Add comments
COMMENT ON COLUMN jobs.definition IS 'Job definition JSON (replaces MinIO storage)';
COMMENT ON COLUMN job_executions.context IS 'Job execution context JSON (replaces MinIO storage)';
