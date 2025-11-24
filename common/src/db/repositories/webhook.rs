use crate::errors::DatabaseError;
use crate::models::Webhook;
use sqlx::PgPool;
use uuid::Uuid;

/// WebhookRepository handles database operations for webhooks
/// Requirements: 16.1, 16.12 - Webhook URL generation and management
pub struct WebhookRepository {
    pool: PgPool,
}

impl WebhookRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new webhook for a job
    /// Requirements: 16.1 - Generate unique webhook URL for job
    #[tracing::instrument(skip(self))]
    pub async fn create(
        &self,
        job_id: Uuid,
        url_path: String,
        secret_key: String,
        rate_limit_max_requests: Option<i32>,
        rate_limit_window_seconds: Option<i32>,
    ) -> Result<Webhook, DatabaseError> {
        let webhook = sqlx::query_as!(
            Webhook,
            r#"
            INSERT INTO webhooks (job_id, url_path, secret_key, enabled, rate_limit_max_requests, rate_limit_window_seconds)
            VALUES ($1, $2, $3, true, $4, $5)
            RETURNING id, job_id, url_path, secret_key, enabled, rate_limit_max_requests, rate_limit_window_seconds, created_at, updated_at
            "#,
            job_id,
            url_path,
            secret_key,
            rate_limit_max_requests,
            rate_limit_window_seconds
        )
        .fetch_one(&self.pool)
        .await?;

        tracing::info!(
            webhook_id = %webhook.id,
            job_id = %job_id,
            url_path = %url_path,
            "Created webhook"
        );

        Ok(webhook)
    }

    /// Find webhook by URL path
    /// Requirements: 16.2 - Lookup webhook by URL for incoming requests
    #[tracing::instrument(skip(self))]
    pub async fn find_by_url_path(&self, url_path: &str) -> Result<Option<Webhook>, DatabaseError> {
        let webhook = sqlx::query_as!(
            Webhook,
            r#"
            SELECT id, job_id, url_path, secret_key, enabled, rate_limit_max_requests, rate_limit_window_seconds, created_at, updated_at
            FROM webhooks
            WHERE url_path = $1
            "#,
            url_path
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(webhook)
    }

    /// Find webhook by job ID
    /// Requirements: 16.1 - Retrieve webhook configuration for a job
    #[tracing::instrument(skip(self))]
    pub async fn find_by_job_id(&self, job_id: Uuid) -> Result<Option<Webhook>, DatabaseError> {
        let webhook = sqlx::query_as!(
            Webhook,
            r#"
            SELECT id, job_id, url_path, secret_key, enabled, rate_limit_max_requests, rate_limit_window_seconds, created_at, updated_at
            FROM webhooks
            WHERE job_id = $1
            "#,
            job_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(webhook)
    }

    /// Update webhook secret key (regenerate)
    /// Requirements: 16.12 - Webhook URL regeneration invalidates previous URL
    #[tracing::instrument(skip(self))]
    pub async fn regenerate_secret(
        &self,
        webhook_id: Uuid,
        new_url_path: String,
        new_secret_key: String,
    ) -> Result<Webhook, DatabaseError> {
        let webhook = sqlx::query_as!(
            Webhook,
            r#"
            UPDATE webhooks
            SET url_path = $2, secret_key = $3, updated_at = NOW()
            WHERE id = $1
            RETURNING id, job_id, url_path, secret_key, enabled, rate_limit_max_requests, rate_limit_window_seconds, created_at, updated_at
            "#,
            webhook_id,
            new_url_path,
            new_secret_key
        )
        .fetch_one(&self.pool)
        .await?;

        tracing::info!(
            webhook_id = %webhook_id,
            new_url_path = %new_url_path,
            "Regenerated webhook URL and secret"
        );

        Ok(webhook)
    }

    /// Enable or disable a webhook
    /// Requirements: 16.10 - Disabled job webhooks should be rejected
    #[tracing::instrument(skip(self))]
    pub async fn set_enabled(&self, webhook_id: Uuid, enabled: bool) -> Result<(), DatabaseError> {
        sqlx::query!(
            r#"
            UPDATE webhooks
            SET enabled = $2, updated_at = NOW()
            WHERE id = $1
            "#,
            webhook_id,
            enabled
        )
        .execute(&self.pool)
        .await?;

        tracing::info!(
            webhook_id = %webhook_id,
            enabled = enabled,
            "Updated webhook enabled status"
        );

        Ok(())
    }

    /// Delete a webhook
    /// Requirements: 16.12 - Webhook URL invalidation
    #[tracing::instrument(skip(self))]
    pub async fn delete(&self, webhook_id: Uuid) -> Result<(), DatabaseError> {
        sqlx::query!(
            r#"
            DELETE FROM webhooks
            WHERE id = $1
            "#,
            webhook_id
        )
        .execute(&self.pool)
        .await?;

        tracing::info!(webhook_id = %webhook_id, "Deleted webhook");

        Ok(())
    }

    /// Update rate limit configuration
    /// Requirements: 16.11 - Webhook rate limiting configuration
    #[tracing::instrument(skip(self))]
    pub async fn update_rate_limit(
        &self,
        webhook_id: Uuid,
        max_requests: Option<i32>,
        window_seconds: Option<i32>,
    ) -> Result<(), DatabaseError> {
        sqlx::query!(
            r#"
            UPDATE webhooks
            SET rate_limit_max_requests = $2, rate_limit_window_seconds = $3, updated_at = NOW()
            WHERE id = $1
            "#,
            webhook_id,
            max_requests,
            window_seconds
        )
        .execute(&self.pool)
        .await?;

        tracing::info!(
            webhook_id = %webhook_id,
            max_requests = ?max_requests,
            window_seconds = ?window_seconds,
            "Updated webhook rate limit"
        );

        Ok(())
    }
}
