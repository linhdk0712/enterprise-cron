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
/// Requirements: 19.1.15-43 - Permission-based endpoint access control
fn determine_required_permission(req: &Request) -> Option<String> {
    let path = req.uri().path();
    let method = req.method().as_str();

    // Job management permissions
    // Requirements: 19.1.15-21 - Jobs API with RBAC
    if path.starts_with("/api/jobs") {
        if path.contains("/trigger") || path.contains("/enable") || path.contains("/disable") {
            return Some("job:execute".to_string());
        }
        if path.contains("/export") {
            return Some("job:export".to_string());
        }
        if path.contains("/import") {
            return Some("job:import".to_string());
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
    // Requirements: 19.1.22-25 - Executions API with RBAC
    if path.starts_with("/api/executions") {
        if path.contains("/stop") {
            return Some("execution:stop".to_string());
        }
        return match method {
            "GET" => Some("execution:read".to_string()),
            _ => None,
        };
    }

    // Variable management permissions
    // Requirements: 19.1.26-31 - Variables API with RBAC
    if path.starts_with("/api/variables") {
        return match method {
            "GET" => Some("variable:read".to_string()),
            "POST" => Some("variable:write".to_string()),
            "PUT" => Some("variable:write".to_string()),
            "DELETE" => Some("variable:write".to_string()),
            _ => None,
        };
    }

    // Webhook management permissions
    // Requirements: 19.1.32-35 - Webhooks API with RBAC
    if path.starts_with("/api/webhooks") && !path.contains("handle") {
        return match method {
            "GET" => Some("webhook:read".to_string()),
            "POST" | "PUT" | "DELETE" => Some("webhook:write".to_string()),
            _ => None,
        };
    }

    // User management permissions (admin only)
    // Requirements: 19.1.36-43 - User Management API with RBAC
    if path.starts_with("/api/users") {
        // Get user ID from path if present (for viewing own profile)
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() >= 4 && method == "GET" {
            // Allow users to view their own profile
            // The handler will check if user_id matches claims.sub
            return Some("job:read".to_string()); // Minimum permission for authenticated users
        }
        return Some("user:manage".to_string());
    }

    // System configuration endpoints (admin only)
    // Requirements: 19.1.55-58 - System Config API (admin-only)
    if path.starts_with("/api/system/config") {
        return Some("system:config".to_string());
    }

    // Audit log endpoints (admin only)
    // Requirements: 19.1.59-60 - Audit Logs API (admin-only)
    if path.starts_with("/api/system/audit-logs") {
        return Some("system:audit".to_string());
    }

    // Dashboard permissions
    // Requirements: 19.1.61-67 - Dashboard with RBAC
    if path.starts_with("/dashboard") {
        return Some("dashboard:user".to_string()); // Minimum permission for dashboard access
    }

    None
}
