-- Create jobs table
-- Requirements: 12.6

CREATE TABLE IF NOT EXISTS jobs (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    schedule_type VARCHAR(50),
    schedule_config JSONB,
    trigger_config JSONB NOT NULL DEFAULT '{"scheduled": true, "manual": false, "webhook": null}'::jsonb,
    minio_definition_path VARCHAR(500) NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    timeout_seconds INTEGER NOT NULL DEFAULT 300,
    max_retries INTEGER NOT NULL DEFAULT 10,
    allow_concurrent BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_jobs_enabled ON jobs(enabled);
CREATE INDEX idx_jobs_schedule_type ON jobs(schedule_type);
CREATE INDEX idx_jobs_created_at ON jobs(created_at);
