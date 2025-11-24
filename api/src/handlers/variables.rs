use axum::{
    extract::{Path, State},
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::handlers::{ErrorResponse, SuccessResponse};
use crate::state::{AppState, SseEvent};
use common::db::repositories::variable::VariableRepository;
use common::models::{Variable, VariableScope};

/// Request to create a new variable
///
/// Requirements: 2.1, 2.2 - Global and job-specific variables
#[derive(Debug, Deserialize)]
pub struct CreateVariableRequest {
    pub name: String,
    pub value: String,
    pub is_sensitive: bool,
    pub scope: VariableScope,
}

/// Request to update an existing variable
///
/// Requirements: 2.6 - Variable CRUD operations
#[derive(Debug, Deserialize)]
pub struct UpdateVariableRequest {
    pub name: Option<String>,
    pub value: Option<String>,
    pub is_sensitive: Option<bool>,
}

/// Response for listing variables with masked sensitive values
///
/// Requirements: 2.8 - Sensitive variable masking
#[derive(Debug, Serialize)]
pub struct VariableResponse {
    pub id: Uuid,
    pub name: String,
    pub value: String,
    pub is_sensitive: bool,
    pub scope: VariableScope,
    pub created_at: chrono::DateTime<Utc>,
    pub updated_at: chrono::DateTime<Utc>,
}

impl From<Variable> for VariableResponse {
    fn from(var: Variable) -> Self {
        Self {
            id: var.id,
            name: var.name,
            value: var.value,
            is_sensitive: var.is_sensitive,
            scope: var.scope,
            created_at: var.created_at,
            updated_at: var.updated_at,
        }
    }
}

/// Create a new variable
///
/// Requirements: 2.1, 2.2, 2.6, 2.7 - Variable creation with encryption for sensitive values
#[tracing::instrument(skip(state, req))]
pub async fn create_variable(
    State(state): State<AppState>,
    Json(req): Json<CreateVariableRequest>,
) -> Result<Json<SuccessResponse<Uuid>>, ErrorResponse> {
    // Validate variable name
    if req.name.trim().is_empty() {
        return Err(ErrorResponse::new(
            "validation_error",
            "Variable name cannot be empty",
        ));
    }

    // Validate variable value
    if req.value.is_empty() {
        return Err(ErrorResponse::new(
            "validation_error",
            "Variable value cannot be empty",
        ));
    }

    // Get encryption key from config (optional)
    let encryption_key = state.config.auth.jwt_secret.clone();
    let repo = VariableRepository::new(state.db_pool.clone(), Some(encryption_key));

    // Check if variable with same name and scope already exists
    if let Ok(Some(_)) = repo.find_by_name_and_scope(&req.name, &req.scope).await {
        return Err(ErrorResponse::new(
            "conflict",
            &format!(
                "Variable with name '{}' already exists in this scope",
                req.name
            ),
        ));
    }

    let variable_id = Uuid::new_v4();
    let now = Utc::now();

    let variable = Variable {
        id: variable_id,
        name: req.name.clone(),
        value: req.value,
        is_sensitive: req.is_sensitive,
        scope: req.scope.clone(),
        created_at: now,
        updated_at: now,
    };

    repo.create(&variable).await.map_err(|e| {
        ErrorResponse::new(
            "database_error",
            &format!("Failed to create variable: {}", e),
        )
    })?;

    // Broadcast SSE event
    state.broadcast_event(SseEvent::JobStatusChanged {
        job_id: Uuid::nil(), // Generic event for variable changes
        status: "variable_created".to_string(),
    });

    tracing::info!(
        variable_id = %variable_id,
        variable_name = %req.name,
        is_sensitive = req.is_sensitive,
        "Variable created successfully"
    );
    Ok(Json(SuccessResponse::new(variable_id)))
}

/// List all variables (with masking for sensitive values)
///
/// Requirements: 2.8 - Display variables with sensitive values masked
#[tracing::instrument(skip(state))]
pub async fn list_variables(
    State(state): State<AppState>,
) -> Result<Json<SuccessResponse<Vec<VariableResponse>>>, ErrorResponse> {
    // Get encryption key from config (optional)
    let encryption_key = state.config.auth.jwt_secret.clone();
    let repo = VariableRepository::new(state.db_pool.clone(), Some(encryption_key));

    // Get all variables with sensitive values masked
    let variables = repo.list_all().await.map_err(|e| {
        ErrorResponse::new(
            "database_error",
            &format!("Failed to fetch variables: {}", e),
        )
    })?;

    let response: Vec<VariableResponse> = variables.into_iter().map(|v| v.into()).collect();

    tracing::debug!(count = response.len(), "Listed variables");
    Ok(Json(SuccessResponse::new(response)))
}

/// Update a variable
///
/// Requirements: 2.6 - Variable update operations
#[tracing::instrument(skip(state, req))]
pub async fn update_variable(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateVariableRequest>,
) -> Result<Json<SuccessResponse<VariableResponse>>, ErrorResponse> {
    // Get encryption key from config (optional)
    let encryption_key = state.config.auth.jwt_secret.clone();
    let repo = VariableRepository::new(state.db_pool.clone(), Some(encryption_key));

    // Get existing variable
    let mut variable = repo
        .find_by_id(id)
        .await
        .map_err(|e| {
            ErrorResponse::new(
                "database_error",
                &format!("Failed to fetch variable: {}", e),
            )
        })?
        .ok_or_else(|| ErrorResponse::new("not_found", &format!("Variable not found: {}", id)))?;

    // Update fields if provided
    if let Some(name) = req.name {
        if name.trim().is_empty() {
            return Err(ErrorResponse::new(
                "validation_error",
                "Variable name cannot be empty",
            ));
        }

        // Check if new name conflicts with existing variable in same scope
        if name != variable.name {
            if let Ok(Some(_)) = repo.find_by_name_and_scope(&name, &variable.scope).await {
                return Err(ErrorResponse::new(
                    "conflict",
                    &format!("Variable with name '{}' already exists in this scope", name),
                ));
            }
        }

        variable.name = name;
    }

    if let Some(value) = req.value {
        if value.is_empty() {
            return Err(ErrorResponse::new(
                "validation_error",
                "Variable value cannot be empty",
            ));
        }
        variable.value = value;
    }

    if let Some(is_sensitive) = req.is_sensitive {
        variable.is_sensitive = is_sensitive;
    }

    variable.updated_at = Utc::now();

    // Update variable in database
    repo.update(&variable).await.map_err(|e| {
        ErrorResponse::new(
            "database_error",
            &format!("Failed to update variable: {}", e),
        )
    })?;

    // Mask sensitive value in response
    let mut response_variable = variable.clone();
    if response_variable.is_sensitive {
        response_variable.value = "***".to_string();
    }

    // Broadcast SSE event
    state.broadcast_event(SseEvent::JobStatusChanged {
        job_id: Uuid::nil(), // Generic event for variable changes
        status: "variable_updated".to_string(),
    });

    tracing::info!(variable_id = %id, "Variable updated successfully");
    Ok(Json(SuccessResponse::new(response_variable.into())))
}

/// Delete a variable
///
/// Requirements: 2.6 - Variable deletion operations
#[tracing::instrument(skip(state))]
pub async fn delete_variable(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<SuccessResponse<()>>, ErrorResponse> {
    // Get encryption key from config (optional)
    let encryption_key = state.config.auth.jwt_secret.clone();
    let repo = VariableRepository::new(state.db_pool.clone(), Some(encryption_key));

    // Delete variable from database
    repo.delete(id).await.map_err(|e| {
        ErrorResponse::new(
            "database_error",
            &format!("Failed to delete variable: {}", e),
        )
    })?;

    // Broadcast SSE event
    state.broadcast_event(SseEvent::JobStatusChanged {
        job_id: Uuid::nil(), // Generic event for variable changes
        status: "variable_deleted".to_string(),
    });

    tracing::info!(variable_id = %id, "Variable deleted successfully");
    Ok(Json(SuccessResponse::new(())))
}
