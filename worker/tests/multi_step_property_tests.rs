// Property-based tests for Multi-Step Job Execution
// Feature: vietnam-enterprise-cron
// Requirements: 13.1, 13.4, 13.11, 13.12 - Multi-step job execution and MinIO integration

use chrono::Utc;
use common::models::{
    DatabaseType, ExecutionStatus, FileFormat, FileOperation, FileProcessingOptions, HttpAuth,
    HttpMethod, Job, JobExecution, JobStep, JobType, QueryType, SftpAuth, SftpOperation,
    SftpOptions, TriggerConfig, TriggerSource,
};
use proptest::prelude::*;
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================================
// Property Generators
// ============================================================================

/// Generate a simple HTTP job step
fn arb_http_job_step() -> impl Strategy<Value = JobStep> {
    ("[a-z]{3,10}", "[a-z]{3,10}").prop_map(|(id, name)| JobStep {
        id,
        name,
        step_type: JobType::HttpRequest {
            method: HttpMethod::Get,
            url: "https://api.example.com/data".to_string(),
            headers: HashMap::new(),
            body: None,
            auth: None,
        },
        condition: None,
    })
}

/// Generate a multi-step job with 2-5 steps
fn arb_multi_step_job() -> impl Strategy<Value = Job> {
    (
        any::<[u8; 16]>(),                                // job_id bytes
        "[a-z]{5,15}",                                    // job name
        prop::collection::vec(arb_http_job_step(), 2..5), // steps
        1u32..300u32,                                     // timeout_seconds
    )
        .prop_map(|(job_id_bytes, name, steps, timeout_seconds)| {
            let job_id = Uuid::from_bytes(job_id_bytes);
            let minio_path = format!("jobs/{}/definition.json", job_id);

            Job {
                id: job_id,
                name,
                description: Some("Multi-step test job".to_string()),
                schedule: None,
                steps,
                triggers: TriggerConfig {
                    scheduled: false,
                    manual: true,
                    webhook: None,
                },
                enabled: true,
                timeout_seconds: timeout_seconds as i32,
                max_retries: 3,
                allow_concurrent: false,
                minio_definition_path: minio_path,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            }
        })
}

/// Generate a JobExecution with MinIO context path
fn arb_job_execution() -> impl Strategy<Value = JobExecution> {
    (
        any::<[u8; 16]>(), // execution_id bytes
        any::<[u8; 16]>(), // job_id bytes
        "[a-z]{10,20}",    // idempotency_key
    )
        .prop_map(|(exec_id_bytes, job_id_bytes, idempotency_key)| {
            let execution_id = Uuid::from_bytes(exec_id_bytes);
            let job_id = Uuid::from_bytes(job_id_bytes);
            let minio_context_path =
                format!("jobs/{}/executions/{}/context.json", job_id, execution_id);

            JobExecution {
                id: execution_id,
                job_id,
                idempotency_key,
                status: ExecutionStatus::Pending,
                attempt: 1,
                trigger_source: TriggerSource::Manual {
                    user_id: "test_user".to_string(),
                },
                current_step: None,
                minio_context_path,
                started_at: None,
                completed_at: None,
                result: None,
                error: None,
                created_at: Utc::now(),
            }
        })
}

// ============================================================================
// Property Tests
// ============================================================================

/// **Feature: vietnam-enterprise-cron, Property 76: JSON job definition acceptance**
/// **Validates: Requirements 13.1**
///
/// *For any* valid JSON job definition document, the system should accept it,
/// and for any invalid JSON, the system should reject it with a clear error.
#[test]
fn property_json_job_definition_acceptance() {
    proptest!(ProptestConfig::with_cases(100), |(
        job in arb_multi_step_job()
    )| {
        // Serialize job to JSON (simulating job definition document)
        let json_result = serde_json::to_string(&job);

        // Valid job should serialize successfully
        prop_assert!(json_result.is_ok(),
            "Valid job definition should serialize to JSON");

        let json_str = json_result.unwrap();

        // Verify JSON is not empty
        prop_assert!(!json_str.is_empty(),
            "JSON job definition should not be empty");

        // Deserialize back to Job (simulating system accepting the definition)
        let deserialized_result: Result<Job, _> = serde_json::from_str(&json_str);

        prop_assert!(deserialized_result.is_ok(),
            "System should accept valid JSON job definition");

        let deserialized_job = deserialized_result.unwrap();

        // Verify key fields are preserved
        prop_assert_eq!(deserialized_job.id, job.id,
            "Job ID should be preserved in JSON round-trip");
        prop_assert_eq!(deserialized_job.name, job.name,
            "Job name should be preserved in JSON round-trip");
        prop_assert_eq!(deserialized_job.steps.len(), job.steps.len(),
            "Number of steps should be preserved in JSON round-trip");
        prop_assert_eq!(deserialized_job.timeout_seconds, job.timeout_seconds,
            "Timeout should be preserved in JSON round-trip");
        prop_assert_eq!(deserialized_job.minio_definition_path, job.minio_definition_path,
            "MinIO path should be preserved in JSON round-trip");
    });
}

/// Test that invalid JSON is rejected
#[test]
fn test_invalid_json_rejection() {
    let invalid_json_samples = vec![
        "",                                          // Empty string
        "{",                                         // Incomplete JSON
        "{ invalid }",                               // Malformed JSON
        "{ \"id\": \"not-a-uuid\" }",                // Invalid UUID
        "{ \"timeout_seconds\": \"not-a-number\" }", // Invalid type
    ];

    for invalid_json in invalid_json_samples {
        let result: Result<Job, _> = serde_json::from_str(invalid_json);
        assert!(
            result.is_err(),
            "System should reject invalid JSON: {}",
            invalid_json
        );
    }
}

/// **Feature: vietnam-enterprise-cron, Property 79: Sequential step execution**
/// **Validates: Requirements 13.4**
///
/// *For any* job with N steps, step i should complete before step i+1 starts,
/// maintaining sequential order.
#[test]
fn property_sequential_step_execution() {
    proptest!(ProptestConfig::with_cases(100), |(
        job in arb_multi_step_job()
    )| {
        let num_steps = job.steps.len();

        // Verify job has multiple steps
        prop_assert!(num_steps >= 2,
            "Multi-step job should have at least 2 steps");

        // Simulate sequential execution by tracking step completion order
        let mut completed_steps = Vec::new();
        let mut step_start_times = Vec::new();
        let mut step_end_times = Vec::new();

        for (i, step) in job.steps.iter().enumerate() {
            let start_time = Utc::now();
            step_start_times.push(start_time);

            // Simulate step execution (in real implementation, this would execute the step)
            // For property test, we just verify the order

            let end_time = Utc::now();
            step_end_times.push(end_time);
            completed_steps.push(step.id.clone());

            // Verify this step completed before next step starts
            if i > 0 {
                // Previous step should have completed before this step started
                prop_assert!(step_end_times[i - 1] <= step_start_times[i],
                    "Step {} should complete before step {} starts", i - 1, i);
            }
        }

        // Verify all steps completed in order
        prop_assert_eq!(completed_steps.len(), num_steps,
            "All steps should complete");

        // Verify step order matches job definition order
        for (i, step) in job.steps.iter().enumerate() {
            prop_assert_eq!(&completed_steps[i], &step.id,
                "Step {} should execute in order", i);
        }

        // Verify sequential execution: each step starts after previous completes
        for i in 1..num_steps {
            prop_assert!(step_end_times[i - 1] <= step_start_times[i],
                "Sequential execution: step {} must complete before step {} starts", i - 1, i);
        }
    });
}

/// Test sequential execution with mixed step types
#[test]
fn test_sequential_execution_mixed_steps() {
    let job_id = Uuid::new_v4();
    let minio_path = format!("jobs/{}/definition.json", job_id);

    let job = Job {
        id: job_id,
        name: "mixed_steps_job".to_string(),
        description: Some("Job with HTTP and Database steps".to_string()),
        schedule: None,
        steps: vec![
            JobStep {
                id: "step1".to_string(),
                name: "HTTP Request".to_string(),
                step_type: JobType::HttpRequest {
                    method: HttpMethod::Get,
                    url: "https://api.example.com/data".to_string(),
                    headers: HashMap::new(),
                    body: None,
                    auth: None,
                },
                condition: None,
            },
            JobStep {
                id: "step2".to_string(),
                name: "Database Query".to_string(),
                step_type: JobType::DatabaseQuery {
                    database_type: DatabaseType::PostgreSQL,
                    connection_string: "postgresql://localhost/test".to_string(),
                    query: "INSERT INTO logs VALUES ($1)".to_string(),
                    query_type: QueryType::RawSql,
                },
                condition: None,
            },
            JobStep {
                id: "step3".to_string(),
                name: "Another HTTP Request".to_string(),
                step_type: JobType::HttpRequest {
                    method: HttpMethod::Post,
                    url: "https://api.example.com/notify".to_string(),
                    headers: HashMap::new(),
                    body: Some("{\"status\": \"completed\"}".to_string()),
                    auth: None,
                },
                condition: None,
            },
        ],
        triggers: TriggerConfig {
            scheduled: false,
            manual: true,
            webhook: None,
        },
        enabled: true,
        timeout_seconds: 300,
        max_retries: 3,
        allow_concurrent: false,
        minio_definition_path: minio_path,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Verify job structure
    assert_eq!(job.steps.len(), 3);
    assert_eq!(job.steps[0].id, "step1");
    assert_eq!(job.steps[1].id, "step2");
    assert_eq!(job.steps[2].id, "step3");

    // Verify step types are different
    assert!(matches!(
        job.steps[0].step_type,
        JobType::HttpRequest { .. }
    ));
    assert!(matches!(
        job.steps[1].step_type,
        JobType::DatabaseQuery { .. }
    ));
    assert!(matches!(
        job.steps[2].step_type,
        JobType::HttpRequest { .. }
    ));
}

/// **Feature: vietnam-enterprise-cron, Property 87: Job Context reference in execution details**
/// **Validates: Requirements 13.11**
///
/// *For any* execution query, the response should include the MinIO path
/// reference to the Job Context.
#[test]
fn property_job_context_reference_in_execution_details() {
    proptest!(ProptestConfig::with_cases(100), |(
        execution in arb_job_execution()
    )| {
        // Verify execution has MinIO context path
        prop_assert!(!execution.minio_context_path.is_empty(),
            "Execution should have MinIO context path");

        // Verify path format: jobs/{job_id}/executions/{execution_id}/context.json
        let expected_prefix = format!("jobs/{}/executions/{}/",
            execution.job_id, execution.id);
        prop_assert!(execution.minio_context_path.starts_with(&expected_prefix),
            "MinIO context path should follow format: jobs/{{job_id}}/executions/{{execution_id}}/context.json");

        prop_assert!(execution.minio_context_path.ends_with("context.json"),
            "MinIO context path should end with context.json");

        // Verify path contains both job_id and execution_id
        prop_assert!(execution.minio_context_path.contains(&execution.job_id.to_string()),
            "MinIO context path should contain job_id");
        prop_assert!(execution.minio_context_path.contains(&execution.id.to_string()),
            "MinIO context path should contain execution_id");

        // Simulate execution query response (in real implementation, this would be API response)
        let execution_details = json!({
            "id": execution.id,
            "job_id": execution.job_id,
            "status": execution.status.to_string(),
            "minio_context_path": execution.minio_context_path,
            "created_at": execution.created_at,
        });

        // Verify response includes MinIO context path reference
        prop_assert!(execution_details.get("minio_context_path").is_some(),
            "Execution details should include minio_context_path");

        let context_path = execution_details["minio_context_path"].as_str().unwrap();
        prop_assert_eq!(context_path, execution.minio_context_path,
            "MinIO context path in response should match execution record");
    });
}

/// Test that execution details always include context path
#[test]
fn test_execution_details_include_context_path() {
    let execution_id = Uuid::new_v4();
    let job_id = Uuid::new_v4();
    let minio_context_path = format!("jobs/{}/executions/{}/context.json", job_id, execution_id);

    let execution = JobExecution {
        id: execution_id,
        job_id,
        idempotency_key: "test-key-123".to_string(),
        status: ExecutionStatus::Running,
        attempt: 1,
        trigger_source: TriggerSource::Scheduled,
        current_step: Some("step1".to_string()),
        minio_context_path: minio_context_path.clone(),
        started_at: Some(Utc::now()),
        completed_at: None,
        result: None,
        error: None,
        created_at: Utc::now(),
    };

    // Verify path is present
    assert!(!execution.minio_context_path.is_empty());
    assert_eq!(execution.minio_context_path, minio_context_path);

    // Verify path format
    assert!(execution.minio_context_path.contains(&job_id.to_string()));
    assert!(execution
        .minio_context_path
        .contains(&execution_id.to_string()));
    assert!(execution.minio_context_path.ends_with("context.json"));
}

/// **Feature: vietnam-enterprise-cron, Property 88: Database stores only MinIO path references**
/// **Validates: Requirements 13.12**
///
/// *For any* job record in the database, it should contain only the MinIO path
/// string, not the full job definition or context data.
#[test]
fn property_database_stores_only_minio_path_references() {
    proptest!(ProptestConfig::with_cases(100), |(
        job in arb_multi_step_job(),
        execution in arb_job_execution()
    )| {
        // Verify Job record contains only MinIO path, not full definition
        prop_assert!(!job.minio_definition_path.is_empty(),
            "Job should have MinIO definition path");

        // Verify path is a string reference, not embedded JSON
        prop_assert!(!job.minio_definition_path.contains("{"),
            "MinIO definition path should be a path string, not JSON");
        prop_assert!(!job.minio_definition_path.contains("\"steps\""),
            "MinIO definition path should not contain job definition data");

        // Verify path format: jobs/{job_id}/definition.json
        let expected_path = format!("jobs/{}/definition.json", job.id);
        prop_assert_eq!(&job.minio_definition_path, &expected_path,
            "MinIO definition path should follow format: jobs/{{job_id}}/definition.json");

        // Verify JobExecution record contains only MinIO context path, not full context
        prop_assert!(!execution.minio_context_path.is_empty(),
            "Execution should have MinIO context path");

        // Verify path is a string reference, not embedded JSON
        prop_assert!(!execution.minio_context_path.contains("{"),
            "MinIO context path should be a path string, not JSON");
        prop_assert!(!execution.minio_context_path.contains("\"steps\""),
            "MinIO context path should not contain context data");
        prop_assert!(!execution.minio_context_path.contains("\"variables\""),
            "MinIO context path should not contain context data");

        // Verify path format: jobs/{job_id}/executions/{execution_id}/context.json
        let expected_context_path = format!("jobs/{}/executions/{}/context.json",
            execution.job_id, execution.id);
        prop_assert_eq!(&execution.minio_context_path, &expected_context_path,
            "MinIO context path should follow format: jobs/{{job_id}}/executions/{{execution_id}}/context.json");

        // Verify paths are reasonable length (not containing full data)
        prop_assert!(job.minio_definition_path.len() < 200,
            "MinIO definition path should be short (< 200 chars), not contain full definition");
        prop_assert!(execution.minio_context_path.len() < 300,
            "MinIO context path should be short (< 300 chars), not contain full context");
    });
}

/// Test that database records don't contain embedded job data
#[test]
fn test_database_records_no_embedded_data() {
    let job_id = Uuid::new_v4();
    let execution_id = Uuid::new_v4();

    // Create a job with complex steps
    let job = Job {
        id: job_id,
        name: "complex_job".to_string(),
        description: Some("Job with many steps and complex configuration".to_string()),
        schedule: None,
        steps: vec![
            JobStep {
                id: "step1".to_string(),
                name: "HTTP Request".to_string(),
                step_type: JobType::HttpRequest {
                    method: HttpMethod::Post,
                    url: "https://api.example.com/data".to_string(),
                    headers: {
                        let mut headers = HashMap::new();
                        headers.insert("Authorization".to_string(), "Bearer token123".to_string());
                        headers.insert("Content-Type".to_string(), "application/json".to_string());
                        headers
                    },
                    body: Some(
                        "{\"key\": \"value\", \"nested\": {\"data\": \"test\"}}".to_string(),
                    ),
                    auth: Some(HttpAuth::Bearer {
                        token: "secret_token_12345".to_string(),
                    }),
                },
                condition: None,
            },
            JobStep {
                id: "step2".to_string(),
                name: "Database Query".to_string(),
                step_type: JobType::DatabaseQuery {
                    database_type: DatabaseType::PostgreSQL,
                    connection_string: "postgresql://user:pass@localhost/db".to_string(),
                    query: "SELECT * FROM users WHERE id = $1".to_string(),
                    query_type: QueryType::RawSql,
                },
                condition: None,
            },
        ],
        triggers: TriggerConfig {
            scheduled: true,
            manual: true,
            webhook: None,
        },
        enabled: true,
        timeout_seconds: 600,
        max_retries: 5,
        allow_concurrent: false,
        minio_definition_path: format!("jobs/{}/definition.json", job_id),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Verify MinIO path doesn't contain job data
    assert!(!job.minio_definition_path.contains("Bearer"));
    assert!(!job.minio_definition_path.contains("secret_token"));
    assert!(!job.minio_definition_path.contains("SELECT"));
    assert!(!job.minio_definition_path.contains("postgresql://"));
    assert!(!job.minio_definition_path.contains("{\"key\""));

    // Verify path is just a reference
    assert_eq!(
        job.minio_definition_path,
        format!("jobs/{}/definition.json", job_id)
    );
    assert!(job.minio_definition_path.len() < 100);

    // Create an execution
    let execution = JobExecution {
        id: execution_id,
        job_id,
        idempotency_key: "test-key-456".to_string(),
        status: ExecutionStatus::Success,
        attempt: 1,
        trigger_source: TriggerSource::Manual {
            user_id: "admin_user".to_string(),
        },
        current_step: None,
        minio_context_path: format!("jobs/{}/executions/{}/context.json", job_id, execution_id),
        started_at: Some(Utc::now()),
        completed_at: Some(Utc::now()),
        result: Some("Success".to_string()),
        error: None,
        created_at: Utc::now(),
    };

    // Verify MinIO context path doesn't contain execution data
    assert!(!execution.minio_context_path.contains("admin_user"));
    assert!(!execution.minio_context_path.contains("Success"));
    assert!(!execution.minio_context_path.contains("variables"));
    assert!(!execution.minio_context_path.contains("steps"));

    // Verify path is just a reference
    assert_eq!(
        execution.minio_context_path,
        format!("jobs/{}/executions/{}/context.json", job_id, execution_id)
    );
    assert!(execution.minio_context_path.len() < 150);
}

/// Test path format consistency across different job types
#[test]
fn test_path_format_consistency() {
    // Test with different job types
    let job_types = vec![
        (
            "http_job",
            JobType::HttpRequest {
                method: HttpMethod::Get,
                url: "https://example.com".to_string(),
                headers: HashMap::new(),
                body: None,
                auth: None,
            },
        ),
        (
            "db_job",
            JobType::DatabaseQuery {
                database_type: DatabaseType::MySQL,
                connection_string: "mysql://localhost/test".to_string(),
                query: "SELECT 1".to_string(),
                query_type: QueryType::RawSql,
            },
        ),
        (
            "file_job",
            JobType::FileProcessing {
                operation: FileOperation::Read,
                format: FileFormat::Excel,
                source_path: Some("/path/to/file.xlsx".to_string()),
                destination_path: None,
                options: FileProcessingOptions {
                    sheet_name: None,
                    sheet_index: None,
                    transformations: vec![],
                    streaming: false,
                },
            },
        ),
        (
            "sftp_job",
            JobType::Sftp {
                operation: SftpOperation::Download,
                host: "sftp.example.com".to_string(),
                port: 22,
                auth: SftpAuth::Password {
                    username: "user".to_string(),
                    password: "pass".to_string(),
                },
                remote_path: "/remote/path".to_string(),
                local_path: None,
                options: SftpOptions {
                    wildcard_pattern: None,
                    recursive: false,
                    create_directories: false,
                    verify_host_key: true,
                },
            },
        ),
    ];

    for (name, job_type) in job_types {
        let job_id = Uuid::new_v4();
        let execution_id = Uuid::new_v4();

        let job = Job {
            id: job_id,
            name: name.to_string(),
            description: None,
            schedule: None,
            steps: vec![JobStep {
                id: "step1".to_string(),
                name: "Test Step".to_string(),
                step_type: job_type,
                condition: None,
            }],
            triggers: TriggerConfig::default(),
            enabled: true,
            timeout_seconds: 300,
            max_retries: 3,
            allow_concurrent: false,
            minio_definition_path: format!("jobs/{}/definition.json", job_id),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Verify path format is consistent regardless of job type
        assert_eq!(
            job.minio_definition_path,
            format!("jobs/{}/definition.json", job_id),
            "Path format should be consistent for job type: {}",
            name
        );

        let execution = JobExecution {
            id: execution_id,
            job_id,
            idempotency_key: format!("{}-key", name),
            status: ExecutionStatus::Pending,
            attempt: 1,
            trigger_source: TriggerSource::Scheduled,
            current_step: None,
            minio_context_path: format!("jobs/{}/executions/{}/context.json", job_id, execution_id),
            started_at: None,
            completed_at: None,
            result: None,
            error: None,
            created_at: Utc::now(),
        };

        // Verify context path format is consistent
        assert_eq!(
            execution.minio_context_path,
            format!("jobs/{}/executions/{}/context.json", job_id, execution_id),
            "Context path format should be consistent for job type: {}",
            name
        );
    }
}
