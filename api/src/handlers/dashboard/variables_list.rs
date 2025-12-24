// Variables list handler
// Requirements: 6.5 - Display variables with pagination

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::Html,
};
use chrono::{DateTime, Utc};
use tera::Context;
use uuid::Uuid;

use super::ExecutionQueryParams;
use crate::handlers::ErrorResponse;
use crate::state::AppState;
use crate::templates::TEMPLATES;

/// Variables partial (HTMX)
#[tracing::instrument(skip(state, headers))]
pub async fn variables_partial(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ExecutionQueryParams>,
) -> Result<Html<String>, ErrorResponse> {
    let mut context = Context::new();
    context.insert("active_page", "variables");

    let is_htmx = headers.get("HX-Request").is_some();
    context.insert("is_htmx", &is_htmx);

    let limit = params.limit.unwrap_or(20);
    let offset = params.offset.unwrap_or(0);
    let page = (offset / limit) + 1;

    // Get total count
    let total_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM variables")
        .fetch_one(state.db_pool.pool())
        .await
        .unwrap_or(0);

    let total_pages = ((total_count as f64) / (limit as f64)).ceil() as i64;

    // Fetch variables with pagination
    let query = format!(
        "SELECT * FROM variables ORDER BY updated_at DESC LIMIT {} OFFSET {}",
        limit, offset
    );

    let rows = sqlx::query(&query)
        .fetch_all(state.db_pool.pool())
        .await
        .map_err(|e| ErrorResponse::new("database_error", &format!("Database error: {}", e)))?;

    // Convert variables to JSON for template
    let variables: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            use sqlx::Row;
            let id: Uuid = row.get("id");
            let name: String = row.get("name");
            let value: String = row.get("value");
            let scope_type: String = row.get("scope_type");
            let is_sensitive: bool = row.get("is_sensitive");
            let updated_at: DateTime<Utc> = row.get("updated_at");

            serde_json::json!({
                "id": id.to_string(),
                "name": name,
                "value": value,
                "scope_type": scope_type,
                "is_sensitive": is_sensitive,
                "updated_at": updated_at.format("%Y-%m-%d %H:%M:%S").to_string(),
            })
        })
        .collect();

    context.insert("variables", &variables);
    context.insert("limit", &limit);
    context.insert("offset", &offset);
    context.insert("page", &page);
    context.insert("total_pages", &total_pages);
    context.insert("total_count", &total_count);

    // If HTMX request, return only the content partial
    // Otherwise, return the full page with layout
    let template = if is_htmx {
        "_variables_content.html"
    } else {
        "variables.html"
    };

    let html = TEMPLATES
        .render(template, &context)
        .map_err(|e| ErrorResponse::new("template_error", &format!("Template error: {}", e)))?;

    Ok(Html(html))
}
