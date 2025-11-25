use axum::{
    extract::State,
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    Json,
};
use axum_extra::extract::cookie::{Cookie, SameSite};
use chrono::{Duration, Utc};
use common::auth::{DatabaseAuthService, JwtService};
use common::db::repositories::user::UserRepository;
use common::models::User;
use serde::{Deserialize, Serialize};

use crate::handlers::{ErrorResponse, SuccessResponse};
use crate::middleware::rate_limit::RateLimiter;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub expires_at: i64,
}

#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub email: Option<String>,
    pub role_ids: Vec<uuid::Uuid>,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: uuid::Uuid,
    pub username: String,
    pub email: Option<String>,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
            enabled: user.enabled,
            created_at: user.created_at,
        }
    }
}

/// Extract client IP from headers
/// Requirements: 19.13 - IP-based rate limiting
fn get_client_ip(headers: &HeaderMap) -> String {
    // Try X-Forwarded-For header first (for proxy/load balancer)
    if let Some(forwarded) = headers.get("X-Forwarded-For") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            if let Some(first_ip) = forwarded_str.split(',').next() {
                return first_ip.trim().to_string();
            }
        }
    }

    // Try X-Real-IP header
    if let Some(real_ip) = headers.get("X-Real-IP") {
        if let Ok(ip_str) = real_ip.to_str() {
            return ip_str.to_string();
        }
    }

    // Fallback to unknown if we can't determine IP
    "unknown".to_string()
}

/// Login endpoint (database mode)
/// Requirements: 10.2 - Validate credentials against database
/// Requirements: 10.3 - Generate JWT token on successful login
/// Requirements: 19.13, 19.14 - Rate limiting for login attempts
/// Requirements: 19.4 - Secure token storage with httpOnly cookies
#[tracing::instrument(skip(state, req))]
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<LoginRequest>,
) -> Result<Response, ErrorResponse> {
    // Get client IP for rate limiting
    let client_ip = get_client_ip(&headers);

    // Check rate limit
    let rate_limiter = RateLimiter::new(state.redis_client.clone());
    if let Err(error_msg) = rate_limiter.check_login_rate_limit(&client_ip).await {
        tracing::warn!(
            ip = %client_ip,
            username = %req.username,
            error = %error_msg,
            "Login rate limit exceeded"
        );
        return Err(ErrorResponse::new("rate_limit_exceeded", error_msg));
    }

    // Validate input
    if req.username.is_empty() {
        return Err(ErrorResponse::new(
            "validation_error",
            "Username is required",
        ));
    }

    if req.password.is_empty() {
        return Err(ErrorResponse::new(
            "validation_error",
            "Password is required",
        ));
    }

    // Create JWT service from config
    let jwt_secret = &state.config.auth.jwt_secret;
    let jwt_expiry_hours = state.config.auth.jwt_expiration_hours;
    let jwt_service = JwtService::new(jwt_secret, jwt_expiry_hours);

    // Create user repository and auth service
    let user_repository = UserRepository::new(state.db_pool.clone());
    let auth_service = DatabaseAuthService::new(jwt_service.clone(), user_repository);

    // Authenticate user
    let token = auth_service
        .login(&req.username, &req.password)
        .await
        .map_err(|e| {
            tracing::warn!(
                username = %req.username,
                ip = %client_ip,
                error = %e,
                "Login failed"
            );

            // Record failed login attempt
            let rate_limiter = RateLimiter::new(state.redis_client.clone());
            let client_ip_clone = client_ip.clone();
            tokio::spawn(async move {
                if let Err(e) = rate_limiter.record_failed_login(&client_ip_clone).await {
                    tracing::error!(error = %e, "Failed to record login attempt");
                }
            });

            match e {
                common::errors::AuthError::InvalidCredentials => {
                    ErrorResponse::new("unauthorized", "Invalid username or password")
                }
                common::errors::AuthError::AuthenticationFailed(msg) => {
                    ErrorResponse::new("unauthorized", msg)
                }
                _ => ErrorResponse::new("internal_error", "Authentication failed"),
            }
        })?;

    // Reset rate limit on successful login
    let rate_limiter = RateLimiter::new(state.redis_client.clone());
    let client_ip_clone = client_ip.clone();
    tokio::spawn(async move {
        if let Err(e) = rate_limiter.reset_login_rate_limit(&client_ip_clone).await {
            tracing::error!(error = %e, "Failed to reset rate limit");
        }
    });

    // Calculate expiration time
    let expires_at = (Utc::now() + Duration::hours(jwt_expiry_hours as i64)).timestamp();

    tracing::info!(
        username = %req.username,
        ip = %client_ip,
        "User logged in successfully"
    );

    // Create httpOnly cookie for secure token storage
    // Requirements: 19.4 - Secure token storage with httpOnly cookies
    let cookie = Cookie::build(("auth_token", token.clone()))
        .path("/")
        .max_age(time::Duration::hours(jwt_expiry_hours as i64))
        .same_site(SameSite::Lax)
        .http_only(true)
        .secure(false) // Set to true in production with HTTPS
        .build();

    // Build response with cookie and JSON body
    let mut response = Json(SuccessResponse::new(LoginResponse {
        token,
        expires_at,
    }))
    .into_response();

    response.headers_mut().insert(
        header::SET_COOKIE,
        cookie.to_string().parse().unwrap(),
    );

    Ok(response)
}

/// Refresh token endpoint
/// Requirements: 10.3 - Generate new JWT token from valid existing token
#[tracing::instrument(skip(state, req))]
pub async fn refresh_token(
    State(state): State<AppState>,
    Json(req): Json<RefreshTokenRequest>,
) -> Result<Json<SuccessResponse<LoginResponse>>, ErrorResponse> {
    // Validate input
    if req.token.is_empty() {
        return Err(ErrorResponse::new("validation_error", "Token is required"));
    }

    // Create JWT service from config
    let jwt_secret = &state.config.auth.jwt_secret;
    let jwt_expiry_hours = state.config.auth.jwt_expiration_hours;
    let jwt_service = JwtService::new(jwt_secret, jwt_expiry_hours);

    // Validate existing token
    let claims = jwt_service.validate_token(&req.token).map_err(|e| {
        tracing::warn!(error = %e, "Token validation failed");
        match e {
            common::errors::AuthError::TokenExpired => {
                ErrorResponse::new("unauthorized", "Token has expired")
            }
            common::errors::AuthError::InvalidToken(msg) => ErrorResponse::new("unauthorized", msg),
            _ => ErrorResponse::new("unauthorized", "Invalid token"),
        }
    })?;

    // Generate new token with same claims
    let new_token = jwt_service
        .encode_token(&claims.sub, &claims.username, claims.permissions)
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to generate new token");
            ErrorResponse::new("internal_error", "Failed to generate new token")
        })?;

    // Calculate expiration time
    let expires_at = (Utc::now() + Duration::hours(jwt_expiry_hours as i64)).timestamp();

    tracing::info!(user_id = %claims.sub, username = %claims.username, "Token refreshed");

    Ok(Json(SuccessResponse::new(LoginResponse {
        token: new_token,
        expires_at,
    })))
}

/// Create user endpoint (database mode)
/// Requirements: 10.2 - Create user with bcrypt-hashed password
/// Requirements: 10.13 - Store user with role assignments
#[tracing::instrument(skip(state, req))]
pub async fn create_user(
    State(state): State<AppState>,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<SuccessResponse<UserResponse>>, ErrorResponse> {
    // Validate input
    if req.username.is_empty() {
        return Err(ErrorResponse::new(
            "validation_error",
            "Username is required",
        ));
    }

    if req.password.is_empty() {
        return Err(ErrorResponse::new(
            "validation_error",
            "Password is required",
        ));
    }

    if req.password.len() < 8 {
        return Err(ErrorResponse::new(
            "validation_error",
            "Password must be at least 8 characters",
        ));
    }

    // Create JWT service from config
    let jwt_secret = &state.config.auth.jwt_secret;
    let jwt_expiry_hours = state.config.auth.jwt_expiration_hours;
    let jwt_service = JwtService::new(jwt_secret, jwt_expiry_hours);

    // Create user repository and auth service
    let user_repository = UserRepository::new(state.db_pool.clone());
    let auth_service = DatabaseAuthService::new(jwt_service, user_repository.clone());

    // Create user
    let user = auth_service
        .create_user(&req.username, &req.password, req.email.clone())
        .await
        .map_err(|e| {
            tracing::error!(
                username = %req.username,
                error = %e,
                "Failed to create user"
            );
            match e {
                common::errors::AuthError::AuthenticationFailed(msg)
                    if msg.contains("already exists") =>
                {
                    ErrorResponse::new("conflict", "Username already exists")
                }
                _ => ErrorResponse::new("internal_error", "Failed to create user"),
            }
        })?;

    // Assign roles to user
    for role_id in &req.role_ids {
        user_repository
            .assign_role(user.id, *role_id)
            .await
            .map_err(|e| {
                tracing::error!(
                    user_id = %user.id,
                    role_id = %role_id,
                    error = %e,
                    "Failed to assign role to user"
                );
                ErrorResponse::new("internal_error", "Failed to assign roles to user")
            })?;
    }

    tracing::info!(
        user_id = %user.id,
        username = %user.username,
        role_count = req.role_ids.len(),
        "User created successfully"
    );

    Ok(Json(SuccessResponse::new(UserResponse::from(user))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_login_request_deserialization() {
        let json = r#"{"username": "testuser", "password": "testpass"}"#;
        let req: LoginRequest = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(req.username, "testuser");
        assert_eq!(req.password, "testpass");
    }

    #[test]
    fn test_login_response_serialization() {
        let response = LoginResponse {
            token: "test-token".to_string(),
            expires_at: 1234567890,
        };
        let json = serde_json::to_string(&response).expect("Failed to serialize");
        assert!(json.contains("test-token"));
        assert!(json.contains("1234567890"));
    }

    #[test]
    fn test_refresh_token_request_deserialization() {
        let json = r#"{"token": "old-token"}"#;
        let req: RefreshTokenRequest = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(req.token, "old-token");
    }

    #[test]
    fn test_create_user_request_deserialization() {
        let json = r#"{
            "username": "newuser",
            "password": "newpass123",
            "email": "user@example.com",
            "role_ids": ["550e8400-e29b-41d4-a716-446655440000"]
        }"#;
        let req: CreateUserRequest = serde_json::from_str(json).expect("Failed to deserialize");
        assert_eq!(req.username, "newuser");
        assert_eq!(req.password, "newpass123");
        assert_eq!(req.email, Some("user@example.com".to_string()));
        assert_eq!(req.role_ids.len(), 1);
    }

    #[test]
    fn test_user_response_from_user() {
        use chrono::Utc;
        use uuid::Uuid;

        let user = User {
            id: Uuid::new_v4(),
            username: "testuser".to_string(),
            password_hash: "hashed".to_string(),
            email: Some("test@example.com".to_string()),
            enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let response = UserResponse::from(user.clone());
        assert_eq!(response.id, user.id);
        assert_eq!(response.username, user.username);
        assert_eq!(response.email, user.email);
        assert_eq!(response.enabled, user.enabled);
    }
}
