// Job import/export service
// Requirements: 18.1-18.14 - Job import/export functionality
// RECC 2025: No unwrap(), use #[tracing::instrument], proper error handling

use crate::db::repositories::job::JobRepository;
use crate::db::DbPool;
use crate::errors::{DatabaseError, StorageError, ValidationError};
use crate::models::{Job, JobStep, Schedule, TriggerConfig};
use crate::storage::service::MinIOService;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

/// General error type for import/export operations
#[derive(Debug, thiserror::Error)]
pub enum ImportExportError {
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Not found: {0}")]
    NotFound(String),
}

/// Export metadata included in exported job definitions
/// Requirements: 18.14 - Export metadata inclusion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportMetadata {
    pub export_date: DateTime<Utc>,
    pub exported_by: String,
    pub system_version: String,
}

/// Exported job definition with metadata
/// Requirements: 18.4, 18.5, 18.14 - Export completeness, sensitive data masking, metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportedJob {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub schedule: Option<Schedule>,
    pub steps: Vec<JobStep>,
    pub triggers: TriggerConfig,
    pub timeout_seconds: i32,
    pub max_retries: i32,
    pub allow_concurrent: bool,
    pub metadata: ExportMetadata,
}

/// Import result for a single job
/// Requirements: 18.13 - Bulk import processing with success/failure reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub success: bool,
    pub job_id: Option<Uuid>,
    pub job_name: String,
    pub error: Option<String>,
}

/// Sensitive field patterns to mask during export
/// Requirements: 18.5 - Sensitive data masking on export
const SENSITIVE_FIELD_PATTERNS: &[&str] = &[
    "password",
    "secret",
    "token",
    "key",
    "api_key",
    "client_secret",
    "private_key",
];

/// Placeholder for masked sensitive data
/// Requirements: 18.5 - Replace sensitive data with placeholder values
const SENSITIVE_DATA_PLACEHOLDER: &str = "***MASKED***";

/// Job import/export service trait
#[async_trait]
pub trait ImportExportService: Send + Sync {
    /// Export a single job
    async fn export_job(
        &self,
        job_id: Uuid,
        exported_by: String,
    ) -> Result<ExportedJob, ImportExportError>;

    /// Export multiple jobs in bulk
    async fn export_jobs_bulk(
        &self,
        job_ids: Vec<Uuid>,
        exported_by: String,
    ) -> Result<Vec<ExportedJob>, ImportExportError>;

    /// Import a single job
    async fn import_job(
        &self,
        job_definition: serde_json::Value,
        sensitive_data: HashMap<String, String>,
    ) -> Result<Uuid, ImportExportError>;

    /// Import multiple jobs in bulk
    async fn import_jobs_bulk(
        &self,
        job_definitions: Vec<serde_json::Value>,
        sensitive_data: HashMap<String, HashMap<String, String>>,
    ) -> Result<Vec<ImportResult>, ImportExportError>;

    /// Generate export filename
    fn generate_export_filename(job_name: &str) -> String;

    /// Mask sensitive data in job definition
    fn mask_sensitive_data(job_definition: &mut serde_json::Value);

    /// Validate job definition JSON schema
    fn validate_job_definition(job_definition: &serde_json::Value)
        -> Result<(), ImportExportError>;

    /// Handle duplicate job names
    async fn generate_unique_job_name(&self, base_name: &str) -> Result<String, ImportExportError>;
}

/// Job import/export service implementation
pub struct ImportExportServiceImpl<M: MinIOService> {
    db_pool: DbPool,
    minio_service: M,
    system_version: String,
}

impl<M: MinIOService> ImportExportServiceImpl<M> {
    /// Create a new import/export service instance
    pub fn new(db_pool: DbPool, minio_service: M, system_version: String) -> Self {
        Self {
            db_pool,
            minio_service,
            system_version,
        }
    }

    /// Check if a field name is sensitive
    fn is_sensitive_field(field_name: &str) -> bool {
        let field_lower = field_name.to_lowercase();
        SENSITIVE_FIELD_PATTERNS
            .iter()
            .any(|pattern| field_lower.contains(pattern))
    }

    /// Recursively mask sensitive data in JSON
    fn mask_sensitive_data_recursive(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, val) in map.iter_mut() {
                    if Self::is_sensitive_field(key) {
                        *val = serde_json::Value::String(SENSITIVE_DATA_PLACEHOLDER.to_string());
                    } else {
                        Self::mask_sensitive_data_recursive(val);
                    }
                }
            }
            serde_json::Value::Array(arr) => {
                for item in arr.iter_mut() {
                    Self::mask_sensitive_data_recursive(item);
                }
            }
            _ => {}
        }
    }

    /// Restore sensitive data from provided values
    fn restore_sensitive_data(
        job_definition: &mut serde_json::Value,
        sensitive_data: &HashMap<String, String>,
    ) {
        Self::restore_sensitive_data_recursive(job_definition, sensitive_data, "");
    }

    /// Recursively restore sensitive data in JSON
    fn restore_sensitive_data_recursive(
        value: &mut serde_json::Value,
        sensitive_data: &HashMap<String, String>,
        path: &str,
    ) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, val) in map.iter_mut() {
                    let current_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", path, key)
                    };

                    if Self::is_sensitive_field(key) {
                        if let Some(replacement) = sensitive_data.get(&current_path) {
                            *val = serde_json::Value::String(replacement.clone());
                        }
                    } else {
                        Self::restore_sensitive_data_recursive(val, sensitive_data, &current_path);
                    }
                }
            }
            serde_json::Value::Array(arr) => {
                for (idx, item) in arr.iter_mut().enumerate() {
                    let current_path = format!("{}[{}]", path, idx);
                    Self::restore_sensitive_data_recursive(item, sensitive_data, &current_path);
                }
            }
            _ => {}
        }
    }
}

#[async_trait]
impl<M: MinIOService> ImportExportService for ImportExportServiceImpl<M> {
    #[instrument(skip(self), fields(job_id = %job_id, exported_by = %exported_by))]
    async fn export_job(
        &self,
        job_id: Uuid,
        exported_by: String,
    ) -> Result<ExportedJob, ImportExportError> {
        info!(job_id = %job_id, "Exporting job");

        let repo = JobRepository::new(self.db_pool.clone());
        let job = repo
            .find_by_id(job_id)
            .await?
            .ok_or_else(|| ImportExportError::NotFound(format!("Job not found: {}", job_id)))?;

        let definition_json = self.minio_service.load_job_definition(job_id).await?;
        let mut job_definition: serde_json::Value = serde_json::from_str(&definition_json)?;

        Self::mask_sensitive_data_recursive(&mut job_definition);

        let schedule: Option<Schedule> = job_definition
            .get("schedule")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        let steps: Vec<JobStep> = job_definition
            .get("steps")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        let triggers: TriggerConfig = job_definition
            .get("triggers")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        let metadata = ExportMetadata {
            export_date: Utc::now(),
            exported_by,
            system_version: self.system_version.clone(),
        };

        let exported_job = ExportedJob {
            id: job.id,
            name: job.name.clone(),
            description: job.description.clone(),
            schedule,
            steps,
            triggers,
            timeout_seconds: job.timeout_seconds,
            max_retries: job.max_retries,
            allow_concurrent: job.allow_concurrent,
            metadata,
        };

        info!(job_id = %job_id, job_name = %job.name, "Job exported successfully");
        Ok(exported_job)
    }

    #[instrument(skip(self), fields(count = job_ids.len(), exported_by = %exported_by))]
    async fn export_jobs_bulk(
        &self,
        job_ids: Vec<Uuid>,
        exported_by: String,
    ) -> Result<Vec<ExportedJob>, ImportExportError> {
        info!(count = job_ids.len(), "Exporting jobs in bulk");

        let mut exported_jobs = Vec::new();

        for job_id in job_ids {
            match self.export_job(job_id, exported_by.clone()).await {
                Ok(exported_job) => {
                    exported_jobs.push(exported_job);
                }
                Err(e) => {
                    warn!(job_id = %job_id, error = %e, "Failed to export job");
                }
            }
        }

        info!(exported = exported_jobs.len(), "Bulk export completed");
        Ok(exported_jobs)
    }

    #[instrument(skip(self, job_definition, sensitive_data))]
    async fn import_job(
        &self,
        mut job_definition: serde_json::Value,
        sensitive_data: HashMap<String, String>,
    ) -> Result<Uuid, ImportExportError> {
        info!("Importing job");

        Self::validate_job_definition(&job_definition)?;
        Self::restore_sensitive_data(&mut job_definition, &sensitive_data);

        let base_name = job_definition
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ImportExportError::Validation(ValidationError::MissingField("name".to_string()))
            })?
            .to_string();

        let unique_name = self.generate_unique_job_name(&base_name).await?;
        job_definition["name"] = serde_json::Value::String(unique_name.clone());

        let job_id = Uuid::new_v4();
        job_definition["id"] = serde_json::Value::String(job_id.to_string());

        let description: Option<String> = job_definition
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let schedule: Option<Schedule> = job_definition
            .get("schedule")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        let steps: Vec<JobStep> = job_definition
            .get("steps")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .ok_or_else(|| {
                ImportExportError::Validation(ValidationError::MissingField("steps".to_string()))
            })?;

        let triggers: TriggerConfig = job_definition
            .get("triggers")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        let timeout_seconds: i32 = job_definition
            .get("timeout_seconds")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32)
            .unwrap_or(300);

        let max_retries: i32 = job_definition
            .get("max_retries")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32)
            .unwrap_or(10);

        let allow_concurrent: bool = job_definition
            .get("allow_concurrent")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let minio_path = format!("jobs/{}/definition.json", job_id);
        let definition_json = serde_json::to_string_pretty(&job_definition)?;
        self.minio_service
            .store_job_definition(job_id, &definition_json)
            .await?;

        let now = Utc::now();
        let job = Job {
            id: job_id,
            name: unique_name.clone(),
            description,
            schedule,
            steps,
            triggers,
            enabled: true,
            timeout_seconds,
            max_retries,
            allow_concurrent,
            minio_definition_path: minio_path,
            created_at: now,
            updated_at: now,
        };

        let repo = JobRepository::new(self.db_pool.clone());
        repo.create(&job).await?;

        info!(job_id = %job_id, job_name = %unique_name, "Job imported successfully");
        Ok(job_id)
    }

    #[instrument(skip(self, job_definitions, sensitive_data))]
    async fn import_jobs_bulk(
        &self,
        job_definitions: Vec<serde_json::Value>,
        sensitive_data: HashMap<String, HashMap<String, String>>,
    ) -> Result<Vec<ImportResult>, ImportExportError> {
        info!(count = job_definitions.len(), "Importing jobs in bulk");

        let mut results = Vec::new();

        for (idx, job_definition) in job_definitions.into_iter().enumerate() {
            let job_name = job_definition
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            let job_sensitive_data = sensitive_data
                .get(&idx.to_string())
                .or_else(|| sensitive_data.get(&job_name))
                .cloned()
                .unwrap_or_default();

            match self.import_job(job_definition, job_sensitive_data).await {
                Ok(job_id) => {
                    results.push(ImportResult {
                        success: true,
                        job_id: Some(job_id),
                        job_name,
                        error: None,
                    });
                }
                Err(e) => {
                    warn!(job_name = %job_name, error = %e, "Failed to import job");
                    results.push(ImportResult {
                        success: false,
                        job_id: None,
                        job_name,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        let success_count = results.iter().filter(|r| r.success).count();
        info!(
            total = results.len(),
            success = success_count,
            "Bulk import completed"
        );

        Ok(results)
    }

    fn generate_export_filename(job_name: &str) -> String {
        let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
        let sanitized_name = job_name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '-'
                }
            })
            .collect::<String>();
        format!("job-{}-{}.json", sanitized_name, timestamp)
    }

    fn mask_sensitive_data(job_definition: &mut serde_json::Value) {
        Self::mask_sensitive_data_recursive(job_definition);
    }

    fn validate_job_definition(
        job_definition: &serde_json::Value,
    ) -> Result<(), ImportExportError> {
        debug!("Validating job definition schema");

        let required_fields = vec!["name", "steps"];
        for field in required_fields {
            if !job_definition.get(field).is_some() {
                return Err(ImportExportError::Validation(
                    ValidationError::MissingField(field.to_string()),
                ));
            }
        }

        if !job_definition
            .get("name")
            .and_then(|v| v.as_str())
            .is_some()
        {
            return Err(ImportExportError::Validation(
                ValidationError::InvalidFieldValue {
                    field: "name".to_string(),
                    reason: "must be a string".to_string(),
                },
            ));
        }

        if !job_definition
            .get("steps")
            .and_then(|v| v.as_array())
            .is_some()
        {
            return Err(ImportExportError::Validation(
                ValidationError::InvalidFieldValue {
                    field: "steps".to_string(),
                    reason: "must be an array".to_string(),
                },
            ));
        }

        let steps = job_definition
            .get("steps")
            .and_then(|v| v.as_array())
            .unwrap();
        if steps.is_empty() {
            return Err(ImportExportError::Validation(
                ValidationError::InvalidFieldValue {
                    field: "steps".to_string(),
                    reason: "must contain at least one step".to_string(),
                },
            ));
        }

        for (idx, step) in steps.iter().enumerate() {
            if !step.get("id").is_some() {
                return Err(ImportExportError::Validation(
                    ValidationError::MissingField(format!("steps[{}].id", idx)),
                ));
            }
            if !step.get("name").is_some() {
                return Err(ImportExportError::Validation(
                    ValidationError::MissingField(format!("steps[{}].name", idx)),
                ));
            }
            if !step.get("type").is_some() {
                return Err(ImportExportError::Validation(
                    ValidationError::MissingField(format!("steps[{}].type", idx)),
                ));
            }
        }

        debug!("Job definition schema validation passed");
        Ok(())
    }

    #[instrument(skip(self), fields(base_name = %base_name))]
    async fn generate_unique_job_name(&self, base_name: &str) -> Result<String, ImportExportError> {
        let repo = JobRepository::new(self.db_pool.clone());

        let existing = repo.find_by_name(base_name).await?;
        if existing.is_none() {
            return Ok(base_name.to_string());
        }

        let mut counter = 1;
        loop {
            let candidate = format!("{}-copy-{}", base_name, counter);
            let existing = repo.find_by_name(&candidate).await?;
            if existing.is_none() {
                debug!(
                    base_name = %base_name,
                    unique_name = %candidate,
                    "Generated unique job name"
                );
                return Ok(candidate);
            }
            counter += 1;

            if counter > 1000 {
                return Err(ImportExportError::Validation(
                    ValidationError::ConstraintViolation(
                        "Could not generate unique job name after 1000 attempts".to_string(),
                    ),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::service::MinIOServiceImpl;

    #[test]
    fn test_export_filename_format() {
        let filename =
            ImportExportServiceImpl::<MinIOServiceImpl>::generate_export_filename("test-job");
        assert!(filename.starts_with("job-test-job-"));
        assert!(filename.ends_with(".json"));
    }

    #[test]
    fn test_is_sensitive_field() {
        assert!(ImportExportServiceImpl::<MinIOServiceImpl>::is_sensitive_field("password"));
        assert!(ImportExportServiceImpl::<MinIOServiceImpl>::is_sensitive_field("api_key"));
        assert!(!ImportExportServiceImpl::<MinIOServiceImpl>::is_sensitive_field("name"));
    }

    #[test]
    fn test_mask_sensitive_data() {
        let mut job_def = serde_json::json!({
            "name": "test-job",
            "steps": [{
                "type": "http_request",
                "auth": {
                    "type": "basic",
                    "username": "user",
                    "password": "secret123"
                }
            }]
        });

        ImportExportServiceImpl::<MinIOServiceImpl>::mask_sensitive_data(&mut job_def);

        let password = job_def["steps"][0]["auth"]["password"].as_str().unwrap();
        assert_eq!(password, SENSITIVE_DATA_PLACEHOLDER);
    }

    #[test]
    fn test_validate_job_definition_valid() {
        let job_def = serde_json::json!({
            "name": "test-job",
            "steps": [{
                "id": "step1",
                "name": "Step 1",
                "type": "http_request"
            }]
        });

        let result = ImportExportServiceImpl::<MinIOServiceImpl>::validate_job_definition(&job_def);
        assert!(result.is_ok());
    }
}
