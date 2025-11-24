-- Create variables table
-- Requirements: 12.6

CREATE TABLE IF NOT EXISTS variables (
    id UUID PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    value TEXT NOT NULL,
    is_sensitive BOOLEAN NOT NULL DEFAULT false,
    scope_type VARCHAR(50) NOT NULL,
    scope_id UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (name, scope_type, scope_id)
);

CREATE INDEX idx_variables_scope ON variables(scope_type, scope_id);
CREATE INDEX idx_variables_name ON variables(name);
