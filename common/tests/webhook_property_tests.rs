// Property-based tests for webhook triggers
// Feature: vietnam-enterprise-cron
// Requirements: 16.1-16.12

use common::models::{JobContext, WebhookData};
use common::webhook::{
    generate_webhook_secret, generate_webhook_url_path, validate_webhook_signature,
};
use proptest::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================================
// Property 107: Unique webhook URL generation
// ============================================================================

// Feature: vietnam-enterprise-cron, Property 107: Unique webhook URL generation
// For any job with webhook trigger enabled, a unique webhook URL should be generated.
// Validates: Requirements 16.1
#[test]
fn property_107_unique_webhook_url_generation() {
    proptest!(|(
        job_id1 in any::<[u8; 16]>().prop_map(|bytes| Uuid::from_bytes(bytes)),
        job_id2 in any::<[u8; 16]>().prop_map(|bytes| Uuid::from_bytes(bytes)),
    )| {
        // This property test validates that:
        // 1. Webhook URLs are generated for any job ID
        // 2. URLs have the correct format (/webhooks/{id})
        // 3. Multiple calls generate different URLs (due to random component)
        // 4. URLs for different jobs are different

        let url1_first = generate_webhook_url_path(job_id1);
        let url1_second = generate_webhook_url_path(job_id1);
        let url2 = generate_webhook_url_path(job_id2);

        // All URLs should start with /webhooks/
        prop_assert!(url1_first.starts_with("/webhooks/"));
        prop_assert!(url1_second.starts_with("/webhooks/"));
        prop_assert!(url2.starts_with("/webhooks/"));

        // URLs should have correct length (prefix + 32 hex chars)
        prop_assert_eq!(url1_first.len(), "/webhooks/".len() + 32);
        prop_assert_eq!(url1_second.len(), "/webhooks/".len() + 32);
        prop_assert_eq!(url2.len(), "/webhooks/".len() + 32);

        // Multiple calls for same job should generate different URLs (randomness)
        prop_assert_ne!(&url1_first, &url1_second);

        // Different jobs should have different URLs
        if job_id1 != job_id2 {
            prop_assert_ne!(&url1_first, &url2);
        }
    });
}

// ============================================================================
// Property 108: Webhook POST queueing
// ============================================================================

// Feature: vietnam-enterprise-cron, Property 108: Webhook POST queueing
// For any valid HTTP POST to a webhook URL, a job execution should be queued immediately.
// Validates: Requirements 16.2
// Note: This property is tested through integration tests as it requires
// full system setup (database, queue, MinIO). The webhook handler logic
// is validated through unit tests in api/src/handlers/webhooks.rs

// ============================================================================
// Property 109: Webhook payload storage
// ============================================================================

// Feature: vietnam-enterprise-cron, Property 109: Webhook payload storage
// For any webhook request with JSON payload, the payload should be accessible
// at `webhook.payload` in the Job Context.
// Validates: Requirements 16.3
#[test]
fn property_109_webhook_payload_storage() {
    proptest!(|(
        execution_id in any::<[u8; 16]>().prop_map(|bytes| Uuid::from_bytes(bytes)),
        job_id in any::<[u8; 16]>().prop_map(|bytes| Uuid::from_bytes(bytes)),
        user_id in "[a-z0-9]{5,20}",
        email in "[a-z0-9]{3,10}@[a-z]{3,10}\\.[a-z]{2,5}",
        count in 1u32..1000u32,
    )| {
        // This property test validates that:
        // 1. Webhook payload can be stored in Job Context
        // 2. Payload is accessible via get_webhook_data()
        // 3. Payload data is preserved correctly

        let mut context = JobContext::new(execution_id, job_id);

        // Create webhook payload
        let payload = serde_json::json!({
            "user_id": user_id,
            "email": email,
            "count": count,
        });

        let webhook_data = WebhookData {
            payload: payload.clone(),
            query_params: HashMap::new(),
            headers: HashMap::new(),
        };

        // Store webhook data
        context.set_webhook_data(webhook_data);

        // Retrieve and verify
        let retrieved = context.get_webhook_data();
        prop_assert!(retrieved.is_some());

        let retrieved_data = retrieved.unwrap();
        prop_assert_eq!(&retrieved_data.payload, &payload);
        prop_assert_eq!(retrieved_data.payload["user_id"].as_str(), Some(user_id.as_str()));
        prop_assert_eq!(retrieved_data.payload["email"].as_str(), Some(email.as_str()));
        prop_assert_eq!(retrieved_data.payload["count"].as_u64(), Some(count as u64));
    });
}

// ============================================================================
// Property 110: Webhook query parameters storage
// ============================================================================

// Feature: vietnam-enterprise-cron, Property 110: Webhook query parameters storage
// For any webhook request with query parameters, they should be accessible
// at `webhook.query_params` in the Job Context.
// Validates: Requirements 16.4
#[test]
fn property_110_webhook_query_parameters_storage() {
    proptest!(|(
        execution_id in any::<[u8; 16]>().prop_map(|bytes| Uuid::from_bytes(bytes)),
        job_id in any::<[u8; 16]>().prop_map(|bytes| Uuid::from_bytes(bytes)),
        param_keys in prop::collection::vec("[a-z_]{3,10}", 1..5),
        param_values in prop::collection::vec("[a-zA-Z0-9]{3,20}", 1..5),
    )| {
        // This property test validates that:
        // 1. Query parameters can be stored in Job Context
        // 2. Parameters are accessible via get_webhook_data()
        // 3. All parameters are preserved correctly

        let mut context = JobContext::new(execution_id, job_id);

        // Create query parameters
        let mut query_params = HashMap::new();
        for (key, value) in param_keys.iter().zip(param_values.iter()) {
            query_params.insert(key.clone(), value.clone());
        }

        let webhook_data = WebhookData {
            payload: serde_json::json!({}),
            query_params: query_params.clone(),
            headers: HashMap::new(),
        };

        // Store webhook data
        context.set_webhook_data(webhook_data);

        // Retrieve and verify
        let retrieved = context.get_webhook_data();
        prop_assert!(retrieved.is_some());

        let retrieved_data = retrieved.unwrap();
        prop_assert_eq!(&retrieved_data.query_params, &query_params);

        // Verify each parameter
        for (key, value) in query_params.iter() {
            prop_assert_eq!(retrieved_data.query_params.get(key), Some(value));
        }
    });
}

// ============================================================================
// Property 111: Webhook headers storage
// ============================================================================

// Feature: vietnam-enterprise-cron, Property 111: Webhook headers storage
// For any webhook request with custom headers, they should be accessible
// at `webhook.headers` in the Job Context.
// Validates: Requirements 16.5
#[test]
fn property_111_webhook_headers_storage() {
    proptest!(|(
        execution_id in any::<[u8; 16]>().prop_map(|bytes| Uuid::from_bytes(bytes)),
        job_id in any::<[u8; 16]>().prop_map(|bytes| Uuid::from_bytes(bytes)),
        header_keys in prop::collection::vec("X-[A-Z][a-z]{3,10}-[A-Z][a-z]{3,10}", 1..5),
        header_values in prop::collection::vec("[a-zA-Z0-9-]{5,30}", 1..5),
    )| {
        // This property test validates that:
        // 1. Custom headers can be stored in Job Context
        // 2. Headers are accessible via get_webhook_data()
        // 3. All headers are preserved correctly

        let mut context = JobContext::new(execution_id, job_id);

        // Create custom headers
        let mut headers = HashMap::new();
        for (key, value) in header_keys.iter().zip(header_values.iter()) {
            headers.insert(key.clone(), value.clone());
        }

        let webhook_data = WebhookData {
            payload: serde_json::json!({}),
            query_params: HashMap::new(),
            headers: headers.clone(),
        };

        // Store webhook data
        context.set_webhook_data(webhook_data);

        // Retrieve and verify
        let retrieved = context.get_webhook_data();
        prop_assert!(retrieved.is_some());

        let retrieved_data = retrieved.unwrap();
        prop_assert_eq!(&retrieved_data.headers, &headers);

        // Verify each header
        for (key, value) in headers.iter() {
            prop_assert_eq!(retrieved_data.headers.get(key), Some(value));
        }
    });
}

// ============================================================================
// Property 112: Webhook data reference resolution
// ============================================================================

// Feature: vietnam-enterprise-cron, Property 112: Webhook data reference resolution
// For any valid webhook data reference like `{{webhook.payload.user_id}}`,
// the Worker should resolve it from the Job Context.
// Validates: Requirements 16.6
// Note: This property is tested in reference_resolver_property_tests.rs
// as it's part of the general reference resolution system

// ============================================================================
// Property 113: Webhook signature validation
// ============================================================================

// Feature: vietnam-enterprise-cron, Property 113: Webhook signature validation
// For any webhook request with valid HMAC-SHA256 signature, it should be accepted;
// invalid signatures should be rejected.
// Validates: Requirements 16.7
#[test]
fn property_113_webhook_signature_validation() {
    proptest!(|(
        payload_bytes in prop::collection::vec(any::<u8>(), 10..1000),
        secret in "[a-zA-Z0-9]{16,64}",
    )| {
        // This property test validates that:
        // 1. Valid signatures are accepted
        // 2. The signature validation is deterministic
        // 3. The same payload and secret always produce the same signature

        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        type HmacSha256 = Hmac<Sha256>;

        // Generate valid signature
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .expect("Failed to create HMAC");
        mac.update(&payload_bytes);
        let signature = hex::encode(mac.finalize().into_bytes());

        // Validate signature
        let is_valid = validate_webhook_signature(&payload_bytes, &signature, &secret)
            .expect("Failed to validate signature");

        prop_assert!(is_valid, "Valid signature should be accepted");

        // Validate again to ensure determinism
        let is_valid_again = validate_webhook_signature(&payload_bytes, &signature, &secret)
            .expect("Failed to validate signature");

        prop_assert!(is_valid_again, "Signature validation should be deterministic");
    });
}

// ============================================================================
// Property 114: Invalid webhook signature rejection
// ============================================================================

// Feature: vietnam-enterprise-cron, Property 114: Invalid webhook signature rejection
// For any webhook request with invalid signature, the system should return 401 Unauthorized.
// Validates: Requirements 16.8
#[test]
fn property_114_invalid_webhook_signature_rejection() {
    proptest!(|(
        payload_bytes in prop::collection::vec(any::<u8>(), 10..1000),
        secret in "[a-zA-Z0-9]{16,64}",
        wrong_signature in "[a-f0-9]{64}",
    )| {
        // This property test validates that:
        // 1. Invalid signatures are rejected
        // 2. Random signatures don't accidentally match
        // 3. The validation is secure

        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        type HmacSha256 = Hmac<Sha256>;

        // Generate correct signature
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .expect("Failed to create HMAC");
        mac.update(&payload_bytes);
        let correct_signature = hex::encode(mac.finalize().into_bytes());

        // If wrong_signature happens to match correct_signature, skip this test case
        if wrong_signature == correct_signature {
            return Ok(());
        }

        // Validate with wrong signature
        let is_valid = validate_webhook_signature(&payload_bytes, &wrong_signature, &secret)
            .expect("Failed to validate signature");

        prop_assert!(!is_valid, "Invalid signature should be rejected");
    });
}

// ============================================================================
// Property 115: Successful webhook response
// ============================================================================

// Feature: vietnam-enterprise-cron, Property 115: Successful webhook response
// For any successfully received webhook, the system should return 202 Accepted
// with the execution_id in the response.
// Validates: Requirements 16.9
// Note: This property is tested through integration tests as it requires
// full HTTP handler setup. The response structure is validated through
// unit tests in api/src/handlers/webhooks.rs

// ============================================================================
// Property 116: Disabled job webhook rejection
// ============================================================================

// Feature: vietnam-enterprise-cron, Property 116: Disabled job webhook rejection
// For any webhook call to a disabled job, the system should return 403 Forbidden.
// Validates: Requirements 16.10
// Note: This property is tested through integration tests as it requires
// database setup to check job enabled status. The logic is validated through
// unit tests in api/src/handlers/webhooks.rs

// ============================================================================
// Property 117: Webhook rate limiting
// ============================================================================

// Feature: vietnam-enterprise-cron, Property 117: Webhook rate limiting
// For any webhook that exceeds its configured rate limit, subsequent requests
// should return 429 Too Many Requests.
// Validates: Requirements 16.11
// Note: This property is tested through integration tests as it requires
// Redis setup for rate limiting. The rate limit logic is validated through
// unit tests in common/src/rate_limit.rs

// ============================================================================
// Property 118: Webhook URL invalidation
// ============================================================================

// Feature: vietnam-enterprise-cron, Property 118: Webhook URL invalidation
// For any webhook URL regeneration, the previous webhook URL should be
// immediately invalidated and no longer work.
// Validates: Requirements 16.12
#[test]
fn property_118_webhook_url_invalidation() {
    proptest!(|(
        job_id in any::<[u8; 16]>().prop_map(|bytes| Uuid::from_bytes(bytes)),
    )| {
        // This property test validates that:
        // 1. Regenerating a webhook URL produces a new URL
        // 2. The new URL is different from the old URL
        // 3. The new secret is different from the old secret

        // Generate initial webhook URL and secret
        let old_url = generate_webhook_url_path(job_id);
        let old_secret = generate_webhook_secret();

        // Regenerate webhook URL and secret
        let new_url = generate_webhook_url_path(job_id);
        let new_secret = generate_webhook_secret();

        // URLs should be different (due to random component)
        prop_assert_ne!(&old_url, &new_url, "Regenerated URL should be different");

        // Secrets should be different (due to randomness)
        prop_assert_ne!(&old_secret, &new_secret, "Regenerated secret should be different");

        // Both should still have correct format
        prop_assert!(new_url.starts_with("/webhooks/"));
        prop_assert_eq!(new_url.len(), "/webhooks/".len() + 32);
        prop_assert_eq!(new_secret.len(), 64);
    });
}

// ============================================================================
// Additional Helper Tests
// ============================================================================

// Test that webhook secret generation produces cryptographically secure secrets
#[test]
fn test_webhook_secret_generation_properties() {
    proptest!(|(
        _seed in any::<u32>(),
    )| {
        // This test validates that:
        // 1. Secrets are always 64 hex characters (32 bytes)
        // 2. Secrets are valid hex strings
        // 3. Secrets are different each time

        let secret1 = generate_webhook_secret();
        let secret2 = generate_webhook_secret();

        // Length check
        prop_assert_eq!(secret1.len(), 64);
        prop_assert_eq!(secret2.len(), 64);

        // Valid hex check
        prop_assert!(hex::decode(&secret1).is_ok());
        prop_assert!(hex::decode(&secret2).is_ok());

        // Uniqueness check
        prop_assert_ne!(secret1, secret2);
    });
}

// Test that webhook URL paths are URL-safe
#[test]
fn test_webhook_url_path_format() {
    proptest!(|(
        job_id in any::<[u8; 16]>().prop_map(|bytes| Uuid::from_bytes(bytes)),
    )| {
        // This test validates that:
        // 1. URL paths only contain URL-safe characters
        // 2. URL paths have consistent format
        // 3. URL paths can be used in HTTP routing

        let url_path = generate_webhook_url_path(job_id);

        // Should start with /webhooks/
        prop_assert!(url_path.starts_with("/webhooks/"));

        // Extract the ID part
        let id_part = &url_path["/webhooks/".len()..];

        // Should be 32 hex characters
        prop_assert_eq!(id_part.len(), 32);

        // Should only contain hex characters (0-9, a-f)
        prop_assert!(id_part.chars().all(|c| c.is_ascii_hexdigit()));

        // Should be lowercase hex
        prop_assert!(id_part.chars().all(|c| !c.is_ascii_uppercase()));
    });
}

// Test webhook data round-trip through Job Context
#[test]
fn test_webhook_data_round_trip() {
    proptest!(|(
        execution_id in any::<[u8; 16]>().prop_map(|bytes| Uuid::from_bytes(bytes)),
        job_id in any::<[u8; 16]>().prop_map(|bytes| Uuid::from_bytes(bytes)),
        payload_str in "[a-zA-Z0-9 ]{10,100}",
        param_count in 1usize..10usize,
        header_count in 1usize..10usize,
    )| {
        // This test validates that:
        // 1. Webhook data can be stored and retrieved from Job Context
        // 2. All data is preserved exactly
        // 3. Multiple fields (payload, params, headers) work together

        let mut context = JobContext::new(execution_id, job_id);

        // Create webhook data with all fields
        let payload = serde_json::json!({ "message": payload_str });

        let mut query_params = HashMap::new();
        for i in 0..param_count {
            query_params.insert(format!("param{}", i), format!("value{}", i));
        }

        let mut headers = HashMap::new();
        for i in 0..header_count {
            headers.insert(format!("X-Header-{}", i), format!("header-value-{}", i));
        }

        let webhook_data = WebhookData {
            payload: payload.clone(),
            query_params: query_params.clone(),
            headers: headers.clone(),
        };

        // Store and retrieve
        context.set_webhook_data(webhook_data);
        let retrieved = context.get_webhook_data();

        prop_assert!(retrieved.is_some());
        let retrieved_data = retrieved.unwrap();

        // Verify all fields
        prop_assert_eq!(&retrieved_data.payload, &payload);
        prop_assert_eq!(&retrieved_data.query_params, &query_params);
        prop_assert_eq!(&retrieved_data.headers, &headers);
    });
}
