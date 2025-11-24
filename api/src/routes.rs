use axum::{
    routing::{delete, get, post, put},
    Router,
};
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::handlers;
use crate::middleware::{auth_middleware, rbac_middleware};
use crate::state::AppState;

/// Create the main application router with all routes and middleware
#[tracing::instrument(skip(state))]
pub fn create_router(state: AppState) -> Router {
    // CORS configuration
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Public routes (no authentication required)
    // Requirements: 19.1, 19.2 - Login page at root URL
    let public_routes = Router::new()
        .route("/", get(handlers::login::login_page))
        .route("/api/info", get(handlers::index::index))
        .route("/health", get(handlers::health::health_check))
        .route("/api/auth/login", post(handlers::auth::login))
        .route("/api/auth/login/form", post(handlers::login::login_form_submit))
        .route("/auth/set-token", get(handlers::login::set_token_page))
        .route("/api/auth/refresh", post(handlers::auth::refresh_token));

    // Protected routes (authentication required)
    let protected_routes = Router::new()
        // Job management endpoints
        .route("/api/jobs", post(handlers::jobs::create_job))
        .route("/api/jobs", get(handlers::jobs::list_jobs))
        .route("/api/jobs/:id", get(handlers::jobs::get_job))
        .route("/api/jobs/:id", put(handlers::jobs::update_job))
        .route("/api/jobs/:id", delete(handlers::jobs::delete_job))
        .route("/api/jobs/:id/trigger", post(handlers::jobs::trigger_job))
        .route("/api/jobs/:id/enable", put(handlers::jobs::enable_job))
        .route("/api/jobs/:id/disable", put(handlers::jobs::disable_job))
        // Execution history endpoints
        .route(
            "/api/executions",
            get(handlers::executions::list_executions),
        )
        .route(
            "/api/executions/:id",
            get(handlers::executions::get_execution),
        )
        // Variable management endpoints
        .route("/api/variables", post(handlers::variables::create_variable))
        .route("/api/variables", get(handlers::variables::list_variables))
        .route(
            "/api/variables/:id",
            put(handlers::variables::update_variable),
        )
        .route(
            "/api/variables/:id",
            delete(handlers::variables::delete_variable),
        )
        // User management endpoints
        // Requirements: 19.1.36-43 - User Management API with RBAC
        .route("/api/users", post(handlers::auth::create_user))
        .route("/api/users", get(handlers::users::list_users))
        .route("/api/users/:id", get(handlers::users::get_user))
        .route("/api/users/:id", put(handlers::users::update_user))
        .route("/api/users/:id", delete(handlers::users::delete_user))
        .route("/api/users/:id/roles", put(handlers::users::assign_roles))
        .route("/api/users/:id/password", put(handlers::users::update_password))
        .route("/api/roles", get(handlers::users::list_roles))
        // Webhook endpoints
        .route(
            "/api/webhooks/:path",
            post(handlers::webhooks::handle_webhook),
        )
        // Job import/export endpoints
        .route(
            "/api/jobs/export",
            post(handlers::import_export::export_job),
        )
        .route(
            "/api/jobs/export/bulk",
            post(handlers::import_export::export_jobs_bulk),
        )
        .route(
            "/api/jobs/import",
            post(handlers::import_export::import_job),
        )
        .route(
            "/api/jobs/import/bulk",
            post(handlers::import_export::import_jobs_bulk),
        )
        // Server-Sent Events for real-time updates
        .route("/api/events", get(handlers::sse::sse_handler))
        // Dashboard routes (HTMX)
        .route("/dashboard", get(handlers::dashboard::dashboard_index))
        .route("/dashboard/jobs", get(handlers::dashboard::jobs_partial))
        .route(
            "/dashboard/jobs/new",
            get(handlers::dashboard::job_form_page),
        )
        .route(
            "/dashboard/jobs/:id",
            get(handlers::dashboard::job_details_partial),
        )
        .route(
            "/dashboard/executions",
            get(handlers::dashboard::executions_partial),
        )
        .route(
            "/dashboard/variables",
            get(handlers::dashboard::variables_partial),
        )
        .layer(
            ServiceBuilder::new()
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    auth_middleware,
                ))
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    rbac_middleware,
                )),
        );

    // Metrics endpoint (no authentication for Prometheus scraping)
    let metrics_routes = Router::new().route("/metrics", get(handlers::metrics::metrics_handler));

    // Combine all routes
    Router::new()
        .merge(public_routes)
        .merge(protected_routes)
        .merge(metrics_routes)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(cors),
        )
        .with_state(state)
}
