// Authentication and JWT token handling
// Requirements: 10.2, 10.3, 10.4

use crate::db::repositories::user::UserRepository;
use crate::errors::{AuthError, DatabaseError};
use crate::models::{User, UserClaims};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, instrument};
use uuid::Uuid;

/// JWT token service for encoding and decoding tokens
#[derive(Clone)]
pub struct JwtService {
    encoding_key: Arc<EncodingKey>,
    decoding_key: Arc<DecodingKey>,
    expiration_hours: i64,
}

impl JwtService {
    /// Create a new JWT service with the given secret and expiration
    #[instrument(skip(secret))]
    pub fn new(secret: &str, expiration_hours: u64) -> Self {
        Self {
            encoding_key: Arc::new(EncodingKey::from_secret(secret.as_bytes())),
            decoding_key: Arc::new(DecodingKey::from_secret(secret.as_bytes())),
            expiration_hours: expiration_hours as i64,
        }
    }

    /// Encode user claims into a JWT token
    /// Requirements: 10.3 - Generate JWT tokens on successful login
    #[instrument(skip(self))]
    pub fn encode_token(
        &self,
        user_id: &str,
        username: &str,
        permissions: Vec<String>,
    ) -> Result<String, AuthError> {
        let now = Utc::now();
        let exp = (now + Duration::hours(self.expiration_hours)).timestamp();
        let iat = now.timestamp();

        let claims = UserClaims {
            sub: user_id.to_string(),
            username: username.to_string(),
            permissions,
            exp,
            iat,
        };

        encode(&Header::default(), &claims, &self.encoding_key).map_err(|e| {
            error!(error = %e, "Failed to encode JWT token");
            AuthError::AuthenticationFailed(format!("Failed to encode token: {}", e))
        })
    }

    /// Decode and validate a JWT token
    /// Requirements: 10.4 - JWT token validation
    #[instrument(skip(self, token))]
    pub fn decode_token(&self, token: &str) -> Result<UserClaims, AuthError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;

        let token_data =
            decode::<UserClaims>(token, &self.decoding_key, &validation).map_err(|e| {
                error!(error = %e, "Failed to decode JWT token");
                match e.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
                    _ => AuthError::InvalidToken(format!("Token validation failed: {}", e)),
                }
            })?;

        Ok(token_data.claims)
    }

    /// Validate a token and return claims if valid
    /// Requirements: 10.4 - JWT token validation
    #[instrument(skip(self, token))]
    pub fn validate_token(&self, token: &str) -> Result<UserClaims, AuthError> {
        self.decode_token(token)
    }
}

/// Database authentication service for validating credentials and managing users
#[derive(Clone)]
pub struct DatabaseAuthService {
    jwt_service: JwtService,
    user_repository: Arc<UserRepository>,
}

impl DatabaseAuthService {
    /// Create a new database authentication service
    pub fn new(jwt_service: JwtService, user_repository: UserRepository) -> Self {
        Self {
            jwt_service,
            user_repository: Arc::new(user_repository),
        }
    }

    /// Authenticate a user with username and password
    /// Requirements: 10.2 - Validate credentials against bcrypt-hashed passwords
    /// Requirements: 10.3 - Generate JWT tokens on successful login
    #[instrument(skip(self, password))]
    pub async fn login(&self, username: &str, password: &str) -> Result<String, AuthError> {
        // Find user by username
        let user = self
            .user_repository
            .find_by_username(username)
            .await
            .map_err(|e| {
                error!(error = %e, username = %username, "Database error during login");
                AuthError::AuthenticationFailed(format!("Database error: {}", e))
            })?
            .ok_or_else(|| {
                error!(username = %username, "User not found");
                AuthError::InvalidCredentials
            })?;

        // Check if user is enabled
        if !user.enabled {
            error!(username = %username, "User account is disabled");
            return Err(AuthError::AuthenticationFailed(
                "User account is disabled".to_string(),
            ));
        }

        // Verify password
        let password_valid = bcrypt::verify(password, &user.password_hash).map_err(|e| {
            error!(error = %e, "Failed to verify password");
            AuthError::AuthenticationFailed(format!("Password verification failed: {}", e))
        })?;

        if !password_valid {
            error!(username = %username, "Invalid password");
            return Err(AuthError::InvalidCredentials);
        }

        // Get user permissions
        let permissions = self
            .user_repository
            .get_user_permissions(user.id)
            .await
            .map_err(|e| {
                error!(error = %e, user_id = %user.id, "Failed to get user permissions");
                AuthError::AuthenticationFailed(format!("Failed to get permissions: {}", e))
            })?;

        // Generate JWT token
        let token =
            self.jwt_service
                .encode_token(&user.id.to_string(), &user.username, permissions)?;

        tracing::info!(
            user_id = %user.id,
            username = %user.username,
            "User logged in successfully"
        );

        Ok(token)
    }

    /// Create a new user with hashed password
    /// Requirements: 10.13 - Store user credentials with bcrypt hashing
    #[instrument(skip(self, password))]
    pub async fn create_user(
        &self,
        username: &str,
        password: &str,
        email: Option<String>,
    ) -> Result<User, AuthError> {
        // Hash password
        let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST).map_err(|e| {
            error!(error = %e, "Failed to hash password");
            AuthError::AuthenticationFailed(format!("Password hashing failed: {}", e))
        })?;

        let user = User {
            id: Uuid::new_v4(),
            username: username.to_string(),
            password_hash,
            email,
            enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        self.user_repository.create(&user).await.map_err(|e| {
            error!(error = %e, username = %username, "Failed to create user");
            match e {
                DatabaseError::DuplicateKey(_) => {
                    AuthError::AuthenticationFailed("Username already exists".to_string())
                }
                _ => AuthError::AuthenticationFailed(format!("Failed to create user: {}", e)),
            }
        })?;

        tracing::info!(user_id = %user.id, username = %username, "User created");
        Ok(user)
    }

    /// Update user password
    /// Requirements: 10.13 - User CRUD operations
    #[instrument(skip(self, new_password))]
    pub async fn update_password(
        &self,
        user_id: Uuid,
        new_password: &str,
    ) -> Result<(), AuthError> {
        // Find user
        let mut user = self
            .user_repository
            .find_by_id(user_id)
            .await
            .map_err(|e| {
                error!(error = %e, user_id = %user_id, "Database error");
                AuthError::AuthenticationFailed(format!("Database error: {}", e))
            })?
            .ok_or_else(|| {
                error!(user_id = %user_id, "User not found");
                AuthError::UserNotFound(user_id.to_string())
            })?;

        // Hash new password
        let password_hash = bcrypt::hash(new_password, bcrypt::DEFAULT_COST).map_err(|e| {
            error!(error = %e, "Failed to hash password");
            AuthError::AuthenticationFailed(format!("Password hashing failed: {}", e))
        })?;

        user.password_hash = password_hash;
        user.updated_at = Utc::now();

        self.user_repository.update(&user).await.map_err(|e| {
            error!(error = %e, user_id = %user_id, "Failed to update user");
            AuthError::AuthenticationFailed(format!("Failed to update password: {}", e))
        })?;

        tracing::info!(user_id = %user_id, "Password updated");
        Ok(())
    }

    /// Validate a JWT token and return claims
    /// Requirements: 10.4 - JWT token validation
    #[instrument(skip(self, token))]
    pub fn validate_token(&self, token: &str) -> Result<UserClaims, AuthError> {
        self.jwt_service.validate_token(token)
    }
}

/// Keycloak JWT service for validating tokens from Keycloak
#[derive(Clone)]
pub struct KeycloakJwtService {
    realm_url: String,
    client_id: String,
    cached_keys: Arc<tokio::sync::RwLock<Option<CachedJwks>>>,
    http_client: reqwest::Client,
}

#[derive(Debug, Clone)]
struct CachedJwks {
    keys: jsonwebtoken::jwk::JwkSet,
    cached_at: chrono::DateTime<Utc>,
    ttl_seconds: i64,
}

impl KeycloakJwtService {
    /// Create a new Keycloak JWT service
    /// Requirements: 10.12 - Support configuring Keycloak realm, client ID, and server URL
    #[instrument(skip(server_url))]
    pub fn new(server_url: &str, realm: &str, client_id: &str) -> Self {
        let realm_url = format!("{}/realms/{}", server_url, realm);
        Self {
            realm_url,
            client_id: client_id.to_string(),
            cached_keys: Arc::new(tokio::sync::RwLock::new(None)),
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Fetch JWKS (JSON Web Key Set) from Keycloak
    /// Requirements: 10.1 - Fetch Keycloak public keys (JWKS)
    #[instrument(skip(self))]
    async fn fetch_jwks(&self) -> Result<jsonwebtoken::jwk::JwkSet, AuthError> {
        let jwks_url = format!("{}/protocol/openid-connect/certs", self.realm_url);

        let response = self.http_client.get(&jwks_url).send().await.map_err(|e| {
            error!(error = %e, "Failed to fetch JWKS from Keycloak");
            AuthError::KeycloakError(format!("Failed to fetch JWKS: {}", e))
        })?;

        if !response.status().is_success() {
            return Err(AuthError::KeycloakError(format!(
                "Keycloak returned status: {}",
                response.status()
            )));
        }

        let jwks: jsonwebtoken::jwk::JwkSet = response.json().await.map_err(|e| {
            error!(error = %e, "Failed to parse JWKS response");
            AuthError::KeycloakError(format!("Failed to parse JWKS: {}", e))
        })?;

        Ok(jwks)
    }

    /// Get JWKS from cache or fetch from Keycloak
    /// Requirements: 10.11 - Cache public keys with TTL
    #[instrument(skip(self))]
    async fn get_jwks(&self) -> Result<jsonwebtoken::jwk::JwkSet, AuthError> {
        // Check cache first
        {
            let cache = self.cached_keys.read().await;
            if let Some(cached) = cache.as_ref() {
                let age = Utc::now().signed_duration_since(cached.cached_at);
                if age.num_seconds() < cached.ttl_seconds {
                    return Ok(cached.keys.clone());
                }
            }
        }

        // Cache miss or expired, fetch new keys
        let jwks = self.fetch_jwks().await?;

        // Update cache
        {
            let mut cache = self.cached_keys.write().await;
            *cache = Some(CachedJwks {
                keys: jwks.clone(),
                cached_at: Utc::now(),
                ttl_seconds: 3600, // 1 hour TTL
            });
        }

        Ok(jwks)
    }

    /// Validate JWT token from Keycloak
    /// Requirements: 10.1 - Validate JWT tokens from Keycloak
    /// Requirements: 10.11 - Handle Keycloak unavailability by using cached keys
    #[instrument(skip(self, token))]
    pub async fn validate_token(&self, token: &str) -> Result<UserClaims, AuthError> {
        // Try to get JWKS (from cache or fetch)
        let jwks = match self.get_jwks().await {
            Ok(keys) => keys,
            Err(e) => {
                // If fetch fails, try to use cached keys even if expired
                let cache = self.cached_keys.read().await;
                if let Some(cached) = cache.as_ref() {
                    tracing::warn!(
                        "Keycloak unavailable, using cached keys (age: {} seconds)",
                        Utc::now()
                            .signed_duration_since(cached.cached_at)
                            .num_seconds()
                    );
                    cached.keys.clone()
                } else {
                    return Err(e);
                }
            }
        };

        // Decode token header to get kid (key ID)
        let header = jsonwebtoken::decode_header(token).map_err(|e| {
            error!(error = %e, "Failed to decode token header");
            AuthError::InvalidToken(format!("Invalid token header: {}", e))
        })?;

        let kid = header.kid.ok_or_else(|| {
            error!("Token missing kid (key ID)");
            AuthError::InvalidToken("Token missing kid".to_string())
        })?;

        // Find the matching key
        let jwk = jwks.find(&kid).ok_or_else(|| {
            error!(kid = %kid, "Key ID not found in JWKS");
            AuthError::InvalidToken(format!("Key ID {} not found", kid))
        })?;

        // Convert JWK to DecodingKey
        let decoding_key = DecodingKey::from_jwk(jwk).map_err(|e| {
            error!(error = %e, "Failed to create decoding key from JWK");
            AuthError::InvalidToken(format!("Invalid JWK: {}", e))
        })?;

        // Validate token
        let mut validation = Validation::new(header.alg);
        validation.validate_exp = true;
        validation.set_audience(&[&self.client_id]);

        let token_data =
            decode::<KeycloakClaims>(token, &decoding_key, &validation).map_err(|e| {
                error!(error = %e, "Failed to validate Keycloak token");
                match e.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => AuthError::TokenExpired,
                    _ => AuthError::InvalidToken(format!("Token validation failed: {}", e)),
                }
            })?;

        // Convert Keycloak claims to UserClaims
        let claims = token_data.claims;
        let permissions = extract_permissions_from_keycloak(&claims);
        let username = claims
            .preferred_username
            .clone()
            .unwrap_or_else(|| claims.sub.clone());

        Ok(UserClaims {
            sub: claims.sub,
            username,
            permissions,
            exp: claims.exp,
            iat: claims.iat,
        })
    }
}

/// Keycloak-specific JWT claims structure
#[derive(Debug, Serialize, Deserialize)]
struct KeycloakClaims {
    sub: String,
    preferred_username: Option<String>,
    exp: i64,
    iat: i64,
    #[serde(default)]
    realm_access: Option<RealmAccess>,
    #[serde(default)]
    resource_access: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RealmAccess {
    #[serde(default)]
    roles: Vec<String>,
}

/// Extract permissions from Keycloak claims
/// Maps Keycloak roles to system permissions
fn extract_permissions_from_keycloak(claims: &KeycloakClaims) -> Vec<String> {
    let mut permissions = Vec::new();

    // Extract realm roles
    if let Some(realm_access) = &claims.realm_access {
        for role in &realm_access.roles {
            // Map Keycloak roles to permissions
            match role.as_str() {
                "admin" => {
                    permissions.push("job:read".to_string());
                    permissions.push("job:write".to_string());
                    permissions.push("job:execute".to_string());
                    permissions.push("job:delete".to_string());
                    permissions.push("execution:read".to_string());
                }
                "operator" => {
                    permissions.push("job:read".to_string());
                    permissions.push("job:execute".to_string());
                    permissions.push("execution:read".to_string());
                }
                "viewer" => {
                    permissions.push("job:read".to_string());
                    permissions.push("execution:read".to_string());
                }
                _ => {}
            }
        }
    }

    permissions.sort();
    permissions.dedup();
    permissions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_service_encode_decode() {
        let service = JwtService::new("test-secret", 24);
        let permissions = vec!["job:read".to_string(), "job:write".to_string()];

        let token = service
            .encode_token("user-123", "testuser", permissions.clone())
            .expect("Failed to encode token");

        let claims = service
            .decode_token(&token)
            .expect("Failed to decode token");

        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.username, "testuser");
        assert_eq!(claims.permissions, permissions);
    }

    #[test]
    fn test_jwt_service_expired_token() {
        // Create a service with very short expiration
        let service = JwtService::new("test-secret", 1); // 1 hour
        let permissions = vec!["job:read".to_string()];

        // Manually create an expired token by modifying the expiration time
        let now = chrono::Utc::now();
        let exp = (now - chrono::Duration::hours(1)).timestamp(); // Expired 1 hour ago
        let iat = (now - chrono::Duration::hours(2)).timestamp();

        let claims = UserClaims {
            sub: "user-123".to_string(),
            username: "testuser".to_string(),
            permissions,
            exp,
            iat,
        };

        let encoding_key = jsonwebtoken::EncodingKey::from_secret("test-secret".as_bytes());
        let token = jsonwebtoken::encode(&jsonwebtoken::Header::default(), &claims, &encoding_key)
            .expect("Failed to encode token");

        let result = service.decode_token(&token);
        assert!(matches!(result, Err(AuthError::TokenExpired)));
    }

    #[test]
    fn test_jwt_service_invalid_token() {
        let service = JwtService::new("test-secret", 24);
        let result = service.decode_token("invalid.token.here");
        assert!(matches!(result, Err(AuthError::InvalidToken(_))));
    }

    #[test]
    fn test_extract_permissions_from_keycloak_admin() {
        let claims = KeycloakClaims {
            sub: "user-123".to_string(),
            preferred_username: Some("admin".to_string()),
            exp: 0,
            iat: 0,
            realm_access: Some(RealmAccess {
                roles: vec!["admin".to_string()],
            }),
            resource_access: None,
        };

        let permissions = extract_permissions_from_keycloak(&claims);
        assert!(permissions.contains(&"job:read".to_string()));
        assert!(permissions.contains(&"job:write".to_string()));
        assert!(permissions.contains(&"job:execute".to_string()));
        assert!(permissions.contains(&"job:delete".to_string()));
        assert!(permissions.contains(&"execution:read".to_string()));
    }

    #[test]
    fn test_extract_permissions_from_keycloak_viewer() {
        let claims = KeycloakClaims {
            sub: "user-123".to_string(),
            preferred_username: Some("viewer".to_string()),
            exp: 0,
            iat: 0,
            realm_access: Some(RealmAccess {
                roles: vec!["viewer".to_string()],
            }),
            resource_access: None,
        };

        let permissions = extract_permissions_from_keycloak(&claims);
        assert!(permissions.contains(&"job:read".to_string()));
        assert!(permissions.contains(&"execution:read".to_string()));
        assert!(!permissions.contains(&"job:write".to_string()));
    }
}
