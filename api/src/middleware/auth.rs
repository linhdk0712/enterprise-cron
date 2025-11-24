use axum::{
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use common::models::UserClaims;

use crate::state::AppState;

/// Authentication middleware that validates JWT tokens
#[tracing::instrument(skip(state, req, next))]
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract Authorization header
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Check if it's a Bearer token
    if !auth_header.starts_with("Bearer ") {
        tracing::warn!("Invalid authorization header format");
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = &auth_header[7..]; // Skip "Bearer "

    // Validate token based on authentication mode
    let claims = match &state.config.auth.mode {
        common::config::AuthMode::Keycloak => validate_keycloak_token(token, &state).await?,
        common::config::AuthMode::Database => validate_database_token(token, &state).await?,
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
