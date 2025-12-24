use common::models::JobExecution;
use common::queue::publisher::JobMessage;
use sqlx::postgres::PgPoolOptions;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://cronuser:cronpass@localhost:5432/vietnam_cron".to_string()
    });
    let nats_url = env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".to_string());

    println!("Connecting to DB: {}", database_url);
    println!("Connecting to NATS: {}", nats_url);

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    let nats_client = async_nats::connect(&nats_url).await?;
    let jetstream = async_nats::jetstream::new(nats_client);

    // Find stuck pending jobs (created > 5 minutes ago and still pending)
    let stuck_executions = sqlx::query_as::<_, JobExecution>(
        r#"
        SELECT 
            id, job_id, idempotency_key, status, 
            attempt, trigger_source, trigger_metadata, 
            current_step, context, started_at, completed_at, result, error, 
            created_at
        FROM job_executions
        WHERE status = 'pending'
          AND created_at < NOW() - INTERVAL '5 minutes'
        "#,
    )
    .fetch_all(&pool)
    .await?;

    println!("Found {} stuck pending executions", stuck_executions.len());

    for execution in stuck_executions {
        println!(
            "Re-queuing execution: {} (Job: {})",
            execution.id, execution.job_id
        );

        let subject = format!("jobs.{}", execution.job_id);
        let message = JobMessage::from(&execution);
        let payload = serde_json::to_vec(&message)?;

        let mut headers = async_nats::HeaderMap::new();
        headers.insert("Nats-Msg-Id", execution.idempotency_key.as_str());
        headers.insert("Job-Id", execution.job_id.to_string().as_str());
        headers.insert("Execution-Id", execution.id.to_string().as_str());

        jetstream
            .publish_with_headers(subject, headers, payload.into())
            .await?
            .await?;

        println!("  -> Published to NATS");
    }

    Ok(())
}
