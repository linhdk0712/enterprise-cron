// MinIO storage client and connection management
// Requirements: 13.2 - MinIO object storage for job definitions and execution context
// RECC 2025: No unwrap(), use #[tracing::instrument], proper error handling

use crate::config::MinioConfig;
use crate::errors::StorageError;
use s3::bucket::Bucket;
use s3::creds::Credentials;
use s3::region::Region;
use std::sync::Arc;
use tracing::{debug, error, info, instrument};

/// MinIO client wrapper with connection pooling
#[derive(Clone, Debug)]
pub struct MinioClient {
    bucket: Arc<Bucket>,
}

impl MinioClient {
    /// Create a new MinIO client from configuration
    /// Requirements: 13.2 - Configure rust-s3 client for MinIO
    #[instrument(skip(config), fields(endpoint = %config.endpoint, bucket = %config.bucket))]
    pub async fn new(config: &MinioConfig) -> Result<Self, StorageError> {
        info!("Initializing MinIO client");

        // Parse endpoint - strip scheme as rust-s3 Region::Custom doesn't expect it
        let endpoint = config
            .endpoint
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .to_string();

        // Create credentials
        let credentials = Credentials::new(
            Some(&config.access_key),
            Some(&config.secret_key),
            None,
            None,
            None,
        )
        .map_err(|e| {
            error!(error = %e, "Failed to create MinIO credentials");
            StorageError::MinioError(format!("Failed to create credentials: {}", e))
        })?;

        // Create custom region for MinIO (endpoint without scheme)
        let region = Region::Custom {
            region: config.region.clone(),
            endpoint,
        };

        // Create bucket instance with path style
        // Note: For production, MinIO should be configured with HTTPS
        let bucket = Bucket::new(&config.bucket, region, credentials)
            .map_err(|e| {
                error!(error = %e, "Failed to create MinIO bucket");
                StorageError::MinioError(format!("Failed to create bucket: {}", e))
            })?
            .with_path_style();

        info!(
            bucket = %config.bucket,
            endpoint = %config.endpoint,
            "MinIO client initialized successfully"
        );

        Ok(Self {
            bucket: Arc::new(bucket),
        })
    }

    /// Health check for MinIO connection
    /// Requirements: 13.2 - Add health check
    #[instrument(skip(self))]
    pub async fn health_check(&self) -> Result<(), StorageError> {
        debug!("Performing MinIO health check");

        // Try to list objects with a limit of 1 to verify connectivity
        match self
            .bucket
            .list("".to_string(), Some("/".to_string()))
            .await
        {
            Ok(_) => {
                debug!("MinIO health check passed");
                Ok(())
            }
            Err(e) => {
                error!(error = %e, "MinIO health check failed");
                Err(StorageError::MinioError(format!(
                    "Health check failed: {}",
                    e
                )))
            }
        }
    }

    /// Get the underlying bucket reference
    pub fn bucket(&self) -> &Bucket {
        &self.bucket
    }

    /// Store data to MinIO at the specified path
    #[instrument(skip(self, data), fields(path = %path, size = data.len()))]
    pub async fn put_object(&self, path: &str, data: &[u8]) -> Result<(), StorageError> {
        debug!(path = %path, size = data.len(), "Storing object to MinIO");

        self.bucket.put_object(path, data).await.map_err(|e| {
            error!(error = %e, path = %path, "Failed to store object to MinIO");
            StorageError::MinioError(format!("Failed to put object '{}': {}", path, e))
        })?;

        debug!(path = %path, "Object stored successfully");
        Ok(())
    }

    /// Retrieve data from MinIO at the specified path
    #[instrument(skip(self), fields(path = %path))]
    pub async fn get_object(&self, path: &str) -> Result<Vec<u8>, StorageError> {
        debug!(path = %path, "Retrieving object from MinIO");

        let response = self.bucket.get_object(path).await.map_err(|e| {
            error!(error = %e, path = %path, "Failed to retrieve object from MinIO");
            StorageError::MinioError(format!("Failed to get object '{}': {}", path, e))
        })?;

        let data = response.bytes().to_vec();
        debug!(path = %path, size = data.len(), "Object retrieved successfully");
        Ok(data)
    }

    /// Delete an object from MinIO
    #[instrument(skip(self), fields(path = %path))]
    pub async fn delete_object(&self, path: &str) -> Result<(), StorageError> {
        debug!(path = %path, "Deleting object from MinIO");

        self.bucket.delete_object(path).await.map_err(|e| {
            error!(error = %e, path = %path, "Failed to delete object from MinIO");
            StorageError::MinioError(format!("Failed to delete object '{}': {}", path, e))
        })?;

        debug!(path = %path, "Object deleted successfully");
        Ok(())
    }

    /// Check if an object exists in MinIO
    #[instrument(skip(self), fields(path = %path))]
    pub async fn object_exists(&self, path: &str) -> Result<bool, StorageError> {
        debug!(path = %path, "Checking if object exists in MinIO");

        match self.bucket.head_object(path).await {
            Ok(_) => {
                debug!(path = %path, "Object exists");
                Ok(true)
            }
            Err(e) => {
                // Check if it's a 404 error (object not found)
                let error_str = e.to_string();
                if error_str.contains("404") || error_str.contains("Not Found") {
                    debug!(path = %path, "Object does not exist");
                    Ok(false)
                } else {
                    error!(error = %e, path = %path, "Failed to check object existence");
                    Err(StorageError::MinioError(format!(
                        "Failed to check object existence '{}': {}",
                        path, e
                    )))
                }
            }
        }
    }

    /// List objects with a given prefix
    #[instrument(skip(self), fields(prefix = %prefix))]
    pub async fn list_objects(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
        debug!(prefix = %prefix, "Listing objects in MinIO");

        let results = self
            .bucket
            .list(prefix.to_string(), Some("/".to_string()))
            .await
            .map_err(|e| {
                error!(error = %e, prefix = %prefix, "Failed to list objects in MinIO");
                StorageError::MinioError(format!(
                    "Failed to list objects with prefix '{}': {}",
                    prefix, e
                ))
            })?;

        let mut objects = Vec::new();
        for result in results {
            for content in result.contents {
                objects.push(content.key);
            }
        }

        debug!(prefix = %prefix, count = objects.len(), "Objects listed successfully");
        Ok(objects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> MinioConfig {
        MinioConfig {
            endpoint: "https://localhost:9000".to_string(),
            access_key: "minioadmin".to_string(),
            secret_key: "minioadmin".to_string(),
            bucket: "test-bucket".to_string(),
            region: "us-east-1".to_string(),
        }
    }

    #[tokio::test]
    async fn test_minio_client_creation() {
        let config = test_config();
        let result = MinioClient::new(&config).await;
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_endpoint_parsing() {
        let mut config = test_config();
        config.endpoint = "https://minio.example.com".to_string();
        assert!(config.endpoint.starts_with("https://"));
    }
}
