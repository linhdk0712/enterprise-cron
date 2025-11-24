// User repository implementation
// Requirements: 10.2, 10.13 - Database authentication with user and role management

use crate::db::DbPool;
use crate::errors::DatabaseError;
use crate::models::{Role, User};
use chrono::Utc;
use tracing::instrument;
use uuid::Uuid;

/// Repository for user-related database operations
#[derive(Clone)]
pub struct UserRepository {
    pool: DbPool,
}

impl UserRepository {
    /// Create a new UserRepository
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Create a new user
    ///
    /// # Requirements
    /// - 10.13: User CRUD operations for database authentication
    #[instrument(skip(self, user))]
    pub async fn create(&self, user: &User) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            INSERT INTO users (
                id, username, password_hash, email, enabled,
                created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(&user.id)
        .bind(&user.username)
        .bind(&user.password_hash)
        .bind(&user.email)
        .bind(user.enabled)
        .bind(user.created_at)
        .bind(user.updated_at)
        .execute(self.pool.pool())
        .await?;

        tracing::info!(user_id = %user.id, username = %user.username, "User created");
        Ok(())
    }

    /// Find a user by username
    ///
    /// # Requirements
    /// - 10.2: Find user by username for login
    #[instrument(skip(self))]
    pub async fn find_by_username(&self, username: &str) -> Result<Option<User>, DatabaseError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, username, password_hash, email, enabled, created_at, updated_at
            FROM users
            WHERE username = $1
            "#,
        )
        .bind(username)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(user)
    }

    /// Find a user by ID
    #[instrument(skip(self))]
    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, DatabaseError> {
        let user = sqlx::query_as::<_, User>(
            r#"
            SELECT id, username, password_hash, email, enabled, created_at, updated_at
            FROM users
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(user)
    }

    /// Find all users
    #[instrument(skip(self))]
    pub async fn find_all(&self) -> Result<Vec<User>, DatabaseError> {
        let users = sqlx::query_as::<_, User>(
            r#"
            SELECT id, username, password_hash, email, enabled, created_at, updated_at
            FROM users
            ORDER BY created_at DESC
            "#,
        )
        .fetch_all(self.pool.pool())
        .await?;

        Ok(users)
    }

    /// Update a user
    ///
    /// # Requirements
    /// - 10.13: User CRUD operations
    #[instrument(skip(self, user))]
    pub async fn update(&self, user: &User) -> Result<(), DatabaseError> {
        let result = sqlx::query(
            r#"
            UPDATE users
            SET username = $2,
                password_hash = $3,
                email = $4,
                enabled = $5,
                updated_at = $6
            WHERE id = $1
            "#,
        )
        .bind(&user.id)
        .bind(&user.username)
        .bind(&user.password_hash)
        .bind(&user.email)
        .bind(user.enabled)
        .bind(Utc::now())
        .execute(self.pool.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::NotFound(format!(
                "User not found: {}",
                user.id
            )));
        }

        tracing::info!(user_id = %user.id, username = %user.username, "User updated");
        Ok(())
    }

    /// Delete a user
    ///
    /// # Requirements
    /// - 10.13: User CRUD operations
    #[instrument(skip(self))]
    pub async fn delete(&self, id: Uuid) -> Result<(), DatabaseError> {
        let result = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(id)
            .execute(self.pool.pool())
            .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::NotFound(format!("User not found: {}", id)));
        }

        tracing::info!(user_id = %id, "User deleted");
        Ok(())
    }

    /// Get roles for a user
    ///
    /// # Requirements
    /// - 10.13: Role and permission queries
    #[instrument(skip(self))]
    pub async fn get_user_roles(&self, user_id: Uuid) -> Result<Vec<Role>, DatabaseError> {
        let roles = sqlx::query_as::<_, Role>(
            r#"
            SELECT r.id, r.name, r.permissions, r.created_at
            FROM roles r
            INNER JOIN user_roles ur ON r.id = ur.role_id
            WHERE ur.user_id = $1
            "#,
        )
        .bind(user_id)
        .fetch_all(self.pool.pool())
        .await?;

        Ok(roles)
    }

    /// Get all permissions for a user (aggregated from all roles)
    ///
    /// # Requirements
    /// - 10.13: Role and permission queries
    #[instrument(skip(self))]
    pub async fn get_user_permissions(&self, user_id: Uuid) -> Result<Vec<String>, DatabaseError> {
        let roles = self.get_user_roles(user_id).await?;

        let mut permissions = Vec::new();
        for role in roles {
            permissions.extend(role.permissions);
        }

        // Remove duplicates
        permissions.sort();
        permissions.dedup();

        tracing::debug!(
            user_id = %user_id,
            permission_count = permissions.len(),
            "Retrieved user permissions"
        );
        Ok(permissions)
    }

    /// Assign a role to a user
    ///
    /// # Requirements
    /// - 10.13: Role and permission management
    #[instrument(skip(self))]
    pub async fn assign_role(&self, user_id: Uuid, role_id: Uuid) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            INSERT INTO user_roles (user_id, role_id)
            VALUES ($1, $2)
            ON CONFLICT (user_id, role_id) DO NOTHING
            "#,
        )
        .bind(user_id)
        .bind(role_id)
        .execute(self.pool.pool())
        .await?;

        tracing::info!(user_id = %user_id, role_id = %role_id, "Role assigned to user");
        Ok(())
    }

    /// Remove a role from a user
    ///
    /// # Requirements
    /// - 10.13: Role and permission management
    #[instrument(skip(self))]
    pub async fn remove_role(&self, user_id: Uuid, role_id: Uuid) -> Result<(), DatabaseError> {
        let result = sqlx::query(
            r#"
            DELETE FROM user_roles
            WHERE user_id = $1 AND role_id = $2
            "#,
        )
        .bind(user_id)
        .bind(role_id)
        .execute(self.pool.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::NotFound(format!(
                "Role assignment not found for user {} and role {}",
                user_id, role_id
            )));
        }

        tracing::info!(user_id = %user_id, role_id = %role_id, "Role removed from user");
        Ok(())
    }

    /// Create a new role
    ///
    /// # Requirements
    /// - 10.13: Role management
    #[instrument(skip(self, role))]
    pub async fn create_role(&self, role: &Role) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            INSERT INTO roles (id, name, permissions, created_at)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(&role.id)
        .bind(&role.name)
        .bind(&role.permissions)
        .bind(role.created_at)
        .execute(self.pool.pool())
        .await?;

        tracing::info!(role_id = %role.id, role_name = %role.name, "Role created");
        Ok(())
    }

    /// Find a role by ID
    #[instrument(skip(self))]
    pub async fn find_role_by_id(&self, id: Uuid) -> Result<Option<Role>, DatabaseError> {
        let role = sqlx::query_as::<_, Role>(
            r#"
            SELECT id, name, permissions, created_at
            FROM roles
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(role)
    }

    /// Find a role by name
    #[instrument(skip(self))]
    pub async fn find_role_by_name(&self, name: &str) -> Result<Option<Role>, DatabaseError> {
        let role = sqlx::query_as::<_, Role>(
            r#"
            SELECT id, name, permissions, created_at
            FROM roles
            WHERE name = $1
            "#,
        )
        .bind(name)
        .fetch_optional(self.pool.pool())
        .await?;

        Ok(role)
    }

    /// Find all roles
    #[instrument(skip(self))]
    pub async fn find_all_roles(&self) -> Result<Vec<Role>, DatabaseError> {
        let roles = sqlx::query_as::<_, Role>(
            r#"
            SELECT id, name, permissions, created_at
            FROM roles
            ORDER BY name
            "#,
        )
        .fetch_all(self.pool.pool())
        .await?;

        Ok(roles)
    }

    /// Update a role
    ///
    /// # Requirements
    /// - 10.13: Role management
    #[instrument(skip(self, role))]
    pub async fn update_role(&self, role: &Role) -> Result<(), DatabaseError> {
        let result = sqlx::query(
            r#"
            UPDATE roles
            SET name = $2,
                permissions = $3
            WHERE id = $1
            "#,
        )
        .bind(&role.id)
        .bind(&role.name)
        .bind(&role.permissions)
        .execute(self.pool.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::NotFound(format!(
                "Role not found: {}",
                role.id
            )));
        }

        tracing::info!(role_id = %role.id, role_name = %role.name, "Role updated");
        Ok(())
    }

    /// Delete a role
    ///
    /// # Requirements
    /// - 10.13: Role management
    #[instrument(skip(self))]
    pub async fn delete_role(&self, id: Uuid) -> Result<(), DatabaseError> {
        let result = sqlx::query("DELETE FROM roles WHERE id = $1")
            .bind(id)
            .execute(self.pool.pool())
            .await?;

        if result.rows_affected() == 0 {
            return Err(DatabaseError::NotFound(format!("Role not found: {}", id)));
        }

        tracing::info!(role_id = %id, "Role deleted");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_repository_creation() {
        // This test verifies the repository can be created
        // Actual database tests require a running PostgreSQL instance
    }
}
