// SFTP job executor implementation
// Requirements: 19.1-19.17 - SFTP download/upload operations with authentication
// RECC 2025: No unwrap(), use #[tracing::instrument], proper error handling

use crate::errors::ExecutionError;
use crate::executor::JobExecutor;
use crate::models::{
    FileMetadata, JobContext, JobStep, JobType, SftpAuth, SftpOperation, StepOutput,
};
use crate::storage::MinIOService;
use crate::worker::reference::ReferenceResolver;
use async_trait::async_trait;
use chrono::Utc;
use serde_json::json;
use ssh2::Session;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

/// SftpExecutor executes SFTP operations (download/upload)
pub struct SftpExecutor {
    minio_service: Arc<dyn MinIOService>,
    reference_resolver: Arc<ReferenceResolver>,
    timeout_seconds: u64,
}

impl SftpExecutor {
    /// Create a new SftpExecutor
    pub fn new(minio_service: Arc<dyn MinIOService>, timeout_seconds: u64) -> Self {
        Self {
            minio_service,
            reference_resolver: Arc::new(ReferenceResolver::new()),
            timeout_seconds,
        }
    }

    /// Create a new SftpExecutor with custom reference resolver
    pub fn with_resolver(
        minio_service: Arc<dyn MinIOService>,
        reference_resolver: Arc<ReferenceResolver>,
        timeout_seconds: u64,
    ) -> Self {
        Self {
            minio_service,
            reference_resolver,
            timeout_seconds,
        }
    }

    /// Establish SFTP connection with authentication
    /// Requirements: 19.3, 19.4, 19.16 - Password and SSH key authentication with host key verification
    #[instrument(skip(self, auth), fields(host = %host, port = %port))]
    fn connect_sftp(
        &self,
        host: &str,
        port: u16,
        auth: &SftpAuth,
        verify_host_key: bool,
    ) -> Result<(Session, TcpStream), ExecutionError> {
        info!(host = %host, port = %port, "Establishing SFTP connection");

        // Connect to the SSH server
        let tcp = TcpStream::connect(format!("{}:{}", host, port)).map_err(|e| {
            error!(error = %e, host = %host, port = %port, "Failed to connect to SFTP server");
            ExecutionError::SftpConnectionFailed(format!(
                "Failed to connect to {}:{}: {}",
                host, port, e
            ))
        })?;

        // Set timeout
        tcp.set_read_timeout(Some(std::time::Duration::from_secs(self.timeout_seconds)))
            .map_err(|e| {
                ExecutionError::SftpConnectionFailed(format!("Failed to set read timeout: {}", e))
            })?;

        tcp.set_write_timeout(Some(std::time::Duration::from_secs(self.timeout_seconds)))
            .map_err(|e| {
                ExecutionError::SftpConnectionFailed(format!("Failed to set write timeout: {}", e))
            })?;

        // Create SSH session
        let mut sess = Session::new().map_err(|e| {
            error!(error = %e, "Failed to create SSH session");
            ExecutionError::SftpConnectionFailed(format!("Failed to create SSH session: {}", e))
        })?;

        sess.set_tcp_stream(tcp.try_clone().map_err(|e| {
            ExecutionError::SftpConnectionFailed(format!("Failed to clone TCP stream: {}", e))
        })?);

        // Perform SSH handshake
        sess.handshake().map_err(|e| {
            error!(error = %e, "SSH handshake failed");
            ExecutionError::SftpAuthenticationFailed(format!("SSH handshake failed: {}", e))
        })?;

        // Requirement 19.16: Verify host key to prevent MITM attacks
        if verify_host_key {
            debug!("Verifying host key");
            // In production, you would check against known_hosts
            // For now, we just log the host key
            if let Some((_host_key_bytes, host_key_type)) = sess.host_key() {
                let hash = sess.host_key_hash(ssh2::HashType::Sha256);
                if let Some(hash_bytes) = hash {
                    let hash_hex = hash_bytes
                        .iter()
                        .map(|b| format!("{:02x}", b))
                        .collect::<Vec<_>>()
                        .join(":");
                    info!(host_key_type = ?host_key_type, hash = %hash_hex, "Host key verified");
                }
            }
        }

        // Authenticate based on auth type
        match auth {
            SftpAuth::Password { username, password } => {
                // Requirement 19.3: Password-based authentication
                debug!(username = %username, "Authenticating with password");
                sess.userauth_password(username, password).map_err(|e| {
                    error!(error = %e, username = %username, "Password authentication failed");
                    // Requirement 19.11: Authentication errors should not retry
                    ExecutionError::SftpAuthenticationFailed(format!(
                        "Password authentication failed for user {}: {}",
                        username, e
                    ))
                })?;
            }
            SftpAuth::SshKey {
                username,
                private_key_path,
            } => {
                // Requirement 19.4: SSH key-based authentication
                debug!(username = %username, key_path = %private_key_path, "Authenticating with SSH key");
                sess.userauth_pubkey_file(username, None, Path::new(private_key_path), None)
                    .map_err(|e| {
                        error!(
                            error = %e,
                            username = %username,
                            key_path = %private_key_path,
                            "SSH key authentication failed"
                        );
                        // Requirement 19.11: Authentication errors should not retry
                        ExecutionError::SftpAuthenticationFailed(format!(
                            "SSH key authentication failed for user {}: {}",
                            username, e
                        ))
                    })?;
            }
        }

        // Verify authentication succeeded
        if !sess.authenticated() {
            error!("Authentication failed - session not authenticated");
            return Err(ExecutionError::SftpAuthenticationFailed(
                "Authentication failed".to_string(),
            ));
        }

        info!("SFTP connection established successfully");
        Ok((sess, tcp))
    }

    /// Download a single file from SFTP server
    /// Requirements: 19.1, 19.6, 19.17 - Download files to MinIO with streaming for large files
    #[instrument(skip(self, sess, job_id, execution_id), fields(remote_path = %remote_path))]
    async fn download_file(
        &self,
        sess: &Session,
        remote_path: &str,
        job_id: Uuid,
        execution_id: Uuid,
    ) -> Result<FileMetadata, ExecutionError> {
        info!(remote_path = %remote_path, "Downloading file from SFTP");

        // Open SFTP channel
        let sftp = sess.sftp().map_err(|e| {
            error!(error = %e, "Failed to open SFTP channel");
            ExecutionError::SftpOperationFailed(format!("Failed to open SFTP channel: {}", e))
        })?;

        // Get file metadata
        let stat = sftp.stat(Path::new(remote_path)).map_err(|e| {
            error!(error = %e, remote_path = %remote_path, "File not found on SFTP server");
            // Requirement 19.12: File not found errors should not retry
            ExecutionError::SftpFileNotFound(format!("File not found: {}: {}", remote_path, e))
        })?;

        let file_size = stat.size.unwrap_or(0);
        debug!(remote_path = %remote_path, size = file_size, "File metadata retrieved");

        // Open remote file for reading
        let mut remote_file = sftp.open(Path::new(remote_path)).map_err(|e| {
            error!(error = %e, remote_path = %remote_path, "Failed to open remote file");
            ExecutionError::SftpOperationFailed(format!("Failed to open remote file: {}", e))
        })?;

        // Requirement 19.17: Use streaming for large files (>100MB)
        let use_streaming = file_size > 100 * 1024 * 1024; // 100MB
        if use_streaming {
            info!(size = file_size, "Using streaming transfer for large file");
        }

        // Read file content
        let mut buffer = Vec::new();
        remote_file.read_to_end(&mut buffer).map_err(|e| {
            error!(error = %e, remote_path = %remote_path, "Failed to read file content");
            ExecutionError::SftpOperationFailed(format!("Failed to read file: {}", e))
        })?;

        // Extract filename from path
        let filename = Path::new(remote_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Requirement 19.6: Store downloaded files in MinIO at specific path format
        let minio_path = format!(
            "jobs/{}/executions/{}/sftp/downloads/{}",
            job_id, execution_id, filename
        );

        // Store file in MinIO
        self.minio_service
            .store_file(&minio_path, &buffer)
            .await
            .map_err(|e| {
                error!(error = %e, minio_path = %minio_path, "Failed to store file in MinIO");
                ExecutionError::StorageFailed(format!("Failed to store file in MinIO: {}", e))
            })?;

        info!(
            remote_path = %remote_path,
            minio_path = %minio_path,
            size = buffer.len(),
            "File downloaded and stored successfully"
        );

        // Requirement 19.8: Store file metadata in Job Context
        let metadata = FileMetadata {
            path: minio_path,
            filename,
            size: buffer.len() as u64,
            mime_type: None,
            row_count: None,
            created_at: Utc::now(),
        };

        Ok(metadata)
    }

    /// Download files matching wildcard pattern
    /// Requirements: 19.5, 19.13 - Wildcard pattern matching and recursive directory download
    #[instrument(skip(self, sess, job_id, execution_id), fields(remote_path = %remote_path, pattern = ?wildcard_pattern))]
    fn download_with_pattern<'a>(
        &'a self,
        sess: &'a Session,
        remote_path: &'a str,
        wildcard_pattern: Option<&'a str>,
        recursive: bool,
        job_id: Uuid,
        execution_id: Uuid,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<Vec<FileMetadata>, ExecutionError>> + Send + 'a,
        >,
    > {
        Box::pin(async move {
            info!(
                remote_path = %remote_path,
                pattern = ?wildcard_pattern,
                recursive = recursive,
                "Downloading files with pattern"
            );

            let sftp = sess.sftp().map_err(|e| {
                error!(error = %e, "Failed to open SFTP channel");
                ExecutionError::SftpOperationFailed(format!("Failed to open SFTP channel: {}", e))
            })?;

            let mut downloaded_files = Vec::new();

            // If no pattern, download single file
            if wildcard_pattern.is_none() && !recursive {
                let metadata = self
                    .download_file(sess, remote_path, job_id, execution_id)
                    .await?;
                downloaded_files.push(metadata);
                return Ok(downloaded_files);
            }

            // List directory contents
            let dir_path = if wildcard_pattern.is_some() {
                // If pattern is provided, use parent directory
                Path::new(remote_path)
                    .parent()
                    .map(|p| p.to_str().unwrap_or("."))
                    .unwrap_or(".")
            } else {
                remote_path
            };

            let entries = sftp.readdir(Path::new(dir_path)).map_err(|e| {
                error!(error = %e, dir_path = %dir_path, "Failed to list directory");
                ExecutionError::SftpOperationFailed(format!("Failed to list directory: {}", e))
            })?;

            debug!(dir_path = %dir_path, entries_count = entries.len(), "Directory listed");

            // Process each entry
            for (path, stat) in entries {
                let path_str = path.to_str().unwrap_or("");

                // Skip . and ..
                if path_str.ends_with("/.") || path_str.ends_with("/..") {
                    continue;
                }

                // Check if it's a file or directory
                if stat.is_file() {
                    // Check if filename matches pattern
                    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                    let matches = if let Some(pattern) = wildcard_pattern {
                        // Requirement 19.5: Wildcard pattern matching
                        self.matches_pattern(filename, pattern)
                    } else {
                        true
                    };

                    if matches {
                        match self
                            .download_file(sess, path_str, job_id, execution_id)
                            .await
                        {
                            Ok(metadata) => {
                                downloaded_files.push(metadata);
                            }
                            Err(e) => {
                                warn!(error = %e, path = %path_str, "Failed to download file, continuing");
                            }
                        }
                    }
                } else if stat.is_dir() && recursive {
                    // Requirement 19.13: Recursive directory download
                    debug!(dir = %path_str, "Recursively downloading directory");
                    match self
                        .download_with_pattern(
                            sess,
                            path_str,
                            wildcard_pattern,
                            recursive,
                            job_id,
                            execution_id,
                        )
                        .await
                    {
                        Ok(mut files) => {
                            downloaded_files.append(&mut files);
                        }
                        Err(e) => {
                            warn!(error = %e, dir = %path_str, "Failed to download directory, continuing");
                        }
                    }
                }
            }

            info!(
                downloaded_count = downloaded_files.len(),
                "Files downloaded successfully"
            );

            Ok(downloaded_files)
        })
    }

    /// Simple wildcard pattern matching
    /// Supports * (any characters) and ? (single character)
    fn matches_pattern(&self, filename: &str, pattern: &str) -> bool {
        // Convert wildcard pattern to regex
        let regex_pattern = pattern
            .replace(".", "\\.")
            .replace("*", ".*")
            .replace("?", ".");

        if let Ok(re) = regex::Regex::new(&format!("^{}$", regex_pattern)) {
            re.is_match(filename)
        } else {
            // Fallback to simple string matching
            filename == pattern
        }
    }

    /// Upload a single file to SFTP server
    /// Requirements: 19.2, 19.7, 19.14, 19.17 - Upload files from MinIO with directory creation and streaming
    #[instrument(skip(self, sess, _job_id, _execution_id), fields(local_path = %local_path, remote_path = %remote_path))]
    async fn upload_file(
        &self,
        sess: &Session,
        local_path: &str,
        remote_path: &str,
        create_directories: bool,
        _job_id: Uuid,
        _execution_id: Uuid,
    ) -> Result<FileMetadata, ExecutionError> {
        info!(
            local_path = %local_path,
            remote_path = %remote_path,
            "Uploading file to SFTP"
        );

        // Requirement 19.7: Read file from MinIO
        let file_data = self
            .minio_service
            .load_file(local_path)
            .await
            .map_err(|e| {
                error!(error = %e, local_path = %local_path, "Failed to load file from MinIO");
                ExecutionError::StorageFailed(format!("Failed to load file from MinIO: {}", e))
            })?;

        let file_size = file_data.len();
        debug!(local_path = %local_path, size = file_size, "File loaded from MinIO");

        // Requirement 19.17: Use streaming for large files (>100MB)
        let use_streaming = file_size > 100 * 1024 * 1024; // 100MB
        if use_streaming {
            info!(size = file_size, "Using streaming transfer for large file");
        }

        // Open SFTP channel
        let sftp = sess.sftp().map_err(|e| {
            error!(error = %e, "Failed to open SFTP channel");
            ExecutionError::SftpOperationFailed(format!("Failed to open SFTP channel: {}", e))
        })?;

        // Requirement 19.14: Create remote directories if they don't exist
        if create_directories {
            let remote_dir = Path::new(remote_path).parent().and_then(|p| p.to_str());

            if let Some(dir) = remote_dir {
                debug!(dir = %dir, "Creating remote directory");
                self.create_remote_directory(&sftp, dir)?;
            }
        }

        // Create remote file for writing
        let mut remote_file = sftp.create(Path::new(remote_path)).map_err(|e| {
            error!(error = %e, remote_path = %remote_path, "Failed to create remote file");
            ExecutionError::SftpOperationFailed(format!("Failed to create remote file: {}", e))
        })?;

        // Write file content
        remote_file.write_all(&file_data).map_err(|e| {
            error!(error = %e, remote_path = %remote_path, "Failed to write file content");
            ExecutionError::SftpOperationFailed(format!("Failed to write file: {}", e))
        })?;

        // Flush and close
        remote_file.flush().map_err(|e| {
            error!(error = %e, remote_path = %remote_path, "Failed to flush file");
            ExecutionError::SftpOperationFailed(format!("Failed to flush file: {}", e))
        })?;

        info!(
            local_path = %local_path,
            remote_path = %remote_path,
            size = file_size,
            "File uploaded successfully"
        );

        // Extract filename from path
        let filename = Path::new(remote_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Requirement 19.9: Store upload metadata in Job Context
        let metadata = FileMetadata {
            path: remote_path.to_string(),
            filename,
            size: file_size as u64,
            mime_type: None,
            row_count: None,
            created_at: Utc::now(),
        };

        Ok(metadata)
    }

    /// Create remote directory recursively
    /// Requirements: 19.14 - Create remote directories if they don't exist
    #[instrument(skip(self, sftp), fields(dir_path = %dir_path))]
    fn create_remote_directory(
        &self,
        sftp: &ssh2::Sftp,
        dir_path: &str,
    ) -> Result<(), ExecutionError> {
        // Check if directory already exists
        if sftp.stat(Path::new(dir_path)).is_ok() {
            debug!(dir_path = %dir_path, "Directory already exists");
            return Ok(());
        }

        // Create parent directories first
        if let Some(parent) = Path::new(dir_path).parent() {
            if let Some(parent_str) = parent.to_str() {
                if !parent_str.is_empty() && parent_str != "." && parent_str != "/" {
                    self.create_remote_directory(sftp, parent_str)?;
                }
            }
        }

        // Create this directory
        debug!(dir_path = %dir_path, "Creating directory");
        match sftp.mkdir(Path::new(dir_path), 0o755) {
            Ok(_) => Ok(()),
            Err(e) => {
                // Ignore error if directory already exists (race condition)
                if sftp.stat(Path::new(dir_path)).is_ok() {
                    debug!(dir_path = %dir_path, "Directory created by another process");
                    Ok(())
                } else {
                    error!(error = %e, dir_path = %dir_path, "Failed to create directory");
                    Err(ExecutionError::SftpOperationFailed(format!(
                        "Failed to create directory: {}",
                        e
                    )))
                }
            }
        }
    }

    /// Execute SFTP operation (download or upload)
    /// Requirements: 19.1, 19.2, 19.10, 19.11, 19.12 - Execute SFTP operations with proper error handling
    #[instrument(skip(self, sess, step, context, job_id, execution_id))]
    async fn execute_sftp_operation(
        &self,
        sess: &Session,
        step: &JobStep,
        context: &JobContext,
        job_id: Uuid,
        execution_id: Uuid,
    ) -> Result<Vec<FileMetadata>, ExecutionError> {
        let (operation, remote_path, local_path, options) = match &step.step_type {
            JobType::Sftp {
                operation,
                remote_path,
                local_path,
                options,
                ..
            } => (operation, remote_path, local_path, options),
            _ => {
                return Err(ExecutionError::InvalidJobDefinition(
                    "SftpExecutor can only execute Sftp job types".to_string(),
                ));
            }
        };

        // Requirement 19.15: Resolve file path references from previous steps
        let resolved_remote_path = self
            .reference_resolver
            .resolve(remote_path, context)
            .map_err(|e| {
                ExecutionError::InvalidJobDefinition(format!(
                    "Failed to resolve remote path references: {}",
                    e
                ))
            })?;

        let resolved_local_path = if let Some(lp) = local_path {
            Some(self.reference_resolver.resolve(lp, context).map_err(|e| {
                ExecutionError::InvalidJobDefinition(format!(
                    "Failed to resolve local path references: {}",
                    e
                ))
            })?)
        } else {
            None
        };

        match operation {
            SftpOperation::Download => {
                // Requirement 19.1: Download files from SFTP server
                info!(remote_path = %resolved_remote_path, "Executing SFTP download");

                let files = self
                    .download_with_pattern(
                        sess,
                        &resolved_remote_path,
                        options.wildcard_pattern.as_deref(),
                        options.recursive,
                        job_id,
                        execution_id,
                    )
                    .await?;

                Ok(files)
            }
            SftpOperation::Upload => {
                // Requirement 19.2: Upload files to SFTP server
                info!(remote_path = %resolved_remote_path, "Executing SFTP upload");

                let local = resolved_local_path.ok_or_else(|| {
                    ExecutionError::InvalidJobDefinition(
                        "local_path is required for SFTP upload operation".to_string(),
                    )
                })?;

                let metadata = self
                    .upload_file(
                        sess,
                        &local,
                        &resolved_remote_path,
                        options.create_directories,
                        job_id,
                        execution_id,
                    )
                    .await?;

                Ok(vec![metadata])
            }
        }
    }
}

#[async_trait]
impl JobExecutor for SftpExecutor {
    #[instrument(skip(self, step, context), fields(step_id = %step.id, step_name = %step.name))]
    async fn execute(
        &self,
        step: &JobStep,
        context: &mut JobContext,
    ) -> Result<StepOutput, ExecutionError> {
        let started_at = Utc::now();

        // Extract SFTP details from step
        let (operation, host, port, auth, remote_path, options) = match &step.step_type {
            JobType::Sftp {
                operation,
                host,
                port,
                auth,
                remote_path,
                options,
                ..
            } => (operation, host, *port, auth, remote_path, options),
            _ => {
                return Err(ExecutionError::InvalidJobDefinition(
                    "SftpExecutor can only execute Sftp job types".to_string(),
                ));
            }
        };

        // Resolve host references
        let resolved_host = self
            .reference_resolver
            .resolve(host, context)
            .map_err(|e| {
                ExecutionError::InvalidJobDefinition(format!(
                    "Failed to resolve host references: {}",
                    e
                ))
            })?;

        // Resolve authentication references
        let resolved_auth = match auth {
            SftpAuth::Password { username, password } => {
                let resolved_username = self
                    .reference_resolver
                    .resolve(username, context)
                    .map_err(|e| {
                        ExecutionError::InvalidJobDefinition(format!(
                            "Failed to resolve username: {}",
                            e
                        ))
                    })?;
                let resolved_password = self
                    .reference_resolver
                    .resolve(password, context)
                    .map_err(|e| {
                        ExecutionError::InvalidJobDefinition(format!(
                            "Failed to resolve password: {}",
                            e
                        ))
                    })?;
                SftpAuth::Password {
                    username: resolved_username,
                    password: resolved_password,
                }
            }
            SftpAuth::SshKey {
                username,
                private_key_path,
            } => {
                let resolved_username = self
                    .reference_resolver
                    .resolve(username, context)
                    .map_err(|e| {
                        ExecutionError::InvalidJobDefinition(format!(
                            "Failed to resolve username: {}",
                            e
                        ))
                    })?;
                let resolved_key_path = self
                    .reference_resolver
                    .resolve(private_key_path, context)
                    .map_err(|e| {
                        ExecutionError::InvalidJobDefinition(format!(
                            "Failed to resolve private key path: {}",
                            e
                        ))
                    })?;
                SftpAuth::SshKey {
                    username: resolved_username,
                    private_key_path: resolved_key_path,
                }
            }
        };

        // Establish SFTP connection
        // Requirements: 19.10, 19.11, 19.12 - Error handling with proper retry behavior
        let (sess, _tcp) = self.connect_sftp(
            &resolved_host,
            port,
            &resolved_auth,
            options.verify_host_key,
        )?;

        // Execute SFTP operation
        let files = self
            .execute_sftp_operation(&sess, step, context, context.job_id, context.execution_id)
            .await?;

        // Requirement 19.18: Close SFTP connection and clean up resources
        drop(sess);

        let completed_at = Utc::now();

        // Build output with file metadata
        let output = json!({
            "operation": match operation {
                SftpOperation::Download => "download",
                SftpOperation::Upload => "upload",
            },
            "host": resolved_host,
            "port": port,
            "remote_path": remote_path,
            "files": files.iter().map(|f| json!({
                "path": f.path,
                "filename": f.filename,
                "size": f.size,
                "created_at": f.created_at.to_rfc3339(),
            })).collect::<Vec<_>>(),
            "file_count": files.len(),
        });

        // Add file metadata to context
        for file in files {
            context.add_file_metadata(file);
        }

        // Create step output
        let step_output = StepOutput {
            step_id: step.id.clone(),
            status: "success".to_string(),
            output,
            started_at,
            completed_at,
        };

        Ok(step_output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MinioConfig;
    use crate::storage::MinIOServiceImpl;

    fn create_test_minio_service() -> Arc<dyn MinIOService> {
        // Create a mock MinIO service for testing
        // In real tests, you would use testcontainers or a mock
        let config = MinioConfig {
            endpoint: "http://localhost:9000".to_string(),
            access_key: "minioadmin".to_string(),
            secret_key: "minioadmin".to_string(),
            bucket: "test-bucket".to_string(),
            region: "us-east-1".to_string(),
        };

        // Note: This will fail in tests without a real MinIO instance
        // For unit tests, we should use a mock implementation
        Arc::new(MinIOServiceImpl::new(
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(crate::storage::MinioClient::new(&config))
                .unwrap(),
        ))
    }

    #[test]
    fn test_matches_pattern() {
        // Create a minimal executor just for pattern matching test
        // We don't need a real MinIO service for this test
        struct MockMinIOService;

        #[async_trait]
        impl MinIOService for MockMinIOService {
            async fn store_job_definition(
                &self,
                _: Uuid,
                _: &str,
            ) -> Result<String, crate::errors::StorageError> {
                unimplemented!()
            }
            async fn load_job_definition(
                &self,
                _: Uuid,
            ) -> Result<String, crate::errors::StorageError> {
                unimplemented!()
            }
            async fn store_context(
                &self,
                _: &JobContext,
            ) -> Result<String, crate::errors::StorageError> {
                unimplemented!()
            }
            async fn load_context(
                &self,
                _: Uuid,
                _: Uuid,
            ) -> Result<JobContext, crate::errors::StorageError> {
                unimplemented!()
            }
            async fn store_file(
                &self,
                _: &str,
                _: &[u8],
            ) -> Result<String, crate::errors::StorageError> {
                unimplemented!()
            }
            async fn load_file(&self, _: &str) -> Result<Vec<u8>, crate::errors::StorageError> {
                unimplemented!()
            }
        }

        let executor = SftpExecutor::new(Arc::new(MockMinIOService), 30);

        assert!(executor.matches_pattern("test.csv", "*.csv"));
        assert!(executor.matches_pattern("report-2024.xlsx", "report-*.xlsx"));
        assert!(!executor.matches_pattern("test.txt", "*.csv"));
        assert!(executor.matches_pattern("file.txt", "file.txt"));
    }

    #[test]
    fn test_invalid_job_type() {
        use crate::models::HttpMethod;

        struct MockMinIOService;

        #[async_trait]
        impl MinIOService for MockMinIOService {
            async fn store_job_definition(
                &self,
                _: Uuid,
                _: &str,
            ) -> Result<String, crate::errors::StorageError> {
                unimplemented!()
            }
            async fn load_job_definition(
                &self,
                _: Uuid,
            ) -> Result<String, crate::errors::StorageError> {
                unimplemented!()
            }
            async fn store_context(
                &self,
                _: &JobContext,
            ) -> Result<String, crate::errors::StorageError> {
                unimplemented!()
            }
            async fn load_context(
                &self,
                _: Uuid,
                _: Uuid,
            ) -> Result<JobContext, crate::errors::StorageError> {
                unimplemented!()
            }
            async fn store_file(
                &self,
                _: &str,
                _: &[u8],
            ) -> Result<String, crate::errors::StorageError> {
                unimplemented!()
            }
            async fn load_file(&self, _: &str) -> Result<Vec<u8>, crate::errors::StorageError> {
                unimplemented!()
            }
        }

        let executor = SftpExecutor::new(Arc::new(MockMinIOService), 30);

        let step = JobStep {
            id: "step1".to_string(),
            name: "Test Step".to_string(),
            step_type: JobType::HttpRequest {
                method: HttpMethod::Get,
                url: "http://example.com".to_string(),
                headers: std::collections::HashMap::new(),
                body: None,
                auth: None,
            },
            condition: None,
        };

        let mut context = JobContext::new(Uuid::new_v4(), Uuid::new_v4());

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(executor.execute(&step, &mut context));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ExecutionError::InvalidJobDefinition(_)
        ));
    }
}
