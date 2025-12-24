use sqlx::postgres::PgPoolOptions;
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
    let execution_id = "e3f59c1e-fdec-45db-a7fc-9e5c9e0e301d";

    println!(
        "Updating execution {} for job {} to 'failed'",
        execution_id, job_id
    );

    let result = sqlx::query(
        r#"
        UPDATE job_executions
        SET status = 'failed', 
            error = 'Manually failed by system administrator to resolve stuck state',
            completed_at = NOW()
        WHERE id = $1::uuid AND job_id = $2::uuid
        "#,
    )
    .bind(execution_id)
    .bind(job_id)
    .execute(&pool)
    .await?;

    println!("Updated {} rows", result.rows_affected());

    Ok(())
}
