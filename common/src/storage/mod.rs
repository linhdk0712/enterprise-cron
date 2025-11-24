// Storage module for MinIO and other storage backends
// Requirements: 13.2 - MinIO object storage integration

pub mod minio;
pub mod service;

pub use minio::MinioClient;
pub use service::{MinIOService, MinIOServiceImpl};
