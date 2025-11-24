// Property-based tests for Reference Resolver
// Feature: vietnam-enterprise-cron
// Requirements: 14.1, 14.2, 14.3, 14.4, 14.6, 14.7 - Reference resolution for variables and step outputs

use chrono::Utc;
use common::models::{JobContext, StepOutput, WebhookData};
use common::worker::ReferenceResolver;
use proptest::prelude::*;
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================================
// Property Generators
// ============================================================================

/// Generate a valid variable name
fn arb_variable_name() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{2,15}"
}

/// Generate a valid step ID
fn arb_step_id() -> impl Strategy<Value = String> {
    "[a-z][a-z0-9_]{2,10}"
}

/// Generate a nested JSON structure
fn arb_nested_json() -> impl Strategy<Value = serde_json::Value> {
    let leaf = prop_oneof![
        any::<i64>().prop_map(|n| json!(n)),
        "[a-z]{3,20}".prop_map(|s| json!(s)),
        any::<bool>().prop_map(|b| json!(b)),
    ];

    leaf.prop_recursive(
        3,  // depth
        10, // max nodes
        5,  // items per collection
        |inner| {
            prop_oneof![
                // Object with nested values
                prop::collection::hash_map("[a-z]{3,10}", inner.clone(), 1..4)
                    .prop_map(|m| json!(m)),
                // Array with nested values
                prop::collection::vec(inner, 1..5).prop_map(|v| json!(v)),
            ]
        },
    )
}

/// Generate a JobContext with variables
fn arb_job_context_with_variables() -> impl Strategy<Value = JobContext> {
    (
        any::<[u8; 16]>(),
        any::<[u8; 16]>(),
        prop::collection::hash_map(arb_variable_name(), any::<i64>(), 1..10),
    )
        .prop_map(|(exec_id_bytes, job_id_bytes, vars)| {
            let execution_id = Uuid::from_bytes(exec_id_bytes);
            let job_id = Uuid::from_bytes(job_id_bytes);
            let mut context = JobContext::new(execution_id, job_id);

            for (key, value) in vars {
                context.set_variable(key, json!(value));
            }

            context
        })
}

/// Generate a JobContext with step outputs
fn arb_job_context_with_steps() -> impl Strategy<Value = JobContext> {
    (
        any::<[u8; 16]>(),
        any::<[u8; 16]>(),
        prop::collection::vec((arb_step_id(), arb_nested_json()), 1..5),
    )
        .prop_map(|(exec_id_bytes, job_id_bytes, steps)| {
            let execution_id = Uuid::from_bytes(exec_id_bytes);
            let job_id = Uuid::from_bytes(job_id_bytes);
            let mut context = JobContext::new(execution_id, job_id);

            let now = Utc::now();
            for (step_id, output_data) in steps {
                let step_output = StepOutput {
                    step_id: step_id.clone(),
                    status: "success".to_string(),
                    output: output_data,
                    started_at: now,
                    completed_at: now,
                };
                context.set_step_output(step_id, step_output);
            }

            context
        })
}

// ============================================================================
// Property Tests
// ============================================================================

/// **Feature: vietnam-enterprise-cron, Property 89: Step output reference resolution**
/// **Validates: Requirements 14.1**
///
/// *For any* valid step output reference, the Worker should successfully resolve
/// it from the Job Context.
#[test]
fn property_step_output_reference_resolution() {
    proptest!(ProptestConfig::with_cases(100), |(
        exec_id_bytes in any::<[u8; 16]>(),
        job_id_bytes in any::<[u8; 16]>(),
        step_id in arb_step_id(),
        value in any::<i64>()
    )| {
        let execution_id = Uuid::from_bytes(exec_id_bytes);
        let job_id = Uuid::from_bytes(job_id_bytes);
        let mut context = JobContext::new(execution_id, job_id);

        // Create a step with a known structure
        let now = Utc::now();
        let step_output = StepOutput {
            step_id: step_id.clone(),
            status: "success".to_string(),
            output: json!({"result": value}),
            started_at: now,
            completed_at: now,
        };
        context.set_step_output(step_id.clone(), step_output);

        let resolver = ReferenceResolver::new();

        // Verify we can resolve a reference to the step output
        let template = format!("{{{{steps.{}.result}}}}", step_id);
        let result = resolver.resolve(&template, &context);

        prop_assert!(result.is_ok(),
            "Should successfully resolve reference to step '{}'", step_id);

        let resolved = result.unwrap();
        prop_assert_eq!(resolved, value.to_string(),
            "Should resolve to the correct value");
    });
}

/// **Feature: vietnam-enterprise-cron, Property 90: Template reference extraction**
/// **Validates: Requirements 14.2**
///
/// *For any* valid template reference like `{{steps.step1.response.data.id}}`,
/// the Worker should extract the correct value from the Job Context.
#[test]
fn property_template_reference_extraction() {
    proptest!(ProptestConfig::with_cases(100), |(
        exec_id_bytes in any::<[u8; 16]>(),
        job_id_bytes in any::<[u8; 16]>(),
        step_id in arb_step_id(),
        user_id in 1i64..1000000i64,
        user_name in "[a-z]{5,20}"
    )| {
        let execution_id = Uuid::from_bytes(exec_id_bytes);
        let job_id = Uuid::from_bytes(job_id_bytes);
        let mut context = JobContext::new(execution_id, job_id);

        // Create a step with nested data structure
        let now = Utc::now();
        let step_output = StepOutput {
            step_id: step_id.clone(),
            status: "success".to_string(),
            output: json!({
                "response": {
                    "data": {
                        "id": user_id,
                        "name": user_name.clone()
                    }
                }
            }),
            started_at: now,
            completed_at: now,
        };
        context.set_step_output(step_id.clone(), step_output);

        let resolver = ReferenceResolver::new();

        // Test extracting nested ID
        let template = format!("{{{{steps.{}.response.data.id}}}}", step_id);
        let result = resolver.resolve(&template, &context);

        prop_assert!(result.is_ok(),
            "Should successfully extract nested ID value");
        prop_assert_eq!(result.unwrap(), user_id.to_string(),
            "Should extract correct ID value");

        // Test extracting nested name
        let template = format!("{{{{steps.{}.response.data.name}}}}", step_id);
        let result = resolver.resolve(&template, &context);

        prop_assert!(result.is_ok(),
            "Should successfully extract nested name value");
        prop_assert_eq!(result.unwrap(), user_name,
            "Should extract correct name value");
    });
}

/// **Feature: vietnam-enterprise-cron, Property 91: Invalid reference error handling**
/// **Validates: Requirements 14.3**
///
/// *For any* invalid step reference or non-existent path, the Worker should fail
/// the execution with a clear error message.
#[test]
fn property_invalid_reference_error_handling() {
    proptest!(ProptestConfig::with_cases(100), |(
        context in arb_job_context_with_steps(),
        invalid_step_id in "[a-z]{3,10}",
        invalid_path in "[a-z]{3,10}"
    )| {
        let resolver = ReferenceResolver::new();

        // Test 1: Reference to non-existent step
        let template = format!("{{{{steps.nonexistent_{}.data}}}}", invalid_step_id);
        let result = resolver.resolve(&template, &context);

        prop_assert!(result.is_err(),
            "Should fail when referencing non-existent step");

        let error = result.unwrap_err();
        prop_assert!(error.contains("not found") || error.contains("Step"),
            "Error message should indicate step not found: {}", error);

        // Test 2: Reference to non-existent path in existing step
        let executed_steps = context.get_executed_step_ids();
        if let Some(step_id) = executed_steps.first() {
            let template = format!("{{{{steps.{}.nonexistent_{}}}}}", step_id, invalid_path);
            let result = resolver.resolve(&template, &context);

            prop_assert!(result.is_err(),
                "Should fail when referencing non-existent path");

            let error = result.unwrap_err();
            prop_assert!(error.contains("not found") || error.contains("Key"),
                "Error message should indicate path not found: {}", error);
        }

        // Test 3: Invalid reference format (missing "steps." prefix)
        let template = format!("{{{{invalid.{}.data}}}}", invalid_step_id);
        let result = resolver.resolve(&template, &context);

        prop_assert!(result.is_err(),
            "Should fail with invalid reference format");
    });
}

/// **Feature: vietnam-enterprise-cron, Property 92: JSONPath nested value access**
/// **Validates: Requirements 14.4**
///
/// *For any* nested JSON structure in step output, JSONPath-style references
/// should correctly extract nested values.
#[test]
fn property_jsonpath_nested_value_access() {
    proptest!(ProptestConfig::with_cases(100), |(
        exec_id_bytes in any::<[u8; 16]>(),
        job_id_bytes in any::<[u8; 16]>(),
        step_id in arb_step_id(),
        nested_data in arb_nested_json()
    )| {
        let execution_id = Uuid::from_bytes(exec_id_bytes);
        let job_id = Uuid::from_bytes(job_id_bytes);
        let mut context = JobContext::new(execution_id, job_id);

        let now = Utc::now();
        let step_output = StepOutput {
            step_id: step_id.clone(),
            status: "success".to_string(),
            output: nested_data.clone(),
            started_at: now,
            completed_at: now,
        };
        context.set_step_output(step_id.clone(), step_output);

        let resolver = ReferenceResolver::new();

        // If the nested data is an object with keys, test accessing those keys
        if let Some(obj) = nested_data.as_object() {
            if !obj.is_empty() {
                // Get the first key and test accessing it
                let first_key = obj.keys().next().unwrap();
                let template = format!("{{{{steps.{}.{}}}}}", step_id, first_key);
                let result = resolver.resolve(&template, &context);

                prop_assert!(result.is_ok(),
                    "Should successfully access nested key '{}'", first_key);
            }
        }

        // If the nested data is an array, test accessing array indices
        if let Some(arr) = nested_data.as_array() {
            if !arr.is_empty() {
                let template = format!("{{{{steps.{}.0}}}}", step_id);
                let result = resolver.resolve(&template, &context);

                prop_assert!(result.is_ok(),
                    "Should successfully access array index 0");
            }
        }

        // If the nested data is a simple value, test accessing it directly
        if nested_data.is_string() || nested_data.is_number() || nested_data.is_boolean() {
            // For simple values, we can't navigate further, but we verified the structure exists
            prop_assert!(context.has_step_output(&step_id),
                "Step output should exist in context");
        }
    });
}

/// **Feature: vietnam-enterprise-cron, Property 94: Conditional logic evaluation**
/// **Validates: Requirements 14.6**
///
/// *For any* conditional expression in a job, it should be evaluated using data
/// from the Job Context.
///
/// Note: This property tests that references can be resolved for use in conditional
/// logic. The actual conditional evaluation logic would be implemented separately.
#[test]
fn property_conditional_logic_evaluation() {
    proptest!(ProptestConfig::with_cases(100), |(
        exec_id_bytes in any::<[u8; 16]>(),
        job_id_bytes in any::<[u8; 16]>(),
        step_id in arb_step_id(),
        status_code in 200u16..599u16,
        success_flag in any::<bool>()
    )| {
        let execution_id = Uuid::from_bytes(exec_id_bytes);
        let job_id = Uuid::from_bytes(job_id_bytes);
        let mut context = JobContext::new(execution_id, job_id);

        // Create a step with data that could be used in conditionals
        let now = Utc::now();
        let step_output = StepOutput {
            step_id: step_id.clone(),
            status: "success".to_string(),
            output: json!({
                "status_code": status_code,
                "success": success_flag,
                "count": 42
            }),
            started_at: now,
            completed_at: now,
        };
        context.set_step_output(step_id.clone(), step_output);

        let resolver = ReferenceResolver::new();

        // Test resolving values that would be used in conditional expressions
        // Example: if {{steps.step1.status_code}} == 200 then ...
        let template = format!("{{{{steps.{}.status_code}}}}", step_id);
        let result = resolver.resolve(&template, &context);

        prop_assert!(result.is_ok(),
            "Should successfully resolve status_code for conditional");
        prop_assert_eq!(result.unwrap(), status_code.to_string(),
            "Should resolve correct status_code value");

        // Test resolving boolean values for conditionals
        let template = format!("{{{{steps.{}.success}}}}", step_id);
        let result = resolver.resolve(&template, &context);

        prop_assert!(result.is_ok(),
            "Should successfully resolve boolean for conditional");
        prop_assert_eq!(result.unwrap(), success_flag.to_string(),
            "Should resolve correct boolean value");

        // Test resolving numeric values for conditionals
        let template = format!("{{{{steps.{}.count}}}}", step_id);
        let result = resolver.resolve(&template, &context);

        prop_assert!(result.is_ok(),
            "Should successfully resolve numeric value for conditional");
        prop_assert_eq!(result.unwrap(), "42",
            "Should resolve correct numeric value");
    });
}

/// **Feature: vietnam-enterprise-cron, Property 95: Missing data reference error**
/// **Validates: Requirements 14.7**
///
/// *For any* step reference to data not populated by a previous step, the Worker
/// should fail with a clear error indicating the missing data path.
#[test]
fn property_missing_data_reference_error() {
    proptest!(ProptestConfig::with_cases(100), |(
        exec_id_bytes in any::<[u8; 16]>(),
        job_id_bytes in any::<[u8; 16]>(),
        step_id in arb_step_id(),
        missing_field in "[a-z]{5,15}"
    )| {
        let execution_id = Uuid::from_bytes(exec_id_bytes);
        let job_id = Uuid::from_bytes(job_id_bytes);
        let mut context = JobContext::new(execution_id, job_id);

        // Create a step with limited data
        let now = Utc::now();
        let step_output = StepOutput {
            step_id: step_id.clone(),
            status: "success".to_string(),
            output: json!({
                "existing_field": "value"
            }),
            started_at: now,
            completed_at: now,
        };
        context.set_step_output(step_id.clone(), step_output);

        let resolver = ReferenceResolver::new();

        // Test 1: Reference to missing field in existing step
        let template = format!("{{{{steps.{}.{}}}}}", step_id, missing_field);
        let result = resolver.resolve(&template, &context);

        prop_assert!(result.is_err(),
            "Should fail when referencing missing field");

        let error = result.unwrap_err();
        prop_assert!(
            error.contains(&missing_field) || error.contains("not found") || error.contains("Key"),
            "Error message should indicate missing field '{}': {}", missing_field, error
        );

        // Test 2: Reference to nested missing path
        let template = format!("{{{{steps.{}.existing_field.nested.deep}}}}", step_id);
        let result = resolver.resolve(&template, &context);

        prop_assert!(result.is_err(),
            "Should fail when referencing missing nested path");

        let error = result.unwrap_err();
        prop_assert!(error.contains("not found") || error.contains("Key"),
            "Error message should indicate missing nested path: {}", error);

        // Test 3: Reference to array index that doesn't exist
        let template = format!("{{{{steps.{}.existing_field.0}}}}", step_id);
        let result = resolver.resolve(&template, &context);

        prop_assert!(result.is_err(),
            "Should fail when referencing array index on non-array");
    });
}

// ============================================================================
// Additional Edge Case Tests
// ============================================================================

/// Test variable reference resolution
#[test]
fn test_variable_reference_resolution() {
    let resolver = ReferenceResolver::new();
    let mut context = JobContext::new(Uuid::new_v4(), Uuid::new_v4());

    context.set_variable("api_key".to_string(), json!("secret123"));
    context.set_variable("base_url".to_string(), json!("https://api.example.com"));

    let template = "{{base_url}}/users?key={{api_key}}";
    let result = resolver.resolve(template, &context).unwrap();

    assert_eq!(result, "https://api.example.com/users?key=secret123");
}

/// Test webhook data reference resolution
#[test]
fn test_webhook_data_reference_resolution() {
    let resolver = ReferenceResolver::new();
    let mut context = JobContext::new(Uuid::new_v4(), Uuid::new_v4());

    let mut query_params = HashMap::new();
    query_params.insert("user_id".to_string(), "123".to_string());

    let mut headers = HashMap::new();
    headers.insert("X-Request-ID".to_string(), "req-456".to_string());

    context.webhook = Some(WebhookData {
        payload: json!({"action": "create", "resource": "user"}),
        query_params,
        headers,
    });

    // Test payload reference
    let template = "Action: {{webhook.payload.action}}";
    let result = resolver.resolve(template, &context).unwrap();
    assert_eq!(result, "Action: create");

    // Test query params reference
    let template = "User ID: {{webhook.query_params.user_id}}";
    let result = resolver.resolve(template, &context).unwrap();
    assert_eq!(result, "User ID: 123");

    // Test headers reference
    let template = "Request ID: {{webhook.headers.X-Request-ID}}";
    let result = resolver.resolve(template, &context).unwrap();
    assert_eq!(result, "Request ID: req-456");
}

/// Test multiple references in single template
#[test]
fn test_multiple_references_in_template() {
    let resolver = ReferenceResolver::new();
    let mut context = JobContext::new(Uuid::new_v4(), Uuid::new_v4());

    context.set_variable("host".to_string(), json!("api.example.com"));
    context.set_variable("port".to_string(), json!("443"));

    let now = Utc::now();
    let step_output = StepOutput {
        step_id: "step1".to_string(),
        status: "success".to_string(),
        output: json!({"user_id": 789}),
        started_at: now,
        completed_at: now,
    };
    context.set_step_output("step1".to_string(), step_output);

    let template = "https://{{host}}:{{port}}/users/{{steps.step1.user_id}}";
    let result = resolver.resolve(template, &context).unwrap();

    assert_eq!(result, "https://api.example.com:443/users/789");
}

/// Test array index access in step output
#[test]
fn test_array_index_access() {
    let resolver = ReferenceResolver::new();
    let mut context = JobContext::new(Uuid::new_v4(), Uuid::new_v4());

    let now = Utc::now();
    let step_output = StepOutput {
        step_id: "step1".to_string(),
        status: "success".to_string(),
        output: json!({
            "users": [
                {"id": 1, "name": "Alice"},
                {"id": 2, "name": "Bob"},
                {"id": 3, "name": "Charlie"}
            ]
        }),
        started_at: now,
        completed_at: now,
    };
    context.set_step_output("step1".to_string(), step_output);

    // Access first user's name
    let template = "{{steps.step1.users.0.name}}";
    let result = resolver.resolve(template, &context).unwrap();
    assert_eq!(result, "Alice");

    // Access second user's ID
    let template = "{{steps.step1.users.1.id}}";
    let result = resolver.resolve(template, &context).unwrap();
    assert_eq!(result, "2");
}

/// Test deeply nested path access
#[test]
fn test_deeply_nested_path_access() {
    let resolver = ReferenceResolver::new();
    let mut context = JobContext::new(Uuid::new_v4(), Uuid::new_v4());

    let now = Utc::now();
    let step_output = StepOutput {
        step_id: "step1".to_string(),
        status: "success".to_string(),
        output: json!({
            "response": {
                "data": {
                    "user": {
                        "profile": {
                            "contact": {
                                "email": "test@example.com"
                            }
                        }
                    }
                }
            }
        }),
        started_at: now,
        completed_at: now,
    };
    context.set_step_output("step1".to_string(), step_output);

    let template = "{{steps.step1.response.data.user.profile.contact.email}}";
    let result = resolver.resolve(template, &context).unwrap();
    assert_eq!(result, "test@example.com");
}

/// Test error message clarity for missing step
#[test]
fn test_missing_step_error_message() {
    let resolver = ReferenceResolver::new();
    let context = JobContext::new(Uuid::new_v4(), Uuid::new_v4());

    let template = "{{steps.nonexistent.data}}";
    let result = resolver.resolve(template, &context);

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.contains("nonexistent"));
    assert!(error.contains("not found") || error.contains("Step"));
}

/// Test error message clarity for missing variable
#[test]
fn test_missing_variable_error_message() {
    let resolver = ReferenceResolver::new();
    let context = JobContext::new(Uuid::new_v4(), Uuid::new_v4());

    let template = "{{missing_variable}}";
    let result = resolver.resolve(template, &context);

    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.contains("missing_variable"));
    assert!(error.contains("not found") || error.contains("Variable"));
}

/// Test null value handling
#[test]
fn test_null_value_handling() {
    let resolver = ReferenceResolver::new();
    let mut context = JobContext::new(Uuid::new_v4(), Uuid::new_v4());

    let now = Utc::now();
    let step_output = StepOutput {
        step_id: "step1".to_string(),
        status: "success".to_string(),
        output: json!({"nullable_field": null}),
        started_at: now,
        completed_at: now,
    };
    context.set_step_output("step1".to_string(), step_output);

    let template = "Value: {{steps.step1.nullable_field}}";
    let result = resolver.resolve(template, &context).unwrap();
    assert_eq!(result, "Value: null");
}

/// Test boolean value handling
#[test]
fn test_boolean_value_handling() {
    let resolver = ReferenceResolver::new();
    let mut context = JobContext::new(Uuid::new_v4(), Uuid::new_v4());

    let now = Utc::now();
    let step_output = StepOutput {
        step_id: "step1".to_string(),
        status: "success".to_string(),
        output: json!({"is_active": true, "is_deleted": false}),
        started_at: now,
        completed_at: now,
    };
    context.set_step_output("step1".to_string(), step_output);

    let template = "Active: {{steps.step1.is_active}}, Deleted: {{steps.step1.is_deleted}}";
    let result = resolver.resolve(template, &context).unwrap();
    assert_eq!(result, "Active: true, Deleted: false");
}

/// Test numeric value handling
#[test]
fn test_numeric_value_handling() {
    let resolver = ReferenceResolver::new();
    let mut context = JobContext::new(Uuid::new_v4(), Uuid::new_v4());

    let now = Utc::now();
    let step_output = StepOutput {
        step_id: "step1".to_string(),
        status: "success".to_string(),
        output: json!({"count": 42, "price": 19.99, "negative": -5}),
        started_at: now,
        completed_at: now,
    };
    context.set_step_output("step1".to_string(), step_output);

    let template = "Count: {{steps.step1.count}}, Price: {{steps.step1.price}}, Negative: {{steps.step1.negative}}";
    let result = resolver.resolve(template, &context).unwrap();
    assert_eq!(result, "Count: 42, Price: 19.99, Negative: -5");
}
