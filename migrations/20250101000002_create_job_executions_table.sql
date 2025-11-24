-- Create job_executions table
-- Requirements: 12.6

CREATE TABLE IF NOT EXISTS job_executions (
    id UUID PRIMARY KEY,
    job_id UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    idempotency_key VARCHAR(255) NOT NULL UNIQUE,
    status VARCHAR(50) NOT NULL,
    attempt INTEGER NOT NULL DEFAULT 1,
    trigger_source VARCHAR(50) NOT NULL,
    trigger_metadata JSONB,
    current_step VARCHAR(255),
    minio_context_path VARCHAR(500) NOT NULL,
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    result TEXT,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_job_executions_job_id ON job_executions(job_id);
CREATE INDEX idx_job_executions_status ON job_executions(status);
CREATE INDEX idx_job_executions_created_at ON job_executions(created_at);
CREATE INDEX idx_job_executions_trigger_source ON job_executions(trigger_source);
CREATE INDEX idx_job_executions_idempotency_key ON job_executions(idempotency_key);
