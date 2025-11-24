use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use common::models::UserClaims;

use crate::state::AppState;

/// RBAC middleware that checks user permissions
#[tracing::instrument(skip(req, next))]
pub async fn rbac_middleware(
    State(_state): State<AppState>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Get user claims from request extensions (set by auth middleware)
    let claims = req
        .extensions()
        .get::<UserClaims>()
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Determine required permission based on request path and method
    let required_permission = determine_required_permission(&req);

    // Check if user has the required permission
    if let Some(permission) = required_permission {
        if !claims.permissions.contains(&permission) {
            tracing::warn!(
                user = %claims.username,
                required_permission = %permission,
                "User lacks required permission"
            );
            return Err(StatusCode::FORBIDDEN);
        }
    }

    // Log the operation for audit purposes
    tracing::info!(
        user_id = %claims.sub,
        username = %claims.username,
        method = %req.method(),
        path = %req.uri().path(),
        "API operation"
    );

    Ok(next.run(req).await)
}

/// Determine the required permission based on request path and method
fn determine_required_permission(req: &Request) -> Option<String> {
    let path = req.uri().path();
    let method = req.method().as_str();

    // Job management permissions
    if path.starts_with("/api/jobs") {
        if path.contains("/trigger") {
            return Some("job:execute".to_string());
        }
        return match method {
            "GET" => Some("job:read".to_string()),
            "POST" => Some("job:write".to_string()),
            "PUT" => Some("job:write".to_string()),
            "DELETE" => Some("job:delete".to_string()),
            _ => None,
        };
    }

    // Execution history permissions
    if path.starts_with("/api/executions") {
        return match method {
            "GET" => Some("execution:read".to_string()),
            _ => None,
        };
    }

    // Variable management permissions
    if path.starts_with("/api/variables") {
        return match method {
            "GET" => Some("job:read".to_string()),
            "POST" | "PUT" | "DELETE" => Some("job:write".to_string()),
            _ => None,
        };
    }

    // User management permissions (admin only)
    if path.starts_with("/api/users") {
        return Some("admin:users".to_string());
    }

    // Dashboard permissions
    if path.starts_with("/dashboard") {
        return Some("job:read".to_string());
    }

    None
}
