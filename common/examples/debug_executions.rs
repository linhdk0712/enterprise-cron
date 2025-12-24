use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgresql://cronuser:cronpass@localhost:5432/vietnam_cron".to_string()
    });

    println!("Connecting to {}", database_url);

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await?;

    let job_id = "d733c6a0-03ea-4eaa-9958-23eb1c2c598c";

    println!("Querying executions for job {}", job_id);

    let rows = sqlx::query(
        r#"
        SELECT id, status, created_at, trigger_source, trigger_metadata
        FROM job_executions
        WHERE job_id = $1::uuid
        ORDER BY created_at DESC
        LIMIT 10
        "#,
    )
    .bind(job_id)
    .fetch_all(&pool)
    .await?;

    println!("Found {} executions", rows.len());

    for row in rows {
        let id: uuid::Uuid = row.try_get("id")?;
        let status: String = row.try_get("status")?;
        let created_at: chrono::DateTime<chrono::Utc> = row.try_get("created_at")?;
        let trigger_source: String = row.try_get("trigger_source")?;

        // Try to get trigger_metadata as Option<serde_json::Value>
        // We use .ok() to swallow errors if mapping fails, but print if it's None
        let trigger_metadata_result: Result<Option<serde_json::Value>, _> =
            row.try_get("trigger_metadata");

        println!(
            "Execution: {} | Status: {} | Created: {} | Source: {}",
            id, status, created_at, trigger_source
        );

        match trigger_metadata_result {
            Ok(meta) => println!("  Metadata: {:?}", meta),
            Err(e) => println!("  Metadata Error: {}", e),
        }
    }

    // Also check for running executions
    let running_rows = sqlx::query(
        r#"
        SELECT id, status
        FROM job_executions
        WHERE job_id = $1::uuid AND (status = 'running' OR status = 'pending')
        "#,
    )
    .bind(job_id)
    .fetch_all(&pool)
    .await?;

    println!("Running/Pending executions: {}", running_rows.len());
    for row in running_rows {
        let id: uuid::Uuid = row.try_get("id")?;
        let status: String = row.try_get("status")?;
        println!("  - {} [{}]", id, status);
    }

    Ok(())
}
