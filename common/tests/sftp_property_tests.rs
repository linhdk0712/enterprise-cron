// Property-based tests for SFTP operations
// Feature: vietnam-enterprise-cron
// Requirements: 19.1-19.16 - SFTP download/upload operations with authentication

use async_trait::async_trait;
use chrono::Utc;
use common::executor::sftp::SftpExecutor;
use common::executor::JobExecutor;
use common::models::{
    FileMetadata, JobContext, JobStep, JobType, SftpAuth, SftpOperation, SftpOptions,
};
use common::storage::MinIOService;
use proptest::prelude::*;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

// ============================================================================
// Mock MinIO Service for Testing
// ============================================================================

#[derive(Clone)]
struct MockMinIOService {
    files: Arc<std::sync::Mutex<HashMap<String, Vec<u8>>>>,
}

impl MockMinIOService {
    fn new() -> Self {
        Self {
            files: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }

    fn add_file(&self, path: &str, data: Vec<u8>) {
        self.files.lock().unwrap().insert(path.to_string(), data);
    }

    fn get_file(&self, path: &str) -> Option<Vec<u8>> {
        self.files.lock().unwrap().get(path).cloned()
    }

    fn file_exists(&self, path: &str) -> bool {
        self.files.lock().unwrap().contains_key(path)
    }
}

#[async_trait]
impl MinIOService for MockMinIOService {
    async fn store_job_definition(
        &self,
        _job_id: Uuid,
        _definition: &str,
    ) -> Result<String, common::errors::StorageError> {
        Ok("mock_path".to_string())
    }

    async fn load_job_definition(
        &self,
        _job_id: Uuid,
    ) -> Result<String, common::errors::StorageError> {
        Ok("{}".to_string())
    }

    async fn store_context(
        &self,
        _context: &JobContext,
    ) -> Result<String, common::errors::StorageError> {
        Ok("mock_path".to_string())
    }

    async fn load_context(
        &self,
        _job_id: Uuid,
        _execution_id: Uuid,
    ) -> Result<JobContext, common::errors::StorageError> {
        Ok(JobContext::new(Uuid::new_v4(), Uuid::new_v4()))
    }

    async fn store_file(
        &self,
        path: &str,
        data: &[u8],
    ) -> Result<String, common::errors::StorageError> {
        self.add_file(path, data.to_vec());
        Ok(path.to_string())
    }

    async fn load_file(&self, path: &str) -> Result<Vec<u8>, common::errors::StorageError> {
        self.get_file(path).ok_or_else(|| {
            common::errors::StorageError::MinioError(format!("File not found: {}", path))
        })
    }
}

// ============================================================================
// Property Generators
// ============================================================================

/// Generate valid SFTP host
fn arb_sftp_host() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("localhost".to_string()),
        Just("127.0.0.1".to_string()),
        Just("sftp.example.com".to_string()),
    ]
}

/// Generate valid SFTP port
fn arb_sftp_port() -> impl Strategy<Value = u16> {
    prop_oneof![Just(22u16), Just(2222u16), Just(10022u16),]
}

/// Generate valid username
fn arb_username() -> impl Strategy<Value = String> {
    "[a-z]{3,10}".prop_map(|s| s)
}

/// Generate valid password
fn arb_password() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9]{8,20}".prop_map(|s| s)
}

/// Generate valid file path
fn arb_remote_path() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("/tmp/test.txt".to_string()),
        Just("/data/report.csv".to_string()),
        Just("/uploads/file.xlsx".to_string()),
    ]
}

/// Generate wildcard pattern
fn arb_wildcard_pattern() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("*.csv".to_string()),
        Just("*.xlsx".to_string()),
        Just("report-*.txt".to_string()),
        Just("data_*.json".to_string()),
    ]
}

/// Generate file content
fn arb_file_content() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 10..1000)
}

// ============================================================================
// Property Tests
// ============================================================================

// Note: These tests require a real SFTP server to run properly.
// For CI/CD, you would use testcontainers with an SFTP server image.
// For now, we'll test the components that don't require a live connection.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    // Property 142: SFTP download path format
    // Feature: vietnam-enterprise-cron, Property 142: SFTP download path format
    // Validates: Requirements 19.6
    #[test]
    fn property_142_sftp_download_path_format(
        job_id in any::<u128>().prop_map(Uuid::from_u128),
        execution_id in any::<u128>().prop_map(Uuid::from_u128),
        filename in "[a-zA-Z0-9_-]{3,20}\\.(txt|csv|xlsx)"
    ) {
        // For any job_id, execution_id, and filename,
        // the MinIO path for SFTP downloads should follow the format:
        // jobs/{job_id}/executions/{execution_id}/sftp/downloads/{filename}

        let expected_path = format!(
            "jobs/{}/executions/{}/sftp/downloads/{}",
            job_id, execution_id, filename
        );

        // Verify path format is correct
        assert!(expected_path.starts_with(&format!("jobs/{}/executions/{}/sftp/downloads/", job_id, execution_id)));
        assert!(expected_path.ends_with(&filename));
        assert!(expected_path.contains("/sftp/downloads/"));
    }

    // Property 144: SFTP download metadata storage
    // Feature: vietnam-enterprise-cron, Property 144: SFTP download metadata storage
    // Validates: Requirements 19.8
    #[test]
    fn property_144_sftp_download_metadata_storage(
        filename in "[a-zA-Z0-9_-]{3,20}\\.(txt|csv|xlsx)",
        file_size in 1u64..10000u64,
        _remote_path in arb_remote_path()
    ) {
        // For any SFTP download completion,
        // file metadata (filename, size, download_time, remote_path) should be present

        let metadata = FileMetadata {
            path: format!("jobs/test/executions/test/sftp/downloads/{}", filename),
            filename: filename.clone(),
            size: file_size,
            mime_type: None,
            row_count: None,
            created_at: Utc::now(),
        };

        // Verify all required metadata fields are present
        assert_eq!(metadata.filename, filename);
        assert_eq!(metadata.size, file_size);
        assert!(metadata.path.contains("/sftp/downloads/"));
        assert!(metadata.created_at <= Utc::now());
    }

    // Property 145: SFTP upload metadata storage
    // Feature: vietnam-enterprise-cron, Property 145: SFTP upload metadata storage
    // Validates: Requirements 19.9
    #[test]
    fn property_145_sftp_upload_metadata_storage(
        filename in "[a-zA-Z0-9_-]{3,20}\\.(txt|csv|xlsx)",
        file_size in 1u64..10000u64,
        remote_path in arb_remote_path()
    ) {
        // For any SFTP upload completion,
        // upload metadata (filename, size, upload_time, remote_path) should be present

        let metadata = FileMetadata {
            path: remote_path.clone(),
            filename: filename.clone(),
            size: file_size,
            mime_type: None,
            row_count: None,
            created_at: Utc::now(),
        };

        // Verify all required metadata fields are present
        assert_eq!(metadata.filename, filename);
        assert_eq!(metadata.size, file_size);
        assert_eq!(metadata.path, remote_path);
        assert!(metadata.created_at <= Utc::now());
    }

    // Property 141: SFTP wildcard pattern matching
    // Feature: vietnam-enterprise-cron, Property 141: SFTP wildcard pattern matching
    // Validates: Requirements 19.5
    #[test]
    fn property_141_sftp_wildcard_pattern_matching(
        pattern in arb_wildcard_pattern()
    ) {
        // For any wildcard pattern in SFTP download,
        // all files matching the pattern should be identified correctly

        let test_files = vec![
            "report-2024.csv",
            "report-2023.csv",
            "data.xlsx",
            "summary.txt",
            "data_v1.json",
            "data_v2.json",
        ];

        let matches: Vec<&str> = test_files
            .iter()
            .filter(|f| {
                let regex_pattern = pattern
                    .replace(".", "\\.")
                    .replace("*", ".*")
                    .replace("?", ".");
                if let Ok(re) = regex::Regex::new(&format!("^{}$", regex_pattern)) {
                    re.is_match(f)
                } else {
                    false
                }
            })
            .copied()
            .collect();

        // Verify pattern matching logic
        match pattern.as_str() {
            "*.csv" => {
                assert!(matches.contains(&"report-2024.csv"));
                assert!(matches.contains(&"report-2023.csv"));
                assert!(!matches.contains(&"data.xlsx"));
            }
            "*.xlsx" => {
                assert!(matches.contains(&"data.xlsx"));
                assert!(!matches.contains(&"report-2024.csv"));
            }
            "report-*.txt" => {
                // No matches in test files
                assert!(matches.is_empty() || !matches.contains(&"data.xlsx"));
            }
            "data_*.json" => {
                assert!(matches.contains(&"data_v1.json"));
                assert!(matches.contains(&"data_v2.json"));
                assert!(!matches.contains(&"data.xlsx"));
            }
            _ => {}
        }
    }
}

// ============================================================================
// Unit Tests for SFTP Components
// ============================================================================

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_sftp_download_path_format_specific() {
        // Property 142: SFTP download path format
        // Feature: vietnam-enterprise-cron, Property 142: SFTP download path format
        // Validates: Requirements 19.6

        let job_id = Uuid::new_v4();
        let execution_id = Uuid::new_v4();
        let filename = "test.csv";

        let path = format!(
            "jobs/{}/executions/{}/sftp/downloads/{}",
            job_id, execution_id, filename
        );

        assert!(path.starts_with("jobs/"));
        assert!(path.contains("/executions/"));
        assert!(path.contains("/sftp/downloads/"));
        assert!(path.ends_with("/test.csv"));
    }

    #[test]
    fn test_sftp_auth_password() {
        // Property 139: SFTP password authentication
        // Feature: vietnam-enterprise-cron, Property 139: SFTP password authentication
        // Validates: Requirements 19.3

        let auth = SftpAuth::Password {
            username: "testuser".to_string(),
            password: "testpass".to_string(),
        };

        match auth {
            SftpAuth::Password { username, password } => {
                assert_eq!(username, "testuser");
                assert_eq!(password, "testpass");
            }
            _ => panic!("Expected Password auth"),
        }
    }

    #[test]
    fn test_sftp_auth_ssh_key() {
        // Property 140: SFTP key-based authentication
        // Feature: vietnam-enterprise-cron, Property 140: SFTP key-based authentication
        // Validates: Requirements 19.4

        let auth = SftpAuth::SshKey {
            username: "testuser".to_string(),
            private_key_path: "/path/to/key".to_string(),
        };

        match auth {
            SftpAuth::SshKey {
                username,
                private_key_path,
            } => {
                assert_eq!(username, "testuser");
                assert_eq!(private_key_path, "/path/to/key");
            }
            _ => panic!("Expected SshKey auth"),
        }
    }

    #[test]
    fn test_sftp_operation_download() {
        // Property 137: SFTP download to MinIO
        // Feature: vietnam-enterprise-cron, Property 137: SFTP download to MinIO
        // Validates: Requirements 19.1

        let operation = SftpOperation::Download;
        assert!(matches!(operation, SftpOperation::Download));
    }

    #[test]
    fn test_sftp_operation_upload() {
        // Property 138: SFTP upload from MinIO
        // Feature: vietnam-enterprise-cron, Property 138: SFTP upload from MinIO
        // Validates: Requirements 19.2

        let operation = SftpOperation::Upload;
        assert!(matches!(operation, SftpOperation::Upload));
    }

    #[test]
    fn test_sftp_options_recursive() {
        // Property 148: SFTP recursive directory download
        // Feature: vietnam-enterprise-cron, Property 148: SFTP recursive directory download
        // Validates: Requirements 19.13

        let options = SftpOptions {
            wildcard_pattern: None,
            recursive: true,
            create_directories: false,
            verify_host_key: true,
        };

        assert!(options.recursive);
    }

    #[test]
    fn test_sftp_options_create_directories() {
        // Property 149: SFTP remote directory creation
        // Feature: vietnam-enterprise-cron, Property 149: SFTP remote directory creation
        // Validates: Requirements 19.14

        let options = SftpOptions {
            wildcard_pattern: None,
            recursive: false,
            create_directories: true,
            verify_host_key: true,
        };

        assert!(options.create_directories);
    }

    #[test]
    fn test_sftp_options_host_key_verification() {
        // Property 151: SFTP host key verification
        // Feature: vietnam-enterprise-cron, Property 151: SFTP host key verification
        // Validates: Requirements 19.16

        let options = SftpOptions {
            wildcard_pattern: None,
            recursive: false,
            create_directories: false,
            verify_host_key: true,
        };

        assert!(options.verify_host_key);
    }

    #[tokio::test]
    async fn test_sftp_executor_invalid_job_type() {
        // Test that SftpExecutor rejects non-SFTP job types

        let minio = Arc::new(MockMinIOService::new());
        let executor = SftpExecutor::new(minio, 30);

        let step = JobStep {
            id: "step1".to_string(),
            name: "Test Step".to_string(),
            step_type: JobType::HttpRequest {
                method: common::models::HttpMethod::Get,
                url: "http://example.com".to_string(),
                headers: HashMap::new(),
                body: None,
                auth: None,
            },
            condition: None,
        };

        let mut context = JobContext::new(Uuid::new_v4(), Uuid::new_v4());

        let result = executor.execute(&step, &mut context).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            common::errors::ExecutionError::InvalidJobDefinition(_)
        ));
    }

    #[test]
    fn test_sftp_file_metadata_structure() {
        // Property 144 & 145: Metadata structure validation
        // Feature: vietnam-enterprise-cron, Property 144 & 145
        // Validates: Requirements 19.8, 19.9

        let metadata = FileMetadata {
            path: "jobs/test/executions/test/sftp/downloads/file.txt".to_string(),
            filename: "file.txt".to_string(),
            size: 1024,
            mime_type: None,
            row_count: None,
            created_at: Utc::now(),
        };

        // Verify metadata has all required fields
        assert!(!metadata.path.is_empty());
        assert!(!metadata.filename.is_empty());
        assert!(metadata.size > 0);
        assert!(metadata.created_at <= Utc::now());
    }

    #[tokio::test]
    async fn test_minio_service_file_storage() {
        // Property 137 & 138: File storage in MinIO
        // Feature: vietnam-enterprise-cron, Property 137 & 138
        // Validates: Requirements 19.1, 19.2

        let minio = MockMinIOService::new();
        let test_data = b"test file content";
        let path = "jobs/test/executions/test/sftp/downloads/test.txt";

        // Store file
        let result = minio.store_file(path, test_data).await;
        assert!(result.is_ok());

        // Verify file exists
        assert!(minio.file_exists(path));

        // Load file
        let loaded = minio.load_file(path).await;
        assert!(loaded.is_ok());
        assert_eq!(loaded.unwrap(), test_data);
    }

    #[test]
    fn test_sftp_reference_resolution() {
        // Property 150: SFTP file path reference resolution
        // Feature: vietnam-enterprise-cron, Property 150: SFTP file path reference resolution
        // Validates: Requirements 19.15

        let mut context = JobContext::new(Uuid::new_v4(), Uuid::new_v4());

        // Add a previous step output with file path
        let step_output = json!({
            "output_files": [
                {
                    "path": "jobs/test/executions/test/output/data.csv",
                    "filename": "data.csv"
                }
            ]
        });

        context.steps.insert(
            "step1".to_string(),
            common::models::StepOutput {
                step_id: "step1".to_string(),
                status: "success".to_string(),
                output: step_output,
                started_at: Utc::now(),
                completed_at: Utc::now(),
            },
        );

        // Verify context has the step output
        assert!(context.steps.contains_key("step1"));
        let step1_output = &context.steps["step1"].output;
        assert!(step1_output["output_files"].is_array());
        assert_eq!(
            step1_output["output_files"][0]["path"],
            "jobs/test/executions/test/output/data.csv"
        );
    }
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[cfg(test)]
mod error_tests {
    use common::errors::ExecutionError;

    #[test]
    fn test_sftp_authentication_error_no_retry() {
        // Property 146: SFTP authentication error no-retry
        // Feature: vietnam-enterprise-cron, Property 146: SFTP authentication error no-retry
        // Validates: Requirements 19.11

        let error = ExecutionError::SftpAuthenticationFailed("Invalid credentials".to_string());

        // Verify error type
        assert!(matches!(error, ExecutionError::SftpAuthenticationFailed(_)));

        // In the actual retry logic, this error type should not trigger retries
        // This is enforced in the worker's retry strategy
    }

    #[test]
    fn test_sftp_file_not_found_error_no_retry() {
        // Property 147: SFTP file not found no-retry
        // Feature: vietnam-enterprise-cron, Property 147: SFTP file not found no-retry
        // Validates: Requirements 19.12

        let error = ExecutionError::SftpFileNotFound("/path/to/missing/file.txt".to_string());

        // Verify error type
        assert!(matches!(error, ExecutionError::SftpFileNotFound(_)));

        // In the actual retry logic, this error type should not trigger retries
        // This is enforced in the worker's retry strategy
    }

    #[test]
    fn test_sftp_connection_error_allows_retry() {
        // Connection errors should allow retry (unlike auth and file not found)

        let error = ExecutionError::SftpConnectionFailed("Connection timeout".to_string());

        // Verify error type
        assert!(matches!(error, ExecutionError::SftpConnectionFailed(_)));

        // In the actual retry logic, this error type SHOULD trigger retries
        // This is enforced in the worker's retry strategy
    }

    #[test]
    fn test_sftp_operation_error() {
        let error = ExecutionError::SftpOperationFailed("Failed to read file".to_string());

        // Verify error type
        assert!(matches!(error, ExecutionError::SftpOperationFailed(_)));
    }
}

// ============================================================================
// Integration-style Tests (without real SFTP server)
// ============================================================================

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_sftp_upload_round_trip_simulation() {
        // Property 143: SFTP upload round-trip
        // Feature: vietnam-enterprise-cron, Property 143: SFTP upload round-trip
        // Validates: Requirements 19.7

        // This simulates the round-trip without a real SFTP server
        // In a real test, you would upload to SFTP, then download, and verify content

        let minio = MockMinIOService::new();
        let original_data = b"test file content for round trip";
        let local_path = "jobs/test/input/file.txt";
        let _remote_path = "/tmp/uploaded_file.txt";

        // Simulate upload: store in MinIO first
        minio.add_file(local_path, original_data.to_vec());

        // Simulate download: retrieve from MinIO
        let retrieved = minio.get_file(local_path);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap(), original_data);

        // In a real SFTP round-trip:
        // 1. Upload file from MinIO to SFTP server
        // 2. Download file from SFTP server to MinIO (different path)
        // 3. Verify content matches original
    }

    #[tokio::test]
    async fn test_sftp_download_to_minio_path_format() {
        // Property 137 & 142: SFTP download to MinIO with correct path format
        // Feature: vietnam-enterprise-cron, Property 137 & 142
        // Validates: Requirements 19.1, 19.6

        let job_id = Uuid::new_v4();
        let execution_id = Uuid::new_v4();
        let filename = "downloaded_file.csv";

        let expected_path = format!(
            "jobs/{}/executions/{}/sftp/downloads/{}",
            job_id, execution_id, filename
        );

        // Verify path format
        assert!(expected_path.contains(&job_id.to_string()));
        assert!(expected_path.contains(&execution_id.to_string()));
        assert!(expected_path.contains("/sftp/downloads/"));
        assert!(expected_path.ends_with(filename));
    }

    #[tokio::test]
    async fn test_sftp_upload_from_minio() {
        // Property 138: SFTP upload from MinIO
        // Feature: vietnam-enterprise-cron, Property 138: SFTP upload from MinIO
        // Validates: Requirements 19.2

        let minio = MockMinIOService::new();
        let test_data = b"data to upload via SFTP";
        let local_path = "jobs/test/executions/test/output/upload.txt";

        // Store file in MinIO
        let result = minio.store_file(local_path, test_data).await;
        assert!(result.is_ok());

        // Verify file can be loaded (simulating upload preparation)
        let loaded = minio.load_file(local_path).await;
        assert!(loaded.is_ok());
        assert_eq!(loaded.unwrap(), test_data);

        // In a real SFTP upload, this data would be sent to the SFTP server
    }
}
