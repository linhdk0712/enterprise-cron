use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use common::models::UserClaims;

use crate::state::AppState;

/// Authentication middleware that validates JWT tokens
/// Requirements: 19.4 - Support both Bearer token and httpOnly cookie authentication
#[tracing::instrument(skip(state, req, next))]
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Try to extract token from Authorization header first
    let token = if let Some(auth_header) = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
    {
        // Check if it's a Bearer token
        if !auth_header.starts_with("Bearer ") {
            tracing::warn!("Invalid authorization header format");
            return Err(StatusCode::UNAUTHORIZED);
        }
        auth_header[7..].to_string() // Skip "Bearer "
    } else {
        // Fallback: Extract token from cookie
        req.headers()
            .get("Cookie")
            .and_then(|h| h.to_str().ok())
            .and_then(|cookies| {
                cookies.split(';').find_map(|cookie| {
                    let parts: Vec<&str> = cookie.trim().splitn(2, '=').collect();
                    if parts.len() == 2 && parts[0] == "auth_token" {
                        Some(parts[1].to_string())
                    } else {
                        None
                    }
                })
            })
            .ok_or_else(|| {
                tracing::warn!("No authorization token found in header or cookie");
                StatusCode::UNAUTHORIZED
            })?
    };

    // Validate token based on authentication mode
    let claims = match &state.config.auth.mode {
        common::config::AuthMode::Keycloak => validate_keycloak_token(&token, &state).await?,
        common::config::AuthMode::Database => validate_database_token(&token, &state).await?,
    };

    // Insert claims into request extensions for use by handlers
    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}

/// Validate JWT token issued by Keycloak
#[tracing::instrument(skip(token, state))]
async fn validate_keycloak_token(token: &str, state: &AppState) -> Result<UserClaims, StatusCode> {
    // Get Keycloak public keys (with caching)
    let keycloak_config = state
        .config
        .auth
        .keycloak
        .as_ref()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    // Fetch JWKS from Keycloak
    let _jwks_url = format!(
        "{}/realms/{}/protocol/openid-connect/certs",
        keycloak_config.server_url, keycloak_config.realm
    );

    // TODO: Implement JWKS caching with TTL
    // For now, we'll use a simple validation
    let validation = jsonwebtoken::Validation::default();

    // Decode and validate token
    let token_data = jsonwebtoken::decode::<UserClaims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(b"secret"), // TODO: Use actual public key
        &validation,
    )
    .map_err(|e| {
        tracing::warn!(error = %e, "Failed to validate Keycloak token");
        StatusCode::UNAUTHORIZED
    })?;

    Ok(token_data.claims)
}

/// Validate JWT token issued by the system (database mode)
#[tracing::instrument(skip(token, state))]
async fn validate_database_token(token: &str, state: &AppState) -> Result<UserClaims, StatusCode> {
    let jwt_secret = &state.config.auth.jwt_secret;

    let validation = jsonwebtoken::Validation::default();

    let token_data = jsonwebtoken::decode::<UserClaims>(
        token,
        &jsonwebtoken::DecodingKey::from_secret(jwt_secret.as_bytes()),
        &validation,
    )
    .map_err(|e| {
        tracing::warn!(error = %e, "Failed to validate database token");
        StatusCode::UNAUTHORIZED
    })?;

    // Check if token is expired
    let now = chrono::Utc::now().timestamp();
    if token_data.claims.exp < now {
        tracing::warn!("Token expired");
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(token_data.claims)
}
