use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::handlers::{ErrorResponse, SuccessResponse};
use crate::state::AppState;
use common::import_export::{
    ExportedJob, ImportExportService, ImportExportServiceImpl, ImportResult,
};
use common::storage::service::MinIOServiceImpl;

/// Request to export a single job
/// Requirements: 18.3 - Export single job
#[derive(Debug, Deserialize)]
pub struct ExportJobRequest {
    pub job_id: Uuid,
}

/// Request to export multiple jobs in bulk
/// Requirements: 18.12 - Bulk export
#[derive(Debug, Deserialize)]
pub struct ExportJobsBulkRequest {
    pub job_ids: Vec<Uuid>,
}

/// Response for job export with filename
/// Requirements: 18.3 - Export filename format
#[derive(Debug, Serialize)]
pub struct ExportJobResponse {
    pub job: ExportedJob,
    pub filename: String,
}

/// Response for bulk job export
/// Requirements: 18.12 - Bulk export format
#[derive(Debug, Serialize)]
pub struct ExportJobsBulkResponse {
    pub jobs: Vec<ExportedJob>,
    pub count: usize,
}

/// Request to import a single job
/// Requirements: 18.9 - Import single job
#[derive(Debug, Deserialize)]
pub struct ImportJobRequest {
    pub job_definition: serde_json::Value,
    #[serde(default)]
    pub sensitive_data: HashMap<String, String>,
}

/// Request to import multiple jobs in bulk
/// Requirements: 18.13 - Bulk import
#[derive(Debug, Deserialize)]
pub struct ImportJobsBulkRequest {
    pub job_definitions: Vec<serde_json::Value>,
    #[serde(default)]
    pub sensitive_data: HashMap<String, HashMap<String, String>>,
}

/// Response for bulk job import
/// Requirements: 18.13 - Bulk import processing with success/failure reporting
#[derive(Debug, Serialize)]
pub struct ImportJobsBulkResponse {
    pub results: Vec<ImportResult>,
    pub total: usize,
    pub success_count: usize,
    pub failed_count: usize,
}

/// Export a single job
/// Requirements: 18.3, 18.4, 18.5 - Export with filename, completeness, masking
#[tracing::instrument(skip(state, req))]
pub async fn export_job(
    State(state): State<AppState>,
    Json(req): Json<ExportJobRequest>,
) -> Result<Json<SuccessResponse<ExportJobResponse>>, ErrorResponse> {
    // Create import/export service
    let minio_client = common::storage::MinioClient::from_bucket(state.minio_client.clone());
    let minio_service = MinIOServiceImpl::new(minio_client);
    let service = ImportExportServiceImpl::new(
        state.db_pool.clone(),
        minio_service,
        env!("CARGO_PKG_VERSION").to_string(),
    );

    // Export job (user ID would come from JWT claims in production)
    let exported_job = service
        .export_job(req.job_id, "system".to_string())
        .await
        .map_err(|e| {
            ErrorResponse::new("export_failed", &format!("Failed to export job: {}", e))
        })?;

    // Generate filename
    let filename =
        ImportExportServiceImpl::<MinIOServiceImpl>::generate_export_filename(&exported_job.name);

    let response = ExportJobResponse {
        job: exported_job,
        filename,
    };

    tracing::info!(job_id = %req.job_id, "Job exported successfully");
    Ok(Json(SuccessResponse::new(response)))
}

/// Export multiple jobs in bulk
/// Requirements: 18.12 - Bulk export format
#[tracing::instrument(skip(state, req))]
pub async fn export_jobs_bulk(
    State(state): State<AppState>,
    Json(req): Json<ExportJobsBulkRequest>,
) -> Result<Json<SuccessResponse<ExportJobsBulkResponse>>, ErrorResponse> {
    // Create import/export service
    let minio_client = common::storage::MinioClient::from_bucket(state.minio_client.clone());
    let minio_service = MinIOServiceImpl::new(minio_client);
    let service = ImportExportServiceImpl::new(
        state.db_pool.clone(),
        minio_service,
        env!("CARGO_PKG_VERSION").to_string(),
    );

    // Export jobs (user ID would come from JWT claims in production)
    let exported_jobs = service
        .export_jobs_bulk(req.job_ids.clone(), "system".to_string())
        .await
        .map_err(|e| {
            ErrorResponse::new(
                "bulk_export_failed",
                &format!("Failed to export jobs: {}", e),
            )
        })?;

    let response = ExportJobsBulkResponse {
        count: exported_jobs.len(),
        jobs: exported_jobs,
    };

    tracing::info!(
        requested = req.job_ids.len(),
        exported = response.count,
        "Bulk export completed"
    );
    Ok(Json(SuccessResponse::new(response)))
}

/// Import a single job
/// Requirements: 18.7, 18.8, 18.9, 18.10 - Import with validation and sensitive data
#[tracing::instrument(skip(state, req))]
pub async fn import_job(
    State(state): State<AppState>,
    Json(req): Json<ImportJobRequest>,
) -> Result<Json<SuccessResponse<Uuid>>, ErrorResponse> {
    // Create import/export service
    let minio_client = common::storage::MinioClient::from_bucket(state.minio_client.clone());
    let minio_service = MinIOServiceImpl::new(minio_client);
    let service = ImportExportServiceImpl::new(
        state.db_pool.clone(),
        minio_service,
        env!("CARGO_PKG_VERSION").to_string(),
    );

    // Import job
    let job_id = service
        .import_job(req.job_definition, req.sensitive_data)
        .await
        .map_err(|e| {
            ErrorResponse::new("import_failed", &format!("Failed to import job: {}", e))
        })?;

    tracing::info!(job_id = %job_id, "Job imported successfully");
    Ok(Json(SuccessResponse::new(job_id)))
}

/// Import multiple jobs in bulk
/// Requirements: 18.13 - Bulk import processing
#[tracing::instrument(skip(state, req))]
pub async fn import_jobs_bulk(
    State(state): State<AppState>,
    Json(req): Json<ImportJobsBulkRequest>,
) -> Result<Json<SuccessResponse<ImportJobsBulkResponse>>, ErrorResponse> {
    // Create import/export service
    let minio_client = common::storage::MinioClient::from_bucket(state.minio_client.clone());
    let minio_service = MinIOServiceImpl::new(minio_client);
    let service = ImportExportServiceImpl::new(
        state.db_pool.clone(),
        minio_service,
        env!("CARGO_PKG_VERSION").to_string(),
    );

    // Import jobs
    let results = service
        .import_jobs_bulk(req.job_definitions, req.sensitive_data)
        .await
        .map_err(|e| {
            ErrorResponse::new(
                "bulk_import_failed",
                &format!("Failed to import jobs: {}", e),
            )
        })?;

    let success_count = results.iter().filter(|r| r.success).count();
    let failed_count = results.len() - success_count;

    let response = ImportJobsBulkResponse {
        total: results.len(),
        success_count,
        failed_count,
        results,
    };

    tracing::info!(
        total = response.total,
        success = response.success_count,
        failed = response.failed_count,
        "Bulk import completed"
    );
    Ok(Json(SuccessResponse::new(response)))
}
