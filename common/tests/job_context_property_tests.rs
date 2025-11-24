// Property-based tests for Job Context operations
// Feature: vietnam-enterprise-cron
// Requirements: 13.5, 13.6, 13.8, 13.9, 13.10, 14.5 - Job Context management and step output storage

use chrono::Utc;
use common::models::{FileMetadata, JobContext, StepOutput, WebhookData};
use proptest::prelude::*;
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================================
// Property Generators
// ============================================================================

/// Generate a StepOutput with random data
fn arb_step_output() -> impl Strategy<Value = StepOutput> {
    (
        "[a-z]{3,10}",                    // step_id
        prop::option::of("[a-z ]{5,20}"), // output data
        prop::bool::ANY,                  // success/failure
    )
        .prop_map(|(step_id, output_data, success)| {
            let now = Utc::now();
            StepOutput {
                step_id: step_id.clone(),
                status: if success {
                    "success".to_string()
                } else {
                    "failed".to_string()
                },
                output: json!({
                    "result": output_data.unwrap_or_else(|| "default".to_string()),
                    "timestamp": now.to_rfc3339()
                }),
                started_at: now,
                completed_at: now,
            }
        })
}

/// Generate an HTTP response as JSON
fn arb_http_response() -> impl Strategy<Value = serde_json::Value> {
    (
        200u16..599u16,                             // status code
        "[a-z]{5,20}",                              // response body
        prop::collection::vec("[a-z]{3,10}", 0..5), // headers
    )
        .prop_map(|(status, body, headers)| {
            json!({
                "status": status,
                "body": body,
                "headers": headers.into_iter().map(|h| (h, "value")).collect::<HashMap<_, _>>()
            })
        })
}

/// Generate a database result set as JSON
fn arb_database_result() -> impl Strategy<Value = serde_json::Value> {
    prop::collection::vec(
        prop::collection::hash_map("[a-z]{3,10}", any::<i64>(), 1..5),
        0..10,
    )
    .prop_map(|rows| {
        json!({
            "rows": rows,
            "row_count": rows.len()
        })
    })
}

/// Generate a JobContext with multiple steps
fn arb_job_context_with_steps() -> impl Strategy<Value = JobContext> {
    (
        any::<[u8; 16]>(),                                             // execution_id bytes
        any::<[u8; 16]>(),                                             // job_id bytes
        prop::collection::vec(arb_step_output(), 1..5),                // step outputs
        prop::collection::hash_map("[a-z]{3,10}", any::<i64>(), 0..5), // variables
    )
        .prop_map(|(exec_id_bytes, job_id_bytes, step_outputs, vars)| {
            let execution_id = Uuid::from_bytes(exec_id_bytes);
            let job_id = Uuid::from_bytes(job_id_bytes);

            let mut context = JobContext::new(execution_id, job_id);

            // Add variables
            for (key, value) in vars {
                context.set_variable(key, json!(value));
            }

            // Add step outputs
            for step_output in step_outputs {
                context.set_step_output(step_output.step_id.clone(), step_output);
            }

            context
        })
}

// ============================================================================
// Property Tests
// ============================================================================

/// **Feature: vietnam-enterprise-cron, Property 80: HTTP response storage in Job Context**
/// **Validates: Requirements 13.5**
///
/// *For any* HTTP step execution, the API response should be present in the
/// Job Context after the step completes.
#[test]
fn property_http_response_storage_in_job_context() {
    proptest!(ProptestConfig::with_cases(100), |(
        exec_id_bytes in any::<[u8; 16]>(),
        job_id_bytes in any::<[u8; 16]>(),
        step_id in "[a-z]{3,10}",
        http_response in arb_http_response()
    )| {
        let execution_id = Uuid::from_bytes(exec_id_bytes);
        let job_id = Uuid::from_bytes(job_id_bytes);

        let mut context = JobContext::new(execution_id, job_id);

        // Simulate HTTP step execution
        let now = Utc::now();
        let step_output = StepOutput {
            step_id: step_id.clone(),
            status: "success".to_string(),
            output: http_response.clone(),
            started_at: now,
            completed_at: now,
        };

        // Store step output (automatic storage as per requirement 14.5)
        context.set_step_output(step_id.clone(), step_output);

        // Verify HTTP response is present in Job Context
        prop_assert!(context.has_step_output(&step_id),
            "HTTP step output should be present in Job Context");

        let stored_output = context.get_step_output(&step_id);
        prop_assert!(stored_output.is_some(),
            "HTTP step output should be retrievable from Job Context");

        let stored_output = stored_output.unwrap();
        prop_assert_eq!(&stored_output.output, &http_response,
            "HTTP response should match stored output");

    });
}

/// **Feature: vietnam-enterprise-cron, Property 81: Database result storage in Job Context**
/// **Validates: Requirements 13.6**
///
/// *For any* database query step execution, the query result set should be
/// present in the Job Context after the step completes.
#[test]
fn property_database_result_storage_in_job_context() {
    proptest!(ProptestConfig::with_cases(100), |(
        exec_id_bytes in any::<[u8; 16]>(),
        job_id_bytes in any::<[u8; 16]>(),
        step_id in "[a-z]{3,10}",
        db_result in arb_database_result()
    )| {
        let execution_id = Uuid::from_bytes(exec_id_bytes);
        let job_id = Uuid::from_bytes(job_id_bytes);

        let mut context = JobContext::new(execution_id, job_id);

        // Simulate database step execution
        let now = Utc::now();
        let step_output = StepOutput {
            step_id: step_id.clone(),
            status: "success".to_string(),
            output: db_result.clone(),
            started_at: now,
            completed_at: now,
        };

        // Store step output (automatic storage as per requirement 14.5)
        context.set_step_output(step_id.clone(), step_output);

        // Verify database result is present in Job Context
        prop_assert!(context.has_step_output(&step_id),
            "Database step output should be present in Job Context");

        let stored_output = context.get_step_output(&step_id);
        prop_assert!(stored_output.is_some(),
            "Database step output should be retrievable from Job Context");

        let stored_output = stored_output.unwrap();
        prop_assert_eq!(&stored_output.output, &db_result,
            "Database result should match stored output");

        // Verify row count is accessible
        if let Some(row_count) = db_result.get("row_count") {
            prop_assert_eq!(stored_output.output.get("row_count"), Some(row_count),
                "Row count should be accessible in stored output");
        }

    });
}

/// **Feature: vietnam-enterprise-cron, Property 84: Job Context loading for subsequent steps**
/// **Validates: Requirements 13.8**
///
/// *For any* multi-step job, step N should have access to outputs from all
/// previous steps (1..N-1) via the Job Context.
#[test]
fn property_job_context_loading_for_subsequent_steps() {
    proptest!(ProptestConfig::with_cases(100), |(
        context in arb_job_context_with_steps()
    )| {
        // Get all executed step IDs
        let executed_steps = context.get_executed_step_ids();

        // Verify that all steps are accessible
        for step_id in &executed_steps {
            prop_assert!(context.has_step_output(step_id),
                "Step '{}' should be accessible in Job Context", step_id);

            let step_output = context.get_step_output(step_id);
            prop_assert!(step_output.is_some(),
                "Step '{}' output should be retrievable", step_id);

            let step_output = step_output.unwrap();
            prop_assert_eq!(&step_output.step_id, step_id,
                "Step ID should match");
        }

        // Verify completed steps count matches
        prop_assert_eq!(context.completed_steps_count(), executed_steps.len(),
            "Completed steps count should match number of executed steps");

        // Simulate subsequent step accessing previous step outputs
        // This verifies that step N can access outputs from steps 1..N-1
        for (i, step_id) in executed_steps.iter().enumerate() {
            // All previous steps should be accessible
            for prev_step_id in &executed_steps[0..i] {
                prop_assert!(context.has_step_output(prev_step_id),
                    "Step '{}' should have access to previous step '{}'", step_id, prev_step_id);
            }
        }

    });
}

/// **Feature: vietnam-enterprise-cron, Property 85: Job Context retention after completion**
/// **Validates: Requirements 13.9**
///
/// *For any* completed job execution, the final Job Context should remain
/// retrievable (simulated by verifying context integrity after completion).
#[test]
fn property_job_context_retention_after_completion() {
    proptest!(ProptestConfig::with_cases(100), |(
        context in arb_job_context_with_steps()
    )| {
        // Simulate job completion by marking all steps as complete
        let execution_id = context.execution_id;
        let job_id = context.job_id;
        let steps_count = context.completed_steps_count();
        let variables_count = context.variables.len();

        // Clone context to simulate persistence
        let final_context = context.clone();

        // Verify final context retains all data
        prop_assert_eq!(final_context.execution_id, execution_id,
            "Execution ID should be retained after completion");
        prop_assert_eq!(final_context.job_id, job_id,
            "Job ID should be retained after completion");
        prop_assert_eq!(final_context.completed_steps_count(), steps_count,
            "All step outputs should be retained after completion");
        prop_assert_eq!(final_context.variables.len(), variables_count,
            "All variables should be retained after completion");

        // Verify all step outputs are still accessible
        for step_id in context.get_executed_step_ids() {
            prop_assert!(final_context.has_step_output(&step_id),
                "Step '{}' output should be retained after completion", step_id);

            let original_output = context.get_step_output(&step_id).unwrap();
            let final_output = final_context.get_step_output(&step_id).unwrap();

            prop_assert_eq!(&final_output.step_id, &original_output.step_id,
                "Step ID should be preserved");
            prop_assert_eq!(&final_output.status, &original_output.status,
                "Step status should be preserved");
            prop_assert_eq!(&final_output.output, &original_output.output,
                "Step output data should be preserved");
        }

    });
}

/// **Feature: vietnam-enterprise-cron, Property 86: Job Context preservation on failure**
/// **Validates: Requirements 13.10**
///
/// *For any* failed job execution, the Job Context up to the point of failure
/// should be persisted and retrievable.
#[test]
fn property_job_context_preservation_on_failure() {
    proptest!(ProptestConfig::with_cases(100), |(
        exec_id_bytes in any::<[u8; 16]>(),
        job_id_bytes in any::<[u8; 16]>(),
        successful_steps in prop::collection::vec(arb_step_output(), 1..5),
        failure_step_id in "[a-z]{3,10}"
    )| {
        let execution_id = Uuid::from_bytes(exec_id_bytes);
        let job_id = Uuid::from_bytes(job_id_bytes);

        let mut context = JobContext::new(execution_id, job_id);

        // Add successful steps
        for step_output in &successful_steps {
            context.set_step_output(step_output.step_id.clone(), step_output.clone());
        }

        let successful_steps_count = successful_steps.len();

        // Add a failed step
        let now = Utc::now();
        let failed_step = StepOutput {
            step_id: failure_step_id.clone(),
            status: "failed".to_string(),
            output: json!({"error": "Step execution failed"}),
            started_at: now,
            completed_at: now,
        };
        context.set_step_output(failure_step_id.clone(), failed_step);

        // Simulate context preservation at point of failure
        let preserved_context = context.clone();

        // Verify all successful steps are preserved
        prop_assert_eq!(preserved_context.completed_steps_count(), successful_steps_count + 1,
            "All steps up to and including the failed step should be preserved");

        for step_output in &successful_steps {
            prop_assert!(preserved_context.has_step_output(&step_output.step_id),
                "Successful step '{}' should be preserved after failure", step_output.step_id);

            let preserved_output = preserved_context.get_step_output(&step_output.step_id).unwrap();
            prop_assert_eq!(&preserved_output.status, "success",
                "Successful step status should be preserved");
        }

        // Verify failed step is also preserved
        prop_assert!(preserved_context.has_step_output(&failure_step_id),
            "Failed step should be preserved in context");

        let failed_output = preserved_context.get_step_output(&failure_step_id).unwrap();
        prop_assert_eq!(&failed_output.status, "failed",
            "Failed step status should be preserved");
        prop_assert!(failed_output.output.get("error").is_some(),
            "Error information should be preserved in failed step output");

    });
}

/// **Feature: vietnam-enterprise-cron, Property 93: Automatic step output storage**
/// **Validates: Requirements 14.5**
///
/// *For any* step execution, the step output should be automatically stored
/// in the Job Context without explicit configuration.
#[test]
fn property_automatic_step_output_storage() {
    proptest!(ProptestConfig::with_cases(100), |(
        exec_id_bytes in any::<[u8; 16]>(),
        job_id_bytes in any::<[u8; 16]>(),
        step_outputs in prop::collection::vec(arb_step_output(), 1..10)
    )| {
        let execution_id = Uuid::from_bytes(exec_id_bytes);
        let job_id = Uuid::from_bytes(job_id_bytes);

        let mut context = JobContext::new(execution_id, job_id);

        // Store each step output (simulating automatic storage)
        for step_output in &step_outputs {
            context.set_step_output(step_output.step_id.clone(), step_output.clone());
        }

        // Verify all step outputs are automatically stored and accessible
        prop_assert_eq!(context.completed_steps_count(), step_outputs.len(),
            "All step outputs should be automatically stored");

        for step_output in &step_outputs {
            // Verify step is stored
            prop_assert!(context.has_step_output(&step_output.step_id),
                "Step '{}' should be automatically stored", step_output.step_id);

            // Verify step output is retrievable
            let stored_output = context.get_step_output(&step_output.step_id);
            prop_assert!(stored_output.is_some(),
                "Step '{}' output should be retrievable", step_output.step_id);

            let stored_output = stored_output.unwrap();

            // Verify output data is preserved
            prop_assert_eq!(&stored_output.step_id, &step_output.step_id,
                "Step ID should be preserved");
            prop_assert_eq!(&stored_output.status, &step_output.status,
                "Step status should be preserved");
            prop_assert_eq!(&stored_output.output, &step_output.output,
                "Step output data should be preserved");

            // Verify timestamps are preserved
            prop_assert_eq!(stored_output.started_at, step_output.started_at,
                "Step start time should be preserved");
            prop_assert_eq!(stored_output.completed_at, step_output.completed_at,
                "Step completion time should be preserved");
        }

        // Verify step execution order can be determined
        let executed_step_ids = context.get_executed_step_ids();
        prop_assert_eq!(executed_step_ids.len(), step_outputs.len(),
            "All executed steps should be listed");

    });
}

// ============================================================================
// Additional Edge Case Tests
// ============================================================================

/// Test that empty Job Context can store and retrieve step outputs
#[test]
fn test_empty_context_step_storage() {
    let execution_id = Uuid::new_v4();
    let job_id = Uuid::new_v4();
    let mut context = JobContext::new(execution_id, job_id);

    assert_eq!(context.completed_steps_count(), 0);
    assert!(!context.has_step_output("step1"));

    let now = Utc::now();
    let step_output = StepOutput {
        step_id: "step1".to_string(),
        status: "success".to_string(),
        output: json!({"result": "data"}),
        started_at: now,
        completed_at: now,
    };

    context.set_step_output("step1".to_string(), step_output);

    assert_eq!(context.completed_steps_count(), 1);
    assert!(context.has_step_output("step1"));
}

/// Test that Job Context can handle large step outputs
#[test]
fn test_large_step_output_storage() {
    let execution_id = Uuid::new_v4();
    let job_id = Uuid::new_v4();
    let mut context = JobContext::new(execution_id, job_id);

    // Create a large output (simulating large database result or API response)
    let large_data: Vec<HashMap<String, i64>> = (0..1000)
        .map(|i| {
            let mut row = HashMap::new();
            row.insert("id".to_string(), i);
            row.insert("value".to_string(), i * 2);
            row
        })
        .collect();

    let now = Utc::now();
    let step_output = StepOutput {
        step_id: "large_query".to_string(),
        status: "success".to_string(),
        output: json!({"rows": large_data, "row_count": 1000}),
        started_at: now,
        completed_at: now,
    };

    context.set_step_output("large_query".to_string(), step_output);

    assert!(context.has_step_output("large_query"));
    let stored = context.get_step_output("large_query").unwrap();
    assert_eq!(stored.output["row_count"], 1000);
    assert_eq!(stored.output["rows"].as_array().unwrap().len(), 1000);
}

/// Test that Job Context preserves step output order
#[test]
fn test_step_output_order_preservation() {
    let execution_id = Uuid::new_v4();
    let job_id = Uuid::new_v4();
    let mut context = JobContext::new(execution_id, job_id);

    let now = Utc::now();

    // Add steps in specific order
    for i in 1..=5 {
        let step_output = StepOutput {
            step_id: format!("step{}", i),
            status: "success".to_string(),
            output: json!({"step_number": i}),
            started_at: now,
            completed_at: now,
        };
        context.set_step_output(format!("step{}", i), step_output);
    }

    // Verify all steps are present
    assert_eq!(context.completed_steps_count(), 5);

    for i in 1..=5 {
        let step_id = format!("step{}", i);
        assert!(context.has_step_output(&step_id));

        let output = context.get_step_output(&step_id).unwrap();
        assert_eq!(output.output["step_number"], i);
    }
}

/// Test that Job Context can handle step output updates
#[test]
fn test_step_output_update() {
    let execution_id = Uuid::new_v4();
    let job_id = Uuid::new_v4();
    let mut context = JobContext::new(execution_id, job_id);

    let now = Utc::now();

    // Add initial step output
    let step_output = StepOutput {
        step_id: "step1".to_string(),
        status: "running".to_string(),
        output: json!({"status": "in_progress"}),
        started_at: now,
        completed_at: now,
    };
    context.set_step_output("step1".to_string(), step_output);

    // Update step output (simulating completion)
    let updated_output = StepOutput {
        step_id: "step1".to_string(),
        status: "success".to_string(),
        output: json!({"status": "completed", "result": "data"}),
        started_at: now,
        completed_at: now,
    };
    context.set_step_output("step1".to_string(), updated_output);

    // Verify update
    let stored = context.get_step_output("step1").unwrap();
    assert_eq!(stored.status, "success");
    assert_eq!(stored.output["status"], "completed");
    assert_eq!(stored.output["result"], "data");
}

/// Test that Job Context handles nested JSON in step outputs
#[test]
fn test_nested_json_in_step_output() {
    let execution_id = Uuid::new_v4();
    let job_id = Uuid::new_v4();
    let mut context = JobContext::new(execution_id, job_id);

    let now = Utc::now();

    // Create deeply nested JSON output
    let nested_output = json!({
        "response": {
            "data": {
                "user": {
                    "id": 123,
                    "profile": {
                        "name": "Test User",
                        "email": "test@example.com"
                    }
                }
            }
        }
    });

    let step_output = StepOutput {
        step_id: "api_call".to_string(),
        status: "success".to_string(),
        output: nested_output.clone(),
        started_at: now,
        completed_at: now,
    };

    context.set_step_output("api_call".to_string(), step_output);

    // Verify nested data is preserved
    let stored = context.get_step_output("api_call").unwrap();
    assert_eq!(stored.output["response"]["data"]["user"]["id"], 123);
    assert_eq!(
        stored.output["response"]["data"]["user"]["profile"]["name"],
        "Test User"
    );
}
