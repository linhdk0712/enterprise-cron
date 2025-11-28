// Configuration management with layered configuration (file, env, CLI)
// Requirements: 7.5

use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Main settings structure containing all configuration options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub nats: NatsConfig,
    pub minio: MinioConfig,
    pub auth: AuthConfig,
    pub scheduler: SchedulerConfig,
    pub worker: WorkerConfig,
    pub observability: ObservabilityConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
    pub max_connections: u32,
    pub min_connections: u32,
    pub connect_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub pool_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NatsConfig {
    pub url: String,
    pub stream_name: String,
    pub consumer_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinioConfig {
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub region: String,
    #[serde(default)]
    pub use_ssl: bool,
    #[serde(default = "default_verify_ssl")]
    pub verify_ssl: bool,
}

fn default_verify_ssl() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub mode: AuthMode,
    pub jwt_secret: String,
    pub jwt_expiration_hours: u64,
    pub keycloak: Option<KeycloakConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    Database,
    Keycloak,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeycloakConfig {
    pub server_url: String,
    pub realm: String,
    pub client_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    pub poll_interval_seconds: u64,
    pub lock_ttl_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    pub concurrency: u32,
    pub max_retries: u32,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    pub log_level: String,
    pub metrics_port: u16,
    pub tracing_endpoint: Option<String>,
}

impl Settings {
    /// Load configuration with layered precedence: defaults → file → env
    /// Requirements: 7.5 - Configuration hot reload support
    pub fn load() -> Result<Self, ConfigError> {
        Self::load_from_path("config")
    }

    /// Load configuration from a specific path
    pub fn load_from_path<P: AsRef<Path>>(config_dir: P) -> Result<Self, ConfigError> {
        let config_dir = config_dir.as_ref();

        let builder = Config::builder()
            // Start with default configuration
            .add_source(File::from(config_dir.join("default.toml")).required(false))
            // Add local configuration (not committed to git)
            .add_source(File::from(config_dir.join("local.toml")).required(false))
            // Add environment-specific configuration
            .add_source(
                Environment::with_prefix("APP")
                    .separator("__")
                    .try_parsing(true),
            );

        let config = builder.build()?;
        config.try_deserialize()
    }

    /// Validate configuration settings
    /// Requirements: 7.5 - Config validation
    pub fn validate(&self) -> Result<(), String> {
        // Validate server config
        if self.server.port == 0 {
            return Err("Server port must be greater than 0".to_string());
        }

        // Validate database config
        if self.database.url.is_empty() {
            return Err("Database URL cannot be empty".to_string());
        }
        if self.database.max_connections == 0 {
            return Err("Database max_connections must be greater than 0".to_string());
        }

        // Validate Redis config
        if self.redis.url.is_empty() {
            return Err("Redis URL cannot be empty".to_string());
        }

        // Validate NATS config
        if self.nats.url.is_empty() {
            return Err("NATS URL cannot be empty".to_string());
        }
        if self.nats.stream_name.is_empty() {
            return Err("NATS stream_name cannot be empty".to_string());
        }

        // Validate MinIO config
        if self.minio.endpoint.is_empty() {
            return Err("MinIO endpoint cannot be empty".to_string());
        }
        if self.minio.bucket.is_empty() {
            return Err("MinIO bucket cannot be empty".to_string());
        }

        // Validate auth config
        if self.auth.jwt_secret.is_empty() {
            return Err("JWT secret cannot be empty".to_string());
        }
        if matches!(self.auth.mode, AuthMode::Keycloak) && self.auth.keycloak.is_none() {
            return Err("Keycloak configuration required when auth mode is 'keycloak'".to_string());
        }

        // Validate scheduler config
        if self.scheduler.poll_interval_seconds == 0 {
            return Err("Scheduler poll_interval_seconds must be greater than 0".to_string());
        }

        // Validate worker config
        if self.worker.concurrency == 0 {
            return Err("Worker concurrency must be greater than 0".to_string());
        }

        Ok(())
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
            },
            database: DatabaseConfig {
                url: "postgresql://localhost/vietnam_cron".to_string(),
                max_connections: 10,
                min_connections: 2,
                connect_timeout_seconds: 30,
            },
            redis: RedisConfig {
                url: "redis://localhost:6379".to_string(),
                pool_size: 10,
            },
            nats: NatsConfig {
                url: "nats://localhost:4222".to_string(),
                stream_name: "job_stream".to_string(),
                consumer_name: "job_consumer".to_string(),
            },
            minio: MinioConfig {
                endpoint: "http://localhost:9000".to_string(),
                access_key: "minioadmin".to_string(),
                secret_key: "minioadmin".to_string(),
                bucket: "vietnam-cron".to_string(),
                region: "us-east-1".to_string(),
                use_ssl: false,
                verify_ssl: true,
            },
            auth: AuthConfig {
                mode: AuthMode::Database,
                jwt_secret: "change-me-in-production".to_string(),
                jwt_expiration_hours: 24,
                keycloak: None,
            },
            scheduler: SchedulerConfig {
                poll_interval_seconds: 10,
                lock_ttl_seconds: 30,
            },
            worker: WorkerConfig {
                concurrency: 10,
                max_retries: 10,
                timeout_seconds: 300,
            },
            observability: ObservabilityConfig {
                log_level: "info".to_string(),
                metrics_port: 9090,
                tracing_endpoint: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings_are_valid() {
        let settings = Settings::default();
        assert!(settings.validate().is_ok());
    }

    #[test]
    fn test_validation_catches_empty_database_url() {
        let mut settings = Settings::default();
        settings.database.url = String::new();
        assert!(settings.validate().is_err());
    }

    #[test]
    fn test_validation_catches_zero_port() {
        let mut settings = Settings::default();
        settings.server.port = 0;
        assert!(settings.validate().is_err());
    }

    #[test]
    fn test_validation_catches_keycloak_mode_without_config() {
        let mut settings = Settings::default();
        settings.auth.mode = AuthMode::Keycloak;
        settings.auth.keycloak = None;
        assert!(settings.validate().is_err());
    }
}
