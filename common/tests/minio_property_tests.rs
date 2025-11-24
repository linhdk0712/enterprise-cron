// Property-based tests for MinIO storage operations
// Feature: vietnam-enterprise-cron
// Requirements: 13.2, 13.3, 13.7 - MinIO job definition and context persistence

use common::models::{FileMetadata, JobContext, StepOutput, WebhookData};
use common::storage::minio::MinioClient;
use common::storage::service::{MinIOService, MinIOServiceImpl};
use proptest::prelude::*;
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================================
// Property Generators
// ============================================================================

/// Generate a valid JSON job definition
fn arb_job_definition() -> impl Strategy<Value = String> {
    (
        "[a-z]{3,10}",                    // job name
        prop::option::of("[a-z ]{5,20}"), // description
        prop::bool::ANY,                  // enabled
        100u32..3600u32,                  // timeout_seconds
        1u32..20u32,                      // max_retries
    )
        .prop_map(|(name, description, enabled, timeout, retries)| {
            json!({
                "name": name,
                "description": description,
                "enabled": enabled,
                "timeout_seconds": timeout,
                "max_retries": retries,
                "schedule": {
                    "type": "cron",
                    "expression": "0 0 * * * *",
                    "timezone": "Asia/Ho_Chi_Minh"
                },
                "steps": [],
                "triggers": {
                    "scheduled": true,
                    "manual": false,
                    "webhook": null
                }
            })
            .to_string()
        })
}

/// Generate a JobContext with random data
fn arb_job_context() -> impl Strategy<Value = JobContext> {
    (
        any::<[u8; 16]>(),                                             // execution_id bytes
        any::<[u8; 16]>(),                                             // job_id bytes
        prop::collection::hash_map("[a-z]{3,10}", any::<i64>(), 0..5), // variables
        prop::collection::hash_map("[a-z]{3,10}", "[a-z]{5,20}", 0..3), // step outputs (simplified)
    )
        .prop_map(|(exec_id_bytes, job_id_bytes, vars, steps)| {
            let execution_id = Uuid::from_bytes(exec_id_bytes);
            let job_id = Uuid::from_bytes(job_id_bytes);

            let mut context = JobContext::new(execution_id, job_id);

            // Add variables
            for (key, value) in vars {
                context.set_variable(key, json!(value));
            }

            // Add step outputs
            for (step_id, output_str) in steps {
                let step_output = StepOutput {
                    step_id: step_id.clone(),
                    status: "success".to_string(),
                    output: json!({"result": output_str}),
                    started_at: chrono::Utc::now(),
                    completed_at: chrono::Utc::now(),
                };
                context.set_step_output(step_id, step_output);
            }

            context
        })
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a test MinIO client (mock or testcontainer)
/// For property tests, we'll use an in-memory mock
async fn create_test_minio_client() -> MinioClient {
    // In a real implementation, this would use testcontainers
    // For now, we'll create a client that connects to a test MinIO instance
    // The tests will be marked as integration tests
    use common::config::MinioConfig;

    let config = MinioConfig {
        endpoint: "http://localhost:9000".to_string(),
        access_key: "minioadmin".to_string(),
        secret_key: "minioadmin".to_string(),
        bucket: "test-bucket".to_string(),
        region: "us-east-1".to_string(),
    };

    MinioClient::new(&config)
        .await
        .expect("Failed to create test MinIO client")
}

// ============================================================================
// Property Tests
// ============================================================================

/// **Feature: vietnam-enterprise-cron, Property 77: MinIO job definition persistence**
/// **Validates: Requirements 13.2**
///
/// *For any* job definition stored in MinIO, retrieving it should return
/// the same definition (round-trip consistency).
#[test]
#[ignore] // Requires MinIO testcontainer
fn property_minio_job_definition_persistence() {
    proptest!(ProptestConfig::with_cases(100), |(
        job_id_bytes in any::<[u8; 16]>(),
        definition in arb_job_definition()
    )| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = create_test_minio_client().await;
            let service = MinIOServiceImpl::new(client);
            let job_id = Uuid::from_bytes(job_id_bytes);

            // Store the job definition
            let path = service.store_job_definition(job_id, &definition).await;
            prop_assert!(path.is_ok(), "Failed to store job definition: {:?}", path.err());

            // Load the job definition
            let loaded = service.load_job_definition(job_id).await;
            prop_assert!(loaded.is_ok(), "Failed to load job definition: {:?}", loaded.err());

            // Verify round-trip consistency
            let loaded_definition = loaded.unwrap();

            // Parse both as JSON to compare semantically (ignoring whitespace)
            let original_json: serde_json::Value = serde_json::from_str(&definition).unwrap();
            let loaded_json: serde_json::Value = serde_json::from_str(&loaded_definition).unwrap();

            prop_assert_eq!(original_json, loaded_json,
                "Job definition round-trip failed: original != loaded");

            Ok(())
        }).unwrap();
    });
}

/// **Feature: vietnam-enterprise-cron, Property 78: MinIO path format for job definitions**
/// **Validates: Requirements 13.3**
///
/// *For any* job_id, the MinIO path for the job definition should be
/// `jobs/{job_id}/definition.json`.
#[test]
fn property_minio_path_format_for_job_definitions() {
    proptest!(ProptestConfig::with_cases(100), |(
        job_id_bytes in any::<[u8; 16]>(),
        definition in arb_job_definition()
    )| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = create_test_minio_client().await;
            let service = MinIOServiceImpl::new(client);
            let job_id = Uuid::from_bytes(job_id_bytes);

            // Store the job definition
            let path = service.store_job_definition(job_id, &definition).await;
            prop_assert!(path.is_ok(), "Failed to store job definition");

            let returned_path = path.unwrap();
            let expected_path = format!("jobs/{}/definition.json", job_id);

            prop_assert_eq!(&returned_path, &expected_path,
                "Path format incorrect: expected '{}', got '{}'", expected_path, returned_path);

            Ok(())
        }).unwrap();
    });
}

/// **Feature: vietnam-enterprise-cron, Property 82: Job Context persistence to MinIO**
/// **Validates: Requirements 13.7**
///
/// *For any* Job Context persisted to MinIO, retrieving it should return
/// the same context (round-trip consistency).
#[test]
#[ignore] // Requires MinIO testcontainer
fn property_job_context_persistence_to_minio() {
    proptest!(ProptestConfig::with_cases(100), |(
        context in arb_job_context()
    )| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = create_test_minio_client().await;
            let service = MinIOServiceImpl::new(client);

            // Store the job context
            let path = service.store_context(&context).await;
            prop_assert!(path.is_ok(), "Failed to store job context: {:?}", path.err());

            // Load the job context
            let loaded = service.load_context(context.job_id, context.execution_id).await;
            prop_assert!(loaded.is_ok(), "Failed to load job context: {:?}", loaded.err());

            let loaded_context = loaded.unwrap();

            // Verify round-trip consistency
            prop_assert_eq!(loaded_context.execution_id, context.execution_id,
                "Execution ID mismatch");
            prop_assert_eq!(loaded_context.job_id, context.job_id,
                "Job ID mismatch");
            prop_assert_eq!(loaded_context.variables.len(), context.variables.len(),
                "Variables count mismatch");
            prop_assert_eq!(loaded_context.steps.len(), context.steps.len(),
                "Steps count mismatch");

            // Verify variables
            for (key, value) in &context.variables {
                prop_assert!(loaded_context.variables.contains_key(key),
                    "Variable '{}' missing after round-trip", key);
                prop_assert_eq!(loaded_context.variables.get(key), Some(value),
                    "Variable '{}' value mismatch", key);
            }

            // Verify steps
            for (step_id, step_output) in &context.steps {
                prop_assert!(loaded_context.steps.contains_key(step_id),
                    "Step '{}' missing after round-trip", step_id);
                let loaded_step = loaded_context.steps.get(step_id).unwrap();
                prop_assert_eq!(&loaded_step.step_id, &step_output.step_id,
                    "Step ID mismatch for step '{}'", step_id);
                prop_assert_eq!(&loaded_step.status, &step_output.status,
                    "Step status mismatch for step '{}'", step_id);
            }

            Ok(())
        }).unwrap();
    });
}

/// **Feature: vietnam-enterprise-cron, Property 83: Job Context path format**
/// **Validates: Requirements 13.7**
///
/// *For any* job_id and execution_id, the MinIO path for Job Context should be
/// `jobs/{job_id}/executions/{execution_id}/context.json`.
#[test]
fn property_job_context_path_format() {
    proptest!(ProptestConfig::with_cases(100), |(
        context in arb_job_context()
    )| {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let client = create_test_minio_client().await;
            let service = MinIOServiceImpl::new(client);

            // Store the job context
            let path = service.store_context(&context).await;
            prop_assert!(path.is_ok(), "Failed to store job context");

            let returned_path = path.unwrap();
            let expected_path = format!(
                "jobs/{}/executions/{}/context.json",
                context.job_id,
                context.execution_id
            );

            prop_assert_eq!(&returned_path, &expected_path,
                "Path format incorrect: expected '{}', got '{}'", expected_path, returned_path);

            Ok(())
        }).unwrap();
    });
}

// ============================================================================
// Additional Edge Case Tests
// ============================================================================

/// Test that invalid JSON is rejected when storing job definitions
#[test]
fn test_invalid_json_job_definition_rejected() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let client = create_test_minio_client().await;
        let service = MinIOServiceImpl::new(client);
        let job_id = Uuid::new_v4();

        let invalid_json = "{ this is not valid json }";

        let result = service.store_job_definition(job_id, invalid_json).await;
        assert!(result.is_err(), "Should reject invalid JSON");
    });
}

/// Test that empty job context can be stored and loaded
#[test]
#[ignore] // Requires MinIO testcontainer
fn test_empty_job_context_round_trip() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let client = create_test_minio_client().await;
        let service = MinIOServiceImpl::new(client);

        let execution_id = Uuid::new_v4();
        let job_id = Uuid::new_v4();
        let context = JobContext::new(execution_id, job_id);

        // Store empty context
        let path = service.store_context(&context).await;
        assert!(path.is_ok(), "Failed to store empty context");

        // Load empty context
        let loaded = service.load_context(job_id, execution_id).await;
        assert!(loaded.is_ok(), "Failed to load empty context");

        let loaded_context = loaded.unwrap();
        assert_eq!(loaded_context.execution_id, execution_id);
        assert_eq!(loaded_context.job_id, job_id);
        assert_eq!(loaded_context.variables.len(), 0);
        assert_eq!(loaded_context.steps.len(), 0);
    });
}

/// Test that job context with webhook data can be stored and loaded
#[test]
#[ignore] // Requires MinIO testcontainer
fn test_job_context_with_webhook_data() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let client = create_test_minio_client().await;
        let service = MinIOServiceImpl::new(client);

        let execution_id = Uuid::new_v4();
        let job_id = Uuid::new_v4();
        let mut context = JobContext::new(execution_id, job_id);

        // Add webhook data
        let mut headers = HashMap::new();
        headers.insert("X-Custom-Header".to_string(), "value".to_string());

        let mut query_params = HashMap::new();
        query_params.insert("param1".to_string(), "value1".to_string());

        let webhook_data = WebhookData {
            payload: json!({"user_id": 123, "action": "create"}),
            query_params,
            headers,
        };
        context.set_webhook_data(webhook_data);

        // Store context
        let path = service.store_context(&context).await;
        assert!(path.is_ok(), "Failed to store context with webhook data");

        // Load context
        let loaded = service.load_context(job_id, execution_id).await;
        assert!(loaded.is_ok(), "Failed to load context with webhook data");

        let loaded_context = loaded.unwrap();
        assert!(loaded_context.webhook.is_some(), "Webhook data missing");

        let loaded_webhook = loaded_context.webhook.unwrap();
        assert_eq!(loaded_webhook.payload["user_id"], 123);
        assert_eq!(loaded_webhook.payload["action"], "create");
        assert_eq!(
            loaded_webhook.query_params.get("param1"),
            Some(&"value1".to_string())
        );
        assert_eq!(
            loaded_webhook.headers.get("X-Custom-Header"),
            Some(&"value".to_string())
        );
    });
}

/// Test that job context with file metadata can be stored and loaded
#[test]
#[ignore] // Requires MinIO testcontainer
fn test_job_context_with_file_metadata() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let client = create_test_minio_client().await;
        let service = MinIOServiceImpl::new(client);

        let execution_id = Uuid::new_v4();
        let job_id = Uuid::new_v4();
        let mut context = JobContext::new(execution_id, job_id);

        // Add file metadata
        let file_metadata = FileMetadata {
            path: format!(
                "jobs/{}/executions/{}/output/data.xlsx",
                job_id, execution_id
            ),
            filename: "data.xlsx".to_string(),
            size: 1024,
            mime_type: Some(
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(),
            ),
            row_count: Some(100),
            created_at: chrono::Utc::now(),
        };
        context.add_file_metadata(file_metadata);

        // Store context
        let path = service.store_context(&context).await;
        assert!(path.is_ok(), "Failed to store context with file metadata");

        // Load context
        let loaded = service.load_context(job_id, execution_id).await;
        assert!(loaded.is_ok(), "Failed to load context with file metadata");

        let loaded_context = loaded.unwrap();
        assert_eq!(loaded_context.files.len(), 1, "File metadata missing");

        let loaded_file = &loaded_context.files[0];
        assert_eq!(loaded_file.filename, "data.xlsx");
        assert_eq!(loaded_file.size, 1024);
        assert_eq!(loaded_file.row_count, Some(100));
    });
}
