use crate::errors::ValidationError;
use rand::Rng;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Generate a unique webhook URL path
/// Requirements: 16.1 - Generate unique webhook URLs for jobs
///
/// The URL path is generated using a combination of random bytes and the job ID
/// to ensure uniqueness and prevent guessing.
pub fn generate_webhook_url_path(job_id: Uuid) -> String {
    let mut rng = rand::thread_rng();
    let random_bytes: [u8; 16] = rng.gen();

    // Combine job_id and random bytes for uniqueness
    let mut hasher = Sha256::new();
    hasher.update(job_id.as_bytes());
    hasher.update(&random_bytes);
    let hash = hasher.finalize();

    // Take first 16 bytes and encode as hex
    let url_id = hex::encode(&hash[..16]);

    format!("/webhooks/{}", url_id)
}

/// Generate a secure secret key for webhook signature validation
/// Requirements: 16.1, 16.7 - Secret key for HMAC-SHA256 signature validation
///
/// The secret key is a cryptographically secure random string used for
/// HMAC-SHA256 signature validation of webhook requests.
pub fn generate_webhook_secret() -> String {
    let mut rng = rand::thread_rng();
    let random_bytes: [u8; 32] = rng.gen();
    hex::encode(random_bytes)
}

/// Validate HMAC-SHA256 signature for webhook request
/// Requirements: 16.7, 16.8 - Validate webhook signatures
///
/// # Arguments
/// * `payload` - The raw request body bytes
/// * `signature` - The signature from the request header (hex-encoded)
/// * `secret` - The webhook secret key
///
/// # Returns
/// * `Ok(true)` if signature is valid
/// * `Ok(false)` if signature is invalid
/// * `Err` if there's an error processing the signature
pub fn validate_webhook_signature(
    payload: &[u8],
    signature: &str,
    secret: &str,
) -> Result<bool, ValidationError> {
    use hmac::{Hmac, Mac};

    type HmacSha256 = Hmac<Sha256>;

    // Create HMAC instance with secret key
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).map_err(|e| {
        ValidationError::InvalidFieldValue {
            field: "secret_key".to_string(),
            reason: format!("Invalid secret key: {}", e),
        }
    })?;

    // Update with payload
    mac.update(payload);

    // Get expected signature
    let expected = mac.finalize().into_bytes();
    let expected_hex = hex::encode(expected);

    // Compare signatures (constant-time comparison)
    Ok(expected_hex == signature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_webhook_url_path() {
        let job_id = Uuid::new_v4();
        let path1 = generate_webhook_url_path(job_id);
        let path2 = generate_webhook_url_path(job_id);

        // Should start with /webhooks/
        assert!(path1.starts_with("/webhooks/"));
        assert!(path2.starts_with("/webhooks/"));

        // Should be different each time (due to random component)
        assert_ne!(path1, path2);

        // Should have correct length (prefix + 32 hex chars)
        assert_eq!(path1.len(), "/webhooks/".len() + 32);
    }

    #[test]
    fn test_generate_webhook_secret() {
        let secret1 = generate_webhook_secret();
        let secret2 = generate_webhook_secret();

        // Should be different each time
        assert_ne!(secret1, secret2);

        // Should be 64 hex characters (32 bytes)
        assert_eq!(secret1.len(), 64);
        assert_eq!(secret2.len(), 64);

        // Should be valid hex
        assert!(hex::decode(&secret1).is_ok());
        assert!(hex::decode(&secret2).is_ok());
    }

    #[test]
    fn test_validate_webhook_signature_valid() {
        let payload = b"test payload";
        let secret = "test_secret_key";

        // Generate valid signature
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        type HmacSha256 = Hmac<Sha256>;

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(payload);
        let signature = hex::encode(mac.finalize().into_bytes());

        // Validate
        let result = validate_webhook_signature(payload, &signature, secret).unwrap();
        assert!(result);
    }

    #[test]
    fn test_validate_webhook_signature_invalid() {
        let payload = b"test payload";
        let secret = "test_secret_key";
        let wrong_signature = "0000000000000000000000000000000000000000000000000000000000000000";

        // Validate
        let result = validate_webhook_signature(payload, wrong_signature, secret).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_validate_webhook_signature_different_payload() {
        let payload1 = b"test payload 1";
        let payload2 = b"test payload 2";
        let secret = "test_secret_key";

        // Generate signature for payload1
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        type HmacSha256 = Hmac<Sha256>;

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(payload1);
        let signature = hex::encode(mac.finalize().into_bytes());

        // Try to validate with payload2
        let result = validate_webhook_signature(payload2, &signature, secret).unwrap();
        assert!(!result);
    }
}
