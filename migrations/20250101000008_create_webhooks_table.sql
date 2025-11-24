-- Create webhooks table for webhook trigger configuration
-- Requirements: 16.1 - Unique webhook URL per job with secret key

CREATE TABLE IF NOT EXISTS webhooks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    job_id UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
    url_path VARCHAR(255) NOT NULL UNIQUE,
    secret_key VARCHAR(255) NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    rate_limit_max_requests INTEGER,
    rate_limit_window_seconds INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create indexes for efficient lookups
CREATE INDEX idx_webhooks_job_id ON webhooks(job_id);
CREATE INDEX idx_webhooks_url_path ON webhooks(url_path);
CREATE INDEX idx_webhooks_enabled ON webhooks(enabled);

-- Add comment for documentation
COMMENT ON TABLE webhooks IS 'Stores webhook trigger configuration for jobs';
COMMENT ON COLUMN webhooks.url_path IS 'Unique URL path for webhook endpoint (e.g., /webhooks/abc123)';
COMMENT ON COLUMN webhooks.secret_key IS 'Secret key for HMAC-SHA256 signature validation';
COMMENT ON COLUMN webhooks.rate_limit_max_requests IS 'Maximum requests allowed in the time window';
COMMENT ON COLUMN webhooks.rate_limit_window_seconds IS 'Time window in seconds for rate limiting';
