-- Create job_stats table for aggregated statistics
-- Requirements: 12.6

CREATE TABLE IF NOT EXISTS job_stats (
    job_id UUID PRIMARY KEY REFERENCES jobs(id) ON DELETE CASCADE,
    total_executions BIGINT NOT NULL DEFAULT 0,
    successful_executions BIGINT NOT NULL DEFAULT 0,
    failed_executions BIGINT NOT NULL DEFAULT 0,
    last_execution_at TIMESTAMPTZ,
    last_success_at TIMESTAMPTZ,
    last_failure_at TIMESTAMPTZ,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_job_stats_consecutive_failures ON job_stats(consecutive_failures);
CREATE INDEX idx_job_stats_last_execution_at ON job_stats(last_execution_at);
