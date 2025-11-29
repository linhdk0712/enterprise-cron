// SFTP operations (download/upload)
// Requirements: 19.1, 19.2, 19.6, 19.7 - File operations
// RECC 2025: Max 300 lines

use crate::errors::ExecutionError;
use crate::models::{FileMetadata, JobContext, JobStep, JobType, SftpOperation, StepOutput};
use crate::storage::StorageService;
use crate::worker::reference::ReferenceResolver;
use chrono::Utc;
use serde_json::Value;
use ssh2::Session;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, error, info, instrument};

use super::connection::SftpConnection;

/// Execute SFTP step
#[instrument(skip(step, context, storage_service, reference_resolver))]
pub async fn execute_sftp_step(
    step: &JobStep,
    context: &mut JobContext,
    storage_service: &Arc<dyn StorageService>,
    reference_resolver: &Arc<ReferenceResolver>,
    timeout_seconds: u64,
) -> Result<StepOutput, ExecutionError> {
    // Extract SFTP configuration from step
    let (operation, host, port, auth, remote_path, local_path, options) = match &step.step_type {
        JobType::Sftp {
            operation,
            host,
            port,
            auth,
            remote_path,
            local_path,
            options,
        } => (operation, host, *port, auth, remote_path, local_path, options),
        _ => {
            return Err(ExecutionError::InvalidJobType(
                "Expected SFTP step, got different type".to_string()
            ))
        }
    };

    // Resolve references in configuration
    let host_resolved = reference_resolver.resolve(host, context).unwrap_or_else(|_| host.clone());
    let verify_host_key = options.verify_host_key;

    // Establish SFTP connection
    let connection = SftpConnection::connect(&host_resolved, port, auth, verify_host_key, timeout_seconds)?;

    // Execute operation based on type
    match operation {
        SftpOperation::Download => {
            download_operation(
                connection.session(),
                remote_path,
                context,
                storage_service,
                reference_resolver,
            )
            .await
        }
        SftpOperation::Upload => {
            upload_operation(
                connection.session(),
                local_path.as_ref().unwrap_or(&String::new()),
                remote_path,
                options.create_directories,
                context,
                storage_service,
                reference_resolver,
            )
            .await
        }
    }
}

/// Download file from SFTP
async fn download_operation(
    sess: &Session,
    remote_path: &str,
    context: &JobContext,
    storage_service: &Arc<dyn StorageService>,
    reference_resolver: &Arc<ReferenceResolver>,
) -> Result<StepOutput, ExecutionError> {
    let remote_path = reference_resolver.resolve(remote_path, context).unwrap_or_else(|_| remote_path.to_string());
    
    info!(remote_path = %remote_path, "Downloading file from SFTP");

    // Open SFTP channel
    let sftp = sess.sftp().map_err(|e| {
        error!(error = %e, "Failed to open SFTP channel");
        ExecutionError::SftpOperationFailed(format!("Failed to open SFTP channel: {}", e))
    })?;

    // Get file metadata
    let stat = sftp.stat(Path::new(&remote_path)).map_err(|e| {
        error!(error = %e, remote_path = %remote_path, "File not found");
        ExecutionError::SftpFileNotFound(format!("File not found: {}: {}", remote_path, e))
    })?;

    let file_size = stat.size.unwrap_or(0);
    debug!(remote_path = %remote_path, size = file_size, "File metadata retrieved");

    // Open remote file
    let mut remote_file = sftp.open(Path::new(&remote_path)).map_err(|e| {
        error!(error = %e, remote_path = %remote_path, "Failed to open remote file");
        ExecutionError::SftpOperationFailed(format!("Failed to open remote file: {}", e))
    })?;

    // Read file content
    let mut buffer = Vec::new();
    remote_file.read_to_end(&mut buffer).map_err(|e| {
        error!(error = %e, remote_path = %remote_path, "Failed to read file");
        ExecutionError::SftpOperationFailed(format!("Failed to read file: {}", e))
    })?;

    // Extract filename
    let filename = Path::new(&remote_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Store in filesystem
    let file_path = format!(
        "jobs/{}/executions/{}/sftp/downloads/{}",
        context.job_id, context.execution_id, filename
    );

    storage_service
        .store_file(&file_path, &buffer)
        .await
        .map_err(|e| {
            error!(error = %e, file_path = %file_path, "Failed to store file");
            ExecutionError::StorageFailed(format!("Failed to store file: {}", e))
        })?;

    info!(
        remote_path = %remote_path,
        file_path = %file_path,
        size = buffer.len(),
        "File downloaded successfully"
    );

    // Create file metadata
    let metadata = FileMetadata {
        path: file_path.clone(),
        filename: filename.clone(),
        size: buffer.len() as u64,
        mime_type: None,
        row_count: None,
        created_at: Utc::now(),
    };

    Ok(StepOutput {
        step_id: "sftp_download".to_string(),
        status: "success".to_string(),
        output: Value::Object(serde_json::Map::from_iter(vec![
            ("operation".to_string(), Value::String("download".to_string())),
            ("remote_path".to_string(), Value::String(remote_path)),
            ("local_path".to_string(), Value::String(file_path)),
            ("bytes_transferred".to_string(), Value::Number(buffer.len().into())),
            ("file".to_string(), serde_json::to_value(&metadata).unwrap_or(Value::Null)),
        ])),
        started_at: Utc::now(),
        completed_at: Utc::now(),
    })
}

/// Upload file to SFTP
async fn upload_operation(
    sess: &Session,
    local_path: &str,
    remote_path: &str,
    create_remote_dirs: bool,
    context: &JobContext,
    storage_service: &Arc<dyn StorageService>,
    reference_resolver: &Arc<ReferenceResolver>,
) -> Result<StepOutput, ExecutionError> {
    let local_path = reference_resolver.resolve(local_path, context).unwrap_or_else(|_| local_path.to_string());
    let remote_path = reference_resolver.resolve(remote_path, context).unwrap_or_else(|_| remote_path.to_string());
    
    info!(local_path = %local_path, remote_path = %remote_path, "Uploading file to SFTP");

    // Load file from filesystem
    let file_data = storage_service
        .load_file(&local_path)
        .await
        .map_err(|e| {
            error!(error = %e, local_path = %local_path, "Failed to load file");
            ExecutionError::StorageFailed(format!("Failed to load file: {}", e))
        })?;

    // Open SFTP channel
    let sftp = sess.sftp().map_err(|e| {
        error!(error = %e, "Failed to open SFTP channel");
        ExecutionError::SftpOperationFailed(format!("Failed to open SFTP channel: {}", e))
    })?;

    // Create remote directories if needed
    if create_remote_dirs {
        if let Some(parent) = Path::new(&remote_path).parent() {
            create_remote_dirs_fn(&sftp, parent)?;
        }
    }

    // Write file to SFTP
    let mut remote_file = sftp.create(Path::new(&remote_path)).map_err(|e| {
        error!(error = %e, remote_path = %remote_path, "Failed to create remote file");
        ExecutionError::SftpOperationFailed(format!("Failed to create remote file: {}", e))
    })?;

    std::io::Write::write_all(&mut remote_file, &file_data).map_err(|e| {
        error!(error = %e, remote_path = %remote_path, "Failed to write file");
        ExecutionError::SftpOperationFailed(format!("Failed to write file: {}", e))
    })?;

    info!(
        local_path = %local_path,
        remote_path = %remote_path,
        size = file_data.len(),
        "File uploaded successfully"
    );

    Ok(StepOutput {
        step_id: "sftp_upload".to_string(),
        status: "success".to_string(),
        output: Value::Object(serde_json::Map::from_iter(vec![
            ("operation".to_string(), Value::String("upload".to_string())),
            ("local_path".to_string(), Value::String(local_path)),
            ("remote_path".to_string(), Value::String(remote_path)),
            ("bytes_transferred".to_string(), Value::Number(file_data.len().into())),
        ])),
        started_at: Utc::now(),
        completed_at: Utc::now(),
    })
}

/// Create remote directories recursively
fn create_remote_dirs_fn(sftp: &ssh2::Sftp, path: &Path) -> Result<(), ExecutionError> {
    if let Some(parent) = path.parent() {
        create_remote_dirs_fn(sftp, parent)?;
    }
    
    // Try to create directory, ignore if exists
    let _ = sftp.mkdir(path, 0o755);
    
    Ok(())
}

/// List files in remote directory
pub async fn list_files(
    sess: &Session,
    remote_path: &str,
) -> Result<Vec<String>, ExecutionError> {
    let sftp = sess.sftp().map_err(|e| {
        ExecutionError::SftpOperationFailed(format!("Failed to open SFTP channel: {}", e))
    })?;

    let dir = sftp.readdir(Path::new(remote_path)).map_err(|e| {
        ExecutionError::SftpOperationFailed(format!("Failed to read directory: {}", e))
    })?;

    let files = dir
        .into_iter()
        .filter_map(|(path, _stat)| path.to_str().map(|s| s.to_string()))
        .collect();

    Ok(files)
}

/// Download file (public API)
pub async fn download_file(
    sess: &Session,
    remote_path: &str,
    local_path: &str,
    storage_service: &Arc<dyn StorageService>,
) -> Result<FileMetadata, ExecutionError> {
    let sftp = sess.sftp().map_err(|e| {
        ExecutionError::SftpOperationFailed(format!("Failed to open SFTP channel: {}", e))
    })?;

    let mut remote_file = sftp.open(Path::new(remote_path)).map_err(|e| {
        ExecutionError::SftpFileNotFound(format!("File not found: {}", e))
    })?;

    let mut buffer = Vec::new();
    remote_file.read_to_end(&mut buffer).map_err(|e| {
        ExecutionError::SftpOperationFailed(format!("Failed to read file: {}", e))
    })?;

    storage_service
        .store_file(local_path, &buffer)
        .await
        .map_err(|e| ExecutionError::StorageFailed(format!("Failed to store file: {}", e)))?;

    Ok(FileMetadata {
        path: local_path.to_string(),
        filename: Path::new(remote_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string(),
        size: buffer.len() as u64,
        mime_type: None,
        row_count: None,
        created_at: Utc::now(),
    })
}

/// Upload file (public API)
pub async fn upload_file(
    sess: &Session,
    local_path: &str,
    remote_path: &str,
    storage_service: &Arc<dyn StorageService>,
) -> Result<(), ExecutionError> {
    let file_data = storage_service
        .load_file(local_path)
        .await
        .map_err(|e| ExecutionError::StorageFailed(format!("Failed to load file: {}", e)))?;

    let sftp = sess.sftp().map_err(|e| {
        ExecutionError::SftpOperationFailed(format!("Failed to open SFTP channel: {}", e))
    })?;

    let mut remote_file = sftp.create(Path::new(remote_path)).map_err(|e| {
        ExecutionError::SftpOperationFailed(format!("Failed to create remote file: {}", e))
    })?;

    std::io::Write::write_all(&mut remote_file, &file_data).map_err(|e| {
        ExecutionError::SftpOperationFailed(format!("Failed to write file: {}", e))
    })?;

    Ok(())
}
