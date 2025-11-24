// HTTP job variable substitution
// Requirements: 2.9, 2.10 - Variable substitution in URLs, headers, and body

use crate::errors::SubstitutionError;
#[cfg(test)]
use crate::models::HttpMethod;
use crate::models::{HttpAuth, JobType};
use crate::substitution::VariableSubstitutor;
use std::collections::HashMap;
use tracing::instrument;

/// Substitute variables in HTTP job configuration
///
/// # Arguments
/// * `job_type` - The HTTP job type configuration
/// * `variables` - HashMap of variable names to values
/// * `substitutor` - The variable substitutor instance
///
/// # Requirements
/// - 2.9: Variable substitution in URLs
/// - 2.10: Variable substitution in headers and body
///
/// # Returns
/// A new JobType with all variables substituted
///
/// # Errors
/// Returns SubstitutionError if any referenced variables are undefined
#[instrument(skip(job_type, variables, substitutor))]
pub fn substitute_http_job(
    job_type: &JobType,
    variables: &HashMap<String, String>,
    substitutor: &VariableSubstitutor,
) -> Result<JobType, SubstitutionError> {
    match job_type {
        JobType::HttpRequest {
            method,
            url,
            headers,
            body,
            auth,
        } => {
            // Substitute URL
            let substituted_url = substitutor.substitute(url, variables)?;
            tracing::debug!(original_url = url, substituted_url = %substituted_url, "Substituted URL");

            // Substitute headers
            let mut substituted_headers = HashMap::new();
            for (key, value) in headers {
                let substituted_key = substitutor.substitute(key, variables)?;
                let substituted_value = substitutor.substitute(value, variables)?;
                substituted_headers.insert(substituted_key, substituted_value);
            }
            tracing::debug!(
                header_count = substituted_headers.len(),
                "Substituted headers"
            );

            // Substitute body if present
            let substituted_body = if let Some(body_str) = body {
                let substituted = substitutor.substitute(body_str, variables)?;
                tracing::debug!(
                    original_len = body_str.len(),
                    substituted_len = substituted.len(),
                    "Substituted body"
                );
                Some(substituted)
            } else {
                None
            };

            // Substitute auth if present
            let substituted_auth = if let Some(auth_config) = auth {
                Some(substitute_http_auth(auth_config, variables, substitutor)?)
            } else {
                None
            };

            Ok(JobType::HttpRequest {
                method: method.clone(),
                url: substituted_url,
                headers: substituted_headers,
                body: substituted_body,
                auth: substituted_auth,
            })
        }
        _ => {
            // Not an HTTP job, return as-is
            Ok(job_type.clone())
        }
    }
}

/// Substitute variables in HTTP authentication configuration
///
/// # Arguments
/// * `auth` - The HTTP authentication configuration
/// * `variables` - HashMap of variable names to values
/// * `substitutor` - The variable substitutor instance
///
/// # Returns
/// A new HttpAuth with all variables substituted
///
/// # Errors
/// Returns SubstitutionError if any referenced variables are undefined
fn substitute_http_auth(
    auth: &HttpAuth,
    variables: &HashMap<String, String>,
    substitutor: &VariableSubstitutor,
) -> Result<HttpAuth, SubstitutionError> {
    match auth {
        HttpAuth::Basic { username, password } => {
            let substituted_username = substitutor.substitute(username, variables)?;
            let substituted_password = substitutor.substitute(password, variables)?;
            tracing::debug!("Substituted Basic auth credentials");
            Ok(HttpAuth::Basic {
                username: substituted_username,
                password: substituted_password,
            })
        }
        HttpAuth::Bearer { token } => {
            let substituted_token = substitutor.substitute(token, variables)?;
            tracing::debug!("Substituted Bearer token");
            Ok(HttpAuth::Bearer {
                token: substituted_token,
            })
        }
        HttpAuth::OAuth2 {
            client_id,
            client_secret,
            token_url,
        } => {
            let substituted_client_id = substitutor.substitute(client_id, variables)?;
            let substituted_client_secret = substitutor.substitute(client_secret, variables)?;
            let substituted_token_url = substitutor.substitute(token_url, variables)?;
            tracing::debug!("Substituted OAuth2 credentials");
            Ok(HttpAuth::OAuth2 {
                client_id: substituted_client_id,
                client_secret: substituted_client_secret,
                token_url: substituted_token_url,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_variables() -> HashMap<String, String> {
        let mut vars = HashMap::new();
        vars.insert("API_HOST".to_string(), "api.example.com".to_string());
        vars.insert("API_PORT".to_string(), "8080".to_string());
        vars.insert("API_KEY".to_string(), "secret123".to_string());
        vars.insert("USER_ID".to_string(), "12345".to_string());
        vars
    }

    #[test]
    fn test_substitute_http_url() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let variables = create_test_variables();

        let job_type = JobType::HttpRequest {
            method: HttpMethod::Get,
            url: "https://${API_HOST}:${API_PORT}/users/${USER_ID}".to_string(),
            headers: HashMap::new(),
            body: None,
            auth: None,
        };

        let result = substitute_http_job(&job_type, &variables, &substitutor).unwrap();

        match result {
            JobType::HttpRequest { url, .. } => {
                assert_eq!(url, "https://api.example.com:8080/users/12345");
            }
            _ => panic!("Expected HttpRequest"),
        }
    }

    #[test]
    fn test_substitute_http_headers() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let variables = create_test_variables();

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer ${API_KEY}".to_string());
        headers.insert("X-User-Id".to_string(), "${USER_ID}".to_string());

        let job_type = JobType::HttpRequest {
            method: HttpMethod::Post,
            url: "https://api.example.com/data".to_string(),
            headers,
            body: None,
            auth: None,
        };

        let result = substitute_http_job(&job_type, &variables, &substitutor).unwrap();

        match result {
            JobType::HttpRequest { headers, .. } => {
                assert_eq!(headers.get("Authorization").unwrap(), "Bearer secret123");
                assert_eq!(headers.get("X-User-Id").unwrap(), "12345");
            }
            _ => panic!("Expected HttpRequest"),
        }
    }

    #[test]
    fn test_substitute_http_body() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let variables = create_test_variables();

        let job_type = JobType::HttpRequest {
            method: HttpMethod::Post,
            url: "https://api.example.com/users".to_string(),
            headers: HashMap::new(),
            body: Some(r#"{"user_id": "${USER_ID}", "api_key": "${API_KEY}"}"#.to_string()),
            auth: None,
        };

        let result = substitute_http_job(&job_type, &variables, &substitutor).unwrap();

        match result {
            JobType::HttpRequest { body, .. } => {
                assert_eq!(
                    body.unwrap(),
                    r#"{"user_id": "12345", "api_key": "secret123"}"#
                );
            }
            _ => panic!("Expected HttpRequest"),
        }
    }

    #[test]
    fn test_substitute_basic_auth() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let mut variables = HashMap::new();
        variables.insert("USERNAME".to_string(), "admin".to_string());
        variables.insert("PASSWORD".to_string(), "secret".to_string());

        let job_type = JobType::HttpRequest {
            method: HttpMethod::Get,
            url: "https://api.example.com/data".to_string(),
            headers: HashMap::new(),
            body: None,
            auth: Some(HttpAuth::Basic {
                username: "${USERNAME}".to_string(),
                password: "${PASSWORD}".to_string(),
            }),
        };

        let result = substitute_http_job(&job_type, &variables, &substitutor).unwrap();

        match result {
            JobType::HttpRequest { auth, .. } => match auth.unwrap() {
                HttpAuth::Basic { username, password } => {
                    assert_eq!(username, "admin");
                    assert_eq!(password, "secret");
                }
                _ => panic!("Expected Basic auth"),
            },
            _ => panic!("Expected HttpRequest"),
        }
    }

    #[test]
    fn test_substitute_bearer_auth() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let mut variables = HashMap::new();
        variables.insert("TOKEN".to_string(), "bearer_token_123".to_string());

        let job_type = JobType::HttpRequest {
            method: HttpMethod::Get,
            url: "https://api.example.com/data".to_string(),
            headers: HashMap::new(),
            body: None,
            auth: Some(HttpAuth::Bearer {
                token: "${TOKEN}".to_string(),
            }),
        };

        let result = substitute_http_job(&job_type, &variables, &substitutor).unwrap();

        match result {
            JobType::HttpRequest { auth, .. } => match auth.unwrap() {
                HttpAuth::Bearer { token } => {
                    assert_eq!(token, "bearer_token_123");
                }
                _ => panic!("Expected Bearer auth"),
            },
            _ => panic!("Expected HttpRequest"),
        }
    }

    #[test]
    fn test_substitute_oauth2_auth() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let mut variables = HashMap::new();
        variables.insert("CLIENT_ID".to_string(), "client123".to_string());
        variables.insert("CLIENT_SECRET".to_string(), "secret456".to_string());
        variables.insert(
            "TOKEN_URL".to_string(),
            "https://auth.example.com/token".to_string(),
        );

        let job_type = JobType::HttpRequest {
            method: HttpMethod::Post,
            url: "https://api.example.com/data".to_string(),
            headers: HashMap::new(),
            body: None,
            auth: Some(HttpAuth::OAuth2 {
                client_id: "${CLIENT_ID}".to_string(),
                client_secret: "${CLIENT_SECRET}".to_string(),
                token_url: "${TOKEN_URL}".to_string(),
            }),
        };

        let result = substitute_http_job(&job_type, &variables, &substitutor).unwrap();

        match result {
            JobType::HttpRequest { auth, .. } => match auth.unwrap() {
                HttpAuth::OAuth2 {
                    client_id,
                    client_secret,
                    token_url,
                } => {
                    assert_eq!(client_id, "client123");
                    assert_eq!(client_secret, "secret456");
                    assert_eq!(token_url, "https://auth.example.com/token");
                }
                _ => panic!("Expected OAuth2 auth"),
            },
            _ => panic!("Expected HttpRequest"),
        }
    }

    #[test]
    fn test_substitute_undefined_variable_in_url() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let variables = HashMap::new();

        let job_type = JobType::HttpRequest {
            method: HttpMethod::Get,
            url: "https://${UNDEFINED_HOST}/api".to_string(),
            headers: HashMap::new(),
            body: None,
            auth: None,
        };

        let result = substitute_http_job(&job_type, &variables, &substitutor);
        assert!(result.is_err());

        match result {
            Err(SubstitutionError::UndefinedVariable { variables, .. }) => {
                assert!(variables.contains(&"UNDEFINED_HOST".to_string()));
            }
            _ => panic!("Expected UndefinedVariable error"),
        }
    }

    #[test]
    fn test_substitute_non_http_job() {
        let substitutor = VariableSubstitutor::new().unwrap();
        let variables = create_test_variables();

        let job_type = JobType::DatabaseQuery {
            database_type: crate::models::DatabaseType::PostgreSQL,
            connection_string: "postgresql://localhost/db".to_string(),
            query: "SELECT * FROM users".to_string(),
            query_type: crate::models::QueryType::RawSql,
        };

        let result = substitute_http_job(&job_type, &variables, &substitutor).unwrap();

        // Should return the same job type unchanged
        match result {
            JobType::DatabaseQuery { .. } => {}
            _ => panic!("Expected DatabaseQuery"),
        }
    }
}
