// Job form page handler
// Requirements: 18.1 - Visual form builder for job creation

use axum::{extract::State, response::Html};
use tera::Context;

use crate::handlers::ErrorResponse;
use crate::state::AppState;
use crate::templates::TEMPLATES;

/// Job form page (HTMX)
#[tracing::instrument(skip(_state))]
pub async fn job_form_page(State(_state): State<AppState>) -> Result<Html<String>, ErrorResponse> {
    let mut context = Context::new();
    context.insert("active_page", "jobs");

    let html = TEMPLATES
        .render("job_form.html", &context)
        .map_err(|e| ErrorResponse::new("template_error", &format!("Template error: {}", e)))?;

    Ok(Html(html))
}
