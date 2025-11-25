// HTTP job executor implementation
// Requirements: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 4.9

use crate::errors::ExecutionError;
use crate::executor::JobExecutor;
use crate::models::{HttpAuth, HttpMethod, JobContext, JobStep, JobType, StepOutput};
use crate::worker::reference::ReferenceResolver;
use async_trait::async_trait;
use chrono::Utc;
use reqwest::{Client, Method, RequestBuilder};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

/// HttpExecutor executes HTTP request jobs
pub struct HttpExecutor {
    client: Client,
    reference_resolver: Arc<ReferenceResolver>,
}

impl HttpExecutor {
    /// Create a new HttpExecutor with the specified timeout
    pub fn new(timeout_seconds: u64) -> Result<Self, ExecutionError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .build()
            .map_err(|e| {
                ExecutionError::HttpRequestFailed(format!("Failed to create HTTP client: {}", e))
            })?;

        Ok(Self {
            client,
            reference_resolver: Arc::new(ReferenceResolver::new()),
        })
    }

    /// Create a new HttpExecutor with a custom reference resolver
    pub fn with_resolver(
        timeout_seconds: u64,
        reference_resolver: Arc<ReferenceResolver>,
    ) -> Result<Self, ExecutionError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(timeout_seconds))
            .build()
            .map_err(|e| {
                ExecutionError::HttpRequestFailed(format!("Failed to create HTTP client: {}", e))
            })?;

        Ok(Self {
            client,
            reference_resolver,
        })
    }

    /// Convert HttpMethod to reqwest Method
    fn convert_method(method: &HttpMethod) -> Method {
        match method {
            HttpMethod::Get => Method::GET,
            HttpMethod::Post => Method::POST,
            HttpMethod::Put => Method::PUT,
        }
    }

    /// Apply authentication to the request
    async fn apply_auth(
        &self,
        mut request: RequestBuilder,
        auth: &Option<HttpAuth>,
    ) -> Result<RequestBuilder, ExecutionError> {
        if let Some(auth_config) = auth {
            request = match auth_config {
                HttpAuth::Basic { username, password } => {
                    // Requirement 3.4: Basic authentication
                    tracing::debug!("Applying Basic authentication for user: {}", username);
                    request.basic_auth(username, Some(password))
                }
                HttpAuth::Bearer { token } => {
                    // Requirement 3.5: Bearer token authentication
                    tracing::debug!("Applying Bearer token authentication");
                    request.bearer_auth(token)
                }
                HttpAuth::OAuth2 {
                    client_id,
                    client_secret,
                    token_url,
                } => {
                    // Requirement 3.6: OAuth2 token acquisition
                    tracing::debug!("Acquiring OAuth2 token from: {}", token_url);
                    let token = self
                        .acquire_oauth2_token(client_id, client_secret, token_url)
                        .await?;
                    request.bearer_auth(token)
                }
            };
        }

        Ok(request)
    }

    /// Acquire OAuth2 access token
    #[tracing::instrument(skip(self, client_secret))]
    async fn acquire_oauth2_token(
        &self,
        client_id: &str,
        client_secret: &str,
        token_url: &str,
    ) -> Result<String, ExecutionError> {
        let params = [
            ("grant_type", "client_credentials"),
            ("client_id", client_id),
            ("client_secret", client_secret),
        ];

        let response = self
            .client
            .post(token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| {
                ExecutionError::HttpRequestFailed(format!("OAuth2 token request failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ExecutionError::HttpRequestFailed(format!(
                "OAuth2 token request failed with status {}: {}",
                status, body
            )));
        }

        let token_response: serde_json::Value = response.json().await.map_err(|e| {
            ExecutionError::HttpRequestFailed(format!(
                "Failed to parse OAuth2 token response: {}",
                e
            ))
        })?;

        token_response
            .get("access_token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                ExecutionError::HttpRequestFailed(
                    "OAuth2 response missing access_token field".to_string(),
                )
            })
    }

    /// Execute HTTP request
    #[tracing::instrument(skip(self))]
    async fn execute_http_request(
        &self,
        method: &HttpMethod,
        url: &str,
        headers: &std::collections::HashMap<String, String>,
        body: &Option<String>,
        auth: &Option<HttpAuth>,
    ) -> Result<serde_json::Value, ExecutionError> {
        // Requirement 3.1: Support GET, POST, PUT methods
        let reqwest_method = Self::convert_method(method);
        tracing::info!("Executing HTTP {} request to: {}", reqwest_method, url);

        // Build the request
        let mut request = self.client.request(reqwest_method, url);

        // Requirement 3.2: Include custom headers
        for (key, value) in headers {
            tracing::debug!("Adding header: {} = {}", key, value);
            request = request.header(key, value);
        }

        // Apply authentication (Requirements 3.4, 3.5, 3.6)
        request = self.apply_auth(request, auth).await?;

        // Requirement 3.3: Include request body
        if let Some(body_content) = body {
            tracing::debug!("Adding request body ({} bytes)", body_content.len());
            request = request.body(body_content.clone());
        }

        // Send the request
        let response = request.send().await.map_err(|e| {
            ExecutionError::HttpRequestFailed(format!("HTTP request failed: {}", e))
        })?;

        let status = response.status();
        let status_code = status.as_u16();
        tracing::info!("HTTP response status: {}", status);

        // Get response headers
        let response_headers: std::collections::HashMap<String, String> = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();

        // Get response body
        let response_body = response.text().await.map_err(|e| {
            ExecutionError::HttpRequestFailed(format!("Failed to read response body: {}", e))
        })?;

        // Try to parse as JSON, fallback to string
        let body_json = serde_json::from_str::<serde_json::Value>(&response_body)
            .unwrap_or_else(|_| json!(response_body));

        // Build response object
        let result = json!({
            "status_code": status_code,
            "status": status.canonical_reason().unwrap_or("Unknown"),
            "headers": response_headers,
            "body": body_json,
        });

        // Check if request was successful
        if !status.is_success() {
            return Err(ExecutionError::HttpRequestFailed(format!(
                "HTTP request failed with status {}: {}",
                status_code, response_body
            )));
        }

        Ok(result)
    }

    /// Resolve references in authentication configuration
    fn resolve_auth_references(
        &self,
        auth: &HttpAuth,
        context: &JobContext,
    ) -> Result<HttpAuth, ExecutionError> {
        match auth {
            HttpAuth::Basic { username, password } => {
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
                Ok(HttpAuth::Basic {
                    username: resolved_username,
                    password: resolved_password,
                })
            }
            HttpAuth::Bearer { token } => {
                let resolved_token =
                    self.reference_resolver
                        .resolve(token, context)
                        .map_err(|e| {
                            ExecutionError::InvalidJobDefinition(format!(
                                "Failed to resolve bearer token: {}",
                                e
                            ))
                        })?;
                Ok(HttpAuth::Bearer {
                    token: resolved_token,
                })
            }
            HttpAuth::OAuth2 {
                client_id,
                client_secret,
                token_url,
            } => {
                let resolved_client_id = self
                    .reference_resolver
                    .resolve(client_id, context)
                    .map_err(|e| {
                        ExecutionError::InvalidJobDefinition(format!(
                            "Failed to resolve OAuth2 client_id: {}",
                            e
                        ))
                    })?;
                let resolved_client_secret = self
                    .reference_resolver
                    .resolve(client_secret, context)
                    .map_err(|e| {
                        ExecutionError::InvalidJobDefinition(format!(
                            "Failed to resolve OAuth2 client_secret: {}",
                            e
                        ))
                    })?;
                let resolved_token_url = self
                    .reference_resolver
                    .resolve(token_url, context)
                    .map_err(|e| {
                        ExecutionError::InvalidJobDefinition(format!(
                            "Failed to resolve OAuth2 token_url: {}",
                            e
                        ))
                    })?;
                Ok(HttpAuth::OAuth2 {
                    client_id: resolved_client_id,
                    client_secret: resolved_client_secret,
                    token_url: resolved_token_url,
                })
            }
        }
    }
}

#[async_trait]
impl JobExecutor for HttpExecutor {
    #[tracing::instrument(skip(self, step, context), fields(step_id = %step.id, step_name = %step.name))]
    async fn execute(
        &self,
        step: &JobStep,
        context: &mut JobContext,
    ) -> Result<StepOutput, ExecutionError> {
        let started_at = Utc::now();

        // Extract HTTP request details from step
        let (method, url, headers, body, auth) = match &step.step_type {
            JobType::HttpRequest {
                method,
                url,
                headers,
                body,
                auth,
            } => (method, url, headers, body, auth),
            _ => {
                return Err(ExecutionError::InvalidJobDefinition(
                    "HttpExecutor can only execute HttpRequest job types".to_string(),
                ));
            }
        };

        // Requirement 14.1: Resolve references in HTTP URLs, headers, body
        // Resolve URL references
        let resolved_url = self.reference_resolver.resolve(url, context).map_err(|e| {
            ExecutionError::InvalidJobDefinition(format!("Failed to resolve URL references: {}", e))
        })?;

        // Resolve header references
        let mut resolved_headers = std::collections::HashMap::new();
        for (key, value) in headers {
            let resolved_key = self.reference_resolver.resolve(key, context).map_err(|e| {
                ExecutionError::InvalidJobDefinition(format!(
                    "Failed to resolve header key '{}': {}",
                    key, e
                ))
            })?;
            let resolved_value = self
                .reference_resolver
                .resolve(value, context)
                .map_err(|e| {
                    ExecutionError::InvalidJobDefinition(format!(
                        "Failed to resolve header value for '{}': {}",
                        key, e
                    ))
                })?;
            resolved_headers.insert(resolved_key, resolved_value);
        }

        // Resolve body references
        let resolved_body = if let Some(body_content) = body {
            Some(
                self.reference_resolver
                    .resolve(body_content, context)
                    .map_err(|e| {
                        ExecutionError::InvalidJobDefinition(format!(
                            "Failed to resolve body references: {}",
                            e
                        ))
                    })?,
            )
        } else {
            None
        };

        // Resolve authentication references
        let resolved_auth = if let Some(auth_config) = auth {
            Some(self.resolve_auth_references(auth_config, context)?)
        } else {
            None
        };

        // Execute the HTTP request with resolved values
        let output = self
            .execute_http_request(
                method,
                &resolved_url,
                &resolved_headers,
                &resolved_body,
                &resolved_auth,
            )
            .await?;

        let completed_at = Utc::now();

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
    use crate::models::HttpMethod;

    #[test]
    fn test_convert_method() {
        assert_eq!(HttpExecutor::convert_method(&HttpMethod::Get), Method::GET);
        assert_eq!(
            HttpExecutor::convert_method(&HttpMethod::Post),
            Method::POST
        );
        assert_eq!(HttpExecutor::convert_method(&HttpMethod::Put), Method::PUT);
    }

    #[tokio::test]
    async fn test_http_executor_creation() {
        let executor = HttpExecutor::new(30);
        assert!(executor.is_ok());
    }

    #[test]
    fn test_invalid_job_type() {
        use crate::models::{DatabaseType, QueryType};
        use uuid::Uuid;

        let executor = HttpExecutor::new(30).unwrap();
        let step = JobStep {
            id: "step1".to_string(),
            name: "Test Step".to_string(),
            step_type: JobType::DatabaseQuery {
                database_type: DatabaseType::PostgreSQL,
                connection_string: "".to_string(),
                query: "".to_string(),
                query_type: QueryType::RawSql,
            },
            condition: None,
        };

        let mut context = JobContext {
            execution_id: Uuid::new_v4(),
            job_id: Uuid::new_v4(),
            variables: std::collections::HashMap::new(),
            steps: std::collections::HashMap::new(),
            webhook: None,
            files: Vec::new(),
        };

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(executor.execute(&step, &mut context));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ExecutionError::InvalidJobDefinition(_)
        ));
    }
}
