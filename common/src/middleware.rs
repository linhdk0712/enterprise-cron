// Authentication and RBAC middleware for Axum
// Requirements: 10.4, 10.5, 10.6, 10.7, 10.8, 10.9, 10.10

use crate::auth::{DatabaseAuthService, KeycloakJwtService};
use crate::errors::AuthError;
use crate::models::UserClaims;
use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::sync::Arc;
use tracing::{error, instrument};

/// Authentication state that can be used in Axum handlers
#[derive(Clone)]
pub enum AuthService {
    Database(Arc<DatabaseAuthService>),
    Keycloak(Arc<KeycloakJwtService>),
}

impl AuthService {
    /// Validate a token and return claims
    #[instrument(skip(self, token))]
    pub async fn validate_token(&self, token: &str) -> Result<UserClaims, AuthError> {
        match self {
            AuthService::Database(service) => service.validate_token(token),
            AuthService::Keycloak(service) => service.validate_token(token).await,
        }
    }
}

/// Extension type to store authenticated user claims in request extensions
#[derive(Clone, Debug)]
pub struct AuthenticatedUser(pub UserClaims);

/// Authentication middleware that validates JWT tokens
/// Requirements: 10.4 - Create Axum middleware for JWT validation
#[instrument(skip(auth_service, request, next))]
pub async fn auth_middleware(
    State(auth_service): State<Arc<AuthService>>,
    mut request: Request,
    next: Next,
) -> Result<Response, AuthError> {
    // Extract Authorization header
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            error!("Missing Authorization header");
            AuthError::InvalidToken("Missing Authorization header".to_string())
        })?;

    // Extract Bearer token
    let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
        error!("Invalid Authorization header format");
        AuthError::InvalidToken("Invalid Authorization header format".to_string())
    })?;

    // Validate token and extract claims
    let claims = auth_service.validate_token(token).await?;

    // Store claims in request extensions for use in handlers
    request.extensions_mut().insert(AuthenticatedUser(claims));

    Ok(next.run(request).await)
}

/// Permission checking function
/// Requirements: 10.5, 10.6, 10.7, 10.8, 10.9 - RBAC permission enforcement
pub fn check_permission(
    user: &AuthenticatedUser,
    required_permission: &str,
) -> Result<(), AuthError> {
    if !user
        .0
        .permissions
        .contains(&required_permission.to_string())
    {
        error!(
            user_id = %user.0.sub,
            username = %user.0.username,
            required_permission = %required_permission,
            "Insufficient permissions"
        );
        return Err(AuthError::InsufficientPermissions(
            required_permission.to_string(),
        ));
    }

    tracing::debug!(
        user_id = %user.0.sub,
        username = %user.0.username,
        permission = %required_permission,
        "Permission check passed"
    );

    Ok(())
}

/// RBAC middleware for job:read permission
/// Requirements: 10.5 - Verify user has "job:read" permission
#[instrument(skip(request, next))]
pub async fn require_job_read(request: Request, next: Next) -> Result<Response, AuthError> {
    let user = request
        .extensions()
        .get::<AuthenticatedUser>()
        .ok_or_else(|| {
            error!("User not authenticated");
            AuthError::InvalidToken("User not authenticated".to_string())
        })?
        .clone();

    check_permission(&user, "job:read")?;
    Ok(next.run(request).await)
}

/// RBAC middleware for job:write permission
/// Requirements: 10.6 - Verify user has "job:write" permission
#[instrument(skip(request, next))]
pub async fn require_job_write(request: Request, next: Next) -> Result<Response, AuthError> {
    let user = request
        .extensions()
        .get::<AuthenticatedUser>()
        .ok_or_else(|| {
            error!("User not authenticated");
            AuthError::InvalidToken("User not authenticated".to_string())
        })?
        .clone();

    check_permission(&user, "job:write")?;
    Ok(next.run(request).await)
}

/// RBAC middleware for job:execute permission
/// Requirements: 10.7 - Verify user has "job:execute" permission
#[instrument(skip(request, next))]
pub async fn require_job_execute(request: Request, next: Next) -> Result<Response, AuthError> {
    let user = request
        .extensions()
        .get::<AuthenticatedUser>()
        .ok_or_else(|| {
            error!("User not authenticated");
            AuthError::InvalidToken("User not authenticated".to_string())
        })?
        .clone();

    check_permission(&user, "job:execute")?;
    Ok(next.run(request).await)
}

/// RBAC middleware for job:delete permission
/// Requirements: 10.8 - Verify user has "job:delete" permission
#[instrument(skip(request, next))]
pub async fn require_job_delete(request: Request, next: Next) -> Result<Response, AuthError> {
    let user = request
        .extensions()
        .get::<AuthenticatedUser>()
        .ok_or_else(|| {
            error!("User not authenticated");
            AuthError::InvalidToken("User not authenticated".to_string())
        })?
        .clone();

    check_permission(&user, "job:delete")?;
    Ok(next.run(request).await)
}

/// RBAC middleware for execution:read permission
/// Requirements: 10.9 - Verify user has "execution:read" permission
#[instrument(skip(request, next))]
pub async fn require_execution_read(request: Request, next: Next) -> Result<Response, AuthError> {
    let user = request
        .extensions()
        .get::<AuthenticatedUser>()
        .ok_or_else(|| {
            error!("User not authenticated");
            AuthError::InvalidToken("User not authenticated".to_string())
        })?
        .clone();

    check_permission(&user, "execution:read")?;
    Ok(next.run(request).await)
}

/// Audit logging middleware
/// Requirements: 10.10 - Log user identity for all operations
#[instrument(skip(request, next))]
pub async fn audit_logging_middleware(request: Request, next: Next) -> Response {
    // Extract user information if authenticated
    let user_info = request
        .extensions()
        .get::<AuthenticatedUser>()
        .map(|user| (user.0.sub.clone(), user.0.username.clone()));

    // Extract request information
    let method = request.method().clone();
    let uri = request.uri().clone();
    let path = uri.path().to_string();

    // Extract resource ID from path if present (e.g., /api/jobs/{id})
    let resource_id = extract_resource_id(&path);

    // Run the request
    let response = next.run(request).await;
    let status = response.status();

    // Log audit entry
    if let Some((user_id, username)) = user_info {
        tracing::info!(
            user_id = %user_id,
            username = %username,
            method = %method,
            path = %path,
            resource_id = ?resource_id,
            status = %status.as_u16(),
            operation = %determine_operation(&method, &path),
            "Audit log: User operation"
        );
    } else {
        // Log unauthenticated requests (e.g., login attempts)
        tracing::info!(
            method = %method,
            path = %path,
            status = %status.as_u16(),
            "Audit log: Unauthenticated request"
        );
    }

    response
}

/// Extract resource ID from path
/// Examples: /api/jobs/123 -> Some("123"), /api/jobs -> None
fn extract_resource_id(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('/').collect();

    // Look for UUID patterns in path segments
    for part in parts {
        if part.len() == 36 && part.contains('-') {
            // Likely a UUID
            return Some(part.to_string());
        }
    }

    None
}

/// Determine operation type from HTTP method and path
fn determine_operation(method: &axum::http::Method, path: &str) -> String {
    let resource = if path.contains("/jobs") {
        "job"
    } else if path.contains("/executions") {
        "execution"
    } else if path.contains("/variables") {
        "variable"
    } else if path.contains("/users") {
        "user"
    } else if path.contains("/auth") {
        "auth"
    } else {
        "unknown"
    };

    match method.as_str() {
        "GET" => format!("read_{}", resource),
        "POST" => {
            if path.contains("/trigger") {
                "execute_job".to_string()
            } else if path.contains("/login") {
                "login".to_string()
            } else {
                format!("create_{}", resource)
            }
        }
        "PUT" | "PATCH" => format!("update_{}", resource),
        "DELETE" => format!("delete_{}", resource),
        _ => format!("unknown_{}", resource),
    }
}

/// Implement IntoResponse for AuthError to handle authentication errors
/// Requirements: 10.4 - Handle authentication errors (401)
impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AuthError::InvalidCredentials => (
                StatusCode::UNAUTHORIZED,
                "INVALID_CREDENTIALS",
                "Invalid username or password",
            ),
            AuthError::InvalidToken(_) => (
                StatusCode::UNAUTHORIZED,
                "INVALID_TOKEN",
                "Invalid or malformed token",
            ),
            AuthError::TokenExpired => (
                StatusCode::UNAUTHORIZED,
                "TOKEN_EXPIRED",
                "Token has expired",
            ),
            AuthError::InsufficientPermissions(_) => (
                StatusCode::FORBIDDEN,
                "INSUFFICIENT_PERMISSIONS",
                "Insufficient permissions",
            ),
            AuthError::UserNotFound(_) => {
                (StatusCode::NOT_FOUND, "USER_NOT_FOUND", "User not found")
            }
            AuthError::KeycloakError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "KEYCLOAK_ERROR",
                "Authentication service error",
            ),
            AuthError::AuthenticationFailed(_) => (
                StatusCode::UNAUTHORIZED,
                "AUTHENTICATION_FAILED",
                "Authentication failed",
            ),
        };

        error!(error = %self, status = %status, "Authentication error");

        let body = Json(json!({
            "error": {
                "code": code,
                "message": message,
                "details": self.to_string()
            }
        }));

        (status, body).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authenticated_user_clone() {
        let claims = UserClaims {
            sub: "user-123".to_string(),
            username: "testuser".to_string(),
            permissions: vec!["job:read".to_string()],
            exp: 0,
            iat: 0,
        };

        let user = AuthenticatedUser(claims);
        let cloned = user.clone();

        assert_eq!(user.0.sub, cloned.0.sub);
        assert_eq!(user.0.username, cloned.0.username);
    }

    #[test]
    fn test_auth_error_into_response() {
        let err = AuthError::InvalidCredentials;
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let err = AuthError::InsufficientPermissions("job:write".to_string());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[test]
    fn test_extract_resource_id() {
        assert_eq!(
            extract_resource_id("/api/jobs/550e8400-e29b-41d4-a716-446655440000"),
            Some("550e8400-e29b-41d4-a716-446655440000".to_string())
        );
        assert_eq!(extract_resource_id("/api/jobs"), None);
        assert_eq!(extract_resource_id("/api/jobs/123"), None); // Not a UUID
    }

    #[test]
    fn test_determine_operation() {
        use axum::http::Method;

        assert_eq!(determine_operation(&Method::GET, "/api/jobs"), "read_job");
        assert_eq!(
            determine_operation(&Method::POST, "/api/jobs"),
            "create_job"
        );
        assert_eq!(
            determine_operation(&Method::POST, "/api/jobs/123/trigger"),
            "execute_job"
        );
        assert_eq!(
            determine_operation(&Method::PUT, "/api/jobs/123"),
            "update_job"
        );
        assert_eq!(
            determine_operation(&Method::DELETE, "/api/jobs/123"),
            "delete_job"
        );
        assert_eq!(
            determine_operation(&Method::GET, "/api/executions"),
            "read_execution"
        );
    }

    #[test]
    fn test_check_permission_success() {
        let claims = UserClaims {
            sub: "user-123".to_string(),
            username: "testuser".to_string(),
            permissions: vec!["job:read".to_string(), "job:write".to_string()],
            exp: 0,
            iat: 0,
        };
        let user = AuthenticatedUser(claims);

        assert!(check_permission(&user, "job:read").is_ok());
        assert!(check_permission(&user, "job:write").is_ok());
    }

    #[test]
    fn test_check_permission_failure() {
        let claims = UserClaims {
            sub: "user-123".to_string(),
            username: "testuser".to_string(),
            permissions: vec!["job:read".to_string()],
            exp: 0,
            iat: 0,
        };
        let user = AuthenticatedUser(claims);

        let result = check_permission(&user, "job:write");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AuthError::InsufficientPermissions(_)
        ));
    }
}
