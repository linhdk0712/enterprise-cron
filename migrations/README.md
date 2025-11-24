# Database Migrations

This directory contains SQL migrations for the Vietnam Enterprise Cron System.

## Running Migrations

### Using sqlx-cli

```bash
# Install sqlx-cli if not already installed
cargo install sqlx-cli --no-default-features --features postgres

# Set database URL
export DATABASE_URL="postgresql://user:password@localhost:5432/vietnam_cron"

# Run all pending migrations
sqlx migrate run

# Revert the last migration
sqlx migrate revert

# Check migration status
sqlx migrate info
```

### Migration Files

Migrations are numbered sequentially and run in order:

1. `20250101000001_create_jobs_table.sql` - Jobs table with schedule and trigger configuration
2. `20250101000002_create_job_executions_table.sql` - Job execution history with idempotency
3. `20250101000003_create_variables_table.sql` - Global and job-specific variables
4. `20250101000004_create_users_table.sql` - Users for database authentication mode
5. `20250101000005_create_roles_table.sql` - Roles for RBAC
6. `20250101000006_create_user_roles_table.sql` - User-role associations
7. `20250101000007_create_job_stats_table.sql` - Aggregated job statistics

## Schema Overview

### jobs
- Stores job definitions with schedule configuration
- References MinIO for full job definition JSON
- Supports scheduled, manual, and webhook triggers

### job_executions
- Tracks individual job execution instances
- Includes idempotency key for exactly-once execution
- References MinIO for execution context

### variables
- Stores global and job-specific variables
- Supports encryption for sensitive values
- Used for template substitution in jobs

### users
- User accounts for database authentication mode
- Password hashes stored with bcrypt

### roles
- Role definitions with permissions array
- Used for RBAC authorization

### user_roles
- Many-to-many relationship between users and roles

### job_stats
- Aggregated statistics per job
- Tracks success/failure rates and consecutive failures
- Used for alerting and monitoring
