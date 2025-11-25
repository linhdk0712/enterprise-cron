use axum::{
    extract::{Path, State},
    Extension, Json,
};
use common::db::repositories::user::UserRepository;
use common::models::{Role, UserClaims};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::handlers::{ErrorResponse, SuccessResponse};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub email: Option<String>,
    pub enabled: bool,
    pub roles: Vec<RoleResponse>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct RoleResponse {
    pub id: Uuid,
    pub name: String,
    pub permissions: Vec<String>,
}

impl From<Role> for RoleResponse {
    fn from(role: Role) -> Self {
        Self {
            id: role.id,
            name: role.name,
            permissions: role.permissions,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub email: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct AssignRolesRequest {
    pub role_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePasswordRequest {
    pub new_password: String,
}

/// List all users (admin only)
/// Requirements: 19.1.44 - Admin can list all users
#[tracing::instrument(skip(state))]
pub async fn list_users(
    State(state): State<AppState>,
    Extension(_claims): Extension<UserClaims>,
) -> Result<Json<SuccessResponse<Vec<UserResponse>>>, ErrorResponse> {
    let user_repository = UserRepository::new(state.db_pool.clone());

    let users = user_repository
        .find_all()
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to list users");
            ErrorResponse::new("internal_error", "Failed to list users")
        })?;

    let mut user_responses = Vec::new();
    for user in users {
        let roles = user_repository
            .get_user_roles(user.id)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, user_id = %user.id, "Failed to get user roles");
                ErrorResponse::new("internal_error", "Failed to get user roles")
            })?;

        user_responses.push(UserResponse {
            id: user.id,
            username: user.username,
            email: user.email,
            enabled: user.enabled,
            roles: roles.into_iter().map(RoleResponse::from).collect(),
            created_at: user.created_at,
        });
    }

    Ok(Json(SuccessResponse::new(user_responses)))
}

/// Get user by ID (admin only, or user viewing their own profile)
/// Requirements: 19.1.45 - Admin can view any user, users can view their own profile
#[tracing::instrument(skip(state))]
pub async fn get_user(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Extension(claims): Extension<UserClaims>,
) -> Result<Json<SuccessResponse<UserResponse>>, ErrorResponse> {
    // Check if user is viewing their own profile or is admin
    let is_own_profile = claims.sub == user_id.to_string();
    let is_admin = claims.permissions.contains(&"user:manage".to_string());

    if !is_own_profile && !is_admin {
        return Err(ErrorResponse::new(
            "forbidden",
            "You can only view your own profile",
        ));
    }

    let user_repository = UserRepository::new(state.db_pool.clone());

    let user = user_repository
        .find_by_id(user_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, user_id = %user_id, "Failed to get user");
            ErrorResponse::new("internal_error", "Failed to get user")
        })?
        .ok_or_else(|| ErrorResponse::new("not_found", "User not found"))?;

    let roles = user_repository
        .get_user_roles(user.id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, user_id = %user.id, "Failed to get user roles");
            ErrorResponse::new("internal_error", "Failed to get user roles")
        })?;

    let response = UserResponse {
        id: user.id,
        username: user.username,
        email: user.email,
        enabled: user.enabled,
        roles: roles.into_iter().map(RoleResponse::from).collect(),
        created_at: user.created_at,
    };

    Ok(Json(SuccessResponse::new(response)))
}

/// Update user (admin only, or user updating their own email)
/// Requirements: 19.1.46 - Admin can update any user, users can update their own email
#[tracing::instrument(skip(state, req))]
pub async fn update_user(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Extension(claims): Extension<UserClaims>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<SuccessResponse<UserResponse>>, ErrorResponse> {
    let is_own_profile = claims.sub == user_id.to_string();
    let is_admin = claims.permissions.contains(&"user:manage".to_string());

    // Users can only update their own email, admins can update everything
    if !is_admin && !is_own_profile {
        return Err(ErrorResponse::new(
            "forbidden",
            "You can only update your own profile",
        ));
    }

    if !is_admin && req.enabled.is_some() {
        return Err(ErrorResponse::new(
            "forbidden",
            "Only admins can enable/disable users",
        ));
    }

    let user_repository = UserRepository::new(state.db_pool.clone());

    let mut user = user_repository
        .find_by_id(user_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, user_id = %user_id, "Failed to get user");
            ErrorResponse::new("internal_error", "Failed to get user")
        })?
        .ok_or_else(|| ErrorResponse::new("not_found", "User not found"))?;

    // Update fields
    if let Some(email) = req.email {
        user.email = Some(email);
    }

    if let Some(enabled) = req.enabled {
        user.enabled = enabled;
    }

    user.updated_at = chrono::Utc::now();

    user_repository
        .update(&user)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, user_id = %user_id, "Failed to update user");
            ErrorResponse::new("internal_error", "Failed to update user")
        })?;

    let roles = user_repository
        .get_user_roles(user.id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, user_id = %user.id, "Failed to get user roles");
            ErrorResponse::new("internal_error", "Failed to get user roles")
        })?;

    let response = UserResponse {
        id: user.id,
        username: user.username,
        email: user.email,
        enabled: user.enabled,
        roles: roles.into_iter().map(RoleResponse::from).collect(),
        created_at: user.created_at,
    };

    tracing::info!(
        user_id = %user_id,
        updated_by = %claims.username,
        "User updated"
    );

    Ok(Json(SuccessResponse::new(response)))
}

/// Delete user (admin only)
/// Requirements: 19.1.47 - Admin can delete users
/// Requirements: 19.1.52 - Prevent users from modifying their own roles
#[tracing::instrument(skip(state))]
pub async fn delete_user(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Extension(claims): Extension<UserClaims>,
) -> Result<Json<SuccessResponse<()>>, ErrorResponse> {
    // Prevent users from deleting themselves
    if claims.sub == user_id.to_string() {
        return Err(ErrorResponse::new(
            "forbidden",
            "You cannot delete your own account",
        ));
    }

    let user_repository = UserRepository::new(state.db_pool.clone());

    user_repository
        .delete(user_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, user_id = %user_id, "Failed to delete user");
            ErrorResponse::new("internal_error", "Failed to delete user")
        })?;

    tracing::info!(
        user_id = %user_id,
        deleted_by = %claims.username,
        "User deleted"
    );

    Ok(Json(SuccessResponse::new(())))
}

/// Assign roles to user (admin only)
/// Requirements: 19.1.48 - Admin can assign roles to users
/// Requirements: 19.1.52 - Prevent users from escalating their own privileges
#[tracing::instrument(skip(state, req))]
pub async fn assign_roles(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Extension(claims): Extension<UserClaims>,
    Json(req): Json<AssignRolesRequest>,
) -> Result<Json<SuccessResponse<UserResponse>>, ErrorResponse> {
    // Prevent users from modifying their own roles (anti-privilege escalation)
    if claims.sub == user_id.to_string() {
        return Err(ErrorResponse::new(
            "forbidden",
            "You cannot modify your own roles",
        ));
    }

    let user_repository = UserRepository::new(state.db_pool.clone());

    // Check if user exists
    let user = user_repository
        .find_by_id(user_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, user_id = %user_id, "Failed to get user");
            ErrorResponse::new("internal_error", "Failed to get user")
        })?
        .ok_or_else(|| ErrorResponse::new("not_found", "User not found"))?;

    // Get existing roles and remove them
    let existing_roles = user_repository
        .get_user_roles(user_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, user_id = %user_id, "Failed to get existing roles");
            ErrorResponse::new("internal_error", "Failed to get existing roles")
        })?;

    for role in existing_roles {
        user_repository
            .remove_role(user_id, role.id)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, user_id = %user_id, role_id = %role.id, "Failed to remove role");
                ErrorResponse::new("internal_error", "Failed to remove role")
            })?;
    }

    // Assign new roles
    for role_id in &req.role_ids {
        user_repository
            .assign_role(user_id, *role_id)
            .await
            .map_err(|e| {
                tracing::error!(
                    error = %e,
                    user_id = %user_id,
                    role_id = %role_id,
                    "Failed to assign role"
                );
                ErrorResponse::new("internal_error", "Failed to assign role")
            })?;
    }

    // Invalidate user's tokens (they need to re-login to get new permissions)
    // TODO: Implement token invalidation mechanism
    tracing::warn!(
        user_id = %user_id,
        "User roles changed. Token invalidation not yet implemented. User should re-login."
    );

    let roles = user_repository
        .get_user_roles(user.id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, user_id = %user.id, "Failed to get user roles");
            ErrorResponse::new("internal_error", "Failed to get user roles")
        })?;

    let response = UserResponse {
        id: user.id,
        username: user.username,
        email: user.email,
        enabled: user.enabled,
        roles: roles.into_iter().map(RoleResponse::from).collect(),
        created_at: user.created_at,
    };

    tracing::info!(
        user_id = %user_id,
        role_count = req.role_ids.len(),
        updated_by = %claims.username,
        "User roles assigned"
    );

    Ok(Json(SuccessResponse::new(response)))
}

/// Update user password (user can update their own, admin can update any)
/// Requirements: 19.1.49 - Users can change their own password
#[tracing::instrument(skip(state, req))]
pub async fn update_password(
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Extension(claims): Extension<UserClaims>,
    Json(req): Json<UpdatePasswordRequest>,
) -> Result<Json<SuccessResponse<()>>, ErrorResponse> {
    let is_own_profile = claims.sub == user_id.to_string();
    let is_admin = claims.permissions.contains(&"user:manage".to_string());

    if !is_own_profile && !is_admin {
        return Err(ErrorResponse::new(
            "forbidden",
            "You can only change your own password",
        ));
    }

    // Validate password
    if req.new_password.len() < 8 {
        return Err(ErrorResponse::new(
            "validation_error",
            "Password must be at least 8 characters",
        ));
    }

    // Hash password
    let password_hash = bcrypt::hash(&req.new_password, bcrypt::DEFAULT_COST).map_err(|e| {
        tracing::error!(error = %e, "Failed to hash password");
        ErrorResponse::new("internal_error", "Failed to hash password")
    })?;

    let user_repository = UserRepository::new(state.db_pool.clone());

    let mut user = user_repository
        .find_by_id(user_id)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, user_id = %user_id, "Failed to get user");
            ErrorResponse::new("internal_error", "Failed to get user")
        })?
        .ok_or_else(|| ErrorResponse::new("not_found", "User not found"))?;

    user.password_hash = password_hash;
    user.updated_at = chrono::Utc::now();

    user_repository
        .update(&user)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, user_id = %user_id, "Failed to update password");
            ErrorResponse::new("internal_error", "Failed to update password")
        })?;

    tracing::info!(
        user_id = %user_id,
        updated_by = %claims.username,
        "Password updated"
    );

    Ok(Json(SuccessResponse::new(())))
}

/// List all roles (any authenticated user)
/// Requirements: 19.1.50 - List available roles for assignment
#[tracing::instrument(skip(state))]
pub async fn list_roles(
    State(state): State<AppState>,
    Extension(_claims): Extension<UserClaims>,
) -> Result<Json<SuccessResponse<Vec<RoleResponse>>>, ErrorResponse> {
    let user_repository = UserRepository::new(state.db_pool.clone());

    let roles = user_repository
        .find_all_roles()
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to list roles");
            ErrorResponse::new("internal_error", "Failed to list roles")
        })?;

    let role_responses: Vec<RoleResponse> = roles.into_iter().map(RoleResponse::from).collect();

    Ok(Json(SuccessResponse::new(role_responses)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_response_from_role() {
        use chrono::Utc;

        let role = Role {
            id: Uuid::new_v4(),
            name: "Admin".to_string(),
            permissions: vec!["job:read".to_string(), "job:write".to_string()],
            created_at: Utc::now(),
        };

        let response = RoleResponse::from(role.clone());
        assert_eq!(response.id, role.id);
        assert_eq!(response.name, role.name);
        assert_eq!(response.permissions, role.permissions);
    }
}
